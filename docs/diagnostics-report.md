# Diagnostics Report

Axion uses `axion.diagnostics-report.v1` for machine-readable release and bridge diagnostics.
The CLI report is generated through the shared `axion_runtime::DiagnosticsReport` Rust model. GUI reports produced by examples use the same top-level schema and may include additional preview fields.

## Producers

- `axion self-test --json`: prints a non-GUI report to stdout.
- `axion self-test --report-path <path>`: writes the same non-GUI report to disk.
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

## CI Usage

```sh
cargo run -p axion-cli -- self-test \
  --manifest-path examples/bridge-diagnostics-demo/axion.toml \
  --report-path target/axion/reports/bridge-diagnostics-self-test.json \
  --quiet
```

The command exits non-zero if manifest loading, runtime diagnostics, asset staging, or icon validation fails.

## Local GUI Smoke

`AXION_GUI_SMOKE=1` runs the Servo-backed window, calls `window.__AXION_GUI_SMOKE__()` after the page loads, prints the returned diagnostics report, and exits. The bridge diagnostics demo implements this hook. The default timeout is 10 seconds; set `AXION_GUI_SMOKE_TIMEOUT_MS=<milliseconds>` when a local debug build needs more time.

```sh
AXION_GUI_SMOKE=1 cargo run -p bridge-diagnostics-demo --features servo-runtime
AXION_GUI_SMOKE=1 AXION_GUI_SMOKE_TIMEOUT_MS=30000 cargo run -p bridge-diagnostics-demo --features servo-runtime
```

This is intended for local GUI regression checks first. It is not yet part of the default GitHub CI gate.
