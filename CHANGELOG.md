# Changelog

## v0.1.1.0 - Preview

Axion v0.1.1.0 completes the next developer-preview milestone with app generation, capability-gated native APIs, stricter dev workflows, executable-aware bundling, and multi-window validation.

### Added

- `axion-cli new --template vanilla` generates a runnable Axion application with frontend assets, manifest capabilities, Rust entrypoint, and a project README.
- Built-in bridge commands for `app.version`, `fs.read_text`, `fs.write_text`, plus preview `dialog.open` and `dialog.save` stubs.
- App-data filesystem sandboxing under `target/axion-data/<app name>` with path traversal, absolute path, and symlink rejection.
- `axion dev --launch` reachability checks with explicit `--fallback-packaged` behavior.
- `axion bundle --build-executable` and automatic executable discovery from Cargo build output.
- Expanded `multi-window` example showing per-window commands, frontend events, host events, trusted origins, and denied command probes.
- CI workflow for formatting, workspace tests, and non-GUI example self-tests.

### Changed

- `doctor` and `self-test` cover more runtime diagnostics, including multi-window capability expectations.
- Public documentation covers CLI, manifest, native API, architecture, security, and getting-started workflows for v0.1.1.0.

### Deferred

- Installer generation, signing, notarization, auto-updates, and platform store packaging.
- Native dialog backend implementation beyond the current headless-safe preview stubs.
- Automated GUI CI across platforms.

## v0.1.0.0 - Preview

Axion v0.1.0.0 establishes the first usable Servo-backed desktop framework preview.

### Added

- Rust workspace with `axion-core`, `axion-runtime`, `axion-window-winit`, `axion-bridge`, `axion-manifest`, `axion-security`, `axion-protocol`, `axion-packager`, and `axion-cli`.
- Manifest-driven app, window, build, dev-server, and capability configuration.
- Servo/winit desktop backend with app protocol loading and multi-window launch planning.
- `window.__AXION__` bootstrap with command invocation, frontend event emit, host event listen, and guarded host dispatch.
- Window-level capability filtering for commands, frontend events, `axion` protocol access, trusted origins, and navigation origins.
- `axion://app` asset resolver with content type, cache, `nosniff`, referrer, CORP, and CSP response policy.
- Runtime diagnostics, `axion doctor`, panic report support, and lifecycle event reporting.
- Build and bundle staging with generated `axion-assets.json`.
- `axion self-test` release gate for manifest/runtime/build staging validation.
- `hello-axion` and `multi-window` examples.

### Security

- Bridge payloads must be valid JSON values.
- Bridge request envelopes enforce command/event name, request id, and payload size boundaries.
- Frontend `emit` events and host-dispatched events are separated.
- Host event dispatch requires the current window bridge token.
- Remote navigation is denied by default unless explicitly allowed.

### Deferred

- Installer generation, signing, notarization, and platform store packaging.
- Full native API surface such as filesystem dialogs.
- Automated GUI CI across platforms.
