# Getting Started

Axion can generate a small Rust desktop app with a frontend page and an `axion.toml` manifest.

## Prerequisites

- The pinned Rust `1.97.1` toolchain used by the currently validated build. Rust `1.88.0` is the declared minimum and has not yet been validated; see the [platform support matrix](platform-support.md).
- A GUI-capable desktop session for `servo-runtime` window launches.
- This repository checked out with the vendored `servo/` directory present.

The repository pins Rust `1.97.1` in `rust-toolchain.toml`. Before building with `servo-runtime`, copy `.cargo/config.macos.example.toml` on macOS, or `.cargo/config.example.toml` on other platforms, to `.cargo/config.toml`; merge the environment and profile sections instead if a local config already exists. Cargo does not load `servo/.cargo/config.toml` for a path dependency. Install the native build dependencies listed in the [Servo build guide](https://book.servo.org/building/building.html); on macOS, LLVM and Python must be available to the build tools.

The macOS config selects Clang from PATH and uses the SDK-provided linker to avoid LLVM lld parsing failures with newer SDKs, and explicitly disables Rust symbol stripping to avoid the [macOS 27 dynamic-library alignment issue](https://github.com/rust-lang/rust/issues/157750) in Rust 1.97.1. The `strip = "none"` settings belong in `[profile.dev]` and `[profile.release]`; putting `CARGO_PROFILE_*` keys under `[env]` does not configure Cargo itself. This can increase release artifact size. If the system Clang is unsupported by the Servo native build, put a newer LLVM Clang on PATH or set `CC`, `CXX`, `HOST_CC`, and `HOST_CXX` to its full paths while retaining the linker arguments.

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

The `main` window can call app-level commands, while the `settings` and `preview` windows are restricted to window-local controls such as `window.info`, `window.focus`, and `window.set_title`.
The updated example also lets the `main` window use `window.list` plus `{ target: "settings" }` or `{ target: "preview" }` to inspect, rename, and close secondary windows.

To inspect controlled filesystem and dialog capabilities:

```sh
cargo run -p file-access-demo --features servo-runtime
```

This example writes and reads `notes/demo.txt` inside its operating system user-data directory, emits `app.log`, and shows the preview `dialog.open` / `dialog.save` responses configured by `[native.dialog]`.
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

The preview flags `--watch`, `--reload`, and `--restart-on-change` are available for frontend development. `--watch` polls `[build].frontend_dist`, ignores common temporary files and cache directories, debounces editor save bursts, and reports created, modified, and deleted files. `--reload` reports `reload_requested`; with `--launch`, Axion asks each live window to reload and prints `reload_applied`, `reload_deferred`, or `restart_required`. `--restart-on-change` relaunches after watched changes when live reload is not requested or cannot cover every window. `--json-events` prints stable `axion.dev-event.v1` JSONL events, `--event-log <path>` writes those events for automation, and `--report-path <path>` writes a stable `axion.dev-report.v1` session summary. Without `--launch`, reload and restart remain diagnostic-only because there is no live window target. `--open-devtools` is accepted for diagnostics, but the current Servo backend does not open devtools yet.

To test live reload and restart fallback, launch with `--features servo-runtime --launch --fallback-packaged --watch --reload --restart-on-change --event-log target/axion/reports/hello-dev-events.jsonl --report-path target/axion/reports/hello-dev-report.json`, then edit a file in the app's `frontend/` directory. `hello-axion` should report `reload_applied: window=main`; if reload is unavailable, Axion reports restart diagnostics and relaunches after the current windows close.

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

`--run-check` immediately runs `axion check --dev --bundle` against the generated manifest. Omit it if you only want to create files.

For an application outside the checkout, use an explicit SDK and an installed CLI. Run the install from the SDK root after applying the prerequisite Cargo configuration:

```sh
cargo install --path crates/axion-cli --features servo-runtime --locked
axion-cli new demo-app --sdk-path "/path/源码 SDK" --path "/path/我的应用" --run-check
cd "/path/我的应用"
axion-cli doctor --json
axion-cli check --dev --bundle
axion-cli gui-smoke --cargo-target-dir target --serial-build
axion-cli release --archive
```

The local source SDK must exactly match the CLI's Axion version; it includes the vendored Servo source and build settings. Both templates copy `rust-toolchain.toml` and the platform Cargo configuration from that selected SDK. SDK paths are local development dependencies; deployed applications still run without the SDK. Doctor discovers the SDK from the application's Cargo metadata rather than its directory ancestors. To relocate or upgrade an existing SDK binding, follow the three-dependency rebinding steps in [CLI Reference](cli.md#rebind-an-existing-application-to-a-local-sdk). Run Cargo-based commands from the application directory to load its generated configuration.

Use `--template native-api-demo` when you want generated UI and README guidance focused on the preview native API surface: app/window metadata, clipboard text, shell URL validation, app-data file lifecycle operations, dialogs, input compatibility, and GUI smoke diagnostics. The generated Native API Workbench includes a "Run all checks" button for manual validation inside the app window.

Generated projects contain:

- `rust-toolchain.toml`: Servo-compatible Rust toolchain
- `.cargo/config.toml`: Servo build environment for the platform where the project was generated
- `Cargo.toml`: path dependencies back to this Axion checkout
- `.gitignore`: ignores `target/` build output, local validation artifacts and bundles
- `README.md`: generated app usage notes
- `axion.toml`: app, window, build, and capability configuration
- `icons/app.icns`: default bundle icon referenced by `[bundle]`
- `src/main.rs`: Rust entrypoint with panic reporting and a `demo.greet` custom command plugin
- `frontend/index.html`: packaged HTML entry
- `frontend/style.css`: CSP-compatible external styles
- `frontend/app.js`: bridge, native API, input-compatibility, custom command, event, and denied-command demos

The generated `demo.greet` command is registered in Rust, allowed in `[capabilities.main]`, and invoked from frontend JavaScript. See `custom-commands.md` for the pattern.

Generated manifests also include optional app metadata (`version`, `description`, `authors`, and `homepage`), `[bundle] icon = "icons/app.icns"`, `[native.dialog] backend = "headless"`, `[native.clipboard] backend = "memory"`, and `shell-access` capability examples. These values appear in `app.info`, `axion doctor`, self-test output, and bundle metadata scaffolds. The generated frontend also demonstrates lifecycle capability reporting, clipboard read/write, shell URL validation, app-data create/exists/list/read/remove/write, `dialog.open` with multi-select and filter metadata, and `dialog.save` with `defaultPath`.

Generated manifests include commented `[dev]` lines. Uncomment them when you attach a frontend toolchain such as Vite, Trunk, or another static server. You can start that server separately before running `axion dev --launch`, or set `[dev] command` / pass `--frontend-command` so Axion starts it for you.

Generated frontends now include a small text-input compatibility panel wired to `window.__AXION__.compat.installTextInputSelectionPatch`. Use it as the starting pattern when a Servo-backed page needs more stable caret placement or drag selection in `input` and `textarea` controls.

Generated apps install Axion panic reporting by default. Production crash reports are written to the `crash-reports/` subdirectory of the operating system user-data directory, using the application identifier. Tests can set `[native.fs] app_data_dir` to an isolated temporary directory.

## Validate a Generated App

From the Axion repository root:

```sh
cargo run -p axion-cli -- check --manifest-path /tmp/demo-app/axion.toml --dev --bundle --report-path target/axion/reports/check.json
cargo run -p axion-cli -- doctor --manifest-path /tmp/demo-app/axion.toml --deny-warnings --max-risk medium
cargo run -p axion-cli -- self-test --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- gui-smoke \
  --manifest-path /tmp/demo-app/axion.toml \
  --report-path target/axion/reports/demo-app-gui-smoke.json \
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
cargo run -p axion-cli -- build --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- bundle --manifest-path /tmp/demo-app/axion.toml --build-executable
cargo run -p axion-cli -- bundle --manifest-path /tmp/demo-app/axion.toml --build-executable --json --report-path target/axion/reports/demo-app-bundle.json
cargo run -p axion-cli -- release --manifest-path /tmp/demo-app/axion.toml --check-report-path target/axion/reports/check.json --json --report-path target/axion/reports/demo-app-release.json --bundle-report-path target/axion/reports/demo-app-bundle.json --archive
cargo run -p axion-cli -- report target/axion/reports/demo-app-release.json --output target/axion/reports/demo-app-release-summary.json
```

`check` is the fastest default validation loop: it runs the doctor gate, readiness, quiet self-test staging, and optional dev/bundle preflight. Use `check --dev --bundle --json --report-path target/axion/reports/check.json` for CI and `doctor` when you need the full diagnostics detail. Continue when development, bundle, and GUI smoke readiness are all `true`; otherwise resolve the printed `readiness.blocker` or `dev.blocker` lines first. `dev.warning` entries are advisory and commonly report a missing or unreachable dev server when packaged fallback is available. The check report includes `artifacts[]` with recommended report paths under `target/axion/reports/`.

`self-test` prints app metadata, native dialog backend, each window's configured commands/events/protocols, runtime command/event counts, host events, navigation origins, and staged asset paths. Add `--json` to print an `axion.diagnostics-report.v1` report, or `--report-path <path>` to write that report while keeping the default text output. Add `--quiet` with `--report-path` in CI when only the exit code and report file are needed.

`gui-smoke` launches the generated app with `servo-runtime`, calls the generated `window.__AXION_GUI_SMOKE__()` hook, and writes a GUI diagnostics report. Use `--require-check`, `--require-command`, `--require-host-event`, and `--require-window` to make runtime coverage explicit, `--cargo-target-dir target` from the Axion checkout to reuse Servo build artifacts, and `--serial-build` when the local machine is resource-constrained.

To customize an application icon in bundle scaffolds, update `[bundle] icon = "icons/app.icns"` in `axion.toml` and keep the icon file inside the project directory. Bundle output includes `target`, `layout`, `bundle_dir`, `bundle_manifest`, `platform_metadata`, `checked_files`, `fingerprinted_files`, `bundle_bytes`, and `axion-bundle-manifest.json`, which records the generated entry, metadata, icon, executable, file sizes, and `fnv1a64` fingerprints. The `bundle` command prints `verification: ok` after checking those references against the generated files. Use `bundle --json` to emit `axion.bundle-report.v1`, and `--report-path` to write it for CI or scripted release checks.

`build` and `bundle` produce staging output, not signed production installers. To include an app executable, build it first or pass `--build-executable` to `bundle`.
`release` runs the preview artifact workflow and can create an unsigned `.tar` archive with `--archive`.

## Vendored Servo Source

`SERVO_PROVENANCE.toml` records the Servo v0.6.0 tag, pinned commit, downloaded archive SHA-256, source comparison and Axion build feature/toolchain policy. Servo remains vendored in `servo/`; local virtual environments and build outputs are excluded from that source record. The archive hash identifies the downloaded bytes and is not a release signature. Update the record, workspace metadata and build configuration together when changing the engine.
