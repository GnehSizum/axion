# Axion Documentation

This directory contains public, user-facing documentation for Axion.

## Start Here

- `getting-started.md`: create and run a minimal Axion app.
- `cli.md`: command reference for `axion-cli`.
- `manifest.md`: `axion.toml` configuration guide.
- `native-api.md`: built-in bridge command reference.
- `versioning.md`: public release and Cargo version mapping.
- `architecture.md`: high-level runtime architecture.
- `security.md`: capabilities, bridge permissions, navigation, and CSP.
- `../CONTRIBUTING.md`: contributor workflow and local checks.
- `../SECURITY.md`: vulnerability reporting and policy summary.

## Current Version

Axion is at **v0.1.1.0 developer preview**. The preview focuses on the core desktop framework loop:

1. load an app manifest
2. build a runtime plan
3. stage frontend assets
4. inject a controlled JavaScript bridge
5. launch a Servo-backed `winit` window when `servo-runtime` is enabled
6. generate, validate, and bundle simple applications through `axion-cli`

Project-internal milestone plans and release notes are intentionally not part of the public documentation set.
