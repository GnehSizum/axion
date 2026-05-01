# CI Validation

Use this flow when a repository wants machine-readable Axion validation without requiring GUI access on every pull request.

## Pull Request Gate

Run formatting, tests, lints, and the lightweight app check:

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p axion-cli -- check \
  --manifest-path examples/hello-axion/axion.toml \
  --dev \
  --bundle \
  --json \
  --report-path target/axion/reports/check.json
```

Upload `target/axion/reports/check.json` as the primary readiness artifact. The report uses `axion.check-report.v1` and includes `failure_phase`, `next_step`, `next_actions[]`, `artifacts[]`, `dev_preflight`, and `bundle_preflight`.

The checked-in `.github/workflows/ci.yml` runs this lightweight check for `examples/hello-axion` and uploads `target/axion/reports/*.json` with the diagnostics artifacts.

## Optional GUI Smoke

Run GUI smoke on manual or platform-specific runners where Servo window startup is available:

```sh
cargo run -p axion-cli -- gui-smoke \
  --manifest-path examples/hello-axion/axion.toml \
  --report-path target/axion/reports/gui-smoke.json \
  --timeout-ms 30000 \
  --cargo-target-dir target \
  --serial-build
```

If this step fails but still writes a report, summarize it without hiding the original failure:

```sh
cargo run -p axion-cli -- report target/axion/reports/gui-smoke.json \
  --allow-failed \
  --output target/axion/reports/gui-smoke-summary.json
```

## Release Preview

For a manual release preview, reuse the successful check report and collect release artifacts:

```sh
cargo run -p axion-cli -- release \
  --manifest-path examples/hello-axion/axion.toml \
  --check-report-path target/axion/reports/check.json \
  --json \
  --report-path target/axion/reports/release.json \
  --bundle-report-path target/axion/reports/bundle.json \
  --archive \
  --archive-path target/axion/reports/bundle.tar

cargo run -p axion-cli -- report target/axion/reports/release.json \
  --output target/axion/reports/release-summary.json
```

`release --check-report-path` only reuses a check report when it matches the manifest and has successful `result`, doctor, self-test, bundle preflight, and release readiness. Upload `check.json`, `release.json`, `release-summary.json`, `bundle.json`, `bundle.tar`, and GUI smoke summaries when present.

The optional `release-preview` workflow job follows this pattern: it writes `hello-check.json`, reuses it during release, writes `hello-release-summary.json`, verifies `release.summary`, and uploads all preview artifacts.
