# CLI Reference

The Axion CLI is currently run through Cargo:

```sh
cargo run -p axion-cli -- <command>
```

## `new`

Generate a minimal application with local artifact hygiene, panic reporting, a default bundle icon, and bridge demos.

```sh
cargo run -p axion-cli -- new demo-app --template vanilla --path /tmp/demo-app
```

Project names are normalized to lowercase kebab-case for package use.

Options:

- `--template vanilla`: generate a plain HTML/CSS/JavaScript app with bridge, native API, custom command, capability-denial, and bundle-icon demos.
- `--path <path>`: choose the output directory.

## `dev`

Print development diagnostics from an `axion.toml` manifest: launch mode, dev server reachability, packaged fallback status, per-window entry URLs, next steps, frontend watch/reload diagnostics, and the runtime plan.

```sh
cargo run -p axion-cli -- dev --manifest-path examples/hello-axion/axion.toml
```

Typical output when the configured dev server is not running:

```text
Axion development diagnostics
manifest: examples/hello-axion/axion.toml
launch_mode: blocked (dev server is not reachable at http://127.0.0.1:3000/; start the frontend dev server, check [dev].url, or pass --fallback-packaged to launch packaged assets)
dev_server: unreachable (http://127.0.0.1:3000/)
packaged_fallback: disabled; available with --fallback-packaged (axion://app/index.html)
window_entries:
- main: unavailable (launch blocked)
next_steps: start the frontend dev server at http://127.0.0.1:3000/, check [dev].url, or pass --fallback-packaged.
frontend_command: not configured
runtime_plan:
...
```

The `launch_mode` line is authoritative for `--launch`. The trailing `runtime_plan` is informational and may still show the manifest-derived development entrypoint.

Use `--launch` with the `servo-runtime` feature to start the app in development mode when the configured dev server is reachable:

```sh
cargo run -p axion-cli --features servo-runtime -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --launch
```

To let Axion start an external frontend command and wait for `[dev].url` to become reachable:

```sh
cargo run -p axion-cli -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --frontend-command "python3 -m http.server 3000 --bind 127.0.0.1 --directory frontend" \
  --frontend-cwd examples/hello-axion \
  --dev-server-timeout-ms 5000
```

When combined with `--launch`, the frontend process stays alive while the Axion window runs and is terminated when the CLI exits. CLI options override `[dev] command`, `cwd`, and `timeout_ms` values from the manifest.

If the dev server is not configured or unreachable, `--launch` fails with a diagnostic. Pass `--fallback-packaged` only when you intentionally want to launch packaged assets instead; the CLI validates that the packaged entry is available before selecting production mode:

```sh
cargo run -p axion-cli --features servo-runtime -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --launch \
  --fallback-packaged
```

`--launch` prints a `launch_summary` before opening windows. The summary includes the selected mode, packaged fallback status, window ids, and final entry URLs.

Watch and reload preview:

- `--watch`: polls `[build].frontend_dist` for created, modified, and deleted files. It ignores common temporary files and cache directories, then debounces editor save bursts before printing diagnostics. With `--launch`, polling runs while the app window is open. Without `--launch`, the command prints the runtime plan and keeps watching until interrupted.
- `--reload`: when combined with `--watch`, prints `reload_requested` when watched files change. With `--launch`, Axion asks each live window to reload and prints `reload_applied`, `reload_deferred`, or `restart_required` per window. Without `--launch`, no live window target exists, so reload remains diagnostic-only.
- `--open-devtools`: accepted as an explicit reserved option and reports that the current Servo backend does not open devtools.

Example:

```sh
cargo run -p axion-cli --features servo-runtime -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --launch \
  --fallback-packaged \
  --watch \
  --reload
```

After the window opens, edit a file under `examples/hello-axion/frontend/`. A successful live reload prints `reload_requested` and `reload_applied: window=main`. Multi-window apps print one reload result per live window.

## `doctor`

Validate Axion version metadata, local tooling, manifest configuration, app metadata, native dialog, clipboard, and lifecycle configuration, effective runtime native backends, frontend assets, runtime diagnostics, capability categories including clipboard access, and Servo path availability.

```sh
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
```

`doctor` prints `axion: cli_version=..., release=..., msrv=...` and `rustc.msrv: ok|failed` so CI and local environments can quickly confirm the active compiler satisfies the workspace `rust-version`.

It also prints capability security diagnostics and release-readiness summaries. Use `security.summary: warnings=0` as the simple CI gate. Per-window lines report declared profiles, profile expansions, bridge status, risk level, command category counts, protocol count, remote-navigation settings, redundant explicit permissions, and recommendations for unsafe or contradictory capability declarations.

Readiness lines summarize whether the manifest is ready for local development, bundle staging, and GUI smoke validation:

```text
readiness.summary: dev=true, bundle=true, gui_smoke=true
```

If a workflow is not ready, `readiness.blocker` explains the specific missing asset, runtime error, security issue, bundle icon problem, Servo source discovery issue, or missing `window.__AXION_GUI_SMOKE__` hook.

Use `--json` to emit the stable `axion.diagnostics-report.v1` schema with structured `diagnostics.security`, `diagnostics.gate`, and `diagnostics.readiness` data:

```sh
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml --json
```

Use gate options in CI:

```sh
cargo run -p axion-cli -- doctor \
  --manifest-path examples/hello-axion/axion.toml \
  --deny-warnings \
  --max-risk medium
```

`--deny-warnings` fails when security warnings are present. `--max-risk` fails when any window exceeds the selected risk level. JSON output includes `diagnostics.gate.passed`, `diagnostics.gate.failed_reasons`, and readiness booleans for downstream CI.

## `check`

Run the recommended lightweight application validation loop. `check` applies the `doctor` security gate, reads release readiness, runs quiet `self-test` staging, and can optionally verify bundle preflight conditions:

```sh
cargo run -p axion-cli -- check --manifest-path examples/hello-axion/axion.toml --bundle
cargo run -p axion-cli -- check --manifest-path examples/hello-axion/axion.toml --bundle --json
```

By default `check` fails on security warnings and requires risk no higher than `medium`. Use `--max-risk low|medium|high` to tune the gate. Pass `--keep-artifacts` to keep the self-test staging directory for inspection. `--bundle` validates that bundle readiness is true and verifies web assets plus the configured icon before you run the heavier `bundle --build-executable` command.

Human output ends with `next_step`, which tells you whether to fix blockers, run `gui-smoke`, or proceed to `bundle --build-executable`. JSON output uses `axion.check-report.v1` and includes `doctor`, `readiness`, `self_test`, `bundle_preflight`, `next_step`, and `result`.

## `self-test`

Run the non-GUI release gate for a manifest. It loads the app, prints app metadata, configured and effective native dialog backend, and per-window capability/runtime summaries, checks runtime diagnostics, stages frontend assets, and removes generated artifacts by default.

```sh
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml --keep-artifacts
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml --json
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml --report-path target/axion/reports/hello-self-test.json
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml --report-path target/axion/reports/hello-self-test.json --quiet
```

Use `--json` to print a machine-readable `axion.diagnostics-report.v1` report. Use `--report-path <path>` to write the same report to disk while keeping the default human-readable output.
Use `--quiet` with `--report-path` in CI when only the exit code and report artifact are needed.

## `gui-smoke`

Run a Servo-backed local GUI smoke check for an application manifest. The command finds `Cargo.toml` next to `axion.toml`, launches `cargo run --features servo-runtime` with `AXION_GUI_SMOKE=1`, captures the returned diagnostics report, and can write it to disk.

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

Useful options:

- `--quiet`: suppress child stdout/stderr after capturing the report.
- `--cargo-target-dir <path>`: set `CARGO_TARGET_DIR` for the launched app. This is useful for generated apps because they can reuse the Axion checkout `target/` cache.
- `--serial-build`: set `CARGO_BUILD_JOBS=1` and `MAKEFLAGS=-j1` for lower-resource Servo builds.
- `--build-env KEY=VALUE`: pass an extra build environment variable to the launched Cargo process. Repeat for multiple variables.

Recommended generated-app validation from the Axion checkout:

```sh
cargo run -p axion-cli -- gui-smoke \
  --manifest-path /tmp/demo-app/axion.toml \
  --report-path target/axion/reports/demo-app-gui-smoke.json \
  --timeout-ms 30000 \
  --cargo-target-dir target \
  --serial-build
```

GUI smoke requires the app frontend to define `window.__AXION_GUI_SMOKE__()`. The CLI validates both the report schema and `result: "ok"` before returning success, and human output includes a `smoke_checks: total=N, failed=...` summary when the returned report contains `diagnostics.smoke_checks`. Lifecycle-aware examples also report close-confirmation, `window.close_prevented`, `app.exit_requested`, `app.exit_prevented`, and `close_timeout_ms` smoke checks. Failure reports include `failure_phase`, `help`, `status_code`, `success`, `report_found`, `timeout_ms`, `cargo_manifest_path`, `cargo_target_dir`, `serial_build`, `build_env_keys`, `stdout`, and `stderr` under `diagnostics`. Runtime failures that include `GUI smoke failed` or `Winit(RegisterProtocol(...))` are classified as `runtime` even when Cargo emitted compile progress before launch.

Troubleshooting:

- `failure_phase = "build"` means Cargo or Servo failed before the window smoke hook ran. Check `stderr` for Rust MSRV, Python >=3.11 or `uv`, LLVM/lld, macOS SDK, and native dependency build errors.
- `failure_phase = "runtime"` means the app started but exited before returning a valid report. Check bridge capabilities, frontend exceptions, and the hook implementation.
- `failure_phase = "report"` means the process exited successfully but did not print a valid `axion.diagnostics-report.v1` report.

## `build`

Stage frontend assets into an Axion app directory.

```sh
cargo run -p axion-cli -- build --manifest-path examples/hello-axion/axion.toml
```

## `bundle`

Create a platform bundle scaffold and copy staged app resources. App metadata from `[app]` and icon configuration from `[bundle]` are written into bundle metadata files where supported. Each bundle also includes `axion-bundle-manifest.json`, a deterministic integrity manifest listing bundle paths, byte sizes, and `fnv1a64` content fingerprints. Before staging, `bundle` checks bundle readiness and points back to `axion check --bundle` when blockers remain. After staging, `bundle` verifies the generated entry, metadata, asset manifest, bundle manifest, icon, executable references, sizes, and fingerprints.

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --json
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --report-path target/axion/reports/hello-bundle.json
```

Executable handling:

- If `--executable <path>` is passed, that binary is copied into the bundle.
- If no executable is passed, Axion searches nearby `target/release/` and `target/debug/` directories for a binary matching the app name.
- Pass `--build-executable` to run `cargo build --release` for the app before bundling.
- Use the printed `layout`, `bundle_dir`, and `bundle_manifest` values to inspect the generated platform structure.
- Use `--json` to emit `axion.bundle-report.v1` with the same paths, copied icon/executable references, platform metadata, verification counters, checked paths, readiness blockers, warnings, and final result.
- Use `--report-path <path>` to write `axion.bundle-report.v1` to disk while keeping the normal stdout mode.
- `verification: ok` means every referenced bundle path exists and the manifest file list matches generated file sizes and `fnv1a64` fingerprints.
- `checked_dirs`, `checked_files`, `fingerprinted_files`, and `bundle_bytes` summarize the verification pass.

```sh
cargo run -p axion-cli -- bundle \
  --manifest-path examples/hello-axion/axion.toml \
  --build-executable \
  --json \
  --report-path target/axion/reports/hello-bundle.json
```

## `release`

Run the preview release artifact workflow. `release` applies the doctor gate, checks readiness, runs quiet `self-test`, stages a bundle, embeds the bundle report, and can optionally create a dependency-free `.tar` artifact:

```sh
cargo run -p axion-cli -- release \
  --manifest-path examples/hello-axion/axion.toml \
  --json \
  --report-path target/axion/reports/hello-release.json \
  --bundle-report-path target/axion/reports/hello-bundle.json
```

Useful options:

- `--archive`: create a `.tar` archive next to the generated bundle and report its bytes plus `fnv1a64` fingerprint.
- `--archive-path <path>`: choose the archive output path.
- `--skip-build-executable`: skip the default release executable build and use an existing or discovered executable.
- `--max-risk low|medium|high`: tune the doctor security gate; default is `medium`.

JSON output uses `axion.release-report.v1` and includes `doctor`, `readiness`, `self_test`, embedded `bundle.report`, optional `archive`, `artifacts[]`, `failure_phase`, `failed_reasons`, `next_step`, and `result`.

When `--archive` is used, `archive.verification` confirms the tar file still exists, is non-empty, and matches the byte count plus `fnv1a64` fingerprint recorded during generation. `artifacts[]` lists generated release outputs for CI upload; the release report itself records only path and existence because size or fingerprint would otherwise be self-referential.
