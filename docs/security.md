# Security Model

Axion is deny-by-default. Windows only receive the commands, events, protocols, and navigation permissions declared in `axion.toml`.

## Capability Scope

Capabilities are scoped to a window id:

```toml
[capabilities.main]
commands = ["app.ping", "window.info", "fs.read_text", "fs.write_text"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = []
allow_remote_navigation = false
```

If a command or event is not declared for the active window, the bridge does not expose it as an allowed operation.

## Bridge Commands

Frontend code invokes Rust-side commands through the injected bridge:

```js
const response = await window.__AXION__.invoke("app.ping", { from: "frontend" });
```

Bridge payloads must be valid JSON values. Request ids, command names, event names, and payload sizes are validated before dispatch.

File commands are restricted to Axion's app-data directory. Absolute paths, `..` components, root components, and symlink targets are rejected.

## Frontend Events

Frontend-originated events require explicit capability entries:

```js
await window.__AXION__.emit("app.log", { message: "ready" });
```

Host-dispatched events are separate from frontend events and are protected by the runtime bridge token.

## Navigation

`axion://app` is the trusted packaged app origin. Remote navigation is blocked unless configured with `allowed_navigation_origins` or `allow_remote_navigation`.

## Content Security Policy

Axion derives a strict CSP for packaged app content. The default policy restricts script, style, image, font, and connection sources to the app origin plus explicitly trusted origins.
