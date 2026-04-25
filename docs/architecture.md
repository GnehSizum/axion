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

## Crate Boundaries

- `axion-core` does not expose Servo internals.
- `axion-runtime` orchestrates framework behavior and delegates desktop details.
- `axion-window-winit` owns Servo/winit integration.
- `axion-bridge` owns JavaScript bridge naming, payload validation, and dispatch contracts.
- `axion-cli` provides developer workflows without becoming part of the runtime API.

## Native Preview Layer

`axion-core` owns native configuration such as `[native.dialog]`. `axion-manifest` parses it, and `axion-runtime` resolves it into capability-gated bridge commands. The default dialog backend is `headless` for deterministic self-tests; `system` is a preview backend that currently opens macOS file dialogs and cancels as `system-unavailable` elsewhere.

## Version Scope

v0.1.4.0 completes the native dialog preview milestone on top of the generated-app workflow, capability-gated native command surface, strict dev-server launch checks, executable-aware bundling, multi-window diagnostics, and packaging quality completed through v0.1.3.0. The release adds manifest-driven dialog backend selection, headless-safe dialog responses, macOS system dialog preview behavior, and diagnostic reporting. Axion public versions use four components to separate Servo baseline, Axion feature milestones, and bugfix releases; Cargo crates keep SemVer-compatible three-component versions. Installer generation, signing, auto-updates, broader native API coverage, and CI GUI integration tests remain later milestones.
