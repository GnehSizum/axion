# Manifest Guide

Axion applications are configured with `axion.toml`.

## Minimal Manifest

```toml
[app]
name = "hello-axion"
identifier = "dev.axion.hello"
version = "0.1.0"
description = "Hello Axion example"
authors = ["Axion Maintainers"]
homepage = "https://example.dev/hello-axion"

[window]
id = "main"
title = "Hello Axion"
width = 960
height = 720
resizable = true
visible = true

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[dev]
url = "http://127.0.0.1:3000"

[native.dialog]
backend = "headless"

[capabilities.main]
commands = ["app.ping", "app.info", "app.version", "app.echo", "window.info", "window.set_title", "window.set_size"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = []
allow_remote_navigation = false
```

## App

- `name`: package-safe application name.
- `identifier`: stable reverse-DNS style app identifier.
- `version`: optional application version used by diagnostics and bundle metadata.
- `description`: optional human-readable app summary.
- `authors`: optional list of maintainers or organizations.
- `homepage`: optional project or product URL.

Optional metadata is surfaced by `app.info`, `axion doctor`, and `axion bundle` scaffolds. Empty metadata strings are ignored when loading the manifest.

## Window

`[window]` configures the default window. Multi-window apps can use a windows array when supported by the loader.

Important fields:

- `id`: unique window identifier.
- `title`: native window title.
- `width`, `height`: non-zero initial size.
- `visible`, `resizable`: native window flags.

Multi-window example:

```toml
[[windows]]
id = "main"
title = "Main"

[[windows]]
id = "settings"
title = "Settings"
visible = true

[capabilities.main]
commands = ["app.ping", "app.info", "window.list", "window.info", "window.focus", "window.set_title"]
events = ["app.log"]
protocols = ["axion"]

[capabilities.settings]
commands = ["window.info", "window.focus", "window.set_title"]
events = ["app.log"]
protocols = ["axion"]
```

When a window has permission for a window command, frontend code can optionally pass `{ target: "<window-id>" }` in the command payload to operate on another runtime window.

## Build

- `frontend_dist`: directory containing frontend assets.
- `entry`: HTML entry file. It must stay inside `frontend_dist`.

## Bundle

```toml
[bundle]
icon = "icons/app.icns"
```

- `icon`: optional project-relative icon file copied into bundle resources.

Icon paths must be relative to the manifest directory and must not contain `..`. `axion doctor`, `axion self-test`, and `axion bundle` validate that the configured icon exists, is a file, and is not a symlink. On macOS, the copied icon is referenced from `Info.plist` using `CFBundleIconFile`.

## Dev

- `url`: frontend dev server URL used by `axion dev --launch`.

If `[dev]` is absent, development planning still works, but `axion dev --launch` requires `--fallback-packaged` to launch packaged assets.

## Native

Native preview behavior is configured under `[native]`.

```toml
[native.dialog]
backend = "headless"
```

- `backend = "headless"`: default, deterministic behavior for CI and non-GUI validation.
- `backend = "system"`: preview system file dialogs. macOS uses `osascript`; unsupported platforms report `system-unavailable` and cancel.

`axion doctor` reports both the configured backend and the effective runtime backend so unsupported-platform fallbacks are visible before launch.

## Capabilities

Capabilities are scoped by window id:

```toml
[capabilities.main]
commands = ["app.ping", "app.version", "window.list", "window.info", "window.focus", "window.set_title", "window.set_size", "fs.read_text", "fs.write_text", "dialog.open", "dialog.save"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = ["https://docs.example"]
allow_remote_navigation = false
```

Only declared commands, frontend events, protocols, and navigation origins are available to that window.

Custom Rust commands use the same capability list as built-in commands. For example, a plugin command registered as `demo.greet` must appear in `commands` before frontend code can call `window.__AXION__.invoke("demo.greet", payload)`.
