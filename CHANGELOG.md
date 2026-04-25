# Changelog

## v0.1.4.0 - Preview

Axion v0.1.4.0 completes the native dialog preview milestone on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.4`.
- Axion public release metadata is `v0.1.4.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `[native.dialog] backend = "headless" | "system"` manifest configuration.
- Added `NativeConfig`, `DialogConfig`, and dialog backend types to `axion-core`.
- `dialog.open` and `dialog.save` now dispatch through a runtime dialog backend abstraction.
- The default `headless` backend returns deterministic canceled responses with `backend = "headless"`.
- The preview `system` backend opens macOS dialogs through `osascript` and reports `system-unavailable` on unsupported platforms.
- `dialog.open` now accepts preview `multiple`, `directory`, and `filters` request metadata; `dialog.save` validates unsupported combinations instead of silently ignoring them.
- `axion doctor`, `axion self-test`, examples, and generated app templates now surface configured versus effective native dialog backend state.

### Deferred

- Rich dialog options such as filters, directory selection, multi-select, and save overwrite policy.
- Windows/Linux native dialog implementations beyond the current unavailable fallback.
- Automated GUI CI across platforms.

## v0.1.3.0 - Preview

Axion v0.1.3.0 completes the native application polish and packaging quality milestone on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.3`.
- Axion public release metadata is `v0.1.3.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Manifest `[bundle] icon = "path/to/icon"` support for project-relative bundle icons.
- `axion doctor` and `axion self-test` validate configured bundle icons.
- `axion bundle` copies configured icons into bundle resources and records them in bundle metadata.
- Example apps and `axion new --template vanilla` include a default `icons/app.icns` bundle icon.
- `axion bundle` writes `axion-bundle-manifest.json` with bundle target, entry, metadata, executable, icon, file sizes, and content fingerprints.
- `axion bundle` performs post-stage verification of required bundle paths, file sizes, and content fingerprints before reporting success.
- Bundle manifests now include per-file `fnv1a64` content fingerprints, and verification detects same-size file tampering.

### Deferred

- Installer generation, signing, notarization, auto-updates, and platform store packaging.
- Native dialog backend implementation beyond the current headless-safe preview stubs.
- Automated GUI CI across platforms.

## v0.1.2.0 - Preview

Axion v0.1.2.0 completes the next developer-preview feature milestone on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.2`.
- Axion public release metadata is `v0.1.2.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Generated vanilla apps now include a `demo.greet` custom Rust command plugin, manifest capability, frontend invocation, startup event listener, and denied-command probe.
- Added `docs/custom-commands.md` with plugin registration, capability, frontend invocation, startup event, and validation guidance.
- Exposed `axion_runtime::json_string_literal` for small JSON command responses.
- Added optional app metadata fields in `axion.toml`: `version`, `description`, `authors`, and `homepage`.
- `axion dev` now prints launch mode, dev-server status, packaged fallback status, and per-window entry URLs before the runtime plan.
- Generated vanilla apps now include a CSP-compatible `frontend/style.css` and clearer card-based bridge, native API, event, custom-command, and capability-denial demos.
- Generated vanilla apps now include `.gitignore` and install Axion panic reporting to `target/axion/crash-reports/`.
- `axion self-test` now prints app metadata and per-window capability/runtime summaries.

### Changed

- `app.info`, `axion doctor`, runtime plans, generated templates, and bundle metadata now surface app metadata.
- `axion dev --launch --fallback-packaged` validates packaged asset availability before selecting production-mode fallback.
- Generated vanilla manifests now allow preview `dialog.open` and `dialog.save` commands so the template demonstrates the current native API surface.

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
