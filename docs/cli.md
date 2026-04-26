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

Validate Axion version metadata, local tooling, manifest configuration, app metadata, native dialog backend configuration, effective runtime dialog backend, frontend assets, runtime diagnostics, and Servo path availability.

```sh
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
```

`doctor` prints `axion: cli_version=..., release=..., msrv=...` and `rustc.msrv: ok|failed` so CI and local environments can quickly confirm the active compiler satisfies the workspace `rust-version`.

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

GUI smoke requires the app frontend to define `window.__AXION_GUI_SMOKE__()`. The CLI validates both the report schema and `result: "ok"` before returning success. Failure reports include `failure_phase`, `help`, `status_code`, `success`, `report_found`, `timeout_ms`, `cargo_manifest_path`, `cargo_target_dir`, `serial_build`, `build_env_keys`, `stdout`, and `stderr` under `diagnostics`.

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

Create a platform bundle scaffold and copy staged app resources. App metadata from `[app]` and icon configuration from `[bundle]` are written into bundle metadata files where supported. Each bundle also includes `axion-bundle-manifest.json`, a deterministic integrity manifest listing bundle paths, byte sizes, and `fnv1a64` content fingerprints. After staging, `bundle` verifies the generated entry, metadata, asset manifest, bundle manifest, icon, executable references, sizes, and fingerprints.

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml
```

Executable handling:

- If `--executable <path>` is passed, that binary is copied into the bundle.
- If no executable is passed, Axion searches nearby `target/release/` and `target/debug/` directories for a binary matching the app name.
- Pass `--build-executable` to run `cargo build --release` for the app before bundling.
- Use the printed `bundle_manifest` path to inspect the generated entry, resource, metadata, icon, executable, and file list.
- `verification: ok` means every referenced bundle path exists and the manifest file list matches generated file sizes and `fnv1a64` fingerprints.

```sh
cargo run -p axion-cli -- bundle \
  --manifest-path examples/hello-axion/axion.toml \
  --build-executable
```
