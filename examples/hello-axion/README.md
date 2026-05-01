# Hello Axion

Minimal single-window smoke app for the Axion runtime.

## What It Covers

- Single Servo-backed window startup.
- App metadata and version commands.
- Window info, title, size, reload, close, and lifecycle commands.
- Clipboard, app-data file, dialog, custom command, and host event basics.
- `window.__AXION_GUI_SMOKE__()` for local GUI smoke diagnostics.

## Run

```sh
cargo run -p hello-axion -- --plan
cargo run -p hello-axion --features servo-runtime
```

Use `AXION_SELFTEST_BRIDGE=1` when you want the app to run the bridge self-test and exit automatically:

```sh
AXION_SELFTEST_BRIDGE=1 cargo run -p hello-axion --features servo-runtime
```

## Validate

```sh
cargo run -p axion-cli -- check --manifest-path examples/hello-axion/axion.toml --dev --bundle --json --report-path target/axion/reports/hello-check.json
cargo run -p axion-cli -- gui-smoke --manifest-path examples/hello-axion/axion.toml --report-path target/axion/reports/hello-gui-smoke.json --timeout-ms 30000 --cargo-target-dir target --serial-build
```

## Bundle Preview

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --json --report-path target/axion/reports/hello-bundle.json
```

`check --dev` may report a warning that `http://127.0.0.1:3000/` is unreachable. That is expected unless you start a dev server; packaged fallback remains valid.
