# Axion

Axion is a Rust desktop application framework built on a vendored Servo engine. It provides an explicit manifest, capability-gated JavaScript bridge, packaged app assets, runtime diagnostics, and a `winit` desktop backend.

Axion is currently a **v0.1.0 developer preview**. It is suitable for framework experiments, examples, and early application prototypes. Production installers, signing, auto-updates, and rich native APIs are intentionally deferred.

## What Works Today

- Generate a minimal Axion application with `axion-cli new`.
- Load and validate `axion.toml` manifests.
- Run runtime planning and diagnostics without opening a window.
- Compile and launch a Servo-backed desktop window behind `servo-runtime`.
- Invoke built-in bridge commands from frontend JavaScript.
- Stage frontend assets for build and bundle scaffolds.

## Quick Start

Run the existing example:

```sh
cargo run -p hello-axion -- --plan
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
AXION_SELFTEST_BRIDGE=1 cargo run -p hello-axion --features servo-runtime
```

Generate a new application:

```sh
cargo run -p axion-cli -- new demo-app --path /tmp/demo-app
cd /tmp/demo-app
cargo run -- --plan
cargo run --features servo-runtime
```

`AXION_SELFTEST_BRIDGE=1` starts a GUI bridge self-test and exits automatically after success. Omit it when you want the application window to stay open.

## Repository Layout

- `crates/axion-core`: app, window, builder, and runtime-plan APIs
- `crates/axion-runtime`: launch planning, diagnostics, plugins, panic reports
- `crates/axion-window-winit`: Servo/winit desktop backend
- `crates/axion-bridge`: JavaScript bootstrap, commands, frontend events, host events
- `crates/axion-manifest`: `axion.toml` parsing and validation
- `crates/axion-security`: capabilities, origins, navigation, CSP
- `crates/axion-protocol`: `axion://app` asset resolver and response policy
- `crates/axion-packager`: build and bundle staging
- `crates/axion-cli`: `new`, `dev`, `build`, `bundle`, `doctor`, `self-test`
- `examples/`: smoke applications
- `docs/`: public user-facing documentation
- `servo/`: vendored engine source; do not modify for Axion framework features

## Documentation

- Public docs: `docs/README.md`
- Getting started: `docs/getting-started.md`
- CLI reference: `docs/cli.md`
- Manifest guide: `docs/manifest.md`
- Architecture overview: `docs/architecture.md`
- Security model: `docs/security.md`

## Development Checks

```sh
cargo fmt --all --check
cargo test --workspace
cargo check -p axion-window-winit --features servo-runtime
cargo check -p hello-axion --features servo-runtime
cargo check -p multi-window --features servo-runtime
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
```

Servo warnings from the vendored `servo/` subtree are not Axion release blockers unless they correspond to an Axion regression.
