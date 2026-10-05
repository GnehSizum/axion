use std::sync::{Arc, Mutex, OnceLock, mpsc};

use tokio::sync::oneshot;

type Job = Box<dyn FnOnce() + Send>;
const WORKERS: usize = 4;
const QUEUE_CAPACITY: usize = 64;
const CONTROL_WORKERS: usize = 2;
const CONTROL_QUEUE_CAPACITY: usize = 32;

fn create_sender(workers: usize, queue_capacity: usize) -> mpsc::SyncSender<Job> {
    let (sender, receiver) = mpsc::sync_channel::<Job>(queue_capacity);
    let receiver = Arc::new(Mutex::new(receiver));
    for index in 0..workers {
        let receiver = receiver.clone();
        std::thread::Builder::new()
            .name(format!("axion-native-{index}"))
            .spawn(move || {
                loop {
                    let job = match receiver.lock() {
                        Ok(receiver) => receiver.recv(),
                        Err(_) => break,
                    };
                    match job {
                        Ok(job) => job(),
                        Err(_) => break,
                    }
                }
            })
            .expect("Axion native command worker must start");
    }
    sender
}

fn sender() -> &'static mpsc::SyncSender<Job> {
    static POOL: OnceLock<mpsc::SyncSender<Job>> = OnceLock::new();
    POOL.get_or_init(|| create_sender(WORKERS, QUEUE_CAPACITY))
}

fn control_sender() -> &'static mpsc::SyncSender<Job> {
    static POOL: OnceLock<mpsc::SyncSender<Job>> = OnceLock::new();
    POOL.get_or_init(|| create_sender(CONTROL_WORKERS, CONTROL_QUEUE_CAPACITY))
}

/// Run bounded blocking native work without occupying an async executor worker.
pub async fn run_blocking(
    job: impl FnOnce() -> Result<String, String> + Send + 'static,
) -> Result<String, String> {
    run_blocking_on(sender(), job).await
}

/// Run short blocking control work independently of long-running native I/O.
///
/// This pool has two workers and a bounded queue of 32 jobs. Queue waiting does
/// not count toward any response timeout implemented by the control backend.
pub async fn run_blocking_control(
    job: impl FnOnce() -> Result<String, String> + Send + 'static,
) -> Result<String, String> {
    run_blocking_on(control_sender(), job).await
}

async fn run_blocking_on(
    sender: &mpsc::SyncSender<Job>,
    job: impl FnOnce() -> Result<String, String> + Send + 'static,
) -> Result<String, String> {
    let (reply, response) = oneshot::channel();
    sender
        .try_send(Box::new(move || {
            if reply.is_closed() {
                return;
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                .unwrap_or_else(|_| {
                    Err("bridge.handler-panicked: native command panicked".to_owned())
                });
            let _ = reply.send(result);
        }))
        .map_err(|error| match error {
            mpsc::TrySendError::Full(_) => "bridge.busy: native command queue is full".to_owned(),
            mpsc::TrySendError::Disconnected(_) => {
                "bridge.unavailable: native command workers stopped".to_owned()
            }
        })?;
    response
        .await
        .map_err(|_| "bridge.unavailable: native command did not respond".to_owned())?
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Wake, Waker};
    use std::time::Duration;

    use super::*;
    use crate::{
        BridgeRequest, BridgeRunMode, CommandContext, CommandRegistry, WindowCommandContext,
    };

    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    fn poll_once<F: Future + ?Sized>(future: Pin<&mut F>) -> Poll<F::Output> {
        let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
        future.poll(&mut Context::from_waker(&waker))
    }

    fn wait<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        loop {
            match poll_once(future.as_mut()) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park_timeout(Duration::from_millis(50)),
            }
        }
    }

    #[test]
    fn blocking_work_returns_result_from_native_worker() {
        let sender = create_sender(1, 1);
        let caller = std::thread::current().id();
        assert_eq!(
            wait(run_blocking_on(&sender, move || {
                assert_ne!(std::thread::current().id(), caller);
                Ok("true".to_owned())
            }))
            .unwrap(),
            "true"
        );
    }

    #[test]
    fn panic_does_not_drop_response_or_kill_worker() {
        let sender = create_sender(1, 1);
        assert!(
            wait(run_blocking_on(&sender, || panic!("test panic")))
                .unwrap_err()
                .starts_with("bridge.handler-panicked:")
        );
        assert_eq!(
            wait(run_blocking_on(&sender, || Ok("null".to_owned()))).unwrap(),
            "null"
        );
    }

    #[test]
    fn queue_saturation_rejects_work_without_blocking_the_caller() {
        let sender = create_sender(WORKERS, QUEUE_CAPACITY);
        let (entered, entries) = mpsc::channel();
        let mut releases = Vec::new();
        let mut pending: Vec<Pin<Box<dyn Future<Output = Result<String, String>>>>> = Vec::new();
        for _ in 0..WORKERS {
            let (release, gate) = mpsc::channel();
            releases.push(release);
            let entered = entered.clone();
            let mut future = Box::pin(run_blocking_on(&sender, move || {
                entered.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok("null".to_owned())
            }));
            assert!(poll_once(future.as_mut()).is_pending());
            pending.push(future);
        }
        for _ in 0..WORKERS {
            entries.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        for _ in 0..QUEUE_CAPACITY {
            let mut future = Box::pin(run_blocking_on(&sender, || Ok("true".to_owned())));
            assert!(poll_once(future.as_mut()).is_pending());
            pending.push(future);
        }
        assert_eq!(
            wait(run_blocking_on(&sender, || Ok("false".to_owned()))).unwrap_err(),
            "bridge.busy: native command queue is full"
        );
        for release in releases {
            release.send(()).unwrap();
        }
        for future in pending {
            wait(future).unwrap();
        }
        assert_eq!(
            wait(run_blocking_on(&sender, || Ok("true".to_owned()))).unwrap(),
            "true"
        );
    }

    #[test]
    fn saturated_io_pool_does_not_delay_control_work() {
        let io_sender = create_sender(WORKERS, QUEUE_CAPACITY);
        let control_sender = create_sender(CONTROL_WORKERS, CONTROL_QUEUE_CAPACITY);
        let (entered, entries) = mpsc::channel();
        let mut releases = Vec::new();
        let mut pending: Vec<Pin<Box<dyn Future<Output = Result<String, String>>>>> = Vec::new();
        for _ in 0..WORKERS {
            let (release, gate) = mpsc::channel();
            releases.push(release);
            let entered = entered.clone();
            let mut future = Box::pin(run_blocking_on(&io_sender, move || {
                entered.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok("null".to_owned())
            }));
            assert!(poll_once(future.as_mut()).is_pending());
            pending.push(future);
        }
        for _ in 0..WORKERS {
            entries.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        for _ in 0..QUEUE_CAPACITY {
            let mut future = Box::pin(run_blocking_on(&io_sender, || Ok("null".to_owned())));
            assert!(poll_once(future.as_mut()).is_pending());
            pending.push(future);
        }
        assert_eq!(
            wait(run_blocking_on(&io_sender, || Ok("null".to_owned()))).unwrap_err(),
            "bridge.busy: native command queue is full"
        );

        let (finished, completion) = mpsc::channel();
        let mut control = Box::pin(run_blocking_on(&control_sender, move || {
            finished.send(()).unwrap();
            Ok("control completed".to_owned())
        }));
        let first_poll = poll_once(control.as_mut());
        completion.recv_timeout(Duration::from_secs(1)).unwrap();
        let result = match first_poll {
            Poll::Ready(result) => result,
            Poll::Pending => wait(control),
        };
        assert_eq!(result.unwrap(), "control completed");
        // Every I/O worker is still held at its gate when control completes.
        for release in releases {
            release.send(()).unwrap();
        }
        for future in pending {
            wait(future).unwrap();
        }
    }

    #[test]
    fn cancellation_skips_queued_work_before_it_starts() {
        let sender = create_sender(1, 2);
        let (release, gate) = mpsc::channel();
        let (entered, entries) = mpsc::channel();
        let mut first = Box::pin(run_blocking_on(&sender, move || {
            entered.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok("null".to_owned())
        }));
        assert!(poll_once(first.as_mut()).is_pending());
        entries.recv_timeout(Duration::from_secs(5)).unwrap();

        let did_run = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let marker = did_run.clone();
        let mut cancelled = Box::pin(run_blocking_on(&sender, move || {
            marker.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok("null".to_owned())
        }));
        assert!(poll_once(cancelled.as_mut()).is_pending());
        drop(cancelled);
        let mut sentinel = Box::pin(run_blocking_on(&sender, || Ok("true".to_owned())));
        assert!(poll_once(sentinel.as_mut()).is_pending());
        release.send(()).unwrap();
        wait(first).unwrap();
        assert_eq!(wait(sentinel).unwrap(), "true");
        assert!(!did_run.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn cancelling_a_running_task_does_not_abort_its_side_effects() {
        let sender = create_sender(1, 1);
        let (release, gate) = mpsc::channel();
        let (entered, entries) = mpsc::channel();
        let (finished, completion) = mpsc::channel();
        let mut task = Box::pin(run_blocking_on(&sender, move || {
            entered.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(5)).unwrap();
            finished.send("side effect completed").unwrap();
            Ok("null".to_owned())
        }));
        assert!(poll_once(task.as_mut()).is_pending());
        entries.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(task);
        release.send(()).unwrap();
        assert_eq!(
            completion.recv_timeout(Duration::from_secs(5)).unwrap(),
            "side effect completed"
        );
    }

    #[test]
    fn slow_native_dispatch_does_not_block_app_ping_on_the_same_executor() {
        let sender = create_sender(1, 1);
        let (release, gate) = mpsc::channel();
        let gate = Arc::new(Mutex::new(gate));
        let (entered, entries) = mpsc::channel();
        let mut registry = CommandRegistry::default();
        registry.register_async("native.slow", move |_, _| {
            let sender = sender.clone();
            let gate = gate.clone();
            let entered = entered.clone();
            async move {
                run_blocking_on(&sender, move || {
                    entered.send(()).unwrap();
                    gate.lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(5))
                        .unwrap();
                    Ok("null".to_owned())
                })
                .await
            }
        });
        registry.register("app.ping", |_, _| Ok("\"pong\"".to_owned()));
        let context = CommandContext {
            app_name: "blocking-test".to_owned(),
            identifier: None,
            version: None,
            description: None,
            authors: Vec::new(),
            homepage: None,
            mode: BridgeRunMode::Development,
            window: WindowCommandContext {
                id: "main".to_owned(),
                title: "test".to_owned(),
                width: 1,
                height: 1,
                resizable: false,
                visible: false,
            },
        };
        let slow_request = BridgeRequest::new("native.slow", "null");
        let mut slow = Box::pin(registry.dispatch(&context, &slow_request));
        assert!(poll_once(slow.as_mut()).is_pending());
        entries.recv_timeout(Duration::from_secs(5)).unwrap();
        let ping_request = BridgeRequest::new("app.ping", "null");
        let mut ping = Box::pin(registry.dispatch(&context, &ping_request));
        let Poll::Ready(result) = poll_once(ping.as_mut()) else {
            panic!("app.ping should finish while native work is still held at its gate");
        };
        assert_eq!(result.unwrap(), "\"pong\"");
        release.send(()).unwrap();
        assert_eq!(wait(slow).unwrap(), "null");
    }
}
