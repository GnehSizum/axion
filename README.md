# Axion

Axion is a Rust desktop application framework built on a vendored Servo engine. It provides an explicit manifest, capability-gated JavaScript bridge, packaged app assets, runtime diagnostics, and a `winit` desktop backend.

Axion is currently at the **v0.1.3.0 developer preview**. It is suitable for framework experiments, examples, and early application prototypes. Production installers, signing, auto-updates, and a complete native API surface are intentionally deferred.

## What Works Today

- Generate a guided Axion application with `axion-cli new --template vanilla`.
- Load and validate `axion.toml` manifests.
- Install crash reporting in generated and example applications.
- Run runtime planning and diagnostics without opening a window.
- Compile and launch a Servo-backed desktop window behind `servo-runtime`.
- Invoke built-in bridge commands from frontend JavaScript.
- Use capability-gated native commands for app metadata and app-data text files.
- Stage and verify bundle scaffolds with app icon, executable, metadata, and fingerprinted bundle manifest output.
- Inspect per-window capabilities with the `multi-window` example.
- Run non-GUI CI checks for formatting, workspace tests, and example self-tests.

## Quick Start

Run the existing example:

```sh
cargo run -p hello-axion -- --plan
cargo run -p multi-window -- --plan
cargo run -p axion-cli -- dev --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
AXION_SELFTEST_BRIDGE=1 cargo run -p hello-axion --features servo-runtime
```

Generate a new application:

```sh
cargo run -p axion-cli -- new demo-app --template vanilla --path /tmp/demo-app
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
- Native API reference: `docs/native-api.md`
- Custom command guide: `docs/custom-commands.md`
- Versioning policy: `docs/versioning.md`
- Architecture overview: `docs/architecture.md`
- Security model: `docs/security.md`
- Contribution guide: `CONTRIBUTING.md`
- Security reporting: `SECURITY.md`

## Development Checks

```sh
cargo fmt --all --check
cargo test --workspace
cargo check -p axion-cli --features servo-runtime
cargo check -p hello-axion --features servo-runtime
cargo check -p multi-window --features servo-runtime
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- doctor --manifest-path examples/multi-window/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/multi-window/axion.toml
cargo run -p axion-cli -- bundle --manifest-path examples/multi-window/axion.toml
```

Servo warnings from the vendored `servo/` subtree are not Axion release blockers unless they correspond to an Axion regression.

## Versioning

Axion public releases use four-part tags such as `v0.1.3.0`: the first two components track the Servo baseline, the third tracks Axion feature milestones, and the fourth tracks bugfix releases. Cargo crates use compatible three-part versions such as `0.1.3`. See `docs/versioning.md`.
