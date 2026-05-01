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

[native.clipboard]
backend = "memory"

[capabilities.main]
profiles = ["app-info", "window-control", "app-events"]
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
profiles = ["app-info", "app-control", "multi-window", "app-events"]

[capabilities.settings]
profiles = ["app-events"]
commands = ["window.info", "window.focus", "window.set_title"]
```

When a window has permission for a window command, frontend code can optionally pass `{ target: "<window-id>" }` in the command payload to operate on another runtime window.

## Build

- `frontend_dist`: directory containing frontend assets.
- `entry`: HTML entry file. It must stay inside `frontend_dist`.

`frontend_dist` and `entry` are always used for packaged `axion://app` launches, self-tests, and bundle staging. Development launches use `[dev]` when present and reachable, but packaged assets remain the validated fallback path.

## Bundle

```toml
[bundle]
icon = "icons/app.icns"
```

- `icon`: optional project-relative icon file copied into bundle resources.

Icon paths must be relative to the manifest directory and must not contain `..`. `axion doctor`, `axion self-test`, and `axion bundle` validate that the configured icon exists, is a file, and is not a symlink. `axion doctor` also reports the active bundle target, layout, metadata summary, and detected icon extension. On macOS, the copied icon is referenced from `Info.plist` using `CFBundleIconFile`.

## Dev

```toml
[dev]
url = "http://127.0.0.1:3000"
command = "python3 -m http.server 3000 --bind 127.0.0.1 --directory frontend"
cwd = "."
timeout_ms = 15000
```

- `url`: frontend dev server URL used by `axion dev --launch`.
- `command`: optional frontend command that `axion dev --frontend-command` can override.
- `cwd`: optional command working directory, relative to the manifest directory.
- `timeout_ms`: optional wait timeout for the dev server to become reachable.

The URL must include a usable host and port. `axion dev` probes the endpoint before launch and reports one of `unconfigured`, `invalid endpoint`, `unreachable`, or `reachable`.

If `[dev]` is absent or unreachable, development planning still works, but `axion dev --launch` requires `--fallback-packaged` to launch packaged assets. With multiple windows, every window uses the same dev server entry URL in development mode; packaged fallback uses the app protocol entry from `[build]`.

CLI options take precedence over manifest development process fields: `--frontend-command`, `--frontend-cwd`, and `--dev-server-timeout-ms`.

## Native

Native preview behavior is configured under `[native]`.

```toml
[native.dialog]
backend = "headless"

[native.clipboard]
backend = "memory"

[native.lifecycle]
close_timeout_ms = 3000
```

- `backend = "headless"`: default, deterministic behavior for CI and non-GUI validation.
- `backend = "system"`: preview system file dialogs. macOS uses `osascript`; unsupported platforms report `system-unavailable` and cancel.

Dialog and clipboard backends are configured independently:

- `[native.dialog] backend = "headless" | "system"`.
- `[native.clipboard] backend = "memory" | "system"`.
- `[native.lifecycle] close_timeout_ms = 3000`: close-confirmation timeout before the preview backend applies its default allow action.

The clipboard `memory` backend is the default and stores text inside the current runtime. The `system` backend uses macOS `pbcopy` / `pbpaste`; unsupported platforms fall back to `memory` and report the effective backend in diagnostics. `axion doctor` reports both configured and effective native backends before launch.

## Capabilities

Capabilities are scoped by window id:

```toml
[capabilities.main]
profiles = ["app-info", "app-control", "multi-window", "clipboard-access", "shell-access", "file-access", "dialog-access", "app-events"]
allowed_navigation_origins = ["https://docs.example"]
allow_remote_navigation = false
```

Only declared commands, frontend events, protocols, and navigation origins are available to that window. Profiles expand during manifest loading and are merged with explicit lists. `axion doctor` reports each profile expansion and flags explicit permissions that are already supplied by a profile.

Built-in profiles:

- `minimal`: enables the `axion` bridge protocol without commands or events.
- `app-info`: enables `app.ping`, `app.info`, `app.version`, and `app.echo`.
- `app-control`: enables `app.exit` for application shutdown.
- `app-events`: enables frontend `app.log` events.
- `window-control`: enables current-window control commands such as `window.info`, `window.close`, `window.confirm_close`, `window.prevent_close`, `window.reload`, `window.focus`, `window.set_title`, and `window.set_size`.
- `multi-window`: enables multi-window coordination commands including `window.list`, `window.info`, `window.close`, `window.confirm_close`, `window.prevent_close`, `window.reload`, `window.focus`, and `window.set_title`.
- `clipboard-access`: enables `clipboard.read_text` and `clipboard.write_text` using the configured preview text clipboard backend.
- `shell-access`: enables validated `shell.open` URL launch requests through the platform opener.
- `file-access`: enables app-data filesystem lifecycle commands: `fs.create_dir`, `fs.exists`, `fs.list_dir`, `fs.read_text`, `fs.remove`, and `fs.write_text`.
- `dialog-access`: enables `dialog.open` and `dialog.save`.

Custom Rust commands use the same capability list as built-in commands. For example, a plugin command registered as `demo.greet` must appear in `commands` before frontend code can call `window.__AXION__.invoke("demo.greet", payload)`.

Avoid duplicating profile-provided permissions in explicit lists:

```toml
[capabilities.main]
profiles = ["app-info"]
commands = ["demo.greet"] # app.ping/app.info/app.version/app.echo already come from app-info
```
