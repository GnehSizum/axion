# Changelog

## v0.1.12.0 - Preview

Axion v0.1.12.0 tightens capability and security diagnostics on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.12`.
- Axion public release metadata is `v0.1.12.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `security.summary`, `security.window.*`, `security.notice.*`, `security.warning.*`, and `security.recommendation.*` lines to `axion doctor`.
- Added `axion doctor --json` with structured `diagnostics.security` output in the existing diagnostics report schema.
- Added per-window security risk classification for disabled, bridge-enabled, remote-origin, and broad remote-navigation capability sets.
- Added command category summaries for app, window, filesystem, dialog, and custom command capabilities.
- Added warnings for bridge command/event declarations without `protocols = ["axion"]`.
- Added warnings for nonstandard protocol capabilities and redundant `allowed_navigation_origins` when `allow_remote_navigation = true`.

### Changed

- Security documentation now includes minimum capability patterns, remote-navigation guidance, and CI-friendly `doctor` checks.

### Deferred

- Policy profiles, signed permission manifests, and automatic capability minimization.

## v0.1.11.0 - Preview

Axion v0.1.11.0 improves bundle reporting, packaging documentation, and release-readiness checks on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.11`.
- Axion public release metadata is `v0.1.11.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added platform layout summaries to `axion bundle` output for macOS `.app`, Linux directory, and Windows directory bundle scaffolds.
- Added richer bundle verification statistics: checked directories, checked files, fingerprinted files, and total bundle bytes.
- Added `bundle.target`, `bundle.layout`, and `bundle.metadata` diagnostics to `axion doctor`.
- Added `docs/packaging.md` with bundle layout, icon, executable, verification, and release checklist guidance.
- Generated app READMEs now include a clearer packaging preview and release validation path.

### Changed

- Bundle target names now use stable string values such as `macos-app`, `linux-dir`, and `windows-dir` in CLI output.
- Bundle icon diagnostics now include the detected file extension format.

### Deferred

- Signed installers, notarization, auto-updates, and platform store packaging.
- Cross-platform installer generation.

## v0.1.10.0 - Preview

Axion v0.1.10.0 stabilizes the development loop and window lifecycle diagnostics on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.10`.
- Axion public release metadata is `v0.1.10.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added debounce handling to `axion dev --watch` so quick editor save bursts are grouped before diagnostics are printed.
- Added watch ignore rules for common temporary files and cache directories such as `.DS_Store`, swap files, logs, `node_modules`, `.vite`, `.next`, `.turbo`, `.git`, and `target`.
- Added `reload_applied`, `reload_deferred`, and `restart_required` diagnostics for `axion dev --watch --reload`; when launched with Servo runtime, watched file changes now request a reload on each live window.
- Added `WindowControlRequest::Reload`, `axion_runtime::reload_window`, and the capability-gated `window.reload` bridge command.
- Added `window.ready` as a built-in listen-only host lifecycle event after `window.created`.
- Generated apps, `hello-axion`, and `multi-window` now expose lifecycle/reload capabilities for development-loop validation.

### Changed

- Runtime diagnostics now include `window.ready` in per-window lifecycle event lists and host event summaries.
- `axion dev --watch` startup output now reports the poll interval, debounce interval, and initial watched file count.
- `axion dev --watch --reload` without `--launch` now clearly reports that no live window control target is available.
- `axion dev --watch --reload --launch` now reports `restart_required` when a live backend cannot apply reload in place.

### Deferred

- Full restart fallback for backends that cannot reload an existing WebView.
- Servo devtools integration.
- Installer generation, signing, notarization, auto-updates, and platform store packaging.

## v0.1.9.0 - Preview

Axion v0.1.9.0 completes the development-server diagnostics milestone on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.9`.
- Axion public release metadata is `v0.1.9.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added explicit `axion dev` preview flags: `--watch`, `--reload`, and `--open-devtools`. `--watch` now polls frontend assets, `--reload` reports reload requests when watched files change, and `--open-devtools` reports current unsupported status.
- Added `axion dev` next-step diagnostics for unconfigured, invalid, unreachable, reachable, and packaged-fallback launch states.
- Added `axion dev --launch` launch summaries showing selected mode, packaged fallback status, window ids, and final entry URLs.
- Added frontend process management through `--frontend-command`, `--frontend-cwd`, and `--dev-server-timeout-ms`, with dev server wait diagnostics and early-exit stderr summaries.
- Added optional `[dev] command`, `cwd`, and `timeout_ms` manifest fields; CLI options take precedence.
- Added generated-template documentation for frontend development and commented `[dev]` manifest configuration.
- Added no-dependency frontend asset polling for `axion dev --watch`, including created, modified, and deleted file diagnostics.

### Changed

- `axion dev` launch errors now point to starting the frontend server, checking `[dev].url`, using `--frontend-command`, or using `--fallback-packaged`.
- Manifest and getting-started docs now describe dev-server URL requirements, managed frontend commands, multi-window behavior, packaged fallback, watch/reload diagnostics, and reserved devtools behavior.

### Deferred

- Backend hot reload that refreshes Servo windows automatically.
- Servo devtools integration.
- Installer generation, signing, notarization, auto-updates, and platform store packaging.

## v0.1.8.0 - Preview

Axion v0.1.8.0 completes the GUI smoke command and optional CI artifact milestone on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.8`.
- Axion public release metadata is `v0.1.8.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `axion-cli gui-smoke` to launch a Servo-backed app with `AXION_GUI_SMOKE=1`, capture the returned diagnostics report, and optionally write it with `--report-path`.
- Added `--timeout-ms` and `--quiet` options for GUI smoke automation.
- Added `--cargo-target-dir`, `--serial-build`, and repeatable `--build-env KEY=VALUE` options for generated-app GUI smoke runs and resource-constrained Servo builds.
- Added failure report generation for `axion gui-smoke --report-path` when the GUI process exits unsuccessfully or does not print a valid diagnostics report.
- Added an optional `workflow_dispatch` GitHub Actions GUI smoke job that runs under `xvfb` and uploads the GUI diagnostics artifact.
- Added GUI smoke hooks for generated vanilla apps, `hello-axion`, and `file-access-demo`.

### Changed

- `examples/bridge-diagnostics-demo` smoke checks now include stable `id`, `label`, `status`, and `detail` fields.
- The optional GUI smoke workflow now collects bridge diagnostics, hello, and file-access reports.
- `axion gui-smoke` failure reports now include process context such as failure phase, help text, status code, timeout, Cargo manifest path, target directory, build environment keys, stdout, and stderr.
- Diagnostics documentation now describes CLI GUI smoke usage, GUI smoke reports, and stable smoke check fields.

### Deferred

- Enabling GUI smoke as a default pull-request gate.
- Cross-platform GUI CI coverage beyond the optional Linux job.
- Installer generation, signing, notarization, auto-updates, and platform store packaging.

## v0.1.7.0 - Preview

Axion v0.1.7.0 completes the diagnostics model and local GUI smoke milestone on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.7`.
- Axion public release metadata is `v0.1.7.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `axion_runtime::DiagnosticsReport` and `DiagnosticsWindowReport` as the shared Rust model for `axion.diagnostics-report.v1`.
- Bridge bootstrap now exposes `window.__AXION__.diagnostics.reportSchema` so frontends can use the active diagnostics schema string.
- Added `AXION_GUI_SMOKE=1` local GUI smoke mode for Servo-backed runs that call `window.__AXION_GUI_SMOKE__()`, print the returned report, and exit.
- Added `AXION_GUI_SMOKE_TIMEOUT_MS` and `AXION_SELFTEST_TIMEOUT_MS` timeout controls for GUI smoke and bridge self-test runs.

### Changed

- `axion self-test --json` and `--report-path` now serialize through the shared runtime diagnostics report model instead of CLI-local report string assembly.
- `examples/bridge-diagnostics-demo` now aligns its GUI-exported report with the shared top-level fields, including `result`.
- Diagnostics report docs now cover the shared model, schema exposure, GUI preview fields, and local GUI smoke usage.

### Deferred

- Running GUI smoke in GitHub CI by default.
- Installer generation, signing, notarization, auto-updates, and platform store packaging.

## v0.1.6.0 - Preview

Axion v0.1.6.0 completes the CI and diagnostics stabilization milestone on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.6`.
- Axion public release metadata is `v0.1.6.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- `axion doctor` now reports Axion CLI version, public release, MSRV, and whether the active `rustc` satisfies the workspace `rust-version`.
- `axion self-test --quiet` supports CI usage with `--report-path` when only the exit code and diagnostics report artifact are needed.
- Public diagnostics report documentation now describes `axion.diagnostics-report.v1` producers, top-level fields, window fields, and CI usage.
- CI now validates every checked-in example manifest, writes a diagnostics JSON report, checks the report schema, and uploads doctor/report artifacts.

### Changed

- CI release gates are aligned with Rust `1.86.0`, the workspace MSRV.
- `axion self-test --report-path` now prints the report path in human-readable output.
- `axion-runtime` exposes the public Axion release constant for CLI diagnostics.

### Deferred

- Automated GUI CI across platforms.
- A typed Rust JSON model shared between GUI-exported diagnostics and CLI `self-test` reports.
- Installer generation, signing, notarization, auto-updates, and platform store packaging.

## v0.1.5.0 - Preview

Axion v0.1.5.0 completes the frontend diagnostics and compatibility milestone on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.5`.
- Axion public release metadata is `v0.1.5.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added shared bridge compatibility helpers under `window.__AXION__.compat`, including `installTextInputSelectionPatch` for Servo-backed text controls.
- Added bridge diagnostics helpers under `window.__AXION__.diagnostics`: `describeBridge`, `snapshotTextControl`, and `toPrettyJson`.
- Added `examples/file-access-demo` as a focused app-data filesystem and dialog capability demo.
- Added `examples/bridge-diagnostics-demo` for bridge snapshots, host events, text-input diagnostics, visual smoke checks, and export/reload of diagnostics reports.
- `axion self-test` can now emit machine-readable `axion.diagnostics-report.v1` JSON with `--json` or write it with `--report-path`.
- Generated vanilla apps and examples now reuse the shared bridge helpers instead of carrying local compatibility code.

### Changed

- Frontend examples have clearer external style loading and more complete input, file, dialog, event, and diagnostics flows.
- Documentation now covers bridge compatibility helpers, diagnostics helpers, report export, and the v0.1.5.0 version baseline.

### Deferred

- Automated GUI CI across platforms.
- A typed Rust JSON model shared between GUI-exported diagnostics and CLI `self-test` reports.
- Deeper engine-level text control fixes inside the vendored Servo subtree.

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
