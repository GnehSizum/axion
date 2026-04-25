# Manifest Guide

Axion applications are configured with `axion.toml`.

## Minimal Manifest

```toml
[app]
name = "hello-axion"
identifier = "dev.axion.hello"

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

[capabilities.main]
commands = ["app.ping", "app.info", "app.version", "app.echo", "window.info"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = []
allow_remote_navigation = false
```

## App

- `name`: package-safe application name.
- `identifier`: stable reverse-DNS style app identifier.

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
commands = ["app.ping", "app.info"]
events = ["app.log"]
protocols = ["axion"]

[capabilities.settings]
commands = ["window.info"]
events = ["app.log"]
protocols = ["axion"]
```

## Build

- `frontend_dist`: directory containing frontend assets.
- `entry`: HTML entry file. It must stay inside `frontend_dist`.

## Dev

- `url`: frontend dev server URL used by `axion dev --launch`.

If `[dev]` is absent, development planning still works, but `axion dev --launch` requires `--fallback-packaged` to launch packaged assets.

## Capabilities

Capabilities are scoped by window id:

```toml
[capabilities.main]
commands = ["app.ping", "app.version", "fs.read_text", "fs.write_text"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = ["https://docs.example"]
allow_remote_navigation = false
```

Only declared commands, frontend events, protocols, and navigation origins are available to that window.
