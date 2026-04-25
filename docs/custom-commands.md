# Custom Commands

Axion applications can expose Rust functions to frontend JavaScript by registering bridge commands through a runtime plugin. Commands are still deny-by-default: the active window must list the command in `axion.toml` before the frontend can invoke it.

## Register a Command

Create a plugin in your app entrypoint and register commands when `servo-runtime` is enabled:

```rust
#[cfg(feature = "servo-runtime")]
struct DemoPlugin;

#[cfg(feature = "servo-runtime")]
impl axion_runtime::RuntimePlugin for DemoPlugin {
    fn register(&self, builder: &mut axion_runtime::RuntimeBridgeBindingsBuilder) {
        builder.register_command("demo.greet", |context, request| {
            Ok(format!(
                "{{\"message\":{},\"appName\":{},\"payload\":{}}}",
                axion_runtime::json_string_literal("Hello from Rust"),
                axion_runtime::json_string_literal(&context.app_name),
                request.payload,
            ))
        });
    }
}
```

Command handlers receive a `CommandContext` and `BridgeRequest`. The request payload is JSON text supplied by the frontend. The response must also be valid JSON text.

## Enable the Command

List the command in the active window capability:

```toml
[capabilities.main]
commands = ["app.ping", "window.info", "demo.greet"]
events = ["app.log"]
protocols = ["axion"]
```

If `demo.greet` is not listed, the command is filtered out before the bridge is exposed to the frontend.

## Run With Plugins

Pass the plugin to the runtime launch path:

```rust
#[cfg(feature = "servo-runtime")]
{
    let demo_plugin = DemoPlugin;
    let plugins: [&dyn axion_runtime::RuntimePlugin; 1] = [&demo_plugin];
    axion_runtime::run_with_plugins(app, RunMode::Production, &plugins)?;
}
```

## Invoke From JavaScript

```js
const greeting = await window.__AXION__.invoke("demo.greet", {
  from: "frontend",
});
```

The frontend should treat denied commands as expected errors during capability checks:

```js
const denied = await window.__AXION__.invoke("demo.missing", null)
  .catch((error) => error instanceof Error ? error.message : String(error));
```

## Startup Events

Plugins can publish host startup events that frontend code can listen to:

```rust
builder.push_startup_event(axion_runtime::RuntimeBridgeEvent::new(
    "demo.ready",
    "{\"source\":\"demo-plugin\"}",
));
```

```js
window.__AXION__.listen("demo.ready", (payload) => {
  console.log("plugin ready", payload);
});
```

## Validation

Use the non-GUI gates before launching a Servo window:

```sh
cargo run -p axion-cli -- doctor --manifest-path examples/hello-axion/axion.toml
cargo run -p axion-cli -- self-test --manifest-path examples/hello-axion/axion.toml
```

These CLI checks validate the manifest and Axion framework path. They do not dynamically load custom commands from an application binary. Use `cargo run -- --plan` in generated applications to verify manifest loading without opening a window, and use `cargo check --features servo-runtime` to compile custom command plugins.
