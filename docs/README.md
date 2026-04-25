# Axion Documentation

This directory contains public, user-facing documentation for Axion.

## Start Here

- `getting-started.md`: create and run a minimal Axion app.
- `cli.md`: command reference for `axion-cli`.
- `manifest.md`: `axion.toml` configuration guide.
- `architecture.md`: high-level runtime architecture.
- `security.md`: capabilities, bridge permissions, navigation, and CSP.

## Current Version

Axion is at **v0.1.0 developer preview**. The preview focuses on the core desktop framework loop:

1. load an app manifest
2. build a runtime plan
3. stage frontend assets
4. inject a controlled JavaScript bridge
5. launch a Servo-backed `winit` window when `servo-runtime` is enabled

Project-internal milestone plans and release notes are intentionally not part of the public documentation set.
