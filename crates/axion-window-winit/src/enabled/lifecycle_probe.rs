use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use super::{AppState, WebViewPolicy, WinitRunError};

const SCHEMA: &str = "axion.lifecycle-probe.v1";
const MISSING_ASSET: &str = "__axion_lifecycle_probe_missing__.txt";
static STARTED: OnceLock<Instant> = OnceLock::new();
static RESOURCE_FAILED: AtomicBool = AtomicBool::new(false);
static CLOSE_CONFIRMED: AtomicUsize = AtomicUsize::new(0);
static RENDER_FAILED: AtomicBool = AtomicBool::new(false);

struct Witness {
    state: Weak<AppState>,
    registry: Arc<Mutex<BTreeMap<String, winit::window::WindowId>>>,
    policies: Arc<Mutex<BTreeMap<servo::WebViewId, WebViewPolicy>>>,
    peak_windows: usize,
}

thread_local! {
    static WITNESS: RefCell<Option<Witness>> = const { RefCell::new(None) };
}

fn scenario() -> Option<String> {
    std::env::var("AXION_LIFECYCLE_PROBE").ok().filter(|value| {
        matches!(
            value.as_str(),
            "normal-close" | "partial-window-fail" | "resource-fail" | "render-fail"
        )
    })
}

fn record(phase: &str, detail: serde_json::Value) {
    let Some(scenario) = scenario() else {
        return;
    };
    let started = STARTED.get_or_init(Instant::now);
    let value = serde_json::json!({
        "schema": SCHEMA,
        "source": "backend",
        "scenario": scenario,
        "phase": phase,
        "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
        "detail": detail,
    });
    println!("{value}");
    let _ = std::io::stdout().flush();
}

pub(super) fn capture_state(state: &Rc<AppState>) {
    if scenario().is_none() {
        return;
    }
    WITNESS.with(|slot| {
        *slot.borrow_mut() = Some(Witness {
            state: Rc::downgrade(state),
            registry: state.window_registry.clone(),
            policies: state.webview_policies.clone(),
            peak_windows: 0,
        });
    });
    record("state.created", serde_json::json!({}));
}

pub(super) fn before_window() -> Result<(), WinitRunError> {
    if scenario().as_deref() == Some("partial-window-fail")
        && WITNESS.with(|slot| {
            slot.borrow()
                .as_ref()
                .is_some_and(|witness| witness.peak_windows == 1)
        })
    {
        record(
            "failure.injected",
            serde_json::json!({"kind": "partial-window", "synthetic": true}),
        );
        return Err(WinitRunError::CreateWindow(
            "lifecycle probe synthetic partial-window failure".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn window_registered(state: &AppState) {
    if scenario().is_none() {
        return;
    }
    let count = state.windows.borrow().len();
    WITNESS.with(|slot| {
        if let Some(witness) = slot.borrow_mut().as_mut() {
            witness.peak_windows = witness.peak_windows.max(count);
        }
    });
    record("window.registered", serde_json::json!({"count": count}));
}

pub(super) fn state_dropping(windows_before_clear: usize) {
    record(
        "state.drop_begin",
        serde_json::json!({"windows_before_clear": windows_before_clear}),
    );
}

pub(super) fn before_redraw() -> Result<(), WinitRunError> {
    if scenario().as_deref() == Some("render-fail") && !RENDER_FAILED.swap(true, Ordering::SeqCst) {
        record(
            "failure.injected",
            serde_json::json!({"kind": "render", "synthetic": true}),
        );
        return Err(WinitRunError::MakeCurrent(
            "lifecycle probe synthetic render failure".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn asset_failed(path: &str) {
    if scenario().as_deref() == Some("resource-fail")
        && path.rsplit('/').next() == Some(MISSING_ASSET)
    {
        RESOURCE_FAILED.store(true, Ordering::SeqCst);
        record(
            "resource.failed",
            serde_json::json!({"asset": MISSING_ASSET}),
        );
    }
}

pub(super) fn close_confirmed() {
    if scenario().is_some() {
        CLOSE_CONFIRMED.fetch_add(1, Ordering::SeqCst);
        record("close.confirmed", serde_json::json!({}));
    }
}

pub(super) fn finish(result_ok: bool) {
    if scenario().is_none() {
        return;
    }
    let detail = WITNESS.with(|slot| {
        let witness = slot.borrow_mut().take();
        match witness {
            Some(witness) => serde_json::json!({
                "state_observed": true,
                "state_released": witness.state.upgrade().is_none(),
                "window_registry_count": witness.registry.lock().ok().map(|map| map.len()),
                "webview_policy_count": witness.policies.lock().ok().map(|map| map.len()),
                "peak_window_count": witness.peak_windows,
                "resource_failure_observed": RESOURCE_FAILED.load(Ordering::SeqCst),
                "close_confirmed_count": CLOSE_CONFIRMED.load(Ordering::SeqCst),
                "result_ok": result_ok,
            }),
            None => serde_json::json!({"state_observed": false, "result_ok": result_ok}),
        }
    });
    record("backend.finished", detail);
}

pub(super) fn script_source() -> Option<String> {
    let scenario = scenario()?;
    let hold_ms = std::env::var("AXION_LIFECYCLE_PROBE_HOLD_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        .min(60_000);
    let config = serde_json::json!({"scenario": scenario, "holdMs": hold_ms});
    Some(format!(
        "const axionLifecycleProbeConfig = {config};\n{}",
        SCRIPT
    ))
}

const SCRIPT: &str = r#"
(() => {
  if (window.__AXION_LIFECYCLE_PROBE_STARTED__) return;
  window.__AXION_LIFECYCLE_PROBE_STARTED__ = true;
  const run = async () => {
    const bridge = window.__AXION__;
    const record = detail => bridge.invoke('probe.record', detail);
    await bridge.invoke('app.ping', { from: 'lifecycle-probe' });
    await record({ phase: 'frontend.ready' });
    if (axionLifecycleProbeConfig.scenario === 'partial-window-fail' ||
        axionLifecycleProbeConfig.scenario === 'render-fail') return;
    if (axionLifecycleProbeConfig.scenario === 'resource-fail') {
      let failed = false;
      try {
        const response = await fetch('/__axion_lifecycle_probe_missing__.txt');
        failed = !response.ok;
      } catch (_) {
        failed = true;
      }
      if (!failed) throw new Error('expected the probe asset request to fail');
      await record({ phase: 'resource.failure' });
    }
    if (axionLifecycleProbeConfig.holdMs > 0) {
      await new Promise(resolve => setTimeout(resolve, axionLifecycleProbeConfig.holdMs));
    }
    const close = await bridge.invoke('window.close', {});
    if (!close || typeof close.requestId !== 'string') {
      throw new Error('window.close did not return a close request id');
    }
    await record({ phase: 'close.requested', requestId: close.requestId });
    await bridge.invoke('window.confirm_close', { requestId: close.requestId });
  };
  const start = () => run().catch(async error => {
    try {
      await window.__AXION__.invoke('probe.record', {
        phase: 'frontend.error', message: String(error)
      });
    } catch (_) {}
  });
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start, { once: true });
  } else {
    queueMicrotask(start);
  }
})();
"#;
