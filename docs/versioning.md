# Versioning Policy

Axion uses a four-part public release version and a three-part Cargo package version.

## Public Release Version

Public releases and Git tags use this format:

```text
v<servo-major>.<servo-minor>.<feature>.<bugfix>
```

Example: `v0.1.29.0`.

- `servo-major.servo-minor`: follows the vendored Servo baseline tracked by Axion.
- `feature`: increments when Axion adds user-visible framework capabilities.
- `bugfix`: increments for compatible fixes that do not add new features.

The current release metadata is recorded in `Cargo.toml` under `[workspace.metadata.axion]`.

## Cargo Package Version

Rust crates in this workspace use Cargo-compatible SemVer:

```text
<servo-major>.<servo-minor>.<feature>
```

For public release `v0.1.29.0`, workspace crates use Cargo version `0.1.29`. Bugfix releases keep the same public feature prefix and update the public bugfix component. If crates are published externally, decide per bugfix release whether the Cargo patch version also needs to move.

## Current Release Baseline

The current release baseline is:

- public release: `v0.1.29.0`
- Cargo workspace version: `0.1.29`
- Servo baseline: `0.1`
- Axion feature version: `28`
- Axion bugfix version: `0`

## Runtime Reporting

- `app.version` returns both the Cargo crate version and the Axion public release.
- `window.__AXION__.version` reports the bridge bootstrap version for the public release.
- Platform bundle metadata uses the Cargo-compatible version where a platform expects three numeric components.
