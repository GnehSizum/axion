# Getting Started

Axion can generate a small Rust desktop app with a frontend page and an `axion.toml` manifest.

## Prerequisites

- Rust toolchain compatible with this workspace (`rust-version = 1.86.0` or newer).
- A GUI-capable desktop session for `servo-runtime` window launches.
- This repository checked out with the vendored `servo/` directory present.

## Run the Example

From the repository root:

```sh
cargo run -p hello-axion -- --plan
cargo run -p multi-window -- --plan
cargo run -p file-access-demo -- --plan
cargo run -p bridge-diagnostics-demo -- --plan
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/multi-window/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/file-access-demo/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/bridge-diagnostics-demo/axion.toml --json
```

To run the GUI bridge self-test:

```sh
AXION_SELFTEST_BRIDGE=1 cargo run -p hello-axion --features servo-runtime
```

The self-test window closes automatically after the bridge verifies `app.ready` and `app.ping`.

To keep the example window open, run without `AXION_SELFTEST_BRIDGE`:

```sh
cargo run -p hello-axion --features servo-runtime
```

`hello-axion` now includes a small input-compatibility panel wired to `window.__AXION__.compat.installTextInputSelectionPatch`, so you can quickly inspect caret placement, drag selection, and textarea `Tab` handling alongside the core bridge smoke checks.

To inspect per-window capability behavior:

```sh
cargo run -p multi-window --features servo-runtime
```

The `main` window can call app-level commands, while the `settings` window is restricted to window-local controls such as `window.info`, `window.focus`, and `window.set_title`.
The updated example also lets the `main` window use `window.list` plus `{ target: "settings" }` to inspect and rename the `settings` window.

To inspect controlled filesystem and dialog capabilities:

```sh
cargo run -p file-access-demo --features servo-runtime
```

This example writes and reads `target/axion-data/file-access-demo/notes/demo.txt`, emits `app.log`, and shows the preview `dialog.open` / `dialog.save` responses configured by `[native.dialog]`.
The page also includes editable file inputs, action buttons, a rejected-path probe, and a live host-event log so you can inspect the bridge behavior without opening developer tools.

To inspect bridge snapshots, frontend self-checks, and unified compat diagnostics:

```sh
cargo run -p bridge-diagnostics-demo --features servo-runtime
```

This example renders `window.__AXION__.diagnostics.describeBridge()`, records host events, exercises built-in bridge commands, previews dialogs, includes a focused input/textarea compatibility panel, runs a visual smoke checklist for bridge, filesystem, dialog, event, and diagnostics helpers, and can export or reload a JSON diagnostics report from app-data.

To inspect the development launch path:

```sh
cargo run -p axion-cli -- dev --manifest-path examples/hello-axion/axion.toml
```

`axion dev` reports the selected launch mode, dev-server reachability, packaged fallback availability, and each window entry URL. `axion dev --launch` requires the configured frontend dev server to be running. If it is not reachable, the command exits with a diagnostic instead of silently launching packaged assets. Use `--fallback-packaged` only when you explicitly want to launch the packaged `axion://app` entry instead.

`axion dev --launch` prints a launch summary before opening windows:

```sh
cargo run -p axion-cli --features servo-runtime -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --launch \
  --fallback-packaged
```

The preview flags `--watch` and `--reload` are available for frontend development. `--watch` polls `[build].frontend_dist`, ignores common temporary files and cache directories, debounces editor save bursts, and reports created, modified, and deleted files. `--reload` reports `reload_requested`; with `--launch`, Axion asks each live window to reload and prints `reload_applied`, `reload_deferred`, or `restart_required`. Without `--launch`, reload remains diagnostic-only because there is no live window target. `--open-devtools` is accepted for diagnostics, but the current Servo backend does not open devtools yet.

To test live reload, launch with `--features servo-runtime --launch --fallback-packaged --watch --reload`, then edit a file in the app's `frontend/` directory. `hello-axion` should report `reload_applied: window=main`; `multi-window` reports one reload result for each live window.

To let Axion start a simple local frontend server, run:

```sh
cargo run -p axion-cli -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --frontend-command "python3 -m http.server 3000 --bind 127.0.0.1 --directory frontend" \
  --frontend-cwd examples/hello-axion \
  --dev-server-timeout-ms 5000
```

With `--launch`, Axion keeps that frontend process alive while the window runs and terminates it when the CLI exits.

## Create a New App

```sh
cargo run -p axion-cli -- new demo-app --template vanilla --path /tmp/demo-app --run-check
cd /tmp/demo-app
cargo run -- --plan
cargo run --features servo-runtime
```

`--run-check` immediately runs `axion check --bundle` against the generated manifest. Omit it if you only want to create files.

Generated projects contain:

- `Cargo.toml`: path dependencies back to this Axion checkout
- `.gitignore`: ignores `target/` build output, runtime data, bundles, and crash reports
- `README.md`: generated app usage notes
- `axion.toml`: app, window, build, and capability configuration
- `icons/app.icns`: default bundle icon referenced by `[bundle]`
- `src/main.rs`: Rust entrypoint with panic reporting and a `demo.greet` custom command plugin
- `frontend/index.html`: packaged HTML entry
- `frontend/style.css`: CSP-compatible external styles
- `frontend/app.js`: bridge, native API, input-compatibility, custom command, event, and denied-command demos

The generated `demo.greet` command is registered in Rust, allowed in `[capabilities.main]`, and invoked from frontend JavaScript. See `custom-commands.md` for the pattern.

Generated manifests also include optional app metadata (`version`, `description`, `authors`, and `homepage`), `[bundle] icon = "icons/app.icns"`, and `[native.dialog] backend = "headless"`. These values appear in `app.info`, `axion doctor`, self-test output, and bundle metadata scaffolds. The generated frontend also demonstrates `dialog.open` with multi-select and filter metadata plus `dialog.save` with `defaultPath`.

Generated manifests include commented `[dev]` lines. Uncomment them when you attach a frontend toolchain such as Vite, Trunk, or another static server. You can start that server separately before running `axion dev --launch`, or set `[dev] command` / pass `--frontend-command` so Axion starts it for you.

Generated frontends now include a small text-input compatibility panel wired to `window.__AXION__.compat.installTextInputSelectionPatch`. Use it as the starting pattern when a Servo-backed page needs more stable caret placement or drag selection in `input` and `textarea` controls.

Generated apps install Axion panic reporting by default. Crash reports are written under `target/axion/crash-reports/`, which is ignored by the generated `.gitignore`.

## Validate a Generated App

From the Axion repository root:

```sh
cargo run -p axion-cli -- check --manifest-path /tmp/demo-app/axion.toml --bundle
cargo run -p axion-cli -- doctor --manifest-path /tmp/demo-app/axion.toml --deny-warnings --max-risk medium
cargo run -p axion-cli -- self-test --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- gui-smoke \
  --manifest-path /tmp/demo-app/axion.toml \
  --report-path target/axion/reports/demo-app-gui-smoke.json \
  --timeout-ms 30000 \
  --cargo-target-dir target \
  --serial-build
cargo run -p axion-cli -- build --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- bundle --manifest-path /tmp/demo-app/axion.toml --build-executable
cargo run -p axion-cli -- bundle --manifest-path /tmp/demo-app/axion.toml --build-executable --json --report-path target/axion/reports/demo-app-bundle.json
```

`check` is the fastest default validation loop: it runs the doctor gate, readiness, quiet self-test staging, and optional bundle preflight. Use `check --json` for CI and `doctor` when you need the full diagnostics detail. Continue when development, bundle, and GUI smoke readiness are all `true`; otherwise resolve the printed `readiness.blocker` lines first.

`self-test` prints app metadata, native dialog backend, each window's configured commands/events/protocols, runtime command/event counts, host events, navigation origins, and staged asset paths. Add `--json` to print an `axion.diagnostics-report.v1` report, or `--report-path <path>` to write that report while keeping the default text output. Add `--quiet` with `--report-path` in CI when only the exit code and report file are needed.

`gui-smoke` launches the generated app with `servo-runtime`, calls the generated `window.__AXION_GUI_SMOKE__()` hook, and writes a GUI diagnostics report. Use `--cargo-target-dir target` from the Axion checkout to reuse Servo build artifacts, and `--serial-build` when the local machine is resource-constrained.

To customize an application icon in bundle scaffolds, update `[bundle] icon = "icons/app.icns"` in `axion.toml` and keep the icon file inside the project directory. Bundle output includes `target`, `layout`, `bundle_dir`, `bundle_manifest`, `platform_metadata`, `checked_files`, `fingerprinted_files`, `bundle_bytes`, and `axion-bundle-manifest.json`, which records the generated entry, metadata, icon, executable, file sizes, and `fnv1a64` fingerprints. The `bundle` command prints `verification: ok` after checking those references against the generated files. Use `bundle --json` to emit `axion.bundle-report.v1`, and `--report-path` to write it for CI or scripted release checks.

`build` and `bundle` produce staging output, not signed production installers. To include an app executable, build it first or pass `--build-executable` to `bundle`.
