# Changelog

## v0.1.28.0 - Preview

Axion v0.1.28.0 improves the development validation loop on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.28`.
- Axion public release metadata is `v0.1.28.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added ordered `next_steps` to `axion check --json` while preserving the existing `next_step` field.
- Added `failure_phase` and typed required/optional `next_actions` to `axion check --json` for CI routing.
- Added `axion report` to summarize existing check, release, bundle, and GUI diagnostics reports.
- Added `release --check-report-path` to reuse a matching successful check report for doctor, readiness, and self-test state.
- Added more specific `check` remediation guidance for doctor failures, readiness blockers, bundle preflight errors, dev preflight blockers, GUI smoke setup, and release artifacts.
- Added GUI smoke failure diagnostics with `next_step`, `failed_check_ids`, and `error_codes` fields in CLI-generated failure reports.
- Added GUI smoke summaries that include failed check ids and extracted error codes.

### Changed

- Generated app Cargo versions now use `0.1.28`.
- Generated app README and next steps now separate app-local commands from Axion-checkout validation commands and include GUI smoke plus release report commands.
- Updated public docs and release metadata for v0.1.28.0.

## v0.1.27.0 - Preview

Axion v0.1.27.0 formalizes bridge error envelopes and Native API error diagnostics on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.27`.
- Axion public release metadata is `v0.1.27.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added structured bridge error envelopes with `{ code, message }` while keeping thrown `Error(message)` compatibility.
- Added `window.__AXION__.diagnostics.normalizeError(error)` for frontend examples and generated apps.
- Added structured error-code coverage to `file-access-demo` and generated `native-api-demo` GUI smoke reports.
- Added stable preview error-code prefixes for clipboard, dialog, window, and app lifecycle command failures.
- Added `check --json` capability summaries with profile expansion, explicit permissions, effective permissions, navigation settings, and per-window risk.
- Added public capability profile documentation and generated-app README guidance for least-privilege inspection.

### Changed

- Generated app Cargo versions now use `0.1.27`.
- Updated public docs and release metadata for v0.1.27.0.

## v0.1.26.0 - Preview

Axion v0.1.26.0 expands app-data filesystem coverage and tightens generated-app and checked-in example guidance on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.26`.
- Axion public release metadata is `v0.1.26.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `template_focus` output to `axion new` so generated project logs show the selected template's intended coverage.
- Added GUI smoke follow-up guidance after generated project `--run-check` so developers run Servo-backed checks from the Axion checkout with a shared `target` directory.
- Added capability-gated app-data filesystem lifecycle commands: `fs.create_dir`, `fs.exists`, `fs.list_dir`, and `fs.remove`.
- Added file lifecycle and expected-error coverage to the generated native API demo and `file-access-demo` GUI smoke paths.
- Added stable preview file error code prefixes such as `fs.invalid-path`, `fs.not-found`, `fs.not-directory`, and `fs.directory-not-empty`.
- Added `axion doctor` warning `remote_origin_native_capability` for remote-navigable windows that expose file, clipboard, or dialog APIs.
- Added bundle preview commands to checked-in example READMEs.
- Added unit coverage that keeps generated template focus text and example README validation commands from drifting.

### Changed

- Expanded the `file-access` profile to cover controlled create, exists, list, read, remove, and write operations.
- Generated app Cargo versions now use `0.1.26`.
- Updated public docs and release metadata for v0.1.26.0.

## v0.1.25.0 - Preview

Axion v0.1.25.0 adds a focused generated native API demo template on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.25`.
- Axion public release metadata is `v0.1.25.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `axion new --template native-api-demo` for a generated no-dependency app focused on app/window metadata, clipboard, app-data filesystem, dialogs, input compatibility, and GUI smoke diagnostics.
- Added template-specific generated README guidance, manifest description, UI copy, and GUI smoke source identifiers for the native API demo.
- Added a Native API Workbench "Run all checks" button that runs the generated app's GUI smoke checks inside the window and renders a structured result.
- Added example README files for `hello-axion`, `file-access-demo`, `multi-window`, and `bridge-diagnostics-demo` with run, check, GUI smoke, and expected-warning notes.
- Added unit coverage for native API demo template generation and generated next-step artifact commands.

### Changed

- Generated app Cargo versions now use `0.1.25`.
- Updated public docs and release metadata for v0.1.25.0.

## v0.1.24.0 - Preview

Axion v0.1.24.0 strengthens CI artifact output and dev preflight guidance on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.24`.
- Axion public release metadata is `v0.1.24.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `axion check --report-path <path>` to write stable `axion.check-report.v1` JSON reports while keeping the existing stdout mode.
- Added `artifacts[]` to `axion.check-report.v1` so CI can discover recommended check, dev, bundle, and release report paths.
- Added `dev_preflight.warnings` and `dev_preflight.recommended_commands` to separate advisory dev-loop issues from blockers.
- Added generated-app README guidance for `check --dev --bundle --report-path target/axion/reports/check.json`.
- Added unit coverage for check artifact inventory, grouped human output, check report writing, dev preflight warning/blocker classification, and recommended command output.

### Changed

- `check` human output now starts with `result` and `next_step`, then groups details under stable sections while preserving grep-friendly line prefixes.
- `check --dev` now treats unreachable or missing dev servers as warnings when packaged fallback and frontend assets remain valid.
- `check --dev` now validates `[dev] cwd` and warns when a configured frontend command relies on the default timeout.
- Generated-app next steps now include CI-style `check --json --report-path` and archived dev-session event/report paths.
- Updated public docs and release metadata for v0.1.24.0.

## v0.1.23.0 - Preview

Axion v0.1.23.0 adds archived development-session reporting on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.23`.
- Axion public release metadata is `v0.1.23.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `axion dev --report-path <path>` for stable `axion.dev-report.v1` JSON reports.
- Added dev report fields for launch mode, dev-server status, packaged fallback status, frontend command wait result, enabled dev options, launch count, restart count, next step, failure, and final result.
- Added `axion check --dev` for lightweight development-loop preflight covering dev-server status, watch-root validation, packaged fallback availability, frontend command settings, and recommended event/report artifact paths.
- Added unit coverage for dev report serialization, blocked launch diagnostics, report parent directory creation, dev preflight JSON, and the new CLI option lines.

### Changed

- Updated generated app README guidance to recommend `--restart-on-change`, `--event-log`, and `--report-path` for local development sessions.
- Updated public docs and release metadata for v0.1.23.0.

## v0.1.22.0 - Preview

Axion v0.1.22.0 strengthens the frontend development loop on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.22`.
- Axion public release metadata is `v0.1.22.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `axion dev --restart-on-change` for watched frontend changes that should relaunch the app.
- Added restart fallback behavior when `--watch --reload --restart-on-change` cannot apply live reload to all windows.
- Added restart diagnostics: `restart_requested`, `restart_exit_requested`, `restart_deferred`, and `restart_applied`.
- Added `axion dev --json-events` and `--event-log <path>` for stable `axion.dev-event.v1` JSONL watch/reload/restart events.
- Added unit coverage for restart-on-change option reporting, restart fallback selection, event-log writing, and restart diagnostic output.

### Changed

- `axion dev --launch` can now relaunch the app in the same CLI session after a restart request closes the current windows.
- Updated development-loop docs and release metadata for v0.1.22.0.

## v0.1.21.0 - Preview

Axion v0.1.21.0 stabilizes lifecycle GUI smoke coverage on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.21`.
- Axion public release metadata is `v0.1.21.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added a third `preview` window to the `multi-window` example so GUI smoke can close secondary windows without losing the reporting window.
- Added GUI smoke coverage for `window.close_completed`, `window.close_timed_out`, and `window.closed` lifecycle events.
- Added targeted close-decision helpers in the `multi-window` frontend for deterministic confirm and timeout paths.

### Changed

- Updated the `multi-window` manual close buttons to explicitly confirm targeted close requests instead of relying on implicit command-close decisions.
- Updated release metadata and public documentation for v0.1.21.0.

## v0.1.20.0 - Preview

Axion v0.1.20.0 adds an application-level exit lifecycle event on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.20`.
- Axion public release metadata is `v0.1.20.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added the listen-only `app.exit_requested` host lifecycle event, emitted when `app.exit` asks the runtime to close all windows.
- Added `app.exit_prevented` and `app.exit_completed` host lifecycle events for app-wide shutdown outcomes.
- Added a stable `requestId` to `app.exit` responses so frontend code can correlate command results with `app.exit_requested`.
- Added app exit request tracking in the winit backend, including closed, prevented, and timed-out window summaries.
- Added `window.close_prevented`, `window.close_completed`, and `window.close_timed_out` host lifecycle events for per-window close outcomes.
- Added close request/window correlation arrays to app exit outcome payloads.
- Added multi-window GUI smoke coverage for `window.close_prevented`, `app.exit` pending responses, `app.exit_requested`, and `app.exit_prevented` payloads.

### Changed

- Updated host event diagnostics and native API documentation to include application-level and window-level lifecycle outcome events.

## v0.1.19.0 - Preview

Axion v0.1.19.0 strengthens lifecycle validation and GUI smoke coverage on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.19`.
- Axion public release metadata is `v0.1.19.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added the capability-gated `app.exit` bridge command for runtime-wide application shutdown.
- Added `app-control` as a built-in capability profile for `app.exit`.
- Expanded `window-control` and `multi-window` profiles to include `window.close`.
- Added `window.confirm_close` and `window.prevent_close` for preview close confirmation, with timeout defaulting to allow close.
- Added `[native.lifecycle] close_timeout_ms` to configure preview close-confirmation timeout behavior.
- Added GUI smoke coverage for `window.close_requested`, `window.prevent_close`, duplicate close-decision errors, and close timeout payloads in the multi-window example.
- Added `axion gui-smoke` human-output summaries for returned `diagnostics.smoke_checks`, including total count and failed check ids.
- Added doctor warnings for incomplete close lifecycle command sets and suspicious close timeout values.
- Added a doctor warning for `app.exit` configurations that have no trusted close-decision command path.
- Improved `axion gui-smoke` failure classification so runtime hook failures are not reported as build failures just because Cargo printed compile progress.
- Added `close_timeout_ms` to diagnostics reports so CI can read lifecycle timeout configuration.
- Added close/exit controls and an unsaved-change prevention demo to `multi-window`, plus lifecycle capability checks to `hello-axion` and generated vanilla apps.
- Added doctor risk handling for `app.exit`, `window.close`, and remote-navigation windows with runtime-control capabilities.

### Changed

- Updated lifecycle, manifest, native API, security, versioning, and generated app documentation for close/exit behavior.

## v0.1.17.0 - Preview

Axion v0.1.17.0 adds capability-gated clipboard text commands and configurable clipboard backend diagnostics on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.17`.
- Axion public release metadata is `v0.1.17.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `clipboard.write_text` and `clipboard.read_text` built-in bridge commands using a deterministic runtime-local text clipboard backend.
- Added `[native.clipboard] backend = "memory" | "system"` with macOS `pbcopy` / `pbpaste` system clipboard support and memory fallback elsewhere.
- Added the `clipboard-access` capability profile.
- Added configured/effective clipboard backend reporting to runtime diagnostics, `axion doctor`, self-test reports, and GUI smoke reports.
- Added clipboard command categorization and remote-navigation security diagnostics to `axion doctor`.
- Added clipboard smoke checks to `hello-axion`, `bridge-diagnostics-demo`, and generated vanilla apps.

### Changed

- Updated native API, manifest, security, architecture, and generated app documentation for clipboard capabilities.

### Deferred

- Cross-platform native clipboard integrations beyond macOS are deferred; unsupported platforms use the CI-safe memory fallback.

## v0.1.16.0 - Preview

Axion v0.1.16.0 adds a preview release workflow on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.16`.
- Axion public release metadata is `v0.1.16.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `axion release` as an aggregate release-preview command for doctor gate, readiness, quiet self-test, bundle staging, optional archive generation, and machine-readable reporting.
- Added stable `axion.release-report.v1` output with doctor, readiness, self-test, embedded bundle report, optional archive metadata, `next_step`, and final result.
- Added `axion release --report-path <path>` and `--bundle-report-path <path>` for CI artifacts.
- Added `axion release --archive` to generate a dependency-free `.tar` preview artifact with byte size and `fnv1a64` fingerprint reporting.
- Added release report artifact inventory entries for release report, bundle report, bundle manifest, and archive outputs.
- Added `failure_phase` and `failed_reasons` to release reports so CI can identify the first blocking release stage.
- Added archive verification that re-reads the generated tar and checks byte count plus `fnv1a64` fingerprint before marking the archive passed.
- Added an optional GitHub Actions `release-preview` workflow_dispatch job that uploads release, bundle, and archive artifacts.

### Changed

- Generated app documentation now includes the release-preview command path.
- Release-check documentation now recommends `axion release` for the full local artifact workflow.
- Release report output now includes artifact existence, byte counts, fingerprints where stable, and archive verification status.

## v0.1.15.0 - Preview

Axion v0.1.15.0 strengthens bundle reporting and preview release validation on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.15`.
- Axion public release metadata is `v0.1.15.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `axion bundle --json` with stable `axion.bundle-report.v1` output for CI and release automation.
- Added `axion bundle --report-path <path>` to write the same bundle report to disk for CI artifacts.
- Added structured bundle report fields for target, layout, metadata paths, platform metadata paths, entry paths, icon, executable, report path, verification counters, checked paths, blockers, warnings, and final result.
- Added JSON failure output when `bundle` is blocked by release-readiness checks.
- Added explicit platform metadata artifacts to bundle verification: macOS `PkgInfo`, Linux `.desktop`, and Windows preview metadata.

### Changed

- `axion bundle --json` suppresses local build progress text before emitting JSON so downstream tools can parse stdout reliably.
- Generated app documentation now includes `bundle --json` in the release validation path.
- Packaging and release-check documentation now describe bundle report usage.

## v0.1.14.0 - Preview

Axion v0.1.14.0 tightens developer workflow readiness on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.14`.
- Axion public release metadata is `v0.1.14.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added `axion doctor` release-readiness summaries for development, bundle, and GUI smoke readiness.
- Added structured `diagnostics.readiness` output to `doctor --json`.
- Added readiness checks for build assets, runtime diagnostics, security warnings, bundle icons, Servo source discovery, bridge enablement, and GUI smoke hooks.
- Added `axion check` as a lightweight aggregate command for doctor gate, readiness, self-test staging, optional bundle preflight, human `next_step`, and `--json` CI output.

### Changed

- Updated generated app Cargo version metadata for v0.1.14.0.
- Updated `axion new` success output and generated README validation commands to point at `axion check`.
- Added `axion new --run-check` for an immediate generated-app validation pass.
- `axion bundle` now checks bundle readiness before running heavier staging/build work.
- Documented readiness output as the recommended pre-release workflow summary.

## v0.1.13.0 - Preview

Axion v0.1.13.0 adds capability profiles on the current Servo `0.1` baseline.

### Baseline

- Cargo workspace version is `0.1.13`.
- Axion public release metadata is `v0.1.13.0`.
- Versioning policy continues to use `v<servo-major>.<servo-minor>.<feature>.<bugfix>` for public releases.

### Added

- Added manifest `profiles` under `[capabilities.<window>]` to expand common permission sets.
- Added built-in profiles: `minimal`, `app-info`, `app-events`, `window-control`, `multi-window`, `file-access`, and `dialog-access`.
- Added profile visibility to human `doctor`, `doctor --json`, `self-test`, and diagnostics reports.
- Added per-profile expansion details and redundant explicit permission notices to `doctor` security diagnostics.
- Added `axion doctor --deny-warnings` and `--max-risk <low|medium|high>` gates for CI release checks.
- Added structured `diagnostics.gate` output to `doctor --json`.

### Changed

- Updated examples and generated app manifests to use profiles plus explicit custom commands.
- Security and manifest documentation now describe profile expansion and recommended profile usage.
- Security diagnostics now warn when unrestricted remote navigation is combined with file or dialog capabilities.

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
