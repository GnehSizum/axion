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
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/multi-window/axion.toml
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

To inspect per-window capability behavior:

```sh
cargo run -p multi-window --features servo-runtime
```

The `main` window can call app-level commands, while the `settings` window is limited to `window.info`.

To inspect the development launch path:

```sh
cargo run -p axion-cli -- dev --manifest-path examples/hello-axion/axion.toml
```

`axion dev` reports the selected launch mode, dev-server reachability, packaged fallback availability, and each window entry URL. `axion dev --launch` requires the configured frontend dev server to be running. If it is not reachable, the command exits with a diagnostic instead of silently launching packaged assets. Use `--fallback-packaged` only when you explicitly want to launch the packaged `axion://app` entry instead.

## Create a New App

```sh
cargo run -p axion-cli -- new demo-app --template vanilla --path /tmp/demo-app
cd /tmp/demo-app
cargo run -- --plan
cargo run --features servo-runtime
```

Generated projects contain:

- `Cargo.toml`: path dependencies back to this Axion checkout
- `.gitignore`: ignores `target/` build output, runtime data, bundles, and crash reports
- `README.md`: generated app usage notes
- `axion.toml`: app, window, build, and capability configuration
- `icons/app.icns`: default bundle icon referenced by `[bundle]`
- `src/main.rs`: Rust entrypoint with panic reporting and a `demo.greet` custom command plugin
- `frontend/index.html`: packaged HTML entry
- `frontend/style.css`: CSP-compatible external styles
- `frontend/app.js`: bridge, native API, filesystem, custom command, event, and denied-command demos

The generated `demo.greet` command is registered in Rust, allowed in `[capabilities.main]`, and invoked from frontend JavaScript. See `custom-commands.md` for the pattern.

Generated manifests also include optional app metadata (`version`, `description`, `authors`, and `homepage`) plus `[bundle] icon = "icons/app.icns"`. These values appear in `app.info`, `axion doctor`, and bundle metadata scaffolds.

Generated apps install Axion panic reporting by default. Crash reports are written under `target/axion/crash-reports/`, which is ignored by the generated `.gitignore`.

## Validate a Generated App

From the Axion repository root:

```sh
cargo run -p axion-cli -- doctor --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- self-test --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- build --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- bundle --manifest-path /tmp/demo-app/axion.toml
```

`self-test` prints app metadata, each window's configured commands/events/protocols, runtime command/event counts, host events, navigation origins, and staged asset paths.

To customize an application icon in bundle scaffolds, update `[bundle] icon = "icons/app.icns"` in `axion.toml` and keep the icon file inside the project directory. Bundle output includes `axion-bundle-manifest.json`, which records the generated entry, metadata, icon, executable, file sizes, and `fnv1a64` fingerprints. The `bundle` command prints `verification: ok` after checking those references against the generated files.

`build` and `bundle` produce staging output, not signed production installers. To include an app executable, build it first or pass `--build-executable` to `bundle`.
