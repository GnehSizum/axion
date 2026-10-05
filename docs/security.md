# Security Model

Axion is deny-by-default. Windows only receive the commands, events, protocols, and navigation permissions declared in `axion.toml`.

Use the smallest capability set that can support the page:

```toml
[capabilities.main]
profiles = ["app-info"]
allowed_navigation_origins = []
allow_remote_navigation = false
```

For a display-only window, omit the section entirely or keep every list empty. Add `protocols = ["axion"]` only when frontend JavaScript needs the Axion bridge.

## Capability Scope

Capabilities are scoped to a window id:

```toml
[capabilities.main]
profiles = ["app-info", "app-control", "multi-window", "clipboard-access", "shell-access", "file-access", "dialog-access", "app-events"]
allowed_navigation_origins = []
allow_remote_navigation = false
```

If a command or event is not declared for the active window, the bridge does not expose it as an allowed operation.
Window commands default to the current window, but payloads may include `target` to operate on another runtime window id. Only grant those commands to windows that are allowed to coordinate other windows.

Higher-risk command groups should stay local to trusted packaged UI:

- `clipboard.*`: reads or writes clipboard text through the configured preview backend.
- `fs.*`: restricted to app-data paths, but still reads or writes user-visible data.
- `dialog.*`: opens native file dialogs and can expose selected paths to the app.
- `shell.*`: opens validated URLs in external apps or browsers.
- `app.exit`, `window.close`, and `window.reload`: affect runtime control flow and user state.

## Capability Profiles

Profiles reduce repetitive manifest entries but do not bypass the deny-by-default model. Axion expands profiles into explicit commands, events, and protocols while loading `axion.toml`.

- `minimal`: bridge protocol only.
- `app-info`: app metadata, version, ping, and echo commands.
- `app-control`: application shutdown command.
- `app-events`: frontend `app.log` events.
- `window-control`: window control commands (current window by default; explicit `target` may address another window), including close confirmation.
- `multi-window`: multi-window coordination commands, including targeted close confirmation.
- `clipboard-access`: clipboard read/write commands.
- `shell-access`: validated URL opening through the platform opener.
- `file-access`: app-data file lifecycle commands for create, exists, list, read, remove, and write.
- `dialog-access`: native open/save dialog commands.

Prefer profiles for common groups and explicit `commands` only for custom or unusual permissions:

```toml
[capabilities.main]
profiles = ["app-info", "window-control", "app-events"]
commands = ["demo.greet"]
```

`axion doctor` explains the effective expansion, for example:

```text
security.window.main.profile.app-info: commands=app.echo,app.info,app.ping,app.version, events=none, protocols=axion
```

If a manifest also lists a permission already supplied by a profile, `doctor` emits a non-failing notice such as `redundant_profile_command`, `redundant_profile_event`, or `redundant_profile_protocol`.

## Bridge Commands

Frontend code invokes Rust-side commands through the injected bridge:

```js
const response = await window.__AXION__.invoke("app.ping", { from: "frontend" });
```

Built-in `window.*` and `app.exit` requests share a five-second total deadline, including the bounded control queue. A `window.control-timeout` response means the caller stopped waiting; side effects that already started may still complete. Expired queued requests do not start.

Bridge payloads must be valid JSON values, with at most 64 KiB of UTF-8 data and 64 nested array/object containers. Request ids, command names, event names, and payload sizes are validated before dispatch.

The bridge uses a per-window bearer token, additionally bound to Servo's native `target_webview_id`. An invoke or emit request must carry the exact token registered for its originating WebView; a missing or unknown native context, an invalid token, or another window's token is rejected. The initiating window's capabilities authorize the command, including an explicitly supplied `target` window. Cross-window control does not change the caller's identity.

`Origin` and `Referer` are request metadata, not authentication gates. This preserves custom-scheme requests with opaque or `null` origins. Bootstrap installation is restricted to the packaged app origin and the configured trusted development origin; permission to navigate to a remote origin does not grant bridge installation. Tokens live for the WebView's lifetime and are removed from the native binding when it closes; they are not rotated on navigation. A token leaked to content in the same WebView remains a bearer credential, so keep native capabilities on trusted UI and do not expose tokens in logs or frontend messages.

File commands are restricted to Axion's app-data directory. Absolute paths, `..` components, root components, and symlink targets are rejected. Directory removal requires `recursive: true` for non-empty directories.

This app-data sandbox is a framework-level path sandbox, not an operating-system permission sandbox. Do not expose `file-access`, `clipboard-access`, `shell-access`, or `dialog-access` to untrusted remote content. `axion doctor` emits `remote_origin_native_capability` when a window can navigate to remote origins while native data capabilities are enabled.

Dialog, clipboard, and shell commands are also capability-gated. Keep `[native.dialog] backend = "headless"` and `[native.clipboard] backend = "memory"` for CI and non-interactive environments. Use shell-opening only for trusted packaged UI, because opener calls can hand control to external apps or browsers.

Servo DOM text editing uses its own clipboard delegate. With Servo's `clipboard` Cargo feature enabled, native Copy/Cut/Paste accesses the operating system clipboard without dispatching Axion `clipboard.*` bridge commands. `[native.clipboard] backend = "memory"` controls only those bridge commands; Servo's in-process fallback and Axion's memory store are separate. Enabling this Cargo feature does not itself enable the asynchronous `navigator.clipboard` API.

Close confirmation is intentionally timeout-bound. Keep `[native.lifecycle] close_timeout_ms` long enough for trusted UI prompts, but do not rely on it as a security boundary; it is a lifecycle safety net for unsaved-state flows. Close deadlines run on the native event loop, and confirmation or prevention cancels the pending deadline. Repeated `app.exit` calls while an exit is pending return the same request id. Any prevention ends that exit request and cancels its remaining close requests.

Built-in control calls measure their five-second deadline from the future's first poll, including control-pool queue time and the native response. Calls from the event-loop thread fail immediately rather than waiting on themselves; requests that expire before worker or event-loop execution do not start. A timeout does not undo an operation that already began.

If a window can call `app.exit`, at least one trusted packaged window should also be able to call both `window.confirm_close` and `window.prevent_close`. Otherwise application exit can request window closes but frontend code has no authorized close-decision path for guarded shutdown flows.

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
profiles = ["minimal"]
allowed_navigation_origins = ["https://docs.example"]
allow_remote_navigation = false
```

Set `allow_remote_navigation = true` only for a window that intentionally behaves like a browser surface. When it is true, `allowed_navigation_origins` is redundant because all remote origins are allowed.

## Content Security Policy

Axion derives CSP separately for each manifest window and selects it using Servo's native `target_webview_id` on packaged resource requests. The WebView binding is registered before its initial fetch can observe the policy map. Unknown or absent native contexts fail resource loading; request headers, bearer tokens, and window ordering cannot select a different window's CSP.

The policy restricts scripts and styles to the app origin, images to self and data URLs, and fonts to self. Connection sources include the app origin, configured trusted origins, and that window's explicit navigation-origin allowlist. Different windows may therefore have different connection policies while sharing the same frontend files. Navigation permission alone does not install the bridge in the destination page.

## Doctor Diagnostics

`axion doctor` prints security diagnostics for each manifest window:

```sh
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml --json
```

Key lines:

- `security.summary: warnings=N`: total capability warnings.
- `security.window.<id>`: bridge status, risk level, command count, event count, protocol count, navigation allowlist count, and remote-navigation flag.
- `security.window.<id>.profiles`: declared capability profiles, or `none`.
- `security.window.<id>.profile.<profile>`: commands, events, and protocols supplied by that profile.
- `security.window.<id>.commands`: command categories: `app`, `window`, `fs`, `clipboard`, `dialog`, `shell`, and `custom`.
- `security.notice.<id>`: non-failing notes such as restricted remote navigation.
- `security.warning.<id>`: configuration that weakens or contradicts the deny-by-default model.
- `security.recommendation.<id>`: suggested tightening step.

Common warnings:

- Commands or events are configured but `protocols` does not include `axion`.
- A nonstandard bridge protocol is declared.
- `allow_remote_navigation = true` allows every remote origin.
- `allowed_navigation_origins` is set while `allow_remote_navigation = true`.
- File, clipboard, or dialog capabilities are enabled on a window that can navigate to remote origins.

CI can fail on newly introduced warnings with a simple grep:

```sh
cargo run -p axion-cli -- doctor --manifest-path axion.toml > target/axion-doctor.txt
grep -q "security.summary: warnings=0" target/axion-doctor.txt
```

For stricter CI, use built-in gates:

```sh
cargo run -p axion-cli -- doctor \
  --manifest-path axion.toml \
  --deny-warnings \
  --max-risk medium
```

For tooling, prefer `--json` and read `diagnostics.security.warning_count`, `diagnostics.security.windows[].profile_expansions`, `diagnostics.security.findings`, `diagnostics.gate`, and `diagnostics.readiness` from the `axion.diagnostics-report.v1` output.
