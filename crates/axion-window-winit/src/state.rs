use std::collections::BTreeMap;
use std::time::{Duration, Instant};

pub(crate) struct PendingCloseRequest<W> {
    pub(crate) window_id: W,
    pub(crate) reason: String,
    pub(crate) deadline: Instant,
}

pub(crate) fn next_close_deadline<W>(
    requests: &BTreeMap<String, PendingCloseRequest<W>>,
) -> Option<Instant> {
    requests.values().map(|request| request.deadline).min()
}

pub(crate) fn expired_close_requests<W>(
    requests: &BTreeMap<String, PendingCloseRequest<W>>,
    now: Instant,
) -> Vec<String> {
    requests
        .iter()
        .filter_map(|(id, request)| (request.deadline <= now).then(|| id.clone()))
        .collect()
}

pub(crate) struct AppExitRequestState {
    pub(crate) request_id: String,
    pub(crate) window_count: usize,
    pub(crate) request_count: usize,
    pub(crate) closed_count: usize,
    pub(crate) prevented_count: usize,
    pub(crate) timed_out_count: usize,
    pub(crate) close_requests: Vec<(String, String)>,
    pub(crate) closed_windows: Vec<String>,
    pub(crate) prevented_windows: Vec<String>,
    pub(crate) timed_out_windows: Vec<String>,
    pub(crate) closed_requests: Vec<(String, String)>,
    pub(crate) prevented_requests: Vec<(String, String)>,
    pub(crate) timed_out_requests: Vec<(String, String)>,
}

#[derive(Default)]
pub(crate) struct AppExitTracker {
    active: Option<AppExitRequestState>,
}

impl AppExitTracker {
    pub(crate) fn active(&self) -> Option<&AppExitRequestState> {
        self.active.as_ref()
    }

    pub(crate) fn start(
        &mut self,
        request_id: String,
        window_count: usize,
        close_requests: Vec<(String, String)>,
    ) -> &AppExitRequestState {
        self.active.get_or_insert_with(|| AppExitRequestState {
            request_id,
            window_count,
            request_count: close_requests.len(),
            close_requests,
            closed_count: 0,
            prevented_count: 0,
            timed_out_count: 0,
            closed_windows: Vec::new(),
            prevented_windows: Vec::new(),
            timed_out_windows: Vec::new(),
            closed_requests: Vec::new(),
            prevented_requests: Vec::new(),
            timed_out_requests: Vec::new(),
        })
    }

    pub(crate) fn record_closed(
        &mut self,
        close_request_id: &str,
        window_id: &str,
        timed_out: bool,
    ) -> Option<AppExitRequestState> {
        let state = self.active.as_mut()?;
        if !state
            .close_requests
            .iter()
            .any(|(id, window)| id == close_request_id && window == window_id)
            || state
                .closed_requests
                .iter()
                .any(|(id, _)| id == close_request_id)
        {
            return None;
        }
        state.closed_count += 1;
        state.closed_windows.push(window_id.to_owned());
        state
            .closed_requests
            .push((close_request_id.to_owned(), window_id.to_owned()));
        if timed_out {
            state.timed_out_count += 1;
            state.timed_out_windows.push(window_id.to_owned());
            state
                .timed_out_requests
                .push((close_request_id.to_owned(), window_id.to_owned()));
        }
        if state.closed_count == state.request_count {
            self.active.take()
        } else {
            None
        }
    }

    pub(crate) fn record_prevented(
        &mut self,
        close_request_id: &str,
        window_id: &str,
    ) -> Option<AppExitRequestState> {
        let state = self.active.as_ref()?;
        if !state
            .close_requests
            .iter()
            .any(|(id, window)| id == close_request_id && window == window_id)
            || state
                .closed_requests
                .iter()
                .any(|(id, _)| id == close_request_id)
        {
            return None;
        }
        let mut state = self.active.take()?;
        state.prevented_count += 1;
        state.prevented_windows.push(window_id.to_owned());
        state
            .prevented_requests
            .push((close_request_id.to_owned(), window_id.to_owned()));
        Some(state)
    }
}

#[derive(Clone)]
pub(crate) struct WebViewPolicy {
    pub(crate) window_id: String,
    pub(crate) bridge_token: String,
    pub(crate) content_security_policy: String,
}

pub(crate) fn policy_for_webview<'a, K: Ord>(
    policies: &'a BTreeMap<K, WebViewPolicy>,
    webview_id: Option<&K>,
) -> Option<&'a WebViewPolicy> {
    policies.get(webview_id?)
}

pub(crate) fn token_matches(policy: &WebViewPolicy, supplied: Option<&str>) -> bool {
    supplied.is_some_and(|token| token == policy.bridge_token)
}

pub(crate) fn receive_control_response<T>(
    receiver: &std::sync::mpsc::Receiver<Result<T, String>>,
    timeout: Duration,
) -> Result<T, String> {
    receiver
        .recv_timeout(timeout)
        .map_err(|error| match error {
            std::sync::mpsc::RecvTimeoutError::Timeout => {
                "window.control-timeout: the event loop did not respond in time".to_owned()
            }
            std::sync::mpsc::RecvTimeoutError::Disconnected => {
                "window control response channel was closed".to_owned()
            }
        })?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close_requests() -> Vec<(String, String)> {
        vec![
            ("close-main".to_owned(), "main".to_owned()),
            ("close-settings".to_owned(), "settings".to_owned()),
        ]
    }

    #[test]
    fn repeated_exit_preserves_original_close_request_ownership() {
        let mut tracker = AppExitTracker::default();
        tracker.start("exit-1".to_owned(), 2, close_requests());
        assert_eq!(
            tracker.start("exit-2".to_owned(), 1, Vec::new()).request_id,
            "exit-1"
        );
        assert_eq!(tracker.active().unwrap().window_count, 2);
        assert!(tracker.record_closed("close-main", "main", false).is_none());
        assert!(tracker.record_closed("close-main", "main", false).is_none());
        let complete = tracker
            .record_closed("close-settings", "settings", true)
            .unwrap();
        assert_eq!(complete.request_id, "exit-1");
        assert_eq!(complete.closed_count, 2);
        assert_eq!(complete.timed_out_count, 1);
        assert!(tracker.active().is_none());
        assert!(
            tracker
                .record_closed("close-settings", "settings", true)
                .is_none()
        );
    }

    #[test]
    fn prevention_terminates_exit_and_allows_a_new_request() {
        let mut tracker = AppExitTracker::default();
        tracker.start("exit-1".to_owned(), 2, close_requests());
        assert!(tracker.record_prevented("unknown", "main").is_none());
        let prevented = tracker.record_prevented("close-main", "main").unwrap();
        assert_eq!(prevented.prevented_count, 1);
        assert_eq!(prevented.close_requests.len(), 2);
        assert!(tracker.active().is_none());
        assert!(
            tracker
                .record_closed("close-settings", "settings", true)
                .is_none()
        );
        assert_eq!(
            tracker
                .start("exit-2".to_owned(), 2, close_requests())
                .request_id,
            "exit-2"
        );
    }

    #[test]
    fn close_deadlines_follow_the_earliest_pending_request_and_cancel_cleanly() {
        let now = Instant::now();
        let mut requests = BTreeMap::from([
            (
                "later".to_owned(),
                PendingCloseRequest {
                    window_id: 1,
                    reason: "system".to_owned(),
                    deadline: now + Duration::from_secs(2),
                },
            ),
            (
                "first".to_owned(),
                PendingCloseRequest {
                    window_id: 2,
                    reason: "app-exit".to_owned(),
                    deadline: now,
                },
            ),
        ]);
        assert_eq!(requests["first"].window_id, 2);
        assert_eq!(requests["first"].reason, "app-exit");
        assert_eq!(next_close_deadline(&requests), Some(now));
        assert_eq!(expired_close_requests(&requests, now), vec!["first"]);
        requests.remove("first");
        assert!(expired_close_requests(&requests, now).is_empty());
        requests.clear();
        assert_eq!(next_close_deadline(&requests), None);
    }

    #[test]
    fn webview_identity_selects_csp_and_rejects_another_windows_token() {
        let policies = BTreeMap::from([
            (
                2,
                WebViewPolicy {
                    window_id: "settings".to_owned(),
                    bridge_token: "settings-token".to_owned(),
                    content_security_policy: "connect-src https://docs.example".to_owned(),
                },
            ),
            (
                1,
                WebViewPolicy {
                    window_id: "main".to_owned(),
                    bridge_token: "main-token".to_owned(),
                    content_security_policy: "connect-src 'self'".to_owned(),
                },
            ),
        ]);
        let main = policy_for_webview(&policies, Some(&1)).unwrap();
        let settings = policy_for_webview(&policies, Some(&2)).unwrap();
        assert_eq!(main.window_id, "main");
        assert_eq!(settings.window_id, "settings");
        assert_eq!(main.content_security_policy, "connect-src 'self'");
        assert_eq!(
            settings.content_security_policy,
            "connect-src https://docs.example"
        );
        assert!(token_matches(main, Some("main-token")));
        assert!(!token_matches(main, Some("settings-token")));
        assert!(!token_matches(main, None));
        assert!(policy_for_webview(&policies, None::<&i32>).is_none());
        assert!(policy_for_webview(&policies, Some(&3)).is_none());
    }

    #[test]
    fn control_response_wait_is_bounded_and_handles_disconnects() {
        let (_sender, receiver) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
        assert!(
            receive_control_response(&receiver, Duration::ZERO)
                .unwrap_err()
                .starts_with("window.control-timeout:")
        );
        let (sender, receiver) = std::sync::mpsc::sync_channel::<Result<u8, String>>(1);
        sender.send(Ok(7)).unwrap();
        assert_eq!(
            receive_control_response(&receiver, Duration::ZERO).unwrap(),
            7
        );
        drop(sender);
        assert_eq!(
            receive_control_response(&receiver, Duration::ZERO).unwrap_err(),
            "window control response channel was closed"
        );
    }
}
