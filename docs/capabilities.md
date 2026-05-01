# Capabilities

Axion bridge access is deny-by-default. A frontend can call only the commands, events, protocols, and navigation origins listed for its active window in `axion.toml`.

## Minimal Example

```toml
[capabilities.main]
profiles = ["app-info", "app-events"]
commands = ["demo.greet"]
events = ["demo.ready"]
protocols = ["axion"]
allowed_navigation_origins = []
allow_remote_navigation = false
```

Prefer profiles for common groups and explicit entries for app-specific commands. Run `axion check --json` or `axion doctor` to inspect the effective capability surface after profile expansion.

## Built-In Profiles

| Profile | Commands | Events | Protocols |
| --- | --- | --- | --- |
| `minimal` | none | none | `axion` |
| `app-info` | `app.echo`, `app.info`, `app.ping`, `app.version` | none | `axion` |
| `app-control` | `app.exit` | none | `axion` |
| `app-events` | none | `app.log` | `axion` |
| `window-control` | `window.close`, `window.confirm_close`, `window.focus`, `window.hide`, `window.info`, `window.prevent_close`, `window.reload`, `window.set_size`, `window.set_title`, `window.show` | none | `axion` |
| `multi-window` | `window.close`, `window.confirm_close`, `window.focus`, `window.info`, `window.list`, `window.prevent_close`, `window.reload`, `window.set_title` | none | `axion` |
| `clipboard-access` | `clipboard.read_text`, `clipboard.write_text` | none | `axion` |
| `shell-access` | `shell.open` | none | `axion` |
| `file-access` | `fs.create_dir`, `fs.exists`, `fs.list_dir`, `fs.read_text`, `fs.remove`, `fs.write_text` | none | `axion` |
| `dialog-access` | `dialog.open`, `dialog.save` | none | `axion` |

## Risk Guidance

- Keep `fs.*`, `clipboard.*`, `shell.*`, and `dialog.*` on packaged app windows. Do not expose them to windows that can navigate to remote content.
- Treat `app.exit`, `window.close`, and `window.reload` as runtime-control capabilities. Pair `window.close` with `window.confirm_close` and `window.prevent_close`.
- Prefer `allowed_navigation_origins` over `allow_remote_navigation = true`.
- Avoid duplicating explicit commands already provided by a profile; `doctor` reports these as notices.

## Diagnostics

Human output:

```sh
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- check --manifest-path examples/hello-axion/axion.toml --json
```

`doctor` prints `security.window.<id>.profile.<profile>` lines. `check --json` includes `capabilities.windows[].profile_expansions`, explicit entries, effective entries, navigation settings, bridge status, and risk.
