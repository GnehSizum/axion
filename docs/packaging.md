# Packaging

Axion v0.6.1 provides bundle scaffolds and preview release artifacts for local validation and early distribution experiments. These bundles are not signed installers yet.

## Bundle Command

Run from the Axion checkout:

```sh
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --json
cargo run -p axion-cli -- bundle --manifest-path examples/hello-axion/axion.toml --report-path target/axion/reports/hello-bundle.json
```

The command copies `[build].frontend_dist` into a platform bundle, writes a deployment `axion.toml`, writes metadata from `[app]`, copies `[bundle].icon` when configured, writes `axion-bundle-manifest.json`, and verifies the generated files. Before removing an existing output, it rejects overlap in either direction with the frontend, entry, source manifest, executable, or icon. Rejected input/output layouts leave the source files intact.

Use `--build-executable` for generated or standalone apps:

```sh
cargo run -p axion-cli -- bundle --manifest-path /tmp/demo-app/axion.toml --build-executable
```

The build path invokes Cargo from the application's directory with `--release --features servo-runtime` and reads `compiler-artifact.executable` from Cargo JSON messages. It supports a custom `CARGO_TARGET_DIR` and binary names that differ from `[app].name`; use `--bin <name>` when Cargo emits multiple binaries. A requested executable build fails if it produces no usable executable. Applications using a different runtime feature contract can supply a prebuilt `--executable` instead.

## Bundle Layouts

- `macos-app`: `<app>.app/Contents/MacOS/`, `Contents/Resources/app/`, `Contents/Info.plist`, `Contents/PkgInfo`.
- `linux-dir`: `<app>/bin/`, `<app>/resources/app/`, `<app>/axion-bundle.txt`, `<app>/<app>.desktop`.
- `windows-dir`: `<app>/bin/<app>.exe`, `<app>/resources/app/`, `<app>/axion-bundle.txt`, `<app>/axion-windows-metadata.txt`.

The deployment configuration lives beside the frontend directory: `Contents/Resources/axion.toml` on macOS and `resources/axion.toml` in directory bundles. Its `[build]` paths are relative to that configuration (`frontend_dist = "app"`, `entry = "app/<entry>"`). App identity, windows, native settings, and capabilities are retained; development-server and packaging-only settings are omitted. The configuration is included in the bundle file inventory and fingerprints.

Generated applications locate this configuration relative to their executable when deployed, so the original source directory and the current working directory are not needed. The lower-level packager APIs that stage only web assets remain available for scaffolds; callers supplying their own executable must also provide a deployment-aware application entry point.

Frontend filenames may contain Unicode, spaces, and URL-reserved characters such as `#`, `?`, and `%`. Axion encodes filesystem path segments when constructing resource URLs and decodes incoming URL segments once. Decoded parent-directory traversal, path separators, NUL, and symlink assets are rejected. Do not pre-encode filesystem filenames.

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
cargo run -p axion-cli -- release --manifest-path path/to/axion.toml --check-report-path target/axion/reports/check.json --json --report-path target/axion/reports/app-release.json --bundle-report-path target/axion/reports/app-bundle.json --archive --archive-path target/axion/reports/app-bundle.tar
cargo run -p axion-cli -- report target/axion/reports/app-release.json --output target/axion/reports/app-release-summary.json
```

`axion.release-report.v1` embeds the bundle report, records optional `check_report` reuse, `failure_phase`, and `failed_reasons`, inventories generated artifacts, includes a compact artifact `summary`, and records archive path, bytes, `fnv1a64`, and verification status when `--archive` is passed. Release always reruns the current doctor gate and readiness checks. A reused report must have successful self-test and bundle preflight, match the canonical manifest location and risk threshold, and carry a SHA-256 content identity covering the manifest, frontend tree, icon, framework version and check parameters. Changed inputs or reports without this identity require a new `check --bundle`. The archive is an unsigned `.tar` preview artifact. Entries have deterministic timestamps and owner fields; files use mode `0755` when executable and `0644` otherwise. Verification reads back every member and checks its path, type, normalized mode, size and content against the bundle, in addition to the archive fingerprint. Archive output must stay outside the bundle.

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
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets
cargo run -p axion-cli -- doctor --manifest-path path/to/axion.toml
cargo run -p axion-cli -- self-test --manifest-path path/to/axion.toml --json
cargo run -p axion-cli -- bundle --manifest-path path/to/axion.toml --build-executable
cargo run -p axion-cli -- bundle --manifest-path path/to/axion.toml --build-executable --json --report-path target/axion/reports/app-bundle.json
cargo run -p axion-cli -- release --manifest-path path/to/axion.toml --check-report-path target/axion/reports/check.json --json --report-path target/axion/reports/app-release.json --bundle-report-path target/axion/reports/app-bundle.json --archive --archive-path target/axion/reports/app-bundle.tar
cargo run -p axion-cli -- report target/axion/reports/app-release.json --output target/axion/reports/app-release-summary.json
```

Signing, notarization, auto-updates, and installer generation are deferred to later milestones.
