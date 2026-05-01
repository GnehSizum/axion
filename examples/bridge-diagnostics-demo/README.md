# Bridge Diagnostics Demo

Bridge surface, diagnostics helper, input compatibility, and frontend self-check demo.

## What It Covers

- `window.__AXION__` command, event, host-event, origin, and protocol snapshots.
- Diagnostics helpers: `describeBridge`, `snapshotTextControl`, and `toPrettyJson`.
- Clipboard, app-data file, dialog, app echo, and window-list commands.
- Frontend event emission through `app.log`.
- GUI smoke report export through `window.__AXION_GUI_SMOKE__()`.

## Run

```sh
cargo run -p bridge-diagnostics-demo -- --plan
cargo run -p bridge-diagnostics-demo --features servo-runtime
```

## Validate

```sh
cargo run -p axion-cli -- check --manifest-path examples/bridge-diagnostics-demo/axion.toml --dev --bundle --json --report-path target/axion/reports/bridge-diagnostics-check.json
cargo run -p axion-cli -- gui-smoke --manifest-path examples/bridge-diagnostics-demo/axion.toml --report-path target/axion/reports/bridge-diagnostics-gui-smoke.json --timeout-ms 30000 --cargo-target-dir target --serial-build
```

## Bundle Preview

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/bridge-diagnostics-demo/axion.toml --json --report-path target/axion/reports/bridge-diagnostics-bundle.json
```

This example intentionally exposes a broad diagnostics-oriented command set. Treat any notice about broad capabilities as expected for diagnostics; use narrower profiles for production-style apps.
