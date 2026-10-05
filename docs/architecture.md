# Architecture Overview

Axion treats Servo as a vendored rendering engine and exposes an Axion-owned application framework boundary.

```text
Application
  ├─ Rust entrypoint
  ├─ frontend assets
  └─ axion.toml
        ↓
Axion framework crates
  ├─ axion-core
  ├─ axion-manifest
  ├─ axion-runtime
  ├─ axion-bridge
  ├─ axion-security
  ├─ axion-protocol
  ├─ axion-packager
  └─ axion-cli
        ↓
Desktop backend
  └─ axion-window-winit + Servo embedder APIs
        ↓
Servo engine
```

## Runtime Flow

1. `axion-manifest` loads and validates `axion.toml`.
2. `axion-core` builds an app model and runtime plan.
3. `axion-runtime` converts the app into launch diagnostics and window bindings.
4. `axion-security` derives per-window policy for commands, events, protocols, navigation, and CSP.
5. `axion-protocol` serves packaged assets through `axion://app`.
6. `axion-window-winit` creates native windows, Servo webviews, and injects the bridge bootstrap.

## Multi-Window Model

Each manifest window receives its own native window, bridge token, command registry, event registry, and security policy. The same frontend entry can be reused across windows, but `window.__AXION__.commands`, `window.__AXION__.events`, and `window.__AXION__.hostEvents` are scoped to the active window.

The backend registers CSP and bridge-token bindings by Servo's native WebView id before the initial request reads them. Asset responses use that native identity to select CSP. Bridge requests require both the caller's token and the same native WebView identity; `Origin` and `Referer` remain metadata. An authorized `target` payload can still address another window, with permissions taken from the caller. Tokens remain stable across navigation and are removed when the WebView closes; only trusted app/development origins receive bootstrap installation.

WebView delegates hold a weak reference to application state. When startup fails or the run loop returns, window ownership can be released without a delegate cycle. Close requests store deadlines that drive the event loop's `WaitUntil`, avoiding a sleeping thread per request. The backend tracks one pending application exit: repeated requests reuse it, completion records each close once, and prevention cancels the remaining closes. Built-in `app.exit` and `window.*` commands use a separate bounded control pool (two workers, 32 queued jobs), while filesystem, clipboard, dialog, and shell work use the native I/O pool (four workers, 64 queued jobs). Long-running I/O cannot consume control workers; either full queue returns `bridge.busy`. Built-in control requests have a total five-second deadline from submission to the control pool, including queue time and event-loop response time. The same deadline reaches the native executor; expired queued requests skip execution, and waiters are woken even while both control workers are occupied. A single process-wide timer drives those deadlines without requiring a Tokio caller runtime. Requests refuse waits on the event-loop thread. A timeout does not undo side effects from work that already started. Lifecycle event names are defined once in `axion-bridge::lifecycle` for runtime diagnostics and backend dispatch.

## Crate Boundaries

- `axion-core` does not expose Servo internals.
- `axion-runtime` orchestrates framework behavior and delegates desktop details.
- `axion-window-winit` owns Servo/winit integration.
- `axion-bridge` owns JavaScript bridge naming, payload validation, dispatch contracts, and small frontend compatibility helpers exposed by the bootstrap.
- `axion-cli` provides developer workflows without becoming part of the runtime API.

## Native Preview Layer

`axion-core` owns native configuration such as `[native.dialog]` and `[native.clipboard]`. `axion-manifest` parses it, and `axion-runtime` resolves it into capability-gated bridge commands. The default dialog backend is `headless` for deterministic self-tests; `system` is a preview backend that currently opens macOS file dialogs and cancels as `system-unavailable` elsewhere. The default clipboard backend is `memory`; `system` uses macOS `pbcopy` / `pbpaste` and falls back to `memory` on unsupported platforms. Shell URL launch is exposed separately through the `shell-access` profile and `shell.open`.

Servo DOM text editing uses a separate clipboard delegate in the desktop backend. Its `clipboard` Cargo feature enables operating system Copy/Cut/Paste, with an independent in-process fallback on access failure. The Axion `[native.clipboard]` setting and `clipboard-access` profile apply to bridge `clipboard.*` commands, not this DOM editing path. Enabling the engine Cargo feature does not itself enable the asynchronous `navigator.clipboard` API.

## Version Scope

The current Axion release is `v0.6.2`, using the vendored Servo `0.6.0` engine and retaining Axion feature milestone `33`.

v0.1.33.0 starts Native API Expansion with a capability-gated `shell.open` command for validated `http`, `https`, and `mailto` targets. v0.1.32.0 completed the previous Runtime Hardening slice by letting `gui-smoke` require bridge commands, host lifecycle events, and expected window entries in addition to named smoke checks. Missing runtime coverage now produces CLI failure diagnostics with `diagnostics.required_runtime` while preserving the original frontend report for inspection. v0.1.31.0 started this hardening phase by requiring named smoke checks such as `bridge.bootstrap`, `app.ping`, and `input.snapshot`. v0.1.30.0 made report consumption stricter for CI: `axion report` rejects unsupported schemas, incomplete JSON objects, and reports without a top-level `result`, while `--allow-failed` remains available for artifact-summary steps. `release --check-report-path` can reuse expensive self-test state only when configuration, frontend, framework version, and gate parameters match; doctor and readiness always run against the current inputs. `check` reports ordered typed next actions and a stable capability summary, while `gui-smoke` failure diagnostics include failure-phase guidance and smoke-check error-code summaries. Bridge error envelopes still use stable `{ code, message }` objects while preserving thrown `Error(message)` compatibility. The Native API examples and generated app smoke paths validate both successful filesystem lifecycle operations and expected error codes through the same GUI smoke reports. `doctor`, `check`, `bundle`, `release`, `report`, and `dev` provide complementary human and machine-readable views of the same development-to-release path: manifest readiness, lightweight validation, staged bundle layout, platform metadata, optional verified tar artifact, artifact inventory, dev-server readiness, launch/restart counters, first failure diagnostics, verification counters, and native-capability security warnings. Axion public releases and Cargo crates now use matching three-part SemVer versions; older four-part public tags remain historical releases. Signed permission manifests, automatic capability minimization, signed installers, cross-platform system clipboard integration, auto-updates, broader native API coverage, devtools integration, and default cross-platform GUI CI remain later milestones.

Engine provenance, the frozen upstream archive checksum, feature choices, and platform toolchain notes are tracked in [`SERVO_PROVENANCE.toml`](../SERVO_PROVENANCE.toml). The vendored source remains unchanged by framework remediation.
