# Contributing

Axion is a Rust workspace built around a vendored Servo engine. Keep changes focused on Axion crates and examples; do not modify `servo/` for framework features.

## Local Checks

Run the narrowest relevant checks while iterating, then run the release gate before opening a pull request:

```sh
cargo fmt --all --check
cargo test --workspace
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/multi-window/axion.toml
```

Use `cargo check -p hello-axion --features servo-runtime` when touching Servo-backed launch code.

## Pull Requests

Include a summary, affected crates, user-visible behavior, and the commands you ran. Add screenshots or logs for window, bridge, or packaging changes.

## Scope

Prefer small patches. Preserve crate layering: `axion-core` owns public types, runtime crates orchestrate behavior, `axion-window-winit` owns desktop integration, and `axion-bridge` owns JavaScript bridge contracts.
