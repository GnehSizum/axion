use std::path::Path;

use axion_core::{Builder, RunMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("axion.toml");
    let config = axion_manifest::load_app_config_from_path(&manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
    axion_runtime::install_panic_reporter(axion_runtime::PanicReportConfig {
        app_name: app.config().identity.name.clone(),
        output_dir: Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("axion")
            .join("crash-reports"),
    });

    if std::env::args().skip(1).any(|arg| arg == "--plan") {
        println!("Multi Window smoke app");
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
        axion_runtime::run(app, RunMode::Production)?;
        Ok(())
    }
}
