# Release Checks

Use this checklist before tagging an Axion preview or sharing a generated app.

## Fast Readiness Check

Run `check` first. It validates the doctor gate, readiness, quiet self-test staging, and optional bundle preflight:

```sh
cargo run -p axion-cli -- check \
  --manifest-path examples/hello-axion/axion.toml \
  --bundle
```

Expected summary:

```text
doctor: ok
readiness: dev=true, bundle=true, gui_smoke=true
self_test: ok
bundle.preflight: ok
result: ok
```

Use `doctor --deny-warnings --max-risk medium` when you need the full human-readable diagnostics, including per-window security details and `readiness.summary`.
Use `--json` in CI and read `diagnostics.readiness.ready_for_dev`, `ready_for_bundle`, `ready_for_gui_smoke`, `blockers`, and `warnings`.
Use `check --json` when CI only needs the aggregate workflow result, `next_step`, and bundle preflight status.

## Full Local Gate

Run these from the Axion checkout:

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo check -p axion-cli -p hello-axion -p multi-window -p file-access-demo -p bridge-diagnostics-demo --features servo-runtime
```

Then validate the example app workflow:

```sh
cargo run -p axion-cli -- check --manifest-path examples/hello-axion/axion.toml --bundle
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- gui-smoke --manifest-path examples/hello-axion/axion.toml --report-path target/axion/reports/hello-gui-smoke.json --timeout-ms 30000 --cargo-target-dir target --serial-build
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --build-executable
```

`gui-smoke` requires a Servo-capable local environment. If it cannot run locally, keep the `doctor` readiness output and Servo compile check in the release notes.
