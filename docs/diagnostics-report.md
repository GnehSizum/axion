# Diagnostics Report

Axion uses `axion.diagnostics-report.v1` for machine-readable release and bridge diagnostics.
The CLI report is generated through the shared `axion_runtime::DiagnosticsReport` Rust model. GUI reports produced by examples use the same top-level schema and may include additional preview fields.

## Producers

- `axion self-test --json`: prints a non-GUI report to stdout.
- `axion self-test --report-path <path>`: writes the same non-GUI report to disk.
- `axion doctor --json`: prints environment, manifest, runtime, and structured security diagnostics.
- `axion check --json`: prints aggregate validation output using `axion.check-report.v1`.
- `axion bundle --json`: prints bundle staging and verification output using `axion.bundle-report.v1`.
- `axion bundle --report-path <path>`: writes the same bundle report to disk.
- `axion release --json`: prints the aggregate release artifact workflow using `axion.release-report.v1`.
- `axion release --report-path <path>`: writes the same release report to disk.
- `axion gui-smoke --report-path <path>`: runs a Servo-backed GUI smoke check and writes the returned GUI report.
- `examples/bridge-diagnostics-demo`: exports a GUI-side report from app-data.
- `window.__AXION__.diagnostics.reportSchema`: exposes the active schema string to frontends.

## Top-Level Fields

- `schema`: always `axion.diagnostics-report.v1`.
- `source`: producer name, such as `axion-cli self-test` or `bridge-diagnostics-demo`.
- `exported_at_unix_seconds`: Unix timestamp for CLI and GUI reports.
- `app_name`, `identifier`, `version`, `description`, `authors`, `homepage`: app metadata.
- `mode`: runtime mode used for the report.
- `window_count`, `windows`: declared windows and capability/runtime summaries.
- `frontend_dist`, `entry`: packaged frontend paths or GUI entry URL.
- `configured_dialog_backend`, `dialog_backend`: manifest backend and effective backend.
- `configured_clipboard_backend`, `clipboard_backend`: manifest backend and effective backend.
- `icon`: validated bundle icon path when available.
- `host_events`: merged host event allowlist.
- `staged_app_dir`, `asset_manifest_path`, `artifacts_removed`: CLI staging results.
- `diagnostics`: optional producer-specific object. `doctor --json` uses `diagnostics.security.warning_count`, `diagnostics.security.windows`, `diagnostics.security.windows[].profile_expansions`, `diagnostics.security.findings`, `diagnostics.gate`, and `diagnostics.readiness`.
- `result`: `ok` or `failed`.

## Window Fields

Each `windows[]` entry includes:

- `id`, `title`
- `bridge_enabled`
- `configured_profiles`, `configured_commands`, `configured_events`, `configured_protocols`
- `runtime_command_count`, `runtime_event_count`
- `host_events`
- `trusted_origins`, `allowed_navigation_origins`, `allow_remote_navigation`

GUI reports may include an additional `diagnostics` object with bridge snapshots, smoke checks, recent host events, lifecycle events, dialog previews, export metadata, and text-control snapshots. GUI window entries may also include preview native state fields such as `width`, `height`, `resizable`, `visible`, and `focused`. Reports can include `close_timeout_ms` when lifecycle timeout configuration is available.

Each `diagnostics.smoke_checks[]` entry should include stable `id`, user-facing `label`, `status` (`pass`, `fail`, or `skip`), and optional `detail`. Check ids use dotted lower-case names such as `bridge.bootstrap`, `app.ping`, `clipboard.roundtrip`, `fs.roundtrip`, `dialog.preview`, and `input.snapshot`.

CLI-generated GUI smoke failure reports use `source = "axion-cli gui-smoke"` and put process context under `diagnostics`: `failure_phase`, `help`, `status_code`, `success`, `report_found`, `timeout_ms`, `cargo_manifest_path`, `cargo_target_dir`, `serial_build`, `build_env_keys`, `stdout`, and `stderr`. The `failure_phase` value is one of `build`, `runtime`, or `report`.

`doctor --json` readiness output contains `ready_for_dev`, `ready_for_bundle`, `ready_for_gui_smoke`, `blockers`, and `warnings`. Use these fields to decide which release workflow can run next before invoking heavier commands such as `gui-smoke` or `bundle`.

## Aggregate CLI Reports

`axion.check-report.v1` summarizes doctor gate status, readiness, quiet self-test, optional bundle preflight, `next_step`, and `result`.

`axion.bundle-report.v1` summarizes the staged bundle target, layout, generated paths, platform metadata paths, copied icon and executable, verification counters, checked paths, readiness blockers, warnings, optional `report_path`, and `result`. It is intended for release automation that needs bundle-specific output rather than the broader diagnostics report schema.

`axion.release-report.v1` summarizes the full preview artifact workflow: doctor gate, readiness, self-test, embedded bundle report, optional archive artifact metadata, artifact inventory, first failure diagnostics, `next_step`, and `result`.

Release reports include:

- `failure_phase`: `doctor`, `readiness`, `self_test`, `bundle`, `archive`, or `null`.
- `failed_reasons`: stable strings explaining the first blocking phase.
- `artifacts[]`: generated file inventory with `kind`, `path`, `exists`, `bytes`, optional `fnv1a64`, and optional `error`. Kinds include `release_report`, `bundle_report`, `bundle_manifest`, and `archive`.
- `archive.verification`: `{ checked, passed, error }` for tar artifacts created with `--archive`.

The release report artifact itself records only path and existence because report size or fingerprint would be self-referential. Other file artifacts include byte counts and `fnv1a64` when they exist and can be read.

## CI Usage

```sh
cargo run -p axion-cli -- self-test \
  --manifest-path examples/bridge-diagnostics-demo/axion.toml \
  --report-path target/axion/reports/bridge-diagnostics-self-test.json \
  --quiet
```

The command exits non-zero if manifest loading, runtime diagnostics, asset staging, or icon validation fails.

## Local GUI Smoke

`axion gui-smoke` is the preferred local entrypoint. It runs the Servo-backed window, captures the returned diagnostics report, validates the schema and `result: "ok"`, optionally writes it to `--report-path`, prints a `smoke_checks` summary for human runs, and exits. The bridge diagnostics demo implements the required `window.__AXION_GUI_SMOKE__()` hook.

```sh
cargo run -p axion-cli -- gui-smoke \
  --manifest-path examples/bridge-diagnostics-demo/axion.toml \
  --report-path target/axion/reports/bridge-diagnostics-gui-smoke.json \
  --timeout-ms 30000
cargo run -p axion-cli -- gui-smoke \
  --manifest-path examples/hello-axion/axion.toml \
  --report-path target/axion/reports/hello-gui-smoke.json \
  --timeout-ms 30000
```

For generated apps outside the Axion workspace, prefer reusing the checkout build cache:

```sh
cargo run -p axion-cli -- gui-smoke \
  --manifest-path /tmp/demo-app/axion.toml \
  --report-path target/axion/reports/demo-app-gui-smoke.json \
  --timeout-ms 30000 \
  --cargo-target-dir target \
  --serial-build
```

The lower-level `AXION_GUI_SMOKE=1` environment variable remains available for direct app runs. The default runtime timeout is 10 seconds; `--timeout-ms` maps to `AXION_GUI_SMOKE_TIMEOUT_MS`.
