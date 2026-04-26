use std::path::{Path, PathBuf};
use std::process::Command;

use axion_core::{AppConfig, Builder, RunMode};
use axion_runtime::{DiagnosticsReport, DiagnosticsWindowReport, json_string_literal};

use crate::cli::DoctorArgs;
use crate::commands::dev::dev_server_is_reachable;
use crate::error::AxionCliError;

pub fn run(args: DoctorArgs) -> Result<(), AxionCliError> {
    if args.json {
        println!("{}", doctor_report(&args.manifest_path)?.to_json());
        return Ok(());
    }

    println!("Axion doctor");
    println!("{}", framework_diagnostic_line());
    print_tool_status("cargo", &["--version"]);
    print_rustc_status();
    print_manifest_status(&args.manifest_path)?;
    print_servo_status(servo_path_for_manifest(&args.manifest_path).as_deref());
    Ok(())
}

fn framework_diagnostic_line() -> String {
    format!(
        "axion: cli_version={}, release={}, msrv={}",
        env!("CARGO_PKG_VERSION"),
        axion_runtime::AXION_RELEASE_VERSION,
        option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown"),
    )
}

fn print_tool_status(program: &str, args: &[&str]) {
    let status = tool_status(program, args);
    println!("{}: {} ({})", status.name, status.status, status.detail);
}

fn tool_status(program: &str, args: &[&str]) -> ToolDiagnostic {
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            ToolDiagnostic {
                name: program.to_owned(),
                status: "ok".to_owned(),
                detail: version.trim().to_owned(),
            }
        }
        Ok(output) => {
            let error = String::from_utf8_lossy(&output.stderr);
            ToolDiagnostic {
                name: program.to_owned(),
                status: "failed".to_owned(),
                detail: error.trim().to_owned(),
            }
        }
        Err(error) => ToolDiagnostic {
            name: program.to_owned(),
            status: "missing".to_owned(),
            detail: error.to_string(),
        },
    }
}

fn print_rustc_status() {
    let status = tool_status("rustc", &["--version"]);
    println!("rustc: {} ({})", status.status, status.detail);
    if status.status == "ok" {
        println!("{}", rustc_msrv_diagnostic_line(&status.detail));
    }
}

fn rustc_msrv_diagnostic_line(rustc_version_output: &str) -> String {
    let required = option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown");
    let Some(active) = parse_rustc_semver(rustc_version_output) else {
        return format!("rustc.msrv: unknown (active=unknown, required={required})");
    };
    let Some(required_version) = parse_semver(required) else {
        return format!(
            "rustc.msrv: unknown (active={}.{}.{}, required={required})",
            active.0, active.1, active.2
        );
    };
    let status = if active >= required_version {
        "ok"
    } else {
        "failed"
    };

    format!(
        "rustc.msrv: {status} (active={}.{}.{}, required={}.{}.{})",
        active.0, active.1, active.2, required_version.0, required_version.1, required_version.2
    )
}

fn parse_rustc_semver(output: &str) -> Option<(u64, u64, u64)> {
    let version = output.strip_prefix("rustc ")?.split_whitespace().next()?;
    parse_semver(version)
}

fn parse_semver(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts
        .next()
        .and_then(|part| part.split('-').next())
        .and_then(|part| part.parse().ok())?;

    Some((major, minor, patch))
}

fn print_manifest_status(manifest_path: &Path) -> Result<(), AxionCliError> {
    if !manifest_path.exists() {
        println!("manifest: missing ({})", manifest_path.display());
        return Ok(());
    }

    let config = axion_manifest::load_app_config_from_path(manifest_path)?;
    println!(
        "manifest: ok (app={}, windows={})",
        config.identity.name,
        config.windows.len()
    );
    for line in manifest_diagnostic_lines(&config) {
        println!("{line}");
    }
    for line in security_diagnostics(&config).to_lines() {
        println!("{line}");
    }
    for line in build_asset_diagnostic_lines(&config) {
        println!("{line}");
    }
    for line in bundle_diagnostic_lines(&config) {
        println!("{line}");
    }
    for line in runtime_diagnostic_lines(&config) {
        println!("{line}");
    }
    println!("{}", dev_server_diagnostic_line(&config));
    Ok(())
}

fn manifest_diagnostic_lines(config: &AppConfig) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(identifier) = &config.identity.identifier {
        lines.push(format!("app.identifier: {identifier}"));
    }
    if let Some(version) = &config.identity.version {
        lines.push(format!("app.version: {version}"));
    }
    if let Some(description) = &config.identity.description {
        lines.push(format!("app.description: {description}"));
    }
    if !config.identity.authors.is_empty() {
        lines.push(format!(
            "app.authors: {}",
            config.identity.authors.join(", ")
        ));
    }
    if let Some(homepage) = &config.identity.homepage {
        lines.push(format!("app.homepage: {homepage}"));
    }
    lines.push(format!(
        "native.dialog.backend: {}",
        config.native.dialog.backend.as_str()
    ));

    lines.extend(config
        .windows
        .iter()
        .flat_map(|window| {
            let window_id = window.id.as_str();
            let window_line = format!(
                "window.{window_id}: title={:?}, size={}x{}, visible={}, resizable={}",
                window.title, window.width, window.height, window.visible, window.resizable
            );
            let capability_line = match config.capabilities.get(window_id) {
                Some(capability) => {
                    let bridge_status = if capability
                        .protocols
                        .iter()
                        .any(|protocol| protocol == "axion")
                    {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    format!(
                        "capabilities.{window_id}: bridge={bridge_status}, commands={}, events={}, protocols={}, navigation_origins={}, remote_navigation={}",
                        list_or_none(&capability.commands),
                        list_or_none(&capability.events),
                        list_or_none(&capability.protocols),
                        list_or_none(&capability.allowed_navigation_origins),
                        capability.allow_remote_navigation
                    )
                }
                None => format!("capabilities.{window_id}: none (bridge=disabled)"),
            };

            [window_line, capability_line]
        })
        .collect::<Vec<_>>());
    lines
}

fn list_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_owned()
    } else {
        values.join(",")
    }
}

fn security_diagnostics(config: &AppConfig) -> SecurityDiagnostics {
    let mut windows = Vec::new();
    let mut findings = Vec::new();

    for window in &config.windows {
        let window_id = window.id.as_str();
        match config.capabilities.get(window_id) {
            Some(capability) => {
                let bridge_enabled = capability
                    .protocols
                    .iter()
                    .any(|protocol| protocol == "axion");
                windows.push(SecurityWindowDiagnostic {
                    id: window_id.to_owned(),
                    bridge_enabled,
                    risk: capability_risk_level(capability, bridge_enabled).to_owned(),
                    command_count: capability.commands.len(),
                    event_count: capability.events.len(),
                    protocol_count: capability.protocols.len(),
                    navigation_origin_count: capability.allowed_navigation_origins.len(),
                    allow_remote_navigation: capability.allow_remote_navigation,
                    command_categories: command_category_counts(&capability.commands),
                });

                if !bridge_enabled
                    && (!capability.commands.is_empty() || !capability.events.is_empty())
                {
                    findings.push(SecurityFinding::warning(
                        window_id,
                        "bridge_protocol_missing",
                        "protocols does not include axion, so configured commands/events are not reachable from frontend code",
                        Some("add protocols=[\"axion\"] only if this window needs bridge access".to_owned()),
                    ));
                }

                for protocol in capability
                    .protocols
                    .iter()
                    .filter(|protocol| protocol.as_str() != "axion")
                {
                    findings.push(SecurityFinding::warning(
                        window_id,
                        "nonstandard_protocol",
                        format!("nonstandard protocol capability '{protocol}' is configured"),
                        None,
                    ));
                }

                if capability.allow_remote_navigation {
                    findings.push(SecurityFinding::warning(
                        window_id,
                        "broad_remote_navigation",
                        "allow_remote_navigation=true permits navigation to any remote origin",
                        Some("prefer explicit allowed_navigation_origins unless this window is intentionally a browser surface".to_owned()),
                    ));
                } else if !capability.allowed_navigation_origins.is_empty() {
                    findings.push(SecurityFinding::notice(
                        window_id,
                        "limited_remote_navigation",
                        format!(
                            "remote navigation is limited to {}",
                            list_or_none(&capability.allowed_navigation_origins)
                        ),
                    ));
                }

                if capability.allow_remote_navigation
                    && !capability.allowed_navigation_origins.is_empty()
                {
                    findings.push(SecurityFinding::warning(
                        window_id,
                        "redundant_navigation_origins",
                        "allowed_navigation_origins is redundant while allow_remote_navigation=true",
                        None,
                    ));
                }
            }
            None => {
                windows.push(SecurityWindowDiagnostic {
                    id: window_id.to_owned(),
                    bridge_enabled: false,
                    risk: "low".to_owned(),
                    command_count: 0,
                    event_count: 0,
                    protocol_count: 0,
                    navigation_origin_count: 0,
                    allow_remote_navigation: false,
                    command_categories: CommandCategoryCounts::default(),
                });
                findings.push(SecurityFinding::recommendation(
                    window_id,
                    "missing_capability_section",
                    format!(
                        "add a [capabilities.{window_id}] section only for commands, events, protocols, and navigation this window actually needs"
                    ),
                ));
            }
        }
    }

    SecurityDiagnostics { windows, findings }
}

fn capability_risk_level(
    capability: &axion_core::CapabilityConfig,
    bridge_enabled: bool,
) -> &'static str {
    if capability.allow_remote_navigation {
        "high"
    } else if !bridge_enabled {
        "low"
    } else if !capability.allowed_navigation_origins.is_empty()
        || capability.commands.iter().any(|command| {
            command.starts_with("fs.")
                || command.starts_with("dialog.")
                || command == "window.close"
                || command == "window.reload"
        })
    {
        "medium"
    } else {
        "low"
    }
}

fn command_category_counts(commands: &[String]) -> CommandCategoryCounts {
    let app = commands
        .iter()
        .filter(|command| command.starts_with("app."))
        .count();
    let window = commands
        .iter()
        .filter(|command| command.starts_with("window."))
        .count();
    let fs = commands
        .iter()
        .filter(|command| command.starts_with("fs."))
        .count();
    let dialog = commands
        .iter()
        .filter(|command| command.starts_with("dialog."))
        .count();
    let custom = commands
        .iter()
        .filter(|command| {
            !command.starts_with("app.")
                && !command.starts_with("window.")
                && !command.starts_with("fs.")
                && !command.starts_with("dialog.")
        })
        .count();

    CommandCategoryCounts {
        app,
        window,
        fs,
        dialog,
        custom,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolDiagnostic {
    name: String,
    status: String,
    detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CommandCategoryCounts {
    app: usize,
    window: usize,
    fs: usize,
    dialog: usize,
    custom: usize,
}

impl CommandCategoryCounts {
    fn summary(&self) -> String {
        format!(
            "app={}, window={}, fs={}, dialog={}, custom={}",
            self.app, self.window, self.fs, self.dialog, self.custom
        )
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"app\":{},\"window\":{},\"fs\":{},\"dialog\":{},\"custom\":{}}}",
            self.app, self.window, self.fs, self.dialog, self.custom
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecurityWindowDiagnostic {
    id: String,
    bridge_enabled: bool,
    risk: String,
    command_count: usize,
    event_count: usize,
    protocol_count: usize,
    navigation_origin_count: usize,
    allow_remote_navigation: bool,
    command_categories: CommandCategoryCounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecurityFinding {
    window_id: String,
    severity: String,
    code: String,
    message: String,
    recommendation: Option<String>,
}

impl SecurityFinding {
    fn warning(
        window_id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
        recommendation: Option<String>,
    ) -> Self {
        Self {
            window_id: window_id.into(),
            severity: "warning".to_owned(),
            code: code.into(),
            message: message.into(),
            recommendation,
        }
    }

    fn notice(
        window_id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            window_id: window_id.into(),
            severity: "notice".to_owned(),
            code: code.into(),
            message: message.into(),
            recommendation: None,
        }
    }

    fn recommendation(
        window_id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            window_id: window_id.into(),
            severity: "recommendation".to_owned(),
            code: code.into(),
            message: message.into(),
            recommendation: None,
        }
    }

    fn to_json(&self) -> String {
        let recommendation = self
            .recommendation
            .as_deref()
            .map(json_string_literal)
            .unwrap_or_else(|| "null".to_owned());
        format!(
            "{{\"window_id\":{},\"severity\":{},\"code\":{},\"message\":{},\"recommendation\":{}}}",
            json_string_literal(&self.window_id),
            json_string_literal(&self.severity),
            json_string_literal(&self.code),
            json_string_literal(&self.message),
            recommendation,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecurityDiagnostics {
    windows: Vec<SecurityWindowDiagnostic>,
    findings: Vec<SecurityFinding>,
}

impl SecurityDiagnostics {
    fn warning_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == "warning")
            .count()
    }

    fn to_lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "security.summary: warnings={}",
            self.warning_count()
        )];

        for window in &self.windows {
            lines.push(format!(
                "security.window.{}: bridge={}, risk={}, commands={}, events={}, protocols={}, navigation_origins={}, remote_navigation={}",
                window.id,
                if window.bridge_enabled { "enabled" } else { "disabled" },
                window.risk,
                window.command_count,
                window.event_count,
                window.protocol_count,
                window.navigation_origin_count,
                window.allow_remote_navigation,
            ));
            lines.push(format!(
                "security.window.{}.commands: {}",
                window.id,
                window.command_categories.summary()
            ));
            for finding in self
                .findings
                .iter()
                .filter(|finding| finding.window_id == window.id)
            {
                lines.push(format!(
                    "security.{}.{}: {}",
                    finding.severity, finding.window_id, finding.message
                ));
                if let Some(recommendation) = &finding.recommendation {
                    lines.push(format!(
                        "security.recommendation.{}: {recommendation}",
                        finding.window_id
                    ));
                }
            }
        }

        lines
    }

    fn to_json(&self) -> String {
        let windows = self
            .windows
            .iter()
            .map(|window| {
                format!(
                    "{{\"id\":{},\"bridge_enabled\":{},\"risk\":{},\"command_count\":{},\"event_count\":{},\"protocol_count\":{},\"navigation_origin_count\":{},\"allow_remote_navigation\":{},\"command_categories\":{}}}",
                    json_string_literal(&window.id),
                    window.bridge_enabled,
                    json_string_literal(&window.risk),
                    window.command_count,
                    window.event_count,
                    window.protocol_count,
                    window.navigation_origin_count,
                    window.allow_remote_navigation,
                    window.command_categories.to_json(),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let findings = self
            .findings
            .iter()
            .map(SecurityFinding::to_json)
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{{\"warning_count\":{},\"windows\":[{}],\"findings\":[{}]}}",
            self.warning_count(),
            windows,
            findings
        )
    }
}

fn build_asset_diagnostic_lines(config: &AppConfig) -> Vec<String> {
    let frontend_dist = &config.build.frontend_dist;
    let entry = &config.build.entry;

    match axion_packager::validate_web_assets(frontend_dist, entry) {
        Ok(validation) => vec![
            format!("frontend_dist: ok ({})", frontend_dist.display()),
            format!(
                "entry: ok ({}; relative={})",
                entry.display(),
                validation.relative_entry.display()
            ),
        ],
        Err(error) => {
            let mut lines = vec![format!("build assets: invalid ({error})")];
            lines.push(format!("frontend_dist: {}", frontend_dist.display()));
            lines.push(format!("entry: {}", entry.display()));
            lines
        }
    }
}

fn bundle_diagnostic_lines(config: &AppConfig) -> Vec<String> {
    let target = axion_packager::current_bundle_target();
    let mut lines = vec![
        format!("bundle.target: {}", target.as_str()),
        format!("bundle.layout: {}", target.layout_summary()),
        format!(
            "bundle.metadata: app={}, version={}, identifier={}",
            config.identity.name,
            config
                .identity
                .version
                .as_deref()
                .unwrap_or("not configured"),
            config
                .identity
                .identifier
                .as_deref()
                .unwrap_or("not configured")
        ),
    ];
    match config.bundle.icon.as_deref() {
        Some(icon) => match axion_packager::validate_bundle_icon(Some(icon)) {
            Ok(_) => lines.push(format!(
                "bundle.icon: ok ({}; format={})",
                icon.display(),
                bundle_icon_format(icon)
            )),
            Err(error) => lines.push(format!("bundle.icon: invalid ({error})")),
        },
        None => lines.push("bundle.icon: not configured".to_owned()),
    }
    lines
}

fn bundle_icon_format(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn runtime_diagnostic_lines(config: &AppConfig) -> Vec<String> {
    let app = match Builder::new().apply_config(config.clone()).build() {
        Ok(app) => app,
        Err(error) => return vec![format!("runtime: invalid ({error})")],
    };
    let report = axion_runtime::diagnostic_report(&app, RunMode::Production);
    let mut lines = vec![format!(
        "runtime: app={}, mode={}, windows={}, errors={}, configured_dialog_backend={}, dialog_backend={}, resource_policy={}",
        report.app_name,
        report.mode,
        report.window_count,
        report.has_errors(),
        report.configured_dialog_backend.as_str(),
        report.dialog_backend.as_str(),
        report.resource_policy
    )];
    for window in report.windows {
        lines.push(format!(
            "runtime.window.{}: bridge={}, commands={}, events={}, frontend_events={}, host_events={}, startup_events={}, lifecycle_events={}, trusted_origins={}, navigation_origins={}, remote_navigation={}, csp={}",
            window.window_id,
            if window.bridge_enabled { "enabled" } else { "disabled" },
            window.command_count,
            window.event_count,
            list_or_none(&window.frontend_events),
            list_or_none(&window.host_events),
            window.startup_event_count,
            list_or_none(&window.lifecycle_events),
            list_or_none(&window.trusted_origins),
            list_or_none(&window.allowed_navigation_origins),
            window.allow_remote_navigation,
            window.content_security_policy,
        ));
    }
    for issue in report.issues {
        lines.push(format!(
            "runtime.issue.{:?}: {}",
            issue.severity, issue.message
        ));
    }
    lines
}

fn dev_server_diagnostic_line(config: &AppConfig) -> String {
    dev_server_diagnostic_line_with(config, dev_server_is_reachable)
}

fn dev_server_diagnostic_line_with(
    config: &AppConfig,
    is_reachable: impl Fn(&AppConfig) -> bool,
) -> String {
    let Some(dev_server) = &config.dev else {
        return "dev_server: not configured".to_owned();
    };

    if is_reachable(config) {
        format!("dev_server: reachable ({})", dev_server.url)
    } else {
        format!("dev_server: unreachable ({})", dev_server.url)
    }
}

fn print_servo_status(servo_path: Option<&Path>) {
    if let Some(servo_path) = servo_path {
        println!("servo: ok ({})", servo_path.display());
    } else {
        println!("servo: missing (searched manifest ancestors)");
    }
}

fn servo_path_for_manifest(manifest_path: &Path) -> Option<PathBuf> {
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));

    for ancestor in manifest_dir.ancestors() {
        let candidate = ancestor.join("servo");
        if candidate.join("components").join("servo").exists() {
            return Some(candidate);
        }
    }

    None
}

fn doctor_report(manifest_path: &Path) -> Result<DiagnosticsReport, AxionCliError> {
    let cargo = tool_status("cargo", &["--version"]);
    let rustc = tool_status("rustc", &["--version"]);
    let servo_path = servo_path_for_manifest(manifest_path);

    if !manifest_path.exists() {
        return Ok(DiagnosticsReport {
            source: "axion-cli doctor".to_owned(),
            exported_at_unix_seconds: Some(current_unix_timestamp_secs()),
            manifest_path: Some(manifest_path.to_path_buf()),
            app_name: "unknown".to_owned(),
            identifier: None,
            version: None,
            description: None,
            authors: Vec::new(),
            homepage: None,
            mode: None,
            window_count: 0,
            windows: Vec::new(),
            frontend_dist: None,
            entry: None,
            configured_dialog_backend: None,
            dialog_backend: None,
            icon: None,
            host_events: Vec::new(),
            staged_app_dir: None,
            asset_manifest_path: None,
            artifacts_removed: None,
            diagnostics: Some(doctor_diagnostics_json(DoctorDiagnosticsInput {
                cargo: &cargo,
                rustc: &rustc,
                rustc_msrv: "unknown",
                security: None,
                servo_path: servo_path.as_deref(),
                dev_server: None,
                runtime: None,
            })),
            result: "failed".to_owned(),
        });
    }

    let config = axion_manifest::load_app_config_from_path(manifest_path)?;
    let app = Builder::new().apply_config(config.clone()).build()?;
    let runtime = axion_runtime::diagnostic_report(&app, RunMode::Production);
    let security = security_diagnostics(&config);
    let rustc_msrv = if rustc.status == "ok" {
        rustc_msrv_diagnostic_line(&rustc.detail)
    } else {
        format!(
            "rustc.msrv: unknown (active=unknown, required={})",
            option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown")
        )
    };

    let windows = config
        .windows
        .iter()
        .map(|window| {
            let window_id = window.id.as_str();
            let capability = config.capabilities.get(window_id);
            let diagnostic = runtime
                .windows
                .iter()
                .find(|diagnostic| diagnostic.window_id == window_id);
            let configured_protocols = capability
                .map(|capability| capability.protocols.clone())
                .unwrap_or_default();

            DiagnosticsWindowReport {
                id: window_id.to_owned(),
                title: window.title.clone(),
                bridge_enabled: configured_protocols
                    .iter()
                    .any(|protocol| protocol == "axion"),
                configured_commands: capability
                    .map(|capability| capability.commands.clone())
                    .unwrap_or_default(),
                configured_events: capability
                    .map(|capability| capability.events.clone())
                    .unwrap_or_default(),
                configured_protocols,
                runtime_command_count: diagnostic
                    .map(|diagnostic| diagnostic.command_count)
                    .unwrap_or_default(),
                runtime_event_count: diagnostic
                    .map(|diagnostic| diagnostic.event_count)
                    .unwrap_or_default(),
                host_events: diagnostic
                    .map(|diagnostic| diagnostic.host_events.clone())
                    .unwrap_or_default(),
                trusted_origins: diagnostic
                    .map(|diagnostic| diagnostic.trusted_origins.clone())
                    .unwrap_or_default(),
                allowed_navigation_origins: diagnostic
                    .map(|diagnostic| diagnostic.allowed_navigation_origins.clone())
                    .unwrap_or_default(),
                allow_remote_navigation: diagnostic
                    .map(|diagnostic| diagnostic.allow_remote_navigation)
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    let host_events = runtime
        .windows
        .iter()
        .flat_map(|window| window.host_events.iter().cloned())
        .fold(Vec::new(), |mut events, event| {
            if !events.contains(&event) {
                events.push(event);
            }
            events
        });
    let dialog_backend = runtime.dialog_backend.as_str().to_owned();

    Ok(DiagnosticsReport {
        source: "axion-cli doctor".to_owned(),
        exported_at_unix_seconds: Some(current_unix_timestamp_secs()),
        manifest_path: Some(manifest_path.to_path_buf()),
        app_name: config.identity.name.clone(),
        identifier: config.identity.identifier.clone(),
        version: config.identity.version.clone(),
        description: config.identity.description.clone(),
        authors: config.identity.authors.clone(),
        homepage: config.identity.homepage.clone(),
        mode: Some("production".to_owned()),
        window_count: config.windows.len(),
        windows,
        frontend_dist: Some(config.build.frontend_dist.clone()),
        entry: Some(config.build.entry.clone()),
        configured_dialog_backend: Some(config.native.dialog.backend.as_str().to_owned()),
        dialog_backend: Some(dialog_backend),
        icon: config.bundle.icon.clone(),
        host_events,
        staged_app_dir: None,
        asset_manifest_path: None,
        artifacts_removed: None,
        diagnostics: Some(doctor_diagnostics_json(DoctorDiagnosticsInput {
            cargo: &cargo,
            rustc: &rustc,
            rustc_msrv: &rustc_msrv,
            security: Some(&security),
            servo_path: servo_path.as_deref(),
            dev_server: Some(dev_server_diagnostic_line(&config)),
            runtime: Some(&runtime),
        })),
        result: "ok".to_owned(),
    })
}

struct DoctorDiagnosticsInput<'a> {
    cargo: &'a ToolDiagnostic,
    rustc: &'a ToolDiagnostic,
    rustc_msrv: &'a str,
    security: Option<&'a SecurityDiagnostics>,
    servo_path: Option<&'a Path>,
    dev_server: Option<String>,
    runtime: Option<&'a axion_runtime::RuntimeDiagnosticReport>,
}

fn doctor_diagnostics_json(input: DoctorDiagnosticsInput<'_>) -> String {
    let security = input
        .security
        .map(SecurityDiagnostics::to_json)
        .unwrap_or_else(|| "null".to_owned());
    let servo = match input.servo_path {
        Some(path) => format!(
            "{{\"status\":\"ok\",\"path\":{}}}",
            json_string_literal(&path.display().to_string())
        ),
        None => "{\"status\":\"missing\",\"path\":null}".to_owned(),
    };
    let runtime = input
        .runtime
        .map(|runtime| {
            format!(
                "{{\"has_errors\":{},\"issue_count\":{},\"resource_policy\":{}}}",
                runtime.has_errors(),
                runtime.issues.len(),
                json_string_literal(&runtime.resource_policy)
            )
        })
        .unwrap_or_else(|| "null".to_owned());
    let dev_server = input
        .dev_server
        .as_deref()
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_owned());

    format!(
        "{{\"framework\":{{\"cli_version\":{},\"release\":{},\"msrv\":{}}},\"tools\":[{},{}],\"rustc_msrv\":{},\"security\":{},\"servo\":{},\"dev_server\":{},\"runtime\":{}}}",
        json_string_literal(env!("CARGO_PKG_VERSION")),
        json_string_literal(axion_runtime::AXION_RELEASE_VERSION),
        json_string_literal(option_env!("CARGO_PKG_RUST_VERSION").unwrap_or("unknown")),
        tool_diagnostic_json(input.cargo),
        tool_diagnostic_json(input.rustc),
        json_string_literal(input.rustc_msrv),
        security,
        servo,
        dev_server,
        runtime,
    )
}

fn tool_diagnostic_json(tool: &ToolDiagnostic) -> String {
    format!(
        "{{\"name\":{},\"status\":{},\"detail\":{}}}",
        json_string_literal(&tool.name),
        json_string_literal(&tool.status),
        json_string_literal(&tool.detail)
    )
}

fn current_unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time must be after unix epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use axion_core::{
        AppConfig, AppIdentity, BuildConfig, CapabilityConfig, DevServerConfig, WindowConfig,
        WindowId,
    };
    use url::Url;

    use super::{
        build_asset_diagnostic_lines, bundle_diagnostic_lines, dev_server_diagnostic_line,
        dev_server_diagnostic_line_with, doctor_report, framework_diagnostic_line,
        manifest_diagnostic_lines, parse_rustc_semver, parse_semver, runtime_diagnostic_lines,
        rustc_msrv_diagnostic_line, security_diagnostics, servo_path_for_manifest,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axion-doctor-test-{unique}-{serial}"))
    }

    #[test]
    fn servo_path_searches_manifest_ancestors() {
        let root = temp_dir();
        let app_dir = root.join("examples").join("hello");
        fs::create_dir_all(root.join("servo").join("components").join("servo")).unwrap();
        fs::create_dir_all(&app_dir).unwrap();

        assert_eq!(
            servo_path_for_manifest(&app_dir.join("axion.toml")),
            Some(root.join("servo"))
        );
    }

    #[test]
    fn framework_diagnostics_include_version_release_and_msrv() {
        let line = framework_diagnostic_line();

        assert!(line.contains("axion: cli_version="));
        assert!(line.contains("release=v0.1.12.0"));
        assert!(line.contains("msrv="));
    }

    #[test]
    fn rustc_msrv_diagnostics_compare_versions() {
        assert_eq!(parse_semver("1.86.0"), Some((1, 86, 0)));
        assert_eq!(
            parse_rustc_semver("rustc 1.94.0 (4a4ef493e 2026-03-02)"),
            Some((1, 94, 0))
        );
        assert_eq!(
            rustc_msrv_diagnostic_line("rustc 1.86.0 (abc 2025-01-01)"),
            "rustc.msrv: ok (active=1.86.0, required=1.86.0)"
        );
        assert_eq!(
            rustc_msrv_diagnostic_line("rustc 1.85.0 (abc 2025-01-01)"),
            "rustc.msrv: failed (active=1.85.0, required=1.86.0)"
        );
        assert_eq!(
            rustc_msrv_diagnostic_line("not rustc"),
            "rustc.msrv: unknown (active=unknown, required=1.86.0)"
        );
    }

    #[test]
    fn manifest_diagnostics_include_windows_and_bridge_status() {
        let config = AppConfig {
            identity: AppIdentity::new("doctor-test")
                .with_identifier("dev.axion.doctor")
                .with_version("1.2.3")
                .with_description("Doctor test app")
                .with_authors(["Axion Maintainers"])
                .with_homepage("https://example.dev/doctor"),
            windows: vec![
                WindowConfig::main("Main"),
                WindowConfig::new(WindowId::new("settings"), "Settings", 480, 360),
            ],
            dev: None,
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: std::collections::BTreeMap::from([(
                "main".to_owned(),
                CapabilityConfig {
                    commands: vec!["app.ping".to_owned(), "window.info".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: vec!["https://docs.example".to_owned()],
                    allow_remote_navigation: false,
                },
            )]),
        };

        let lines = manifest_diagnostic_lines(&config);

        assert!(
            lines
                .iter()
                .any(|line| line == "app.identifier: dev.axion.doctor")
        );
        assert!(lines.iter().any(|line| line == "app.version: 1.2.3"));
        assert!(
            lines
                .iter()
                .any(|line| line == "app.description: Doctor test app")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "app.authors: Axion Maintainers")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "app.homepage: https://example.dev/doctor")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "native.dialog.backend: headless")
        );
        assert!(lines.iter().any(|line| line.contains("window.main")));
        assert!(
            lines
                .iter()
                .any(|line| line == "capabilities.main: bridge=enabled, commands=app.ping,window.info, events=app.log, protocols=axion, navigation_origins=https://docs.example, remote_navigation=false")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "capabilities.settings: none (bridge=disabled)")
        );
    }

    #[test]
    fn security_diagnostics_report_window_risk_and_command_categories() {
        let config = AppConfig {
            identity: AppIdentity::new("doctor-security"),
            windows: vec![
                WindowConfig::main("Main"),
                WindowConfig::new(WindowId::new("viewer"), "Viewer", 480, 360),
                WindowConfig::new(WindowId::new("locked"), "Locked", 320, 240),
            ],
            dev: None,
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: std::collections::BTreeMap::from([
                (
                    "main".to_owned(),
                    CapabilityConfig {
                        commands: vec![
                            "app.ping".to_owned(),
                            "window.reload".to_owned(),
                            "fs.read_text".to_owned(),
                            "dialog.open".to_owned(),
                            "demo.greet".to_owned(),
                        ],
                        events: vec!["app.log".to_owned()],
                        protocols: vec!["axion".to_owned()],
                        allowed_navigation_origins: vec!["https://docs.example".to_owned()],
                        allow_remote_navigation: false,
                    },
                ),
                (
                    "viewer".to_owned(),
                    CapabilityConfig {
                        commands: vec!["app.ping".to_owned()],
                        events: vec!["app.log".to_owned()],
                        protocols: vec!["preview".to_owned()],
                        allowed_navigation_origins: vec!["https://docs.example".to_owned()],
                        allow_remote_navigation: true,
                    },
                ),
            ]),
        };

        let lines = security_diagnostics(&config).to_lines();

        assert!(
            lines
                .iter()
                .any(|line| line == "security.summary: warnings=4")
        );
        assert!(lines.iter().any(|line| {
            line == "security.window.main: bridge=enabled, risk=medium, commands=5, events=1, protocols=1, navigation_origins=1, remote_navigation=false"
        }));
        assert!(lines.iter().any(|line| {
            line == "security.window.main.commands: app=1, window=1, fs=1, dialog=1, custom=1"
        }));
        assert!(lines.iter().any(|line| {
            line == "security.notice.main: remote navigation is limited to https://docs.example"
        }));
        assert!(lines.iter().any(|line| {
            line == "security.warning.viewer: protocols does not include axion, so configured commands/events are not reachable from frontend code"
        }));
        assert!(lines.iter().any(|line| {
            line == "security.warning.viewer: allow_remote_navigation=true permits navigation to any remote origin"
        }));
        assert!(lines.iter().any(|line| {
            line == "security.warning.viewer: allowed_navigation_origins is redundant while allow_remote_navigation=true"
        }));
        assert!(lines.iter().any(|line| {
            line == "security.window.locked: bridge=disabled, risk=low, commands=0, events=0, protocols=0, navigation_origins=0, remote_navigation=false"
        }));
    }

    #[test]
    fn doctor_report_serializes_security_diagnostics_json() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(frontend.join("index.html"), "<!doctype html><html></html>").unwrap();
        let manifest = root.join("axion.toml");
        fs::write(
            &manifest,
            r#"
[app]
name = "doctor-json"
identifier = "dev.axion.doctor-json"
version = "1.2.3"

[window]
id = "main"
title = "Doctor JSON"

[build]
frontend_dist = "frontend"
entry = "frontend/index.html"

[capabilities.main]
commands = ["app.ping", "fs.read_text"]
events = ["app.log"]
protocols = ["axion"]
allowed_navigation_origins = ["https://docs.example"]
"#,
        )
        .unwrap();

        let json = doctor_report(&manifest)
            .expect("doctor report should build")
            .to_json();

        assert!(json.contains("\"schema\":\"axion.diagnostics-report.v1\""));
        assert!(json.contains("\"source\":\"axion-cli doctor\""));
        assert!(json.contains("\"app_name\":\"doctor-json\""));
        assert!(json.contains("\"security\":{\"warning_count\":0"));
        assert!(json.contains("\"risk\":\"medium\""));
        assert!(json.contains(
            "\"command_categories\":{\"app\":1,\"window\":0,\"fs\":1,\"dialog\":0,\"custom\":0}"
        ));
        assert!(json.contains("\"code\":\"limited_remote_navigation\""));
        assert!(json.contains("\"result\":\"ok\""));
    }

    #[test]
    fn build_asset_diagnostics_report_valid_paths() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        let lines = build_asset_diagnostic_lines(&config);

        assert_eq!(
            lines,
            vec![
                format!("frontend_dist: ok ({})", frontend.display()),
                format!("entry: ok ({}; relative=index.html)", entry.display())
            ]
        );
    }

    #[test]
    fn bundle_diagnostics_report_icon_status() {
        let root = temp_dir();
        let icon = root.join("icons").join("app.icns");
        fs::create_dir_all(icon.parent().unwrap()).unwrap();
        fs::write(&icon, "icon").unwrap();

        let mut config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: axion_core::BundleConfig::new().with_icon(&icon),
            native: Default::default(),
            capabilities: Default::default(),
        };

        let lines = bundle_diagnostic_lines(&config);
        assert!(lines.iter().any(|line| line.starts_with("bundle.target: ")));
        assert!(lines.iter().any(|line| line.starts_with("bundle.layout: ")));
        assert!(
            lines
                .iter()
                .any(|line| line == "bundle.metadata: app=doctor-test, version=not configured, identifier=not configured")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == &format!("bundle.icon: ok ({}; format=icns)", icon.display()))
        );

        config.bundle = axion_core::BundleConfig::new().with_icon(root.join("missing.icns"));
        assert!(
            bundle_diagnostic_lines(&config)
                .iter()
                .any(|line| line.starts_with("bundle.icon: invalid"))
        );
    }

    #[test]
    fn runtime_diagnostics_report_launch_plan() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: std::collections::BTreeMap::from([(
                "main".to_owned(),
                CapabilityConfig {
                    commands: vec!["app.ping".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                },
            )]),
        };

        let lines = runtime_diagnostic_lines(&config);

        assert!(lines.iter().any(|line| line.starts_with(
            "runtime: app=doctor-test, mode=production, windows=1, errors=false, configured_dialog_backend=headless, dialog_backend=headless, resource_policy="
        )));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("runtime.window.main: bridge=enabled"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("events=1, frontend_events=app.log, host_events=app.ready,window.created,window.ready,window.close_requested,window.closed,window.resized,window.focused,window.blurred,window.moved,window.redraw_failed"))
        );
        assert!(lines.iter().any(|line| line.contains(
            "lifecycle_events=window.created,window.ready,window.close_requested,window.closed,window.resized,window.focused,window.blurred,window.moved,window.redraw_failed"
        )));
    }

    #[test]
    fn runtime_diagnostics_report_multi_window_capabilities() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-multi-window"),
            windows: vec![
                WindowConfig::main("Main"),
                WindowConfig::new(WindowId::new("settings"), "Settings", 520, 420),
            ],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: std::collections::BTreeMap::from([
                (
                    "main".to_owned(),
                    CapabilityConfig {
                        commands: vec!["app.ping".to_owned(), "app.info".to_owned()],
                        events: vec!["app.log".to_owned()],
                        protocols: vec!["axion".to_owned()],
                        allowed_navigation_origins: Vec::new(),
                        allow_remote_navigation: false,
                    },
                ),
                (
                    "settings".to_owned(),
                    CapabilityConfig {
                        commands: vec![
                            "window.info".to_owned(),
                            "window.focus".to_owned(),
                            "window.set_title".to_owned(),
                        ],
                        events: vec!["app.log".to_owned()],
                        protocols: vec!["axion".to_owned()],
                        allowed_navigation_origins: vec!["https://docs.example".to_owned()],
                        allow_remote_navigation: false,
                    },
                ),
            ]),
        };

        let lines = runtime_diagnostic_lines(&config);

        assert!(lines.iter().any(|line| line.starts_with(
            "runtime: app=doctor-multi-window, mode=production, windows=2, errors=false, configured_dialog_backend=headless, dialog_backend=headless"
        )));
        assert!(lines.iter().any(|line| {
            line.starts_with("runtime.window.main: bridge=enabled, commands=2")
                && line.contains("frontend_events=app.log")
        }));
        assert!(lines.iter().any(|line| {
            line.starts_with("runtime.window.settings: bridge=enabled, commands=3")
                && line.contains("navigation_origins=https://docs.example")
        }));
    }

    #[test]
    fn build_asset_diagnostics_report_missing_and_outside_entry() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = root.join("index.html");

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        let lines = build_asset_diagnostic_lines(&config);

        assert_eq!(
            lines,
            vec![
                format!(
                    "build assets: invalid (frontend_dist '{}' must exist and be a directory)",
                    frontend.display()
                ),
                format!("frontend_dist: {}", frontend.display()),
                format!("entry: {}", entry.display())
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn build_asset_diagnostics_report_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        let external = root.join("external.txt");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();
        fs::write(&external, "external").unwrap();
        symlink(&external, frontend.join("external.txt")).unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        let lines = build_asset_diagnostic_lines(&config);

        assert!(
            lines
                .first()
                .is_some_and(|line| line.starts_with("build assets: invalid"))
        );
        assert!(lines[0].contains("must not contain symlinks"));
    }

    #[test]
    fn build_asset_diagnostics_report_reserved_asset_manifest_path() {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();
        fs::write(frontend.join("axion-assets.json"), "{}").unwrap();

        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new(&frontend, &entry),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        let lines = build_asset_diagnostic_lines(&config);

        assert!(
            lines
                .first()
                .is_some_and(|line| line.starts_with("build assets: invalid"))
        );
        assert!(lines[0].contains("reserved generated asset path"));
    }

    #[test]
    fn dev_server_diagnostics_report_unconfigured() {
        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: None,
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        assert_eq!(
            dev_server_diagnostic_line(&config),
            "dev_server: not configured"
        );
    }

    #[test]
    fn dev_server_diagnostics_report_reachable() {
        let config = AppConfig {
            identity: AppIdentity::new("doctor-test"),
            windows: vec![WindowConfig::main("Main")],
            dev: Some(DevServerConfig {
                url: Url::parse("http://127.0.0.1:3000").unwrap(),
                command: None,
                cwd: None,
                timeout_ms: None,
            }),
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        };

        assert_eq!(
            dev_server_diagnostic_line_with(&config, |_| true),
            "dev_server: reachable (http://127.0.0.1:3000/)"
        );
        assert_eq!(
            dev_server_diagnostic_line_with(&config, |_| false),
            "dev_server: unreachable (http://127.0.0.1:3000/)"
        );
    }
}
