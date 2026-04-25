# CLI Reference

The Axion CLI is currently run through Cargo:

```sh
cargo run -p axion-cli -- <command>
```

## `new`

Generate a minimal application.

```sh
cargo run -p axion-cli -- new demo-app --template vanilla --path /tmp/demo-app
```

Project names are normalized to lowercase kebab-case for package use.

Options:

- `--template vanilla`: generate a plain HTML/CSS/JavaScript app with a bridge demo.
- `--path <path>`: choose the output directory.

## `dev`

Print the development launch plan from an `axion.toml` manifest and report dev server reachability.

```sh
cargo run -p axion-cli -- dev --manifest-path examples/hello-axion/axion.toml
```

Use `--launch` with the `servo-runtime` feature to start the app in development mode when the configured dev server is reachable:

```sh
cargo run -p axion-cli --features servo-runtime -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --launch
```

If the dev server is not configured or unreachable, `--launch` fails with a diagnostic. Pass `--fallback-packaged` only when you intentionally want to launch packaged assets instead:

```sh
cargo run -p axion-cli --features servo-runtime -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --launch \
  --fallback-packaged
```

## `doctor`

Validate local tooling, manifest configuration, frontend assets, runtime diagnostics, and Servo path availability.

```sh
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
```

## `self-test`

Run the non-GUI release gate for a manifest. It loads the app, checks runtime diagnostics, stages frontend assets, and removes generated artifacts by default.

```sh
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml --keep-artifacts
```

## `build`

Stage frontend assets into an Axion app directory.

```sh
cargo run -p axion-cli -- build --manifest-path examples/hello-axion/axion.toml
```

## `bundle`

Create a platform bundle scaffold and copy staged app resources.

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml
```

Executable handling:

- If `--executable <path>` is passed, that binary is copied into the bundle.
- If no executable is passed, Axion searches nearby `target/release/` and `target/debug/` directories for a binary matching the app name.
- Pass `--build-executable` to run `cargo build --release` for the app before bundling.

```sh
cargo run -p axion-cli -- bundle \
  --manifest-path examples/hello-axion/axion.toml \
  --build-executable
```
