use std::path::Path;

use axion_core::{Builder, RunMode};
#[cfg(feature = "servo-runtime")]
use axion_runtime::json_string_literal;

#[cfg(feature = "servo-runtime")]
struct GreetingPlugin;

#[cfg(feature = "servo-runtime")]
impl axion_runtime::RuntimePlugin for GreetingPlugin {
    fn register(&self, builder: &mut axion_runtime::RuntimeBridgeBindingsBuilder) {
        builder.register_command("demo.greet", |context, request| {
            Ok(format!(
                "{{\"appName\":{},\"windowId\":{},\"payload\":{}}}",
                json_string_literal(&context.app_name),
                json_string_literal(&context.window.id),
                request.payload,
            ))
        });
        builder.push_startup_event(axion_runtime::RuntimeBridgeEvent::new(
            "demo.ready",
            "{\"source\":\"greeting-plugin\"}",
        ));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("axion.toml");
    let config = axion_manifest::load_app_config_for_executable(&manifest_path)?;
    let crash_report_dir = axion_runtime::app_data_dir_for_config(&config)?.join("crash-reports");
    let app = Builder::new().apply_config(config).build()?;
    axion_runtime::install_panic_reporter(axion_runtime::PanicReportConfig {
        app_name: app.config().identity.name.clone(),
        output_dir: crash_report_dir,
    });

    if std::env::args().skip(1).any(|arg| arg == "--plan") {
        println!("Hello Axion smoke app");
        println!("{plan}", plan = app.runtime_plan(RunMode::Production));
        return Ok(());
    }

    #[cfg(not(feature = "servo-runtime"))]
    {
        Err(std::io::Error::other(
            "Servo runtime is disabled for this example; rebuild with `--features servo-runtime` or run with `--plan`",
        )
        .into())
    }

    #[cfg(feature = "servo-runtime")]
    {
        let greeting_plugin = GreetingPlugin;
        let plugins: [&dyn axion_runtime::RuntimePlugin; 1] = [&greeting_plugin];
        axion_runtime::run_with_plugins(app, RunMode::Production, &plugins)?;
        Ok(())
    }
}
