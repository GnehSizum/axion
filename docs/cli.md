# CLI Reference

The Axion CLI is currently run through Cargo:

```sh
cargo run -p axion-cli -- <command>
```

## `new`

Generate a minimal application.

```sh
cargo run -p axion-cli -- new demo-app --path /tmp/demo-app
```

Project names are normalized to lowercase kebab-case for package use.

## `dev`

Print the development launch plan from an `axion.toml` manifest.

```sh
cargo run -p axion-cli -- dev --manifest-path examples/hello-axion/axion.toml
```

Use `--launch` to request runtime launch behavior where supported.

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

Pass `--executable <path>` to include an already-built executable in the bundle scaffold.
