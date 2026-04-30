# Packaging

Axion v0.1.22.0 provides bundle scaffolds and preview release artifacts for local validation and early distribution experiments. These bundles are not signed installers yet.

## Bundle Command

Run from the Axion checkout:

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --json
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --report-path target/axion/reports/hello-bundle.json
```

The command copies `[build].frontend_dist` into a platform bundle, writes metadata from `[app]`, copies `[bundle].icon` when configured, writes `axion-bundle-manifest.json`, and verifies the generated files.

Use `--build-executable` for generated or standalone apps:

```sh
cargo run -p axion-cli -- bundle --manifest-path /tmp/demo-app/axion.toml --build-executable
```

## Bundle Layouts

- `macos-app`: `<app>.app/Contents/MacOS/`, `Contents/Resources/app/`, `Contents/Info.plist`, `Contents/PkgInfo`.
- `linux-dir`: `<app>/bin/`, `<app>/resources/app/`, `<app>/axion-bundle.txt`, `<app>/<app>.desktop`.
- `windows-dir`: `<app>/bin/<app>.exe`, `<app>/resources/app/`, `<app>/axion-bundle.txt`, `<app>/axion-windows-metadata.txt`.

`axion bundle` prints `target`, `layout`, `bundle_dir`, `resources_app_dir`, `entry_path`, `metadata`, `platform_metadata`, `bundle_manifest`, and verification counters. `--json` emits the stable `axion.bundle-report.v1` schema for CI and release automation.

## Verification

`verification: ok` means Axion checked required directories, required files, platform metadata, optional icon and executable references, bundle manifest references, byte sizes, and `fnv1a64` fingerprints.

Inspect:

```sh
cat target/axion/hello-axion/bundle/hello-axion/axion-bundle-manifest.json
```

The exact bundle root differs by platform; use the printed `bundle_manifest` path.

## Bundle Report JSON

Use `--json` when automation needs a single parseable result. Use `--report-path <path>` to write the same schema to disk for upload as a CI artifact:

```sh
cargo run -p axion-cli -- bundle --manifest-path path/to/axion.toml --build-executable --json --report-path target/axion/reports/app-bundle.json
```

The report includes `target`, `layout`, generated paths, platform metadata paths, copied `icon` and `executable`, `verification.checked_paths`, `bundle_files`, `fingerprinted_files`, `bundle_bytes`, `blockers`, `warnings`, `report_path`, and `result`. When readiness blocks bundling, JSON output still uses the same schema with `result = "failed"`.

## Release Preview

Use `release` when you want one command to run the release gate, stage the bundle, write reports, and optionally archive the output:

```sh
cargo run -p axion-cli -- release --manifest-path path/to/axion.toml --json --report-path target/axion/reports/app-release.json --bundle-report-path target/axion/reports/app-bundle.json --archive --archive-path target/axion/reports/app-bundle.tar
```

`axion.release-report.v1` embeds the bundle report, records `failure_phase` and `failed_reasons`, inventories generated artifacts, and records archive path, bytes, `fnv1a64`, and verification status when `--archive` is passed. The archive is an unsigned `.tar` preview artifact.

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
cargo run -p axion-cli -- bundle --manifest-path path/to/axion.toml --build-executable --json --report-path target/axion/reports/app-bundle.json
cargo run -p axion-cli -- release --manifest-path path/to/axion.toml --json --report-path target/axion/reports/app-release.json --bundle-report-path target/axion/reports/app-bundle.json --archive --archive-path target/axion/reports/app-bundle.tar
```

Signing, notarization, auto-updates, and installer generation are deferred to later milestones.
