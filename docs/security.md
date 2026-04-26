# Security Model

Axion is deny-by-default. Windows only receive the commands, events, protocols, and navigation permissions declared in `axion.toml`.

Use the smallest capability set that can support the page:

```toml
[capabilities.main]
commands = ["app.info", "app.version"]
events = []
protocols = ["axion"]
allowed_navigation_origins = []
allow_remote_navigation = false
```

For a display-only window, omit the section entirely or keep every list empty. Add `protocols = ["axion"]` only when frontend JavaScript needs the Axion bridge.

## Capability Scope

Capabilities are scoped to a window id:

```toml
[capabilities.main]
commands = ["app.ping", "window.list", "window.info", "window.set_title", "fs.read_text", "fs.write_text", "dialog.open"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = []
allow_remote_navigation = false
```

If a command or event is not declared for the active window, the bridge does not expose it as an allowed operation.
Window commands default to the current window, but payloads may include `target` to operate on another runtime window id. Only grant those commands to windows that are allowed to coordinate other windows.

Higher-risk command groups should stay local to trusted packaged UI:

- `fs.*`: restricted to app-data paths, but still reads or writes user-visible data.
- `dialog.*`: opens native file dialogs and can expose selected paths to the app.
- `window.close` and `window.reload`: affect runtime control flow and user state.

## Bridge Commands

Frontend code invokes Rust-side commands through the injected bridge:

```js
const response = await window.__AXION__.invoke("app.ping", { from: "frontend" });
```

Bridge payloads must be valid JSON values. Request ids, command names, event names, and payload sizes are validated before dispatch.

File commands are restricted to Axion's app-data directory. Absolute paths, `..` components, root components, and symlink targets are rejected.

Dialog commands are also capability-gated. Keep `[native.dialog] backend = "headless"` for CI and non-interactive environments; use `system` only when interactive native dialogs are expected.

## Frontend Events

Frontend-originated events require explicit capability entries:

```js
await window.__AXION__.emit("app.log", { message: "ready" });
```

Host-dispatched events are separate from frontend events and are protected by the runtime bridge token.

## Navigation

`axion://app` is the trusted packaged app origin. Remote navigation is blocked unless configured with `allowed_navigation_origins` or `allow_remote_navigation`.

Prefer origin allowlists:

```toml
[capabilities.docs]
protocols = ["axion"]
allowed_navigation_origins = ["https://docs.example"]
allow_remote_navigation = false
```

Set `allow_remote_navigation = true` only for a window that intentionally behaves like a browser surface. When it is true, `allowed_navigation_origins` is redundant because all remote origins are allowed.

## Content Security Policy

Axion derives a strict CSP for packaged app content. The default policy restricts script, style, image, font, and connection sources to the app origin plus explicitly trusted origins.

## Doctor Diagnostics

`axion doctor` prints security diagnostics for each manifest window:

```sh
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml --json
```

Key lines:

- `security.summary: warnings=N`: total capability warnings.
- `security.window.<id>`: bridge status, risk level, command count, event count, protocol count, navigation allowlist count, and remote-navigation flag.
- `security.window.<id>.commands`: command categories: `app`, `window`, `fs`, `dialog`, and `custom`.
- `security.notice.<id>`: non-failing notes such as restricted remote navigation.
- `security.warning.<id>`: configuration that weakens or contradicts the deny-by-default model.
- `security.recommendation.<id>`: suggested tightening step.

Common warnings:

- Commands or events are configured but `protocols` does not include `axion`.
- A nonstandard bridge protocol is declared.
- `allow_remote_navigation = true` allows every remote origin.
- `allowed_navigation_origins` is set while `allow_remote_navigation = true`.

CI can fail on newly introduced warnings with a simple grep:

```sh
cargo run -p axion-cli -- doctor --manifest-path axion.toml > target/axion-doctor.txt
grep -q "security.summary: warnings=0" target/axion-doctor.txt
```

For tooling, prefer `--json` and read `diagnostics.security.warning_count`, `diagnostics.security.windows`, and `diagnostics.security.findings` from the `axion.diagnostics-report.v1` output.
