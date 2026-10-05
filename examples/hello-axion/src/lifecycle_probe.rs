use std::io::{BufRead, Write};
use std::time::Instant;

use axion_core::{AppConfig, Builder, RunMode, WindowId};
use axion_runtime::{RuntimePlugin, json_string_literal};

const SCHEMA: &str = "axion.lifecycle-probe.v1";

fn emit(scenario: &str, started: Instant, phase: &str, detail: &str) {
    println!(
        "{{\"schema\":{},\"source\":\"host\",\"scenario\":{},\"phase\":{},\"elapsed_ms\":{:.3},\"detail\":{detail}}}",
        json_string_literal(SCHEMA),
        json_string_literal(scenario),
        json_string_literal(phase),
        started.elapsed().as_secs_f64() * 1000.0,
    );
    let _ = std::io::stdout().flush();
}

struct ProbePlugin {
    scenario: String,
    started: Instant,
}

impl RuntimePlugin for ProbePlugin {
    fn register(&self, builder: &mut axion_runtime::RuntimeBridgeBindingsBuilder) {
        let scenario = self.scenario.clone();
        let started = self.started;
        builder.register_command("probe.record", move |context, request| {
            emit(
                &scenario,
                started,
                "frontend.record",
                &format!(
                    "{{\"window_id\":{},\"record\":{}}}",
                    json_string_literal(&context.window.id),
                    request.payload
                ),
            );
            Ok("{}".to_owned())
        });
    }
}

pub(super) fn run(
    mut config: AppConfig,
    greeting_plugin: &dyn RuntimePlugin,
) -> Result<(), Box<dyn std::error::Error>> {
    let scenario = std::env::var("AXION_LIFECYCLE_PROBE")?;
    if !matches!(
        scenario.as_str(),
        "normal-close" | "partial-window-fail" | "resource-fail" | "render-fail"
    ) {
        return Err(std::io::Error::other("unknown lifecycle probe scenario").into());
    }
    let started = Instant::now();
    let windows = if scenario == "partial-window-fail" {
        2
    } else {
        std::env::var("AXION_LIFECYCLE_PROBE_WINDOWS")
            .ok()
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(1)
    };
    if !matches!(windows, 1 | 2) {
        return Err(std::io::Error::other("lifecycle probe supports one or two windows").into());
    }
    let data_dir = std::env::var_os("AXION_LIFECYCLE_PROBE_DATA_DIR")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| {
            std::io::Error::other("lifecycle probe requires an isolated data directory")
        })?;
    if !data_dir.is_absolute() {
        return Err(
            std::io::Error::other("lifecycle probe data directory must be absolute").into(),
        );
    }
    config.native = config.native.with_app_data_dir(data_dir);
    config.windows.truncate(1);
    let primary = config
        .windows
        .first()
        .cloned()
        .ok_or_else(|| std::io::Error::other("lifecycle probe requires a window"))?;
    if windows == 2 {
        let mut secondary = primary.clone();
        secondary.id = WindowId::new("probe-secondary");
        secondary.title = "Axion Lifecycle Probe Secondary".to_owned();
        let capability = config
            .capabilities
            .get(primary.id.as_str())
            .cloned()
            .ok_or_else(|| {
                std::io::Error::other("lifecycle probe requires primary capabilities")
            })?;
        config
            .capabilities
            .insert(secondary.id.as_str().to_owned(), capability);
        config.windows.push(secondary);
    }
    for capability in config.capabilities.values_mut() {
        for commands in [&mut capability.explicit_commands, &mut capability.commands] {
            commands.push("probe.record".to_owned());
            commands.sort();
            commands.dedup();
        }
    }
    emit(
        &scenario,
        started,
        "host.started",
        &format!("{{\"windows\":{windows}}}"),
    );
    let app = Builder::new().apply_config(config).build()?;
    let probe_plugin = ProbePlugin {
        scenario: scenario.clone(),
        started,
    };
    let plugins: [&dyn RuntimePlugin; 2] = [greeting_plugin, &probe_plugin];
    let result = axion_runtime::run_with_plugins(app, RunMode::Production, &plugins);
    let result_ok = result.is_ok();
    let error = result
        .as_ref()
        .err()
        .map(|error| json_string_literal(&error.to_string()))
        .unwrap_or_else(|| "null".to_owned());
    emit(
        &scenario,
        started,
        "backend.returned",
        &format!("{{\"result_ok\":{result_ok},\"error\":{error}}}"),
    );
    // Each process calls the backend once. Keep this same host alive for external probes.
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim() == "QUIT" {
            break;
        }
        let sequence = line
            .strip_prefix("PING ")
            .ok_or_else(|| std::io::Error::other("expected PING <sequence> or QUIT"))?
            .trim()
            .parse::<u64>()?;
        emit(
            &scenario,
            started,
            "heartbeat",
            &format!("{{\"sequence\":{sequence}}}"),
        );
    }
    emit(&scenario, started, "host.done", "{}");
    let expected_error = matches!(scenario.as_str(), "partial-window-fail" | "render-fail");
    if result_ok == expected_error {
        return Err(std::io::Error::other("lifecycle probe backend result was unexpected").into());
    }
    Ok(())
}
