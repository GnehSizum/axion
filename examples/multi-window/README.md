# Multi Window

Multi-window lifecycle and targeted window-control smoke app.

## What It Covers

- Three declared windows: `main`, `settings`, and `preview`.
- Per-window capability configuration.
- Targeted `window.info`, `window.focus`, `window.reload`, `window.set_title`, and `window.close`.
- Close prevention, confirmation, timeout, and app-exit lifecycle events.
- Limited remote-navigation diagnostics for secondary windows.

## Run

```sh
cargo run -p multi-window -- --plan
cargo run -p multi-window --features servo-runtime
```

## Validate

```sh
cargo run -p axion-cli -- check --manifest-path examples/multi-window/axion.toml --dev --bundle --json --report-path target/axion/reports/multi-window-check.json
cargo run -p axion-cli -- gui-smoke --manifest-path examples/multi-window/axion.toml --report-path target/axion/reports/multi-window-gui-smoke.json --timeout-ms 30000 --cargo-target-dir target --serial-build
```

## Bundle Preview

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/multi-window/axion.toml --json --report-path target/axion/reports/multi-window-bundle.json
```

`doctor --json` includes notice-level findings for remote navigation limited to `https://docs.example`; those notices are expected for this example.
