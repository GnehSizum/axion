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

`axion dev --launch` requires the configured frontend dev server to be running. If it is not reachable, the command exits with a diagnostic instead of silently launching packaged assets.

## Create a New App

```sh
cargo run -p axion-cli -- new demo-app --template vanilla --path /tmp/demo-app
cd /tmp/demo-app
cargo run -- --plan
cargo run --features servo-runtime
```

Generated projects contain:

- `Cargo.toml`: path dependencies back to this Axion checkout
- `README.md`: generated app usage notes
- `axion.toml`: app, window, build, and capability configuration
- `src/main.rs`: Rust entrypoint
- `frontend/index.html` and `frontend/app.js`: minimal bridge demo

## Validate a Generated App

From the Axion repository root:

```sh
cargo run -p axion-cli -- doctor --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- self-test --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- build --manifest-path /tmp/demo-app/axion.toml
cargo run -p axion-cli -- bundle --manifest-path /tmp/demo-app/axion.toml
```

`build` and `bundle` produce staging output, not signed production installers. To include an app executable, build it first or pass `--build-executable` to `bundle`.
