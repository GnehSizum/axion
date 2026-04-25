use axion_bridge::{
    BridgeBindings, BridgeBindingsBuilder, BridgeBindingsPlugin, BridgeEvent, BridgeRunMode,
    CommandContext, WindowCommandContext,
};
use axion_core::{App, RunMode, RuntimeLaunchConfig, WindowLaunchConfig};
use axion_protocol::AppAssetResolver;
use axion_security::SecurityPolicy;
use thiserror::Error;

pub use axion_bridge::BridgeBindingsBuilder as RuntimeBridgeBindingsBuilder;
pub use axion_bridge::{
    BridgeEmitRequest, BridgeEvent as RuntimeBridgeEvent, BridgeRequest, CommandRegistryError,
};

pub trait RuntimePlugin: Send + Sync {
    fn register(&self, builder: &mut RuntimeBridgeBindingsBuilder);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDiagnosticIssue {
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowDiagnostic {
    pub window_id: String,
    pub title: String,
    pub bridge_enabled: bool,
    pub command_count: usize,
    pub event_count: usize,
    pub frontend_events: Vec<String>,
    pub host_events: Vec<String>,
    pub startup_event_count: usize,
    pub lifecycle_events: Vec<String>,
    pub trusted_origins: Vec<String>,
    pub allowed_navigation_origins: Vec<String>,
    pub allow_remote_navigation: bool,
    pub content_security_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDiagnosticReport {
    pub app_name: String,
    pub mode: RunMode,
    pub target: Option<RuntimeLaunchTarget>,
    pub frontend_dist: std::path::PathBuf,
    pub resource_policy: String,
    pub window_count: usize,
    pub windows: Vec<WindowDiagnostic>,
    pub issues: Vec<RuntimeDiagnosticIssue>,
}

impl RuntimeDiagnosticReport {
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| matches!(issue.severity, DiagnosticSeverity::Error))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowLifecycleEventKind {
    Created,
    CloseRequested,
    Closed,
    Resized,
    RedrawFailed,
}

impl WindowLifecycleEventKind {
    pub const fn event_name(self) -> &'static str {
        match self {
            Self::Created => "window.created",
            Self::CloseRequested => "window.close_requested",
            Self::Closed => "window.closed",
            Self::Resized => "window.resized",
            Self::RedrawFailed => "window.redraw_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowLifecycleEvent {
    pub window_id: String,
    pub kind: WindowLifecycleEventKind,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub message: Option<String>,
}

pub fn window_lifecycle_event_names() -> Vec<String> {
    [
        WindowLifecycleEventKind::Created,
        WindowLifecycleEventKind::CloseRequested,
        WindowLifecycleEventKind::Closed,
        WindowLifecycleEventKind::Resized,
        WindowLifecycleEventKind::RedrawFailed,
    ]
    .into_iter()
    .map(|kind| kind.event_name().to_owned())
    .collect()
}

fn host_event_names(startup_events: &[BridgeEvent]) -> Vec<String> {
    let mut events = Vec::new();
    for event in startup_events {
        if !events.contains(&event.name) {
            events.push(event.name.clone());
        }
    }
    for event in window_lifecycle_event_names() {
        if !events.contains(&event) {
            events.push(event);
        }
    }
    events
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanicReportConfig {
    pub app_name: String,
    pub output_dir: std::path::PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanicReport {
    pub path: std::path::PathBuf,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLaunchTarget {
    DevServer(url::Url),
    AppProtocol(AppProtocolLaunch),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppProtocolLaunch {
    pub initial_url: url::Url,
    pub resolver: AppAssetResolver,
}

#[derive(Debug, Clone)]
pub struct RuntimeLaunchRequest {
    pub app_name: String,
    pub window_bindings: Vec<RuntimeWindowBinding>,
    pub identifier: Option<String>,
    pub app_protocol: AppProtocolLaunch,
    pub mode: RunMode,
    pub target: RuntimeLaunchTarget,
    pub frontend_dist: std::path::PathBuf,
    pub windows: Vec<axion_core::WindowLaunchConfig>,
}

#[derive(Debug, Clone)]
pub struct RuntimeWindowBinding {
    pub window_id: String,
    pub bridge_token: String,
    pub command_context: CommandContext,
    pub bridge_bindings: BridgeBindings,
    pub security_policy: SecurityPolicy,
}

#[derive(Debug, Default)]
pub struct RuntimeHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBackendKind {
    Winit,
}

pub trait RuntimeBackend {
    fn kind(&self) -> RuntimeBackendKind;
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("the Servo desktop runtime is disabled; rebuild with `--features servo-runtime`")]
    ServoRuntimeDisabled,
    #[error(transparent)]
    Protocol(#[from] axion_protocol::ProtocolError),
    #[cfg(feature = "servo-runtime")]
    #[error(transparent)]
    Winit(#[from] axion_window_winit::WinitRunError),
}

pub fn launch_config(app: &App, mode: RunMode) -> RuntimeLaunchConfig {
    app.runtime_launch_config(mode)
}

pub fn launch_request(app: &App, mode: RunMode) -> Result<RuntimeLaunchRequest, RuntimeError> {
    launch_request_with_plugins(app, mode, &[])
}

pub fn diagnostic_report(app: &App, mode: RunMode) -> RuntimeDiagnosticReport {
    match launch_request(app, mode) {
        Ok(request) => diagnostic_report_from_launch_request(request),
        Err(error) => RuntimeDiagnosticReport {
            app_name: app.config().identity.name.clone(),
            mode,
            target: None,
            frontend_dist: app.config().build.frontend_dist.clone(),
            resource_policy: axion_protocol::default_resource_policy_summary().to_owned(),
            window_count: app.config().windows.len(),
            windows: Vec::new(),
            issues: vec![RuntimeDiagnosticIssue {
                severity: DiagnosticSeverity::Error,
                message: error.to_string(),
            }],
        },
    }
}

fn diagnostic_report_from_launch_request(request: RuntimeLaunchRequest) -> RuntimeDiagnosticReport {
    let target = request.target.clone();
    let windows = request
        .window_bindings
        .iter()
        .map(|binding| {
            let bridge_enabled = binding.security_policy.allows_protocol("axion");
            WindowDiagnostic {
                window_id: binding.window_id.clone(),
                title: binding.command_context.window.title.clone(),
                bridge_enabled,
                command_count: binding
                    .bridge_bindings
                    .command_registry
                    .command_names()
                    .len(),
                event_count: binding.bridge_bindings.event_registry.event_names().len(),
                frontend_events: binding.bridge_bindings.event_registry.event_names(),
                host_events: if bridge_enabled {
                    host_event_names(&binding.bridge_bindings.startup_events)
                } else {
                    Vec::new()
                },
                startup_event_count: binding.bridge_bindings.startup_events.len(),
                lifecycle_events: window_lifecycle_event_names(),
                trusted_origins: binding.security_policy.trusted_origins(),
                allowed_navigation_origins: binding
                    .security_policy
                    .capabilities()
                    .navigation_origin_names(),
                allow_remote_navigation: binding
                    .security_policy
                    .capabilities()
                    .allow_remote_navigation,
                content_security_policy: binding.security_policy.content_security_policy(),
            }
        })
        .collect::<Vec<_>>();
    let mut issues = Vec::new();
    if windows.is_empty() {
        issues.push(RuntimeDiagnosticIssue {
            severity: DiagnosticSeverity::Error,
            message: "runtime launch has no windows".to_owned(),
        });
    }
    for window in &windows {
        if !window.bridge_enabled {
            issues.push(RuntimeDiagnosticIssue {
                severity: DiagnosticSeverity::Warning,
                message: format!("window '{}' has bridge disabled", window.window_id),
            });
        }
        if window.allow_remote_navigation {
            issues.push(RuntimeDiagnosticIssue {
                severity: DiagnosticSeverity::Warning,
                message: format!(
                    "window '{}' allows unrestricted remote navigation",
                    window.window_id
                ),
            });
        }
    }

    RuntimeDiagnosticReport {
        app_name: request.app_name,
        mode: request.mode,
        target: Some(target),
        frontend_dist: request.frontend_dist,
        resource_policy: axion_protocol::default_resource_policy_summary().to_owned(),
        window_count: request.windows.len(),
        windows,
        issues,
    }
}

pub fn install_panic_reporter(config: PanicReportConfig) {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        if let Ok(report) = write_panic_report(&config, panic_info) {
            eprintln!("Axion crash report written: {}", report.path.display());
        }
        previous_hook(panic_info);
    }));
}

fn write_panic_report(
    config: &PanicReportConfig,
    panic_info: &std::panic::PanicHookInfo<'_>,
) -> std::io::Result<PanicReport> {
    std::fs::create_dir_all(&config.output_dir)?;
    let path = panic_report_path(config, current_unix_timestamp_secs());
    let body = format_panic_report(config, panic_info);
    std::fs::write(&path, &body)?;
    Ok(PanicReport { path, body })
}

fn panic_report_path(config: &PanicReportConfig, timestamp_secs: u64) -> std::path::PathBuf {
    let app_name = config
        .app_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    config
        .output_dir
        .join(format!("axion-crash-{app_name}-{timestamp_secs}.log"))
}

fn current_unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn format_panic_report(
    config: &PanicReportConfig,
    panic_info: &std::panic::PanicHookInfo<'_>,
) -> String {
    let payload = panic_info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| {
            panic_info
                .payload()
                .downcast_ref::<String>()
                .map(String::as_str)
        })
        .unwrap_or("<non-string panic payload>");
    let location = panic_info
        .location()
        .map(|location| {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("unnamed");

    format_panic_report_body(&config.app_name, thread_name, &location, payload)
}

fn format_panic_report_body(
    app_name: &str,
    thread_name: &str,
    location: &str,
    payload: &str,
) -> String {
    format!(
        "Axion crash report\napp={app_name}\nthread={thread_name}\nlocation={location}\npanic={payload}\n"
    )
}

pub fn launch_request_with_plugins(
    app: &App,
    mode: RunMode,
    plugins: &[&dyn RuntimePlugin],
) -> Result<RuntimeLaunchRequest, RuntimeError> {
    let launch_config = launch_config(app, mode);
    let app_protocol_resolver = AppAssetResolver::new(
        launch_config.frontend_dist.clone(),
        launch_config.packaged_entry.clone(),
    )?;
    let app_protocol = AppProtocolLaunch {
        initial_url: app_protocol_resolver.initial_url(),
        resolver: app_protocol_resolver,
    };
    let target = match launch_config.entrypoint.clone() {
        axion_core::LaunchEntrypoint::DevServer(url) => RuntimeLaunchTarget::DevServer(url),
        axion_core::LaunchEntrypoint::Packaged(_) => {
            app_protocol
                .resolver
                .resolve_existing_request_path(app_protocol.resolver.default_document())?;
            RuntimeLaunchTarget::AppProtocol(app_protocol.clone())
        }
    };
    let window_bindings = launch_config
        .windows
        .iter()
        .map(|window| {
            let command_context = build_command_context(&launch_config, window);
            let security_policy = build_security_policy(app, &target, &app_protocol, &window.id);
            RuntimeWindowBinding {
                window_id: window.id.clone(),
                bridge_token: uuid::Uuid::new_v4().to_string(),
                bridge_bindings: build_bridge_bindings(&security_policy, &command_context, plugins),
                security_policy,
                command_context,
            }
        })
        .collect();

    Ok(RuntimeLaunchRequest {
        app_name: launch_config.app_name,
        identifier: launch_config.identifier,
        window_bindings,
        app_protocol,
        mode: launch_config.mode,
        target,
        frontend_dist: launch_config.frontend_dist,
        windows: launch_config.windows,
    })
}

fn build_command_context(
    launch_config: &RuntimeLaunchConfig,
    window: &WindowLaunchConfig,
) -> CommandContext {
    CommandContext {
        app_name: launch_config.app_name.clone(),
        identifier: launch_config.identifier.clone(),
        mode: match launch_config.mode {
            RunMode::Development => BridgeRunMode::Development,
            RunMode::Production => BridgeRunMode::Production,
        },
        window: WindowCommandContext {
            id: window.id.clone(),
            title: window.title.clone(),
            width: window.width,
            height: window.height,
            resizable: window.resizable,
            visible: window.visible,
        },
    }
}

fn build_bridge_bindings(
    security_policy: &SecurityPolicy,
    command_context: &CommandContext,
    plugins: &[&dyn RuntimePlugin],
) -> BridgeBindings {
    let allowed_commands = if security_policy.allows_protocol("axion") {
        security_policy.capabilities().command_names()
    } else {
        Vec::new()
    };
    let allowed_events = if security_policy.allows_protocol("axion") {
        security_policy.capabilities().event_names()
    } else {
        Vec::new()
    };
    let mut builder = BridgeBindingsBuilder::new(command_context.clone());
    let plugin = BuiltinBridgePlugin {
        allowed_commands: allowed_commands.clone(),
        allowed_events: allowed_events.clone(),
    };
    builder.apply_plugin(&plugin);
    for plugin in plugins {
        plugin.register(&mut builder);
    }

    let mut bindings = builder.finish();
    bindings.retain_commands(allowed_commands);
    bindings.retain_events(allowed_events);
    bindings
}

struct BuiltinBridgePlugin {
    allowed_commands: Vec<String>,
    allowed_events: Vec<String>,
}

impl BridgeBindingsPlugin for BuiltinBridgePlugin {
    fn register(&self, builder: &mut BridgeBindingsBuilder) {
        let command_context = builder.command_context().clone();

        register_builtin_commands(builder, &self.allowed_commands);
        register_builtin_events(builder, &self.allowed_events);

        builder.push_startup_event(BridgeEvent::new(
            "app.ready",
            format!(
                "{{\"appName\":{},\"identifier\":{},\"mode\":{},\"windowId\":{},\"protocol\":\"axion\"}}",
                json_string_literal(&command_context.app_name),
                optional_json_string_literal(command_context.identifier.as_deref()),
                json_string_literal(match command_context.mode {
                    BridgeRunMode::Development => "development",
                    BridgeRunMode::Production => "production",
                }),
                json_string_literal(&command_context.window.id),
            ),
        ));
        builder.push_startup_event(BridgeEvent::new(
            WindowLifecycleEventKind::Created.event_name(),
            format!(
                "{{\"windowId\":{},\"title\":{},\"width\":{},\"height\":{},\"resizable\":{},\"visible\":{}}}",
                json_string_literal(&command_context.window.id),
                json_string_literal(&command_context.window.title),
                command_context.window.width,
                command_context.window.height,
                command_context.window.resizable,
                command_context.window.visible,
            ),
        ));
    }
}

fn register_builtin_commands(builder: &mut BridgeBindingsBuilder, allowed_commands: &[String]) {
    if allowed_commands.iter().any(|command| command == "app.ping") {
        builder.register_command("app.ping", |context, request| {
            Ok(format!(
                "{{\"appName\":{},\"message\":\"pong\",\"payload\":{}}}",
                json_string_literal(&context.app_name),
                request.payload,
            ))
        });
    }

    if allowed_commands.iter().any(|command| command == "app.info") {
        builder.register_command("app.info", |context, _request| {
            Ok(format!(
                "{{\"appName\":{},\"identifier\":{},\"mode\":{}}}",
                json_string_literal(&context.app_name),
                optional_json_string_literal(context.identifier.as_deref()),
                json_string_literal(match context.mode {
                    BridgeRunMode::Development => "development",
                    BridgeRunMode::Production => "production",
                }),
            ))
        });
    }

    if allowed_commands.iter().any(|command| command == "app.echo") {
        builder.register_command_async("app.echo", |context, request| async move {
            Ok(format!(
                "{{\"appName\":{},\"requestId\":{},\"metadata\":{},\"payload\":{}}}",
                json_string_literal(&context.app_name),
                optional_json_string_literal(Some(&request.id)),
                json_string_map_literal(&request.metadata),
                request.payload,
            ))
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.info")
    {
        builder.register_command("window.info", |context, _request| {
            Ok(format!(
                "{{\"id\":{},\"title\":{},\"width\":{},\"height\":{},\"resizable\":{},\"visible\":{}}}",
                json_string_literal(&context.window.id),
                json_string_literal(&context.window.title),
                context.window.width,
                context.window.height,
                context.window.resizable,
                context.window.visible,
            ))
        });
    }
}

fn register_builtin_events(builder: &mut BridgeBindingsBuilder, allowed_events: &[String]) {
    if allowed_events.iter().any(|event| event == "app.log") {
        builder.register_event("app.log", |_context, _request| Ok(()));
    }
}

fn build_security_policy(
    app: &App,
    target: &RuntimeLaunchTarget,
    app_protocol: &AppProtocolLaunch,
    window_id: &str,
) -> SecurityPolicy {
    let app_origin = SecurityPolicy::origin_string(&app_protocol.initial_url);
    let mut origins = vec![app_origin.clone()];
    if let RuntimeLaunchTarget::DevServer(url) = target {
        origins.push(SecurityPolicy::origin_string(url));
    }
    SecurityPolicy::from_capabilities(
        app.config().capabilities.get(window_id).into_iter(),
        app_origin,
        origins,
    )
}

pub fn run(app: App, mode: RunMode) -> Result<(), RuntimeError> {
    run_with_plugins(app, mode, &[])
}

pub fn run_with_plugins(
    app: App,
    mode: RunMode,
    plugins: &[&dyn RuntimePlugin],
) -> Result<(), RuntimeError> {
    let launch_request = launch_request_with_plugins(&app, mode, plugins)?;

    #[cfg(not(feature = "servo-runtime"))]
    {
        let _ = (app, launch_request);
        Err(RuntimeError::ServoRuntimeDisabled)
    }

    #[cfg(feature = "servo-runtime")]
    {
        let _ = app;
        let window_bindings = launch_request
            .window_bindings
            .into_iter()
            .map(|binding| axion_window_winit::WindowBridgeBinding {
                window_id: binding.window_id,
                bridge_token: binding.bridge_token,
                command_context: binding.command_context,
                bridge_bindings: binding.bridge_bindings,
                security_policy: binding.security_policy,
            })
            .collect();

        match launch_request.target {
            RuntimeLaunchTarget::DevServer(url) => axion_window_winit::run_dev_server(
                launch_request.app_name,
                launch_request.identifier,
                launch_request.mode,
                launch_request.app_protocol.resolver,
                launch_request.windows,
                window_bindings,
                url,
            )
            .map_err(RuntimeError::from),
            RuntimeLaunchTarget::AppProtocol(app_protocol) => axion_window_winit::run_app_protocol(
                launch_request.app_name,
                launch_request.identifier,
                launch_request.mode,
                launch_request.windows,
                window_bindings,
                app_protocol.initial_url,
                app_protocol.resolver,
            )
            .map_err(RuntimeError::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use axion_core::{BuildConfig, Builder, CapabilityConfig, DevServerConfig, WindowConfig};
    use axion_protocol::ProtocolError;
    use url::Url;

    use super::{
        AppProtocolLaunch, DiagnosticSeverity, PanicReportConfig, RuntimeBridgeBindingsBuilder,
        RuntimeError, RuntimeLaunchTarget, RuntimePlugin, diagnostic_report,
        format_panic_report_body, launch_request, launch_request_with_plugins, panic_report_path,
    };

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axion-runtime-{name}-{unique}-{serial}"))
    }

    fn frontend_fixture(name: &str) -> (PathBuf, PathBuf) {
        let frontend = temp_dir(name);
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).expect("test frontend directory should be created");
        fs::write(&entry, "<html>Axion</html>").expect("test frontend entry should be written");
        (frontend, entry)
    }

    fn app_with_build(dev: Option<&str>) -> axion_core::App {
        let (frontend_dist, entry) = frontend_fixture("single-window");
        let mut builder = Builder::new()
            .with_name("axion-runtime-test")
            .with_window(WindowConfig::main("Runtime Test"))
            .with_build(BuildConfig::new(frontend_dist, entry));

        if let Some(value) = dev {
            builder = builder.with_dev_server(DevServerConfig {
                url: Url::parse(value).expect("test URL must parse"),
            });
        }

        builder = builder.with_capability(
            "main",
            CapabilityConfig {
                commands: vec!["app.ping".to_owned()],
                events: vec!["app.log".to_owned()],
                protocols: vec!["axion".to_owned()],
                allowed_navigation_origins: Vec::new(),
                allow_remote_navigation: false,
            },
        );

        builder.build().expect("test app should build")
    }

    fn multi_window_app() -> axion_core::App {
        let (frontend_dist, entry) = frontend_fixture("multi-window");
        Builder::new()
            .with_name("axion-runtime-test")
            .with_window(WindowConfig::main("Runtime Test"))
            .with_window(WindowConfig::new(
                axion_core::WindowId::new("settings"),
                "Settings",
                480,
                360,
            ))
            .with_build(BuildConfig::new(frontend_dist, entry))
            .with_capability(
                "main",
                CapabilityConfig {
                    commands: vec!["app.ping".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                },
            )
            .with_capability(
                "settings",
                CapabilityConfig {
                    commands: vec!["window.info".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: vec!["https://docs.example".to_owned()],
                    allow_remote_navigation: true,
                },
            )
            .build()
            .expect("test app should build")
    }

    fn app_with_commands_without_axion_protocol() -> axion_core::App {
        let (frontend_dist, entry) = frontend_fixture("commands-without-axion");
        Builder::new()
            .with_name("axion-runtime-test")
            .with_window(WindowConfig::main("Runtime Test"))
            .with_build(BuildConfig::new(frontend_dist, entry))
            .with_capability(
                "main",
                CapabilityConfig {
                    commands: vec!["app.ping".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: Vec::new(),
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                },
            )
            .build()
            .expect("test app should build")
    }

    fn app_with_plugin_command(command: &str) -> axion_core::App {
        let (frontend_dist, entry) = frontend_fixture("plugin-command");
        Builder::new()
            .with_name("axion-runtime-test")
            .with_window(WindowConfig::main("Runtime Test"))
            .with_build(BuildConfig::new(frontend_dist, entry))
            .with_capability(
                "main",
                CapabilityConfig {
                    commands: vec!["app.ping".to_owned(), command.to_owned()],
                    events: vec!["app.log".to_owned(), "plugin.event".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                },
            )
            .build()
            .expect("test app should build")
    }

    fn app_with_missing_entry(dev: Option<&str>) -> axion_core::App {
        let frontend_dist = temp_dir("missing-entry");
        fs::create_dir_all(&frontend_dist).expect("test frontend directory should be created");
        let entry = frontend_dist.join("index.html");
        let mut builder = Builder::new()
            .with_name("axion-runtime-test")
            .with_window(WindowConfig::main("Runtime Test"))
            .with_build(BuildConfig::new(frontend_dist, entry));

        if let Some(value) = dev {
            builder = builder.with_dev_server(DevServerConfig {
                url: Url::parse(value).expect("test URL must parse"),
            });
        }

        builder.build().expect("test app should build")
    }

    #[test]
    fn launch_request_uses_dev_server_when_available() {
        let request = launch_request(
            &app_with_build(Some("http://127.0.0.1:3000")),
            axion_core::RunMode::Development,
        )
        .expect("launch request should build");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");

        assert!(matches!(request.target, RuntimeLaunchTarget::DevServer(_)));
        assert!(!binding.bridge_token.is_empty());
        assert_eq!(binding.window_id, "main");
        assert_eq!(binding.bridge_bindings.startup_events.len(), 2);
        assert!(
            binding
                .bridge_bindings
                .startup_events
                .iter()
                .any(|event| event.name == "window.created")
        );
        assert_eq!(
            binding.bridge_bindings.event_registry.event_names(),
            vec!["app.log".to_owned()]
        );
        assert_eq!(
            binding.bridge_bindings.command_registry.command_names(),
            vec!["app.ping".to_owned()]
        );
        assert!(binding.security_policy.allows_protocol("axion"));
        assert!(binding.security_policy.allows_command("app.ping"));
        assert!(
            !binding
                .security_policy
                .capabilities()
                .allow_remote_navigation
        );
        assert!(
            binding
                .security_policy
                .is_trusted_origin("http://127.0.0.1:3000")
        );
    }

    #[test]
    fn launch_request_allows_missing_packaged_entry_for_dev_server_target() {
        let request = launch_request(
            &app_with_missing_entry(Some("http://127.0.0.1:3000")),
            axion_core::RunMode::Development,
        )
        .expect("dev server launch should not require packaged entry");

        assert!(matches!(request.target, RuntimeLaunchTarget::DevServer(_)));
    }

    #[test]
    fn launch_request_rejects_missing_packaged_entry_for_app_protocol_target() {
        let error = launch_request(
            &app_with_missing_entry(None),
            axion_core::RunMode::Production,
        )
        .expect_err("packaged launch should require existing entry");

        assert!(matches!(
            error,
            RuntimeError::Protocol(ProtocolError::MissingAsset { .. })
        ));
    }

    #[test]
    fn launch_request_uses_app_protocol_for_packaged_mode() {
        let request = launch_request(&app_with_build(None), axion_core::RunMode::Production)
            .expect("launch request should build");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");

        match request.target {
            RuntimeLaunchTarget::AppProtocol(AppProtocolLaunch { initial_url, .. }) => {
                assert_eq!(initial_url.as_str(), "axion://app/index.html");
            }
            RuntimeLaunchTarget::DevServer(_) => panic!("expected app protocol launch target"),
        }
        assert!(!binding.bridge_token.is_empty());
        assert_eq!(binding.bridge_bindings.startup_events.len(), 2);
        assert_eq!(
            binding.bridge_bindings.command_registry.command_names(),
            vec!["app.ping".to_owned()]
        );
        assert!(binding.security_policy.allows_protocol("axion"));
        assert!(binding.security_policy.allows_command("app.ping"));
        assert!(
            !binding
                .security_policy
                .capabilities()
                .allow_remote_navigation
        );
        assert_eq!(
            request.app_protocol.initial_url.as_str(),
            "axion://app/index.html"
        );
        assert!(binding.security_policy.is_trusted_origin("axion://app"));
    }

    #[test]
    fn launch_request_builds_per_window_capabilities() {
        let request = launch_request(&multi_window_app(), axion_core::RunMode::Production)
            .expect("launch request should build");
        let main = request
            .window_bindings
            .iter()
            .find(|binding| binding.window_id == "main")
            .expect("main binding should exist");
        let settings = request
            .window_bindings
            .iter()
            .find(|binding| binding.window_id == "settings")
            .expect("settings binding should exist");

        assert_eq!(request.windows.len(), 2);
        assert_eq!(request.window_bindings.len(), 2);
        assert_eq!(main.command_context.window.id, "main");
        assert_eq!(settings.command_context.window.id, "settings");
        assert_ne!(main.bridge_token, settings.bridge_token);
        assert_eq!(
            main.bridge_bindings.command_registry.command_names(),
            vec!["app.ping".to_owned()]
        );
        assert_eq!(
            settings.bridge_bindings.command_registry.command_names(),
            vec!["window.info".to_owned()]
        );
        assert!(main.security_policy.allows_command("app.ping"));
        assert!(!main.security_policy.allows_command("window.info"));
        assert!(settings.security_policy.allows_command("window.info"));
        assert!(!main.security_policy.capabilities().allow_remote_navigation);
        assert!(
            settings
                .security_policy
                .capabilities()
                .allow_remote_navigation
        );
        assert!(
            settings
                .security_policy
                .allows_navigation_origin("https://docs.example")
        );
        assert!(
            settings
                .security_policy
                .allows_navigation(&Url::parse("https://docs.example/guide").unwrap())
        );
    }

    #[test]
    fn launch_request_requires_axion_protocol_for_bridge_commands() {
        let request = launch_request(
            &app_with_commands_without_axion_protocol(),
            axion_core::RunMode::Production,
        )
        .expect("launch request should build");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");

        assert!(!binding.security_policy.allows_protocol("axion"));
        assert!(
            binding
                .bridge_bindings
                .command_registry
                .command_names()
                .is_empty()
        );
        assert!(
            binding
                .bridge_bindings
                .event_registry
                .event_names()
                .is_empty()
        );
    }

    struct EchoPlugin;

    impl RuntimePlugin for EchoPlugin {
        fn register(&self, builder: &mut RuntimeBridgeBindingsBuilder) {
            builder.register_command("plugin.echo", |_context, request| {
                Ok(request.payload.clone())
            });
            builder.register_event("plugin.event", |_context, _request| Ok(()));
            builder.push_startup_event(super::RuntimeBridgeEvent::new(
                "plugin.ready",
                "{\"ready\":true}",
            ));
        }
    }

    #[test]
    fn launch_request_applies_runtime_plugins_when_capability_allows_command() {
        let plugin = EchoPlugin;
        let plugins: [&dyn RuntimePlugin; 1] = [&plugin];
        let request = launch_request_with_plugins(
            &app_with_plugin_command("plugin.echo"),
            axion_core::RunMode::Production,
            &plugins,
        )
        .expect("launch request should build with plugin");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");

        assert_eq!(
            binding.bridge_bindings.command_registry.command_names(),
            vec!["app.ping".to_owned(), "plugin.echo".to_owned()]
        );
        assert!(
            binding
                .bridge_bindings
                .startup_events
                .iter()
                .any(|event| event.name == "plugin.ready")
        );
        assert_eq!(
            binding.bridge_bindings.event_registry.event_names(),
            vec!["app.log".to_owned(), "plugin.event".to_owned()]
        );
    }

    #[test]
    fn launch_request_filters_plugin_commands_without_capability() {
        let plugin = EchoPlugin;
        let plugins: [&dyn RuntimePlugin; 1] = [&plugin];
        let request = launch_request_with_plugins(
            &app_with_build(None),
            axion_core::RunMode::Production,
            &plugins,
        )
        .expect("launch request should build with filtered plugin");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");

        assert_eq!(
            binding.bridge_bindings.command_registry.command_names(),
            vec!["app.ping".to_owned()]
        );
        assert!(
            binding
                .bridge_bindings
                .startup_events
                .iter()
                .any(|event| event.name == "plugin.ready")
        );
    }

    #[test]
    fn diagnostic_report_summarizes_runtime_launch() {
        let report = diagnostic_report(&multi_window_app(), axion_core::RunMode::Production);
        let settings = report
            .windows
            .iter()
            .find(|window| window.window_id == "settings")
            .expect("settings diagnostics should exist");

        assert_eq!(report.app_name, "axion-runtime-test");
        assert_eq!(report.window_count, 2);
        assert!(report.resource_policy.contains("nosniff=true"));
        assert!(!report.has_errors());
        assert_eq!(settings.command_count, 1);
        assert_eq!(settings.event_count, 1);
        assert_eq!(settings.frontend_events, vec!["app.log".to_owned()]);
        assert_eq!(
            settings.host_events,
            vec![
                "app.ready".to_owned(),
                "window.created".to_owned(),
                "window.close_requested".to_owned(),
                "window.closed".to_owned(),
                "window.resized".to_owned(),
                "window.redraw_failed".to_owned(),
            ]
        );
        assert_eq!(settings.startup_event_count, 2);
        assert_eq!(
            settings.lifecycle_events,
            vec![
                "window.created".to_owned(),
                "window.close_requested".to_owned(),
                "window.closed".to_owned(),
                "window.resized".to_owned(),
                "window.redraw_failed".to_owned(),
            ]
        );
        assert!(settings.bridge_enabled);
        assert_eq!(
            settings.allowed_navigation_origins,
            vec!["https://docs.example".to_owned()]
        );
        assert!(settings.allow_remote_navigation);
        assert!(
            settings
                .content_security_policy
                .contains("default-src 'self'")
        );
        assert!(!settings.content_security_policy.contains("'unsafe-inline'"));
        assert!(
            report
                .issues
                .iter()
                .any(|issue| matches!(issue.severity, DiagnosticSeverity::Warning))
        );
    }

    #[test]
    fn host_event_names_deduplicate_startup_and_lifecycle_events() {
        let events = super::host_event_names(&[
            super::RuntimeBridgeEvent::new("window.created", "{}"),
            super::RuntimeBridgeEvent::new("plugin.ready", "{}"),
        ]);

        assert_eq!(
            events,
            vec![
                "window.created".to_owned(),
                "plugin.ready".to_owned(),
                "window.close_requested".to_owned(),
                "window.closed".to_owned(),
                "window.resized".to_owned(),
                "window.redraw_failed".to_owned(),
            ]
        );
    }

    #[test]
    fn diagnostic_report_captures_launch_errors() {
        let report = diagnostic_report(
            &app_with_missing_entry(None),
            axion_core::RunMode::Production,
        );

        assert!(report.has_errors());
        assert_eq!(report.windows.len(), 0);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| matches!(issue.severity, DiagnosticSeverity::Error))
        );
    }

    #[test]
    fn window_lifecycle_event_names_are_stable() {
        assert_eq!(
            super::window_lifecycle_event_names(),
            vec![
                "window.created".to_owned(),
                "window.close_requested".to_owned(),
                "window.closed".to_owned(),
                "window.resized".to_owned(),
                "window.redraw_failed".to_owned(),
            ]
        );
    }

    #[test]
    fn panic_report_path_sanitizes_app_name() {
        let config = PanicReportConfig {
            app_name: "hello/axion".to_owned(),
            output_dir: PathBuf::from("crash-reports"),
        };

        assert_eq!(
            panic_report_path(&config, 123),
            PathBuf::from("crash-reports").join("axion-crash-hello-axion-123.log")
        );
    }

    #[test]
    fn panic_report_contains_core_context() {
        let body = format_panic_report_body(
            "hello-axion",
            "main",
            "examples/hello-axion/src/main.rs:1:1",
            "synthetic panic",
        );

        assert!(body.contains("Axion crash report"));
        assert!(body.contains("app=hello-axion"));
        assert!(body.contains("thread=main"));
        assert!(body.contains("location=examples/hello-axion/src/main.rs:1:1"));
        assert!(body.contains("panic=synthetic panic"));
    }
}

fn json_string_literal(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

fn optional_json_string_literal(value: Option<&str>) -> String {
    value
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_owned())
}

fn json_string_map_literal(values: &std::collections::BTreeMap<String, String>) -> String {
    let entries = values
        .iter()
        .map(|(key, value)| {
            format!(
                "{}:{}",
                json_string_literal(key),
                json_string_literal(value)
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!("{{{entries}}}")
}
