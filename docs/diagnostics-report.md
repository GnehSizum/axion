# Diagnostics Report

Axion uses `axion.diagnostics-report.v1` for machine-readable release and bridge diagnostics.
The CLI report is generated through the shared `axion_runtime::DiagnosticsReport` Rust model. GUI reports produced by examples use the same top-level schema and may include additional preview fields.

## Producers

- `axion self-test --json`: prints a non-GUI report to stdout.
- `axion self-test --report-path <path>`: writes the same non-GUI report to disk.
- `axion doctor --json`: prints environment, manifest, runtime, and structured security diagnostics.
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
- `icon`: validated bundle icon path when available.
- `host_events`: merged host event allowlist.
- `staged_app_dir`, `asset_manifest_path`, `artifacts_removed`: CLI staging results.
- `diagnostics`: optional producer-specific object. `doctor --json` uses `diagnostics.security.warning_count`, `diagnostics.security.windows`, and `diagnostics.security.findings`.
- `result`: `ok` or `failed`.

## Window Fields

Each `windows[]` entry includes:

- `id`, `title`
- `bridge_enabled`
- `configured_commands`, `configured_events`, `configured_protocols`
- `runtime_command_count`, `runtime_event_count`
- `host_events`
- `trusted_origins`, `allowed_navigation_origins`, `allow_remote_navigation`

GUI reports may include an additional `diagnostics` object with bridge snapshots, smoke checks, recent host events, dialog previews, export metadata, and text-control snapshots. GUI window entries may also include preview native state fields such as `width`, `height`, `resizable`, `visible`, and `focused`.

Each `diagnostics.smoke_checks[]` entry should include stable `id`, user-facing `label`, `status` (`pass`, `fail`, or `skip`), and optional `detail`. Check ids use dotted lower-case names such as `bridge.bootstrap`, `app.ping`, `fs.roundtrip`, `dialog.preview`, and `input.snapshot`.

CLI-generated GUI smoke failure reports use `source = "axion-cli gui-smoke"` and put process context under `diagnostics`: `failure_phase`, `help`, `status_code`, `success`, `report_found`, `timeout_ms`, `cargo_manifest_path`, `cargo_target_dir`, `serial_build`, `build_env_keys`, `stdout`, and `stderr`. The `failure_phase` value is one of `build`, `runtime`, or `report`.

## CI Usage

```sh
cargo run -p axion-cli -- self-test \
  --manifest-path examples/bridge-diagnostics-demo/axion.toml \
  --report-path target/axion/reports/bridge-diagnostics-self-test.json \
  --quiet
```

The command exits non-zero if manifest loading, runtime diagnostics, asset staging, or icon validation fails.

## Local GUI Smoke

`axion gui-smoke` is the preferred local entrypoint. It runs the Servo-backed window, captures the returned diagnostics report, validates the schema and `result: "ok"`, optionally writes it to `--report-path`, and exits. The bridge diagnostics demo implements the required `window.__AXION_GUI_SMOKE__()` hook.

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
