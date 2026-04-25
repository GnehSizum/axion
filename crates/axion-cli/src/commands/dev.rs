use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use axion_core::{AppConfig, Builder, RunMode};

use crate::cli::DevArgs;
use crate::error::AxionCliError;

pub fn run(args: DevArgs) -> Result<(), AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;

    if args.launch {
        let launch_mode = select_launch_mode(app.config());
        if launch_mode == RunMode::Production {
            println!(
                "Axion dev launch fallback: dev server is unavailable, launching packaged app instead."
            );
        }

        axion_runtime::run(app, launch_mode)?;
        return Ok(());
    }

    let plan = app.runtime_plan(RunMode::Development);

    println!("Axion development plan");
    println!("manifest: {}", args.manifest_path.display());
    println!("{plan}");

    Ok(())
}

fn select_launch_mode(config: &AppConfig) -> RunMode {
    if dev_server_is_reachable(config) {
        RunMode::Development
    } else {
        RunMode::Production
    }
}

pub(crate) fn dev_server_is_reachable(config: &AppConfig) -> bool {
    let Some(dev_server) = &config.dev else {
        return false;
    };

    let Some(host) = dev_server.url.host_str() else {
        return false;
    };
    let Some(port) = dev_server.url.port_or_known_default() else {
        return false;
    };

    let Ok(addresses) = (host, port).to_socket_addrs() else {
        return false;
    };

    let timeout = Duration::from_millis(300);
    addresses
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, timeout).is_ok())
}

#[cfg(test)]
mod tests {
    use axion_core::{AppConfig, AppIdentity, BuildConfig, RunMode, WindowConfig};
    use url::Url;

    use super::select_launch_mode;

    fn config_with_dev_url(dev_url: Option<&str>) -> AppConfig {
        AppConfig {
            identity: AppIdentity::new("axion-cli-test"),
            windows: vec![WindowConfig::main("CLI Test")],
            dev: dev_url.map(|value| axion_core::DevServerConfig {
                url: Url::parse(value).expect("test URL must parse"),
            }),
            build: BuildConfig::new("frontend", "frontend/index.html"),
            capabilities: Default::default(),
        }
    }

    #[test]
    fn launch_mode_is_production_without_dev_server() {
        assert_eq!(
            select_launch_mode(&config_with_dev_url(None)),
            RunMode::Production
        );
    }
}
