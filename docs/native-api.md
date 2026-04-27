# Native API

Axion exposes built-in native APIs as bridge commands. Every command must be listed in the active window's capability section before frontend code can call it.

For application-defined Rust commands, see `custom-commands.md`.

## Bridge Compatibility Helpers

The injected `window.__AXION__` bootstrap also exposes small frontend compatibility helpers under `window.__AXION__.compat`.

### `installTextInputSelectionPatch`

Installs a targeted text-selection workaround for Servo-backed `input` and `textarea` controls. This is useful when a page needs more stable caret placement or drag-selection behavior during development.

```js
const dispose = window.__AXION__.compat.installTextInputSelectionPatch(
  document.getElementById("file-contents"),
  {
    manualPointerSelection: true,
    onStatus(message) {
      console.log(message);
    },
    onUpdate(snapshot) {
      console.log(snapshot.detail);
    }
  }
);
```

Options:

- `manualPointerSelection: boolean` — fully manages pointer drag selection for the target control
- `onStatus(message)` — receives short status strings when the helper corrects selection
- `onUpdate(snapshot)` — receives selection and metric snapshots after corrections

## Bridge Diagnostics Helpers

The bootstrap also exposes lightweight diagnostics under `window.__AXION__.diagnostics`.

### `describeBridge`

Returns a frontend-friendly snapshot of the injected bridge surface.

```js
const bridgeSnapshot = window.__AXION__.diagnostics.describeBridge();
```

### `snapshotTextControl`

Returns a structured snapshot for an `input` or `textarea`, including selection, scrolling, active-element state, and custom detail payload.

```js
const textarea = document.getElementById("file-contents");
const snapshot = window.__AXION__.diagnostics.snapshotTextControl(textarea, {
  source: "manual-check"
});
```

### `toPrettyJson`

Formats a value using the same pretty JSON layout used by the examples.

```js
pre.textContent = window.__AXION__.diagnostics.toPrettyJson(snapshot);
```

## App Commands

### `app.ping`

Checks bridge connectivity.

```js
await window.__AXION__.invoke("app.ping", { from: "frontend" });
```

### `app.info`

Returns app name, identifier, optional app metadata, and run mode.

```js
await window.__AXION__.invoke("app.info", null);
// {
//   appName: "hello-axion",
//   identifier: "dev.axion.hello",
//   version: "0.1.0",
//   description: "Hello Axion example",
//   authors: ["Axion Maintainers"],
//   homepage: "https://example.dev/hello-axion",
//   mode: "production"
// }
```

### `app.version`

Returns the Axion runtime Cargo version and public release version used by the app.

```js
await window.__AXION__.invoke("app.version", null);
// { version: "0.1.15", release: "v0.1.15.0", framework: "axion" }
```

### `app.echo`

Returns the request payload with request metadata. This is useful for bridge smoke tests and examples.

```js
await window.__AXION__.invoke("app.echo", { value: 1 });
```

## Window Commands

Most window commands operate on the current window by default. Pass `{ target: "settings" }` to address another window by id.

### `window.list`

Returns every native window currently managed by the runtime.

```js
await window.__AXION__.invoke("window.list", null);
// { windows: [{ id: "main", ... }, { id: "settings", ... }] }
```

### `window.info`

Returns the target window id, title, size, and native flags.

```js
await window.__AXION__.invoke("window.info", null);
await window.__AXION__.invoke("window.info", { target: "settings" });
```

### `window.show`

Shows the target native window and returns the updated window state.

```js
await window.__AXION__.invoke("window.show", null);
```

### `window.hide`

Hides the target native window and returns the updated window state.

```js
await window.__AXION__.invoke("window.hide", null);
```

### `window.focus`

Requests focus for the target native window and returns the updated window state.

```js
await window.__AXION__.invoke("window.focus", null);
await window.__AXION__.invoke("window.focus", { target: "settings" });
```

### `window.set_title`

Updates the target native window title and returns the updated window state.

```js
await window.__AXION__.invoke("window.set_title", {
  title: "Hello Axion · Preview",
});
await window.__AXION__.invoke("window.set_title", {
  target: "settings",
  title: "Settings · Controlled",
});
```

### `window.set_size`

Updates the target native window size and returns the updated window state.

```js
await window.__AXION__.invoke("window.set_size", {
  width: 960,
  height: 720,
});
```

### `window.reload`

Requests a reload of the target WebView and returns the current window state. This is the same control path used by `axion dev --watch --reload` when a live window is running.

```js
await window.__AXION__.invoke("window.reload", null);
await window.__AXION__.invoke("window.reload", { target: "settings" });
```

### Host Lifecycle Events

Axion host events are listen-only and come from the native runtime. Window lifecycle events currently include:

- `window.created`
- `window.ready`
- `window.close_requested`
- `window.closed`
- `window.resized`
- `window.focused`
- `window.blurred`
- `window.moved`
- `window.redraw_failed`

```js
window.__AXION__.listen("window.focused", (payload) => {
  console.log("focused", payload);
});
```

## File Commands

File commands operate only inside Axion's app-data directory:

```text
<app root>/target/axion-data/<app name>/
```

They reject absolute paths, parent-directory traversal, root components, and symlinks.

### `fs.write_text`

Writes UTF-8 text.

```js
await window.__AXION__.invoke("fs.write_text", {
  path: "notes/hello.txt",
  contents: "Hello from Axion",
});
```

### `fs.read_text`

Reads UTF-8 text.

```js
await window.__AXION__.invoke("fs.read_text", {
  path: "notes/hello.txt",
});
```

## Dialog Commands

`dialog.open` and `dialog.save` are registered capability-gated preview commands. The dialog backend is configured in `[native.dialog]`:

```toml
[native.dialog]
backend = "headless" # or "system"
```

- `headless`: default, CI-safe behavior. Always returns `{ canceled: true, path: null, paths: null, backend: "headless" }`.
- `system`: preview native backend. On macOS it uses the platform file dialog through `osascript`; on unsupported platforms it returns `{ canceled: true, path: null, paths: null, backend: "system-unavailable" }`.

Supported request fields:

- `title: string`
- `defaultPath: string`
- `directory: boolean` for `dialog.open`
- `multiple: boolean` for `dialog.open`
- `filters: [{ name: string, extensions: string[] }]`

`dialog.save` rejects `directory=true` and `multiple=true`. `filters` are validated for shape today and reserved for richer native backends.

```js
await window.__AXION__.invoke("dialog.open", {
  title: "Select input files",
  multiple: true,
  filters: [
    { name: "Text", extensions: ["txt", "md"] },
    { name: "Images", extensions: ["png", "jpg"] }
  ]
});

await window.__AXION__.invoke("dialog.save", {
  title: "Choose an export path",
  defaultPath: "notes/export.txt",
});
```

Response shape:

```js
// {
//   canceled: false,
//   path: "/tmp/example.txt",
//   paths: ["/tmp/example.txt", "/tmp/example-2.txt"], // only for multi-select
//   backend: "system"
// }
```

## Capability Example

```toml
[capabilities.main]
profiles = ["app-info", "multi-window", "file-access", "dialog-access", "app-events"]
```
