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
// { version: "0.1.21", release: "v0.1.21.0", framework: "axion" }
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

### `window.close`

Requests the target native window to close. The command returns a pending close request. Closing one window does not exit the application while other windows remain open.

```js
await window.__AXION__.invoke("window.close", null);
await window.__AXION__.invoke("window.close", { target: "settings" });
// { pending: true, requestId: "axion-close-1", window: { id: "settings", ... } }
```

### `window.confirm_close`

Accepts a pending close request and lets the runtime remove the window.

```js
await window.__AXION__.invoke("window.confirm_close", {
  requestId: payload.requestId,
});
```

### `window.prevent_close`

Rejects a pending close request. Use this for unsaved-change prompts or other guarded flows.

```js
await window.__AXION__.invoke("window.prevent_close", {
  requestId: payload.requestId,
});
```

## App Lifecycle Commands

### `app.exit`

Requests application shutdown by asking all runtime windows to close. If windows do not answer, the preview backend defaults to allowing close after the reported timeout from `[native.lifecycle] close_timeout_ms`.

```js
await window.__AXION__.invoke("app.exit", null);
// { pending: true, requestId: "axion-exit-1", windowCount: 3, requestCount: 3 }
```

### Host Lifecycle Events

Axion host events are listen-only and come from the native runtime. Application lifecycle events currently include:

- `app.exit_requested`
- `app.exit_prevented`
- `app.exit_completed`

Window lifecycle events currently include:

- `window.created`
- `window.ready`
- `window.close_requested`
- `window.close_prevented`
- `window.close_completed`
- `window.close_timed_out`
- `window.closed`
- `window.resized`
- `window.focused`
- `window.blurred`
- `window.moved`
- `window.redraw_failed`

`app.exit_requested` is emitted to all runtime windows before `app.exit` starts per-window close requests. Its payload includes `requestId`, `reason`, `windowCount`, `defaultAction`, and `timeoutMs`.

`app.exit_prevented` is emitted when any window rejects a close request that belongs to the app exit request. The backend cancels the remaining pending close requests for that app exit attempt. `app.exit_completed` is emitted when all close requests associated with an app exit request complete. Both outcome events include `requestId`, `status`, `windowCount`, `requestCount`, `closedCount`, `preventedCount`, `timedOutCount`, `closeRequests`, `closedWindows`, `preventedWindows`, `timedOutWindows`, `closedRequests`, `preventedRequests`, and `timedOutRequests`. Request arrays contain `{ requestId, windowId }` entries so frontends can correlate app-level and window-level lifecycle results.

`window.close_requested` is emitted before a window is removed and includes `requestId`, `reason`, `defaultAction`, and `timeoutMs`. Frontend code can call `window.confirm_close` or `window.prevent_close` with that `requestId`. `window.close_prevented` is emitted after a prevent decision. `window.close_completed` is emitted after an explicit confirm decision. If no decision arrives before `timeoutMs`, the preview backend emits `window.close_timed_out` and applies `defaultAction = "allow"`. The timeout defaults to `3000` and can be configured with `[native.lifecycle] close_timeout_ms`. `window.closed` is emitted after the close has been accepted.

Close decision commands reject unknown, duplicate, or already timed-out `requestId` values. Treat these failures as terminal for that close request and wait for the next `window.close_requested` event before retrying.

```js
window.__AXION__.listen("window.focused", (payload) => {
  console.log("focused", payload);
});
```

## Clipboard Commands

Clipboard commands are capability-gated text commands. The default backend is runtime-local `memory` for deterministic tests. Apps can opt into the preview macOS system clipboard backend:

```toml
[native.clipboard]
backend = "system"
```

`system` uses `pbcopy` / `pbpaste` on macOS. Unsupported platforms or command failures fall back to `memory`, and command responses include the effective `backend`.

### `clipboard.write_text`

Stores UTF-8 text in the configured clipboard backend.

```js
await window.__AXION__.invoke("clipboard.write_text", {
  text: "Hello from Axion",
});
// { bytes: 16, backend: "memory" }
```

### `clipboard.read_text`

Reads the current clipboard text from the configured backend.

```js
await window.__AXION__.invoke("clipboard.read_text", null);
// { text: "Hello from Axion", backend: "memory" }
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
profiles = ["app-info", "app-control", "multi-window", "clipboard-access", "file-access", "dialog-access", "app-events"]
```
