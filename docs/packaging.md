# Packaging

Axion v0.1.12.0 provides bundle scaffolds for local validation and early distribution experiments. These bundles are not signed installers yet.

## Bundle Command

Run from the Axion checkout:

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml
```

The command copies `[build].frontend_dist` into a platform bundle, writes metadata from `[app]`, copies `[bundle].icon` when configured, writes `axion-bundle-manifest.json`, and verifies the generated files.

Use `--build-executable` for generated or standalone apps:

```sh
cargo run -p axion-cli -- bundle --manifest-path /tmp/demo-app/axion.toml --build-executable
```

## Bundle Layouts

- `macos-app`: `<app>.app/Contents/MacOS/`, `Contents/Resources/app/`, `Contents/Info.plist`.
- `linux-dir`: `<app>/bin/`, `<app>/resources/app/`, `<app>/axion-bundle.txt`.
- `windows-dir`: `<app>/bin/<app>.exe`, `<app>/resources/app/`, `<app>/axion-bundle.txt`.

`axion bundle` prints `target`, `layout`, `bundle_dir`, `resources_app_dir`, `entry_path`, `metadata`, `bundle_manifest`, and verification counters.

## Verification

`verification: ok` means Axion checked required directories, required files, optional icon and executable references, bundle manifest references, byte sizes, and `fnv1a64` fingerprints.

Inspect:

```sh
cat target/axion/hello-axion/bundle/hello-axion/axion-bundle-manifest.json
```

The exact bundle root differs by platform; use the printed `bundle_manifest` path.

## Icons And Metadata

Set a project-local icon in `axion.toml`:

```toml
[bundle]
icon = "icons/app.icns"
```

`axion doctor` validates that the icon exists, is a file, is not a symlink, and reports its detected extension. macOS bundles reference the copied icon from `Info.plist`; Linux and Windows directory bundles copy it under `resources/`.

## Release Checklist

Before sharing a bundle, run:

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p axion-cli -- doctor --manifest-path path/to/axion.toml
cargo run -p axion-cli -- self-test --manifest-path path/to/axion.toml --json
cargo run -p axion-cli -- bundle --manifest-path path/to/axion.toml --build-executable
```

Signing, notarization, auto-updates, and installer generation are deferred to later milestones.
