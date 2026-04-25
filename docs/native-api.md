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
// { version: "0.1.3", release: "v0.1.3.0", framework: "axion" }
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

`dialog.open` and `dialog.save` are registered capability-gated preview commands. In v0.1.3.0 they are headless-safe stubs and return `{ canceled: true, path: null }` until a native dialog backend is added.

```js
await window.__AXION__.invoke("dialog.open", null);
await window.__AXION__.invoke("dialog.save", null);
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
]
events = ["app.log"]
protocols = ["axion"]
```
