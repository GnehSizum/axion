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

[capabilities.main]
commands = ["app.ping", "app.info", "app.echo", "window.info"]
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

## Build

- `frontend_dist`: directory containing frontend assets.
- `entry`: HTML entry file. It must stay inside `frontend_dist`.

## Capabilities

Capabilities are scoped by window id:

```toml
[capabilities.main]
commands = ["app.ping"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = ["https://docs.example"]
allow_remote_navigation = false
```

Only declared commands, frontend events, protocols, and navigation origins are available to that window.
