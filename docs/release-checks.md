# Release Checks

Use this checklist before tagging an Axion preview or sharing a generated app.

## Fast Readiness Check

Run `check` first. It validates the doctor gate, readiness, quiet self-test staging, optional dev preflight, and optional bundle preflight:

```sh
cargo run -p axion-cli -- check \
  --manifest-path examples/hello-axion/axion.toml \
  --dev \
  --bundle \
  --report-path target/axion/reports/check.json
```

Expected summary:

```text
doctor: ok
readiness: dev=true, bundle=true, gui_smoke=true
self_test: ok
dev.preflight: ok
bundle.preflight: ok
result: ok
```

Use `doctor --deny-warnings --max-risk medium` when you need the full human-readable diagnostics, including per-window security details and `readiness.summary`.
Use `--json` in CI and read `diagnostics.readiness.ready_for_dev`, `ready_for_bundle`, `ready_for_gui_smoke`, `blockers`, and `warnings`.
Use `check --dev --bundle --json --report-path target/axion/reports/check.json` when CI only needs the aggregate workflow result, `failure_phase`, `next_step`, ordered `next_steps`, typed `next_actions`, dev preflight status, and bundle preflight status. Upload `target/axion/reports/check.json` as the lightweight readiness artifact, and read `artifacts[]` for the recommended dev, bundle, and release report paths to collect next.
Use `bundle --json` when CI needs the generated bundle layout, platform metadata, copied icon/executable paths, verification counters, checked paths, and final `result`. Add `--report-path <path>` to upload the bundle report as an artifact.
Use `release --json` when CI needs the full preview artifact workflow result in `axion.release-report.v1`. Pass `--check-report-path target/axion/reports/check.json` to reuse a matching successful check report for doctor, readiness, self-test, and bundle-preflight state. The report includes `check_report`, `failure_phase`, `failed_reasons`, an `artifacts[]` inventory, and archive verification details when `--archive` is used.

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
cargo run -p axion-cli -- check --manifest-path examples/hello-axion/axion.toml --dev --bundle --report-path target/axion/reports/check.json
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- gui-smoke --manifest-path examples/hello-axion/axion.toml --report-path target/axion/reports/hello-gui-smoke.json --timeout-ms 30000 --cargo-target-dir target --serial-build
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --build-executable
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --build-executable --json --report-path target/axion/reports/hello-bundle.json
cargo run -p axion-cli -- release --manifest-path examples/hello-axion/axion.toml --check-report-path target/axion/reports/check.json --json --report-path target/axion/reports/hello-release.json --bundle-report-path target/axion/reports/hello-bundle.json --archive --archive-path target/axion/reports/hello-bundle.tar
cargo run -p axion-cli -- report target/axion/reports/hello-release.json --output target/axion/reports/hello-release-summary.json
cargo run -p axion-cli -- report target/axion/reports/hello-gui-smoke.json --allow-failed --output target/axion/reports/hello-gui-smoke-summary.json
```

`gui-smoke` requires a Servo-capable local environment. If it cannot run locally, keep the `doctor` readiness output and Servo compile check in the release notes.

## Optional CI Preview

The GitHub Actions workflow includes a manual `workflow_dispatch` input named `run_release_preview`. It is intentionally not a default pull request gate. When enabled, it runs the hello release preview, checks `axion.release-report.v1`, verifies `result = "ok"`, confirms archive verification passed, and uploads:

- `target/axion/reports/hello-release.json`
- `target/axion/reports/hello-bundle.json`
- `target/axion/reports/hello-bundle.tar`
