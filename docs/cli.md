# CLI Reference

The Axion CLI is currently run through Cargo:

```sh
cargo run -p axion-cli -- <command>
```

## `new`

Generate a minimal application with local artifact hygiene, panic reporting, a default bundle icon, and bridge demos.

```sh
cargo run -p axion-cli -- new demo-app --template vanilla --path /tmp/demo-app
```

Project names are normalized to lowercase kebab-case for package use.

Options:

- `--template vanilla`: generate a plain HTML/CSS/JavaScript app with bridge, native API, custom command, capability-denial, and bundle-icon demos.
- `--path <path>`: choose the output directory.

## `dev`

Print development diagnostics from an `axion.toml` manifest: launch mode, dev server reachability, packaged fallback status, per-window entry URLs, and the runtime plan.

```sh
cargo run -p axion-cli -- dev --manifest-path examples/hello-axion/axion.toml
```

Typical output when the configured dev server is not running:

```text
Axion development diagnostics
manifest: examples/hello-axion/axion.toml
launch_mode: blocked (dev server is not reachable at http://127.0.0.1:3000/; start the frontend dev server or pass --fallback-packaged)
dev_server: unreachable (http://127.0.0.1:3000/)
packaged_fallback: disabled; available with --fallback-packaged (axion://app/index.html)
window_entries:
- main: unavailable (launch blocked)
runtime_plan:
...
```

The `launch_mode` line is authoritative for `--launch`. The trailing `runtime_plan` is informational and may still show the manifest-derived development entrypoint.

Use `--launch` with the `servo-runtime` feature to start the app in development mode when the configured dev server is reachable:

```sh
cargo run -p axion-cli --features servo-runtime -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --launch
```

If the dev server is not configured or unreachable, `--launch` fails with a diagnostic. Pass `--fallback-packaged` only when you intentionally want to launch packaged assets instead; the CLI validates that the packaged entry is available before selecting production mode:

```sh
cargo run -p axion-cli --features servo-runtime -- dev \
  --manifest-path examples/hello-axion/axion.toml \
  --launch \
  --fallback-packaged
```

## `doctor`

Validate local tooling, manifest configuration, app metadata, native dialog backend configuration, effective runtime dialog backend, frontend assets, runtime diagnostics, and Servo path availability.

```sh
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
```

## `self-test`

Run the non-GUI release gate for a manifest. It loads the app, prints app metadata, configured and effective native dialog backend, and per-window capability/runtime summaries, checks runtime diagnostics, stages frontend assets, and removes generated artifacts by default.

```sh
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml --keep-artifacts
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml --json
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml --report-path target/axion/reports/hello-self-test.json
```

Use `--json` to print a machine-readable `axion.diagnostics-report.v1` report. Use `--report-path <path>` to write the same report to disk while keeping the default human-readable output.

## `build`

Stage frontend assets into an Axion app directory.

```sh
cargo run -p axion-cli -- build --manifest-path examples/hello-axion/axion.toml
```

## `bundle`

Create a platform bundle scaffold and copy staged app resources. App metadata from `[app]` and icon configuration from `[bundle]` are written into bundle metadata files where supported. Each bundle also includes `axion-bundle-manifest.json`, a deterministic integrity manifest listing bundle paths, byte sizes, and `fnv1a64` content fingerprints. After staging, `bundle` verifies the generated entry, metadata, asset manifest, bundle manifest, icon, executable references, sizes, and fingerprints.

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml
```

Executable handling:

- If `--executable <path>` is passed, that binary is copied into the bundle.
- If no executable is passed, Axion searches nearby `target/release/` and `target/debug/` directories for a binary matching the app name.
- Pass `--build-executable` to run `cargo build --release` for the app before bundling.
- Use the printed `bundle_manifest` path to inspect the generated entry, resource, metadata, icon, executable, and file list.
- `verification: ok` means every referenced bundle path exists and the manifest file list matches generated file sizes and `fnv1a64` fingerprints.

```sh
cargo run -p axion-cli -- bundle \
  --manifest-path examples/hello-axion/axion.toml \
  --build-executable
```
