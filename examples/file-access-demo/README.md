# File Access Demo

Controlled file, dialog, clipboard, input, and style-loading demo.

## What It Covers

- Capability-gated `fs.write_text` and `fs.read_text` under app data.
- Headless `dialog.open` and `dialog.save` preview responses.
- Memory clipboard backend.
- Text input compatibility helpers for caret placement and selection.
- External `style.css` and `app.js` loading through packaged assets.

## Run

```sh
cargo run -p file-access-demo -- --plan
cargo run -p file-access-demo --features servo-runtime
```

## Validate

```sh
cargo run -p axion-cli -- check --manifest-path examples/file-access-demo/axion.toml --dev --bundle --json --report-path target/axion/reports/file-access-check.json
cargo run -p axion-cli -- gui-smoke --manifest-path examples/file-access-demo/axion.toml --report-path target/axion/reports/file-access-gui-smoke.json --timeout-ms 30000 --cargo-target-dir target --serial-build
```

This manifest does not configure a `[dev]` server. `check --dev` reports that as a warning, not a blocker, because packaged assets are available.
