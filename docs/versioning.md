# Versioning Policy

Axion public releases and Cargo workspace packages use matching three-part SemVer versions.

## Public Release Version

Public releases and Git tags use this format:

```text
v<major>.<minor>.<patch>
```

Example: `v0.6.2`.

The public version does not append the internal Axion feature milestone or a separate bugfix component. The Servo baseline and internal feature milestone are recorded separately in `Cargo.toml` under `[workspace.metadata.axion]`.

## Cargo Package Version

Rust crates in this workspace use the same version without the `v` prefix:

```text
<major>.<minor>.<patch>
```

For public release `v0.6.2`, workspace crates use Cargo version `0.6.2`.

## Current Release Baseline

The current release baseline is:

- public release: `v0.6.2`
- Cargo workspace version: `0.6.2`
- Servo baseline: `0.6` (vendored engine release `0.6.0`)
- internal Axion feature milestone: `33`
- internal Axion bugfix milestone: `2`

## Historical Versions

Earlier releases used four-part public tags such as `v0.1.33.0`, with Cargo version `0.1.33`. Those tags describe historical releases and are not the current version format.

## Runtime Reporting

- `app.version` returns both the Cargo crate version and the Axion public release.
- `window.__AXION__.version` reports the bridge bootstrap version for the public release.
- Platform bundle metadata uses the Cargo-compatible version where a platform expects three numeric components.
