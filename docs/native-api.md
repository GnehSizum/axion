# Native API

Axion exposes built-in native APIs as bridge commands. Every command must be listed in the active window's capability section before frontend code can call it.

For application-defined Rust commands, see `custom-commands.md`.

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
// { version: "0.1.4", release: "v0.1.4.0", framework: "axion" }
```

### `app.echo`

Returns the request payload with request metadata. This is useful for bridge smoke tests and examples.

```js
await window.__AXION__.invoke("app.echo", { value: 1 });
```

## Window Commands

### `window.info`

Returns the current window id, title, size, and native flags.

```js
await window.__AXION__.invoke("window.info", null);
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
commands = [
  "app.ping",
  "app.info",
  "app.version",
  "app.echo",
  "window.info",
  "fs.write_text",
  "fs.read_text",
  "dialog.open",
  "dialog.save",
]
events = ["app.log"]
protocols = ["axion"]
```
