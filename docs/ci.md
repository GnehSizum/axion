# CI Validation

Use this flow when a repository wants machine-readable Axion validation without requiring GUI access on every pull request.

## Pull Request Gate

Run formatting, tests, lints, and the lightweight app check:

```sh
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p axion-cli -- check \
  --manifest-path examples/hello-axion/axion.toml \
  --dev \
  --bundle \
  --json \
  --report-path target/axion/reports/check.json
```

`cargo fmt --check` checks the Axion workspace. Avoid `--all` here: it also formats local path dependencies, including the vendored Servo workspace with its own upstream formatting configuration.

Upload `target/axion/reports/check.json` as the primary readiness artifact. The report uses `axion.check-report.v1` and includes `failure_phase`, `next_step`, `next_actions[]`, `artifacts[]`, `dev_preflight`, and `bundle_preflight`.

The checked-in `.github/workflows/ci.yml` also selects Node.js 24 LTS, syntax-checks the three bridge JavaScript assets and runs the bootstrap behavior tests. It runs this lightweight check for `examples/hello-axion` and uploads `target/axion/reports/*.json` with the diagnostics artifacts.

## Servo Feature Compile Gate

Code changes in `crates`, `examples`, `servo`, Cargo/toolchain/build configuration or this workflow trigger the Ubuntu 24.04 `native-check` job. It caches registry/git dependencies and `target`, selects Clang 19, installs native dependencies and runs `cargo check --workspace --features servo-runtime --locked`. Pure documentation changes keep the lightweight gate without rebuilding the engine. Default workspace tests include JSON, input/output path, archive permission and state-boundary regressions.

## Optional GUI Smoke

Run GUI smoke on manual or platform-specific runners where Servo window startup is available:

The optional Ubuntu GUI job installs `clang-19` and `libclang-19-dev`, explicitly selects the Clang 19 compilers, and points `LIBCLANG_PATH` at `/usr/lib/llvm-19/lib` to meet Servo 0.6.0's native compiler requirement.

```sh
cargo run -p axion-cli -- gui-smoke \
  --manifest-path examples/hello-axion/axion.toml \
  --report-path target/axion/reports/gui-smoke.json \
  --timeout-ms 30000 \
  --require-check bridge.bootstrap \
  --require-check app.ping \
  --require-check input.snapshot \
  --require-command app.ping \
  --require-command window.info \
  --require-host-event window.ready \
  --require-window main \
  --cargo-target-dir target \
  --serial-build
```

Use `--require-check`, `--require-command`, `--require-host-event`, and `--require-window` to keep optional GUI jobs useful as runtime regression gates. The command fails if the frontend omits, skips, or fails required coverage, and the written report records `diagnostics.required_checks` plus `diagnostics.required_runtime` for artifact summaries.

The manual GUI job runs the hello, diagnostics, file-access and multi-window examples. The multi-window step validates the smoke checks provided by its partial report: `app.ping`, `window.info`, close prevention, confirmed close completion, close timeout, application exit prevention and `app.exit.idempotent`. Its report and summary use the existing `*-gui-smoke*.json` artifact collection.

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

`release --check-report-path` always reruns the current doctor gate and readiness. It reuses self-test and bundle-preflight success only when the report matches the canonical manifest location, risk threshold and content identity; changing configuration, frontend files, icon or framework version requires a fresh check. Upload `check.json`, `release.json`, `release-summary.json`, `bundle.json`, `bundle.tar`, and GUI smoke summaries when present.

The optional `release-preview` workflow job generates an application, builds a Servo release bundle, unpacks the archive, hides the generated source directory and starts the executable under xvfb from another working directory. It checks GUI smoke results and executable permissions, then uploads the reports, archive and `release-deployment.log`. The report filenames retain the `hello-` prefix. This job requires a native Linux runner; local macOS unit tests do not establish that the workflow itself passed.
