# Axion Documentation

This directory contains public, user-facing documentation for Axion.

## Start Here

- `getting-started.md`: create and run a minimal Axion app.
- `cli.md`: command reference for `axion-cli`.
- `manifest.md`: `axion.toml` configuration guide.
- `packaging.md`: bundle layouts, verification, icons, and release checks.
- `native-api.md`: built-in bridge command reference.
- `diagnostics-report.md`: machine-readable diagnostics report schema.
- `release-checks.md`: pre-release and generated-app validation checklist.
- `custom-commands.md`: Rust command registration and frontend invocation.
- `versioning.md`: public release and Cargo version mapping.
- `architecture.md`: high-level runtime architecture.
- `security.md`: capabilities, bridge permissions, navigation, and CSP.
- `../CONTRIBUTING.md`: contributor workflow and local checks.
- `../SECURITY.md`: vulnerability reporting and policy summary.

## Current Version

Axion is at **v0.1.19.0 developer preview**. The current preview focuses on the core desktop framework loop:

1. load an app manifest
2. build a runtime plan
3. stage frontend assets
4. inject a controlled JavaScript bridge
5. launch a Servo-backed `winit` window when `servo-runtime` is enabled
6. generate, validate, fingerprint, bundle, and inspect release-ready application scaffolds through `axion-cli`
7. configure preview file-dialog and clipboard backends through `[native.dialog]` and `[native.clipboard]`
8. validate controlled clipboard, app-data filesystem, and dialog flows through examples and generated apps
9. reuse bridge-provided text-input compatibility helpers in examples and generated apps
10. inspect bridge snapshots and run frontend diagnostics through `examples/bridge-diagnostics-demo`
11. export machine-readable diagnostics through GUI examples and `axion self-test --json`
12. validate release gates through MSRV-aware `axion doctor`, CI example self-tests, and diagnostics artifacts
13. reuse a shared diagnostics report model and run GUI smoke checks for examples and generated apps through `axion-cli gui-smoke`
14. optionally run Servo-backed GUI smoke in GitHub Actions through `workflow_dispatch`
15. inspect frontend dev-server readiness, run external frontend commands, watch frontend assets with debounce/ignore rules, reload live windows during `--launch`, use packaged fallback, and inspect reserved devtools behavior through `axion-cli dev`
16. observe built-in host lifecycle events such as `app.ready`, `window.created`, `window.ready`, focus, resize, move, and close events from frontend code
17. close windows and request application shutdown through capability-gated lifecycle commands
18. inspect per-window capability risk, remote-navigation scope, protocol consistency, profile expansion, command categories, release readiness, and CI gate results through `axion doctor` or `doctor --json`
19. run the default generated-app validation loop through `axion check`
20. export stable bundle reports through `axion bundle --json` and `--report-path`
21. run a preview artifact workflow through `axion release`, including artifact inventory and archive verification

Project-internal milestone plans and release notes are intentionally not part of the public documentation set.
