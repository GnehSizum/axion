use axion_bridge::{
    BridgeBindings, BridgeBindingsBuilder, BridgeBindingsPlugin, BridgeEvent, BridgeRunMode,
    CommandContext, WindowCommandContext,
};
use axion_core::{
    App, ClipboardBackendConfig, DialogBackendConfig, RunMode, RuntimeLaunchConfig,
    WindowLaunchConfig,
};
use axion_protocol::AppAssetResolver;
use axion_security::SecurityPolicy;
use thiserror::Error;

pub use axion_bridge::BridgeBindingsBuilder as RuntimeBridgeBindingsBuilder;
pub use axion_bridge::{
    BridgeEmitRequest, BridgeEvent as RuntimeBridgeEvent, BridgeRequest, CommandRegistryError,
    WindowControlHandle, WindowControlRequest, WindowControlResponse, WindowStateSnapshot,
};

pub const AXION_RELEASE_VERSION: &str = "v0.1.25.0";
pub const AXION_DIAGNOSTICS_REPORT_SCHEMA: &str = "axion.diagnostics-report.v1";

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
    pub configured_dialog_backend: DialogBackendKind,
    pub dialog_backend: DialogBackendKind,
    pub configured_clipboard_backend: ClipboardBackendKind,
    pub clipboard_backend: ClipboardBackendKind,
    pub close_timeout_ms: u64,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsReport {
    pub source: String,
    pub exported_at_unix_seconds: Option<u64>,
    pub manifest_path: Option<std::path::PathBuf>,
    pub app_name: String,
    pub identifier: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub homepage: Option<String>,
    pub mode: Option<String>,
    pub window_count: usize,
    pub windows: Vec<DiagnosticsWindowReport>,
    pub frontend_dist: Option<std::path::PathBuf>,
    pub entry: Option<std::path::PathBuf>,
    pub configured_dialog_backend: Option<String>,
    pub dialog_backend: Option<String>,
    pub configured_clipboard_backend: Option<String>,
    pub clipboard_backend: Option<String>,
    pub close_timeout_ms: Option<u64>,
    pub icon: Option<std::path::PathBuf>,
    pub host_events: Vec<String>,
    pub staged_app_dir: Option<std::path::PathBuf>,
    pub asset_manifest_path: Option<std::path::PathBuf>,
    pub artifacts_removed: Option<bool>,
    pub diagnostics: Option<String>,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsWindowReport {
    pub id: String,
    pub title: String,
    pub bridge_enabled: bool,
    pub configured_profiles: Vec<String>,
    pub configured_commands: Vec<String>,
    pub configured_events: Vec<String>,
    pub configured_protocols: Vec<String>,
    pub runtime_command_count: usize,
    pub runtime_event_count: usize,
    pub host_events: Vec<String>,
    pub trusted_origins: Vec<String>,
    pub allowed_navigation_origins: Vec<String>,
    pub allow_remote_navigation: bool,
}

impl DiagnosticsReport {
    pub fn to_json(&self) -> String {
        let windows = self
            .windows
            .iter()
            .map(DiagnosticsWindowReport::to_json)
            .collect::<Vec<_>>()
            .join(",");

        let diagnostics = self
            .diagnostics
            .as_deref()
            .map(|diagnostics| format!(",\"diagnostics\":{diagnostics}"))
            .unwrap_or_default();

        format!(
            "{{\"schema\":{},\"source\":{},\"exported_at_unix_seconds\":{},\"manifest_path\":{},\"app_name\":{},\"identifier\":{},\"version\":{},\"description\":{},\"authors\":{},\"homepage\":{},\"mode\":{},\"window_count\":{},\"windows\":[{}],\"frontend_dist\":{},\"entry\":{},\"configured_dialog_backend\":{},\"dialog_backend\":{},\"configured_clipboard_backend\":{},\"clipboard_backend\":{},\"close_timeout_ms\":{},\"icon\":{},\"host_events\":{},\"staged_app_dir\":{},\"asset_manifest_path\":{},\"artifacts_removed\":{}{},\"result\":{}}}",
            json_string_literal(AXION_DIAGNOSTICS_REPORT_SCHEMA),
            json_string_literal(&self.source),
            optional_json_u64(self.exported_at_unix_seconds),
            optional_json_path(self.manifest_path.as_deref()),
            json_string_literal(&self.app_name),
            optional_json_string_literal(self.identifier.as_deref()),
            optional_json_string_literal(self.version.as_deref()),
            optional_json_string_literal(self.description.as_deref()),
            json_string_array_literal(&self.authors),
            optional_json_string_literal(self.homepage.as_deref()),
            optional_json_string_literal(self.mode.as_deref()),
            self.window_count,
            windows,
            optional_json_path(self.frontend_dist.as_deref()),
            optional_json_path(self.entry.as_deref()),
            optional_json_string_literal(self.configured_dialog_backend.as_deref()),
            optional_json_string_literal(self.dialog_backend.as_deref()),
            optional_json_string_literal(self.configured_clipboard_backend.as_deref()),
            optional_json_string_literal(self.clipboard_backend.as_deref()),
            optional_json_u64(self.close_timeout_ms),
            optional_json_path(self.icon.as_deref()),
            json_string_array_literal(&self.host_events),
            optional_json_path(self.staged_app_dir.as_deref()),
            optional_json_path(self.asset_manifest_path.as_deref()),
            optional_json_bool(self.artifacts_removed),
            diagnostics,
            json_string_literal(&self.result),
        )
    }
}

impl DiagnosticsWindowReport {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},\"title\":{},\"bridge_enabled\":{},\"configured_profiles\":{},\"configured_commands\":{},\"configured_events\":{},\"configured_protocols\":{},\"runtime_command_count\":{},\"runtime_event_count\":{},\"host_events\":{},\"trusted_origins\":{},\"allowed_navigation_origins\":{},\"allow_remote_navigation\":{}}}",
            json_string_literal(&self.id),
            json_string_literal(&self.title),
            self.bridge_enabled,
            json_string_array_literal(&self.configured_profiles),
            json_string_array_literal(&self.configured_commands),
            json_string_array_literal(&self.configured_events),
            json_string_array_literal(&self.configured_protocols),
            self.runtime_command_count,
            self.runtime_event_count,
            json_string_array_literal(&self.host_events),
            json_string_array_literal(&self.trusted_origins),
            json_string_array_literal(&self.allowed_navigation_origins),
            self.allow_remote_navigation,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowLifecycleEventKind {
    Created,
    Ready,
    CloseRequested,
    ClosePrevented,
    CloseCompleted,
    CloseTimedOut,
    Closed,
    Resized,
    Focused,
    Blurred,
    Moved,
    RedrawFailed,
}

impl WindowLifecycleEventKind {
    pub const fn event_name(self) -> &'static str {
        match self {
            Self::Created => "window.created",
            Self::Ready => "window.ready",
            Self::CloseRequested => "window.close_requested",
            Self::ClosePrevented => "window.close_prevented",
            Self::CloseCompleted => "window.close_completed",
            Self::CloseTimedOut => "window.close_timed_out",
            Self::Closed => "window.closed",
            Self::Resized => "window.resized",
            Self::Focused => "window.focused",
            Self::Blurred => "window.blurred",
            Self::Moved => "window.moved",
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
        WindowLifecycleEventKind::Ready,
        WindowLifecycleEventKind::CloseRequested,
        WindowLifecycleEventKind::ClosePrevented,
        WindowLifecycleEventKind::CloseCompleted,
        WindowLifecycleEventKind::CloseTimedOut,
        WindowLifecycleEventKind::Closed,
        WindowLifecycleEventKind::Resized,
        WindowLifecycleEventKind::Focused,
        WindowLifecycleEventKind::Blurred,
        WindowLifecycleEventKind::Moved,
        WindowLifecycleEventKind::RedrawFailed,
    ]
    .into_iter()
    .map(|kind| kind.event_name().to_owned())
    .collect()
}

pub fn app_lifecycle_event_names() -> Vec<String> {
    vec![
        "app.exit_requested".to_owned(),
        "app.exit_prevented".to_owned(),
        "app.exit_completed".to_owned(),
    ]
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
    for event in app_lifecycle_event_names() {
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
    pub configured_dialog_backend: DialogBackendKind,
    pub dialog_backend: DialogBackendKind,
    pub configured_clipboard_backend: ClipboardBackendKind,
    pub clipboard_backend: ClipboardBackendKind,
    pub close_timeout_ms: u64,
    pub windows: Vec<axion_core::WindowLaunchConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogBackendKind {
    Headless,
    System,
    SystemUnavailable,
}

impl DialogBackendKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Headless => "headless",
            Self::System => "system",
            Self::SystemUnavailable => "system-unavailable",
        }
    }

    pub const fn resolve_for_current_platform(self) -> Self {
        match self {
            Self::System => {
                #[cfg(target_os = "macos")]
                {
                    Self::System
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Self::SystemUnavailable
                }
            }
            other => other,
        }
    }
}

impl From<DialogBackendConfig> for DialogBackendKind {
    fn from(value: DialogBackendConfig) -> Self {
        match value {
            DialogBackendConfig::Headless => Self::Headless,
            DialogBackendConfig::System => Self::System,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClipboardBackendKind {
    #[default]
    Memory,
    System,
    SystemUnavailable,
}

impl ClipboardBackendKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::System => "system",
            Self::SystemUnavailable => "system-unavailable",
        }
    }

    pub const fn resolve_for_current_platform(self) -> Self {
        match self {
            Self::System => {
                #[cfg(target_os = "macos")]
                {
                    Self::System
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Self::SystemUnavailable
                }
            }
            other => other,
        }
    }
}

impl From<ClipboardBackendConfig> for ClipboardBackendKind {
    fn from(value: ClipboardBackendConfig) -> Self {
        match value {
            ClipboardBackendConfig::Memory => Self::Memory,
            ClipboardBackendConfig::System => Self::System,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogRequestKind {
    Open,
    Save,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogRequest {
    pub kind: DialogRequestKind,
    pub title: Option<String>,
    pub default_path: Option<std::path::PathBuf>,
    pub directory: bool,
    pub multiple: bool,
    pub filters: Vec<DialogFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogResponse {
    pub canceled: bool,
    pub path: Option<std::path::PathBuf>,
    pub paths: Option<Vec<std::path::PathBuf>>,
    pub backend: DialogBackendKind,
}

#[derive(Debug, Clone, Default)]
struct ClipboardStore {
    text: std::sync::Arc<std::sync::Mutex<String>>,
    backend: ClipboardBackendKind,
}

impl ClipboardStore {
    fn new(backend: ClipboardBackendKind) -> Self {
        Self {
            text: std::sync::Arc::default(),
            backend,
        }
    }

    fn read_text(&self) -> Result<ClipboardResponse, String> {
        match self.backend {
            ClipboardBackendKind::Memory => self.read_memory_text().map(|text| ClipboardResponse {
                text,
                backend: ClipboardBackendKind::Memory,
            }),
            ClipboardBackendKind::System => read_system_clipboard_text()
                .or_else(|_| {
                    self.read_memory_text()
                        .map(|text| (text, ClipboardBackendKind::Memory))
                })
                .map(|(text, backend)| ClipboardResponse { text, backend }),
            ClipboardBackendKind::SystemUnavailable => {
                self.read_memory_text().map(|text| ClipboardResponse {
                    text,
                    backend: ClipboardBackendKind::Memory,
                })
            }
        }
    }

    fn write_text(&self, text: String) -> Result<ClipboardWriteResponse, String> {
        let bytes = text.len();
        match self.backend {
            ClipboardBackendKind::Memory => {
                self.write_memory_text(text)?;
                Ok(ClipboardWriteResponse {
                    bytes,
                    backend: ClipboardBackendKind::Memory,
                })
            }
            ClipboardBackendKind::System => match write_system_clipboard_text(&text) {
                Ok(()) => {
                    self.write_memory_text(text)?;
                    Ok(ClipboardWriteResponse {
                        bytes,
                        backend: ClipboardBackendKind::System,
                    })
                }
                Err(_) => {
                    self.write_memory_text(text)?;
                    Ok(ClipboardWriteResponse {
                        bytes,
                        backend: ClipboardBackendKind::Memory,
                    })
                }
            },
            ClipboardBackendKind::SystemUnavailable => {
                self.write_memory_text(text)?;
                Ok(ClipboardWriteResponse {
                    bytes,
                    backend: ClipboardBackendKind::Memory,
                })
            }
        }
    }

    fn read_memory_text(&self) -> Result<String, String> {
        self.text
            .lock()
            .map(|text| text.clone())
            .map_err(|_| "clipboard state lock was poisoned".to_owned())
    }

    fn write_memory_text(&self, text: String) -> Result<(), String> {
        self.text
            .lock()
            .map(|mut stored| {
                *stored = text;
            })
            .map_err(|_| "clipboard state lock was poisoned".to_owned())
    }
}

struct ClipboardResponse {
    text: String,
    backend: ClipboardBackendKind,
}

struct ClipboardWriteResponse {
    bytes: usize,
    backend: ClipboardBackendKind,
}

fn read_system_clipboard_text() -> Result<(String, ClipboardBackendKind), String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("pbpaste")
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err("pbpaste exited unsuccessfully".to_owned());
        }
        String::from_utf8(output.stdout)
            .map(|text| (text, ClipboardBackendKind::System))
            .map_err(|error| error.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("system clipboard backend is unavailable on this platform".to_owned())
    }
}

fn write_system_clipboard_text(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;

        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to open pbcopy stdin".to_owned())?;
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| error.to_string())?;
        drop(stdin);
        let status = child.wait().map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("pbcopy exited unsuccessfully".to_owned())
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        Err("system clipboard backend is unavailable on this platform".to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogRequestError {
    InvalidPayload { message: String },
}

impl std::fmt::Display for DialogRequestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPayload { message } => formatter.write_str(message),
        }
    }
}

impl std::error::Error for DialogRequestError {}

#[derive(Debug, Clone)]
pub struct RuntimeWindowBinding {
    pub window_id: String,
    pub bridge_token: String,
    pub command_context: CommandContext,
    pub bridge_bindings: BridgeBindings,
    pub security_policy: SecurityPolicy,
    pub window_control: WindowControlHandle,
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
            configured_dialog_backend: DialogBackendKind::from(app.config().native.dialog.backend),
            dialog_backend: DialogBackendKind::from(app.config().native.dialog.backend)
                .resolve_for_current_platform(),
            configured_clipboard_backend: ClipboardBackendKind::from(
                app.config().native.clipboard.backend,
            ),
            clipboard_backend: ClipboardBackendKind::from(app.config().native.clipboard.backend)
                .resolve_for_current_platform(),
            close_timeout_ms: app.config().native.lifecycle.close_timeout_ms,
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
    if request.configured_dialog_backend != request.dialog_backend {
        issues.push(RuntimeDiagnosticIssue {
            severity: DiagnosticSeverity::Warning,
            message: format!(
                "native dialog backend '{}' resolves to '{}' on this platform",
                request.configured_dialog_backend.as_str(),
                request.dialog_backend.as_str()
            ),
        });
    }
    if request.configured_clipboard_backend != request.clipboard_backend {
        issues.push(RuntimeDiagnosticIssue {
            severity: DiagnosticSeverity::Warning,
            message: format!(
                "native clipboard backend '{}' resolves to '{}' on this platform",
                request.configured_clipboard_backend.as_str(),
                request.clipboard_backend.as_str()
            ),
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
        configured_dialog_backend: request.configured_dialog_backend,
        dialog_backend: request.dialog_backend,
        configured_clipboard_backend: request.configured_clipboard_backend,
        clipboard_backend: request.clipboard_backend,
        close_timeout_ms: request.close_timeout_ms,
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
    let configured_dialog_backend = DialogBackendKind::from(launch_config.native.dialog.backend);
    let dialog_backend = configured_dialog_backend.resolve_for_current_platform();
    let configured_clipboard_backend =
        ClipboardBackendKind::from(launch_config.native.clipboard.backend);
    let clipboard_backend = configured_clipboard_backend.resolve_for_current_platform();
    let app_protocol_resolver = AppAssetResolver::new(
        launch_config.frontend_dist.clone(),
        launch_config.packaged_entry.clone(),
    )?;
    let app_protocol = AppProtocolLaunch {
        initial_url: app_protocol_resolver.initial_url(),
        resolver: app_protocol_resolver,
    };
    let clipboard = ClipboardStore::new(clipboard_backend);
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
            let app_data_dir = app_data_dir(&launch_config);
            let window_control = WindowControlHandle::new();
            RuntimeWindowBinding {
                window_id: window.id.clone(),
                bridge_token: uuid::Uuid::new_v4().to_string(),
                bridge_bindings: build_bridge_bindings(
                    &security_policy,
                    &command_context,
                    app_data_dir,
                    clipboard.clone(),
                    window_control.clone(),
                    dialog_backend,
                    plugins,
                ),
                security_policy,
                command_context,
                window_control,
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
        configured_dialog_backend,
        dialog_backend,
        configured_clipboard_backend,
        clipboard_backend,
        close_timeout_ms: launch_config.native.lifecycle.close_timeout_ms,
        windows: launch_config.windows,
    })
}

fn app_data_dir(launch_config: &RuntimeLaunchConfig) -> std::path::PathBuf {
    let app_root = launch_config
        .frontend_dist
        .parent()
        .unwrap_or(&launch_config.frontend_dist);
    app_root
        .join("target")
        .join("axion-data")
        .join(sanitize_path_segment(&launch_config.app_name))
}

fn sanitize_path_segment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();

    if sanitized.is_empty() {
        "app".to_owned()
    } else {
        sanitized
    }
}

fn build_command_context(
    launch_config: &RuntimeLaunchConfig,
    window: &WindowLaunchConfig,
) -> CommandContext {
    CommandContext {
        app_name: launch_config.app_name.clone(),
        identifier: launch_config.identifier.clone(),
        version: launch_config.version.clone(),
        description: launch_config.description.clone(),
        authors: launch_config.authors.clone(),
        homepage: launch_config.homepage.clone(),
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
    app_data_dir: std::path::PathBuf,
    clipboard: ClipboardStore,
    window_control: WindowControlHandle,
    dialog_backend: DialogBackendKind,
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
        app_data_dir,
        clipboard,
        window_control,
        dialog_backend,
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
    app_data_dir: std::path::PathBuf,
    clipboard: ClipboardStore,
    window_control: WindowControlHandle,
    dialog_backend: DialogBackendKind,
}

impl BridgeBindingsPlugin for BuiltinBridgePlugin {
    fn register(&self, builder: &mut BridgeBindingsBuilder) {
        let command_context = builder.command_context().clone();

        register_builtin_commands(
            builder,
            &self.allowed_commands,
            self.app_data_dir.clone(),
            self.clipboard.clone(),
            self.window_control.clone(),
            self.dialog_backend,
        );
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
        builder.push_startup_event(BridgeEvent::new(
            WindowLifecycleEventKind::Ready.event_name(),
            format!(
                "{{\"windowId\":{},\"title\":{},\"bridgeReady\":true}}",
                json_string_literal(&command_context.window.id),
                json_string_literal(&command_context.window.title),
            ),
        ));
    }
}

fn register_builtin_commands(
    builder: &mut BridgeBindingsBuilder,
    allowed_commands: &[String],
    app_data_dir: std::path::PathBuf,
    clipboard: ClipboardStore,
    window_control: WindowControlHandle,
    dialog_backend: DialogBackendKind,
) {
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
                "{{\"appName\":{},\"identifier\":{},\"version\":{},\"description\":{},\"authors\":{},\"homepage\":{},\"mode\":{}}}",
                json_string_literal(&context.app_name),
                optional_json_string_literal(context.identifier.as_deref()),
                optional_json_string_literal(context.version.as_deref()),
                optional_json_string_literal(context.description.as_deref()),
                json_string_array_literal(&context.authors),
                optional_json_string_literal(context.homepage.as_deref()),
                json_string_literal(match context.mode {
                    BridgeRunMode::Development => "development",
                    BridgeRunMode::Production => "production",
                }),
            ))
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "app.version")
    {
        builder.register_command("app.version", |_context, _request| {
            Ok(format!(
                "{{\"version\":{},\"release\":{},\"framework\":\"axion\"}}",
                json_string_literal(env!("CARGO_PKG_VERSION")),
                json_string_literal(AXION_RELEASE_VERSION),
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

    if allowed_commands.iter().any(|command| command == "app.exit") {
        let window_control = window_control.clone();
        builder.register_command_async("app.exit", move |_context, _request| {
            let window_control = window_control.clone();
            async move { execute_app_exit_json(&window_control) }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "clipboard.read_text")
    {
        let clipboard = clipboard.clone();
        builder.register_command("clipboard.read_text", move |_context, _request| {
            let response = clipboard.read_text()?;
            Ok(format!(
                "{{\"text\":{},\"backend\":{}}}",
                json_string_literal(&response.text),
                json_string_literal(response.backend.as_str()),
            ))
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "clipboard.write_text")
    {
        let clipboard = clipboard.clone();
        builder.register_command("clipboard.write_text", move |_context, request| {
            let text = json_string_field(&request.payload, "text").ok_or_else(|| {
                "clipboard.write_text requires a JSON string field named 'text'".to_owned()
            })?;
            let response = clipboard.write_text(text)?;
            Ok(format!(
                "{{\"bytes\":{},\"backend\":{}}}",
                response.bytes,
                json_string_literal(response.backend.as_str()),
            ))
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.list")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.list", move |_context, _request| {
            let window_control = window_control.clone();
            async move {
                execute_window_control_json(&window_control, None, WindowControlRequest::ListStates)
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.info")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.info", move |context, request| {
            let window_control = window_control.clone();
            async move {
                let target_window_id = json_string_field(&request.payload, "target");
                let state =
                    current_window_state(&window_control, &context, target_window_id.as_deref())?;
                Ok(window_state_json(&state))
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.show")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.show", move |_context, request| {
            let window_control = window_control.clone();
            async move {
                let target_window_id = json_string_field(&request.payload, "target");
                execute_window_control_json(
                    &window_control,
                    target_window_id.as_deref(),
                    WindowControlRequest::Show,
                )
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.hide")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.hide", move |_context, request| {
            let window_control = window_control.clone();
            async move {
                let target_window_id = json_string_field(&request.payload, "target");
                execute_window_control_json(
                    &window_control,
                    target_window_id.as_deref(),
                    WindowControlRequest::Hide,
                )
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.close")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.close", move |_context, request| {
            let window_control = window_control.clone();
            async move {
                let target_window_id = json_string_field(&request.payload, "target");
                execute_window_control_json(
                    &window_control,
                    target_window_id.as_deref(),
                    WindowControlRequest::Close,
                )
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.confirm_close")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.confirm_close", move |_context, request| {
            let window_control = window_control.clone();
            async move {
                let request_id =
                    json_string_field(&request.payload, "requestId").ok_or_else(|| {
                        "window.confirm_close requires a JSON string field named 'requestId'"
                            .to_owned()
                    })?;
                execute_window_control_json(
                    &window_control,
                    None,
                    WindowControlRequest::ConfirmClose { request_id },
                )
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.prevent_close")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.prevent_close", move |_context, request| {
            let window_control = window_control.clone();
            async move {
                let request_id =
                    json_string_field(&request.payload, "requestId").ok_or_else(|| {
                        "window.prevent_close requires a JSON string field named 'requestId'"
                            .to_owned()
                    })?;
                execute_window_control_json(
                    &window_control,
                    None,
                    WindowControlRequest::PreventClose { request_id },
                )
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.focus")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.focus", move |_context, request| {
            let window_control = window_control.clone();
            async move {
                let target_window_id = json_string_field(&request.payload, "target");
                execute_window_control_json(
                    &window_control,
                    target_window_id.as_deref(),
                    WindowControlRequest::Focus,
                )
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.reload")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.reload", move |_context, request| {
            let window_control = window_control.clone();
            async move {
                let target_window_id = json_string_field(&request.payload, "target");
                execute_window_control_json(
                    &window_control,
                    target_window_id.as_deref(),
                    WindowControlRequest::Reload,
                )
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.set_title")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.set_title", move |_context, request| {
            let window_control = window_control.clone();
            async move {
                let target_window_id = json_string_field(&request.payload, "target");
                let title = json_string_field(&request.payload, "title").ok_or_else(|| {
                    "window.set_title requires a JSON string field named 'title'".to_owned()
                })?;
                execute_window_control_json(
                    &window_control,
                    target_window_id.as_deref(),
                    WindowControlRequest::SetTitle { title },
                )
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "window.set_size")
    {
        let window_control = window_control.clone();
        builder.register_command_async("window.set_size", move |_context, request| {
            let window_control = window_control.clone();
            async move {
                let target_window_id = json_string_field(&request.payload, "target");
                let width = json_u32_field(&request.payload, "width").ok_or_else(|| {
                    "window.set_size requires a JSON number field named 'width'".to_owned()
                })?;
                let height = json_u32_field(&request.payload, "height").ok_or_else(|| {
                    "window.set_size requires a JSON number field named 'height'".to_owned()
                })?;
                if width == 0 || height == 0 {
                    return Err("window.set_size requires non-zero width and height".to_owned());
                }
                execute_window_control_json(
                    &window_control,
                    target_window_id.as_deref(),
                    WindowControlRequest::SetSize { width, height },
                )
            }
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "fs.read_text")
    {
        let app_data_dir = app_data_dir.clone();
        builder.register_command("fs.read_text", move |_context, request| {
            let relative_path = json_string_field(&request.payload, "path").ok_or_else(|| {
                "fs.read_text requires a JSON string field named 'path'".to_owned()
            })?;
            let path = resolve_app_data_path(&app_data_dir, &relative_path, false)?;
            let contents = std::fs::read_to_string(&path)
                .map_err(|error| format!("failed to read app data file: {error}"))?;
            Ok(format!(
                "{{\"path\":{},\"contents\":{}}}",
                json_string_literal(&relative_path),
                json_string_literal(&contents),
            ))
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "fs.write_text")
    {
        let app_data_dir = app_data_dir.clone();
        builder.register_command("fs.write_text", move |_context, request| {
            let relative_path = json_string_field(&request.payload, "path").ok_or_else(|| {
                "fs.write_text requires a JSON string field named 'path'".to_owned()
            })?;
            let contents = json_string_field(&request.payload, "contents").ok_or_else(|| {
                "fs.write_text requires a JSON string field named 'contents'".to_owned()
            })?;
            let path = resolve_app_data_path(&app_data_dir, &relative_path, true)?;
            std::fs::write(&path, &contents)
                .map_err(|error| format!("failed to write app data file: {error}"))?;
            Ok(format!(
                "{{\"path\":{},\"bytes\":{}}}",
                json_string_literal(&relative_path),
                contents.len(),
            ))
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "dialog.open")
    {
        builder.register_command("dialog.open", move |_context, request| {
            let request = DialogRequest::from_payload(DialogRequestKind::Open, &request.payload)
                .map_err(|error| error.to_string())?;
            Ok(execute_dialog_request(dialog_backend, request).to_json())
        });
    }

    if allowed_commands
        .iter()
        .any(|command| command == "dialog.save")
    {
        builder.register_command("dialog.save", move |_context, request| {
            let request = DialogRequest::from_payload(DialogRequestKind::Save, &request.payload)
                .map_err(|error| error.to_string())?;
            Ok(execute_dialog_request(dialog_backend, request).to_json())
        });
    }
}

impl DialogRequest {
    fn from_payload(kind: DialogRequestKind, payload: &str) -> Result<Self, DialogRequestError> {
        let request = Self {
            kind,
            title: json_string_field(payload, "title"),
            default_path: json_string_field(payload, "defaultPath").map(std::path::PathBuf::from),
            directory: json_bool_field(payload, "directory").unwrap_or(false),
            multiple: json_bool_field(payload, "multiple").unwrap_or(false),
            filters: dialog_filters_field(payload, "filters")?,
        };
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<(), DialogRequestError> {
        if matches!(self.kind, DialogRequestKind::Save) && self.directory {
            return Err(DialogRequestError::InvalidPayload {
                message: "dialog.save does not support 'directory=true'".to_owned(),
            });
        }
        if matches!(self.kind, DialogRequestKind::Save) && self.multiple {
            return Err(DialogRequestError::InvalidPayload {
                message: "dialog.save does not support 'multiple=true'".to_owned(),
            });
        }
        if self
            .filters
            .iter()
            .any(|filter| filter.name.trim().is_empty())
        {
            return Err(DialogRequestError::InvalidPayload {
                message: "dialog filters require a non-empty 'name'".to_owned(),
            });
        }
        if self.filters.iter().any(|filter| {
            filter.extensions.is_empty()
                || filter.extensions.iter().any(|ext| ext.trim().is_empty())
        }) {
            return Err(DialogRequestError::InvalidPayload {
                message: "dialog filters require at least one non-empty extension".to_owned(),
            });
        }
        Ok(())
    }
}

impl DialogResponse {
    fn canceled(backend: DialogBackendKind) -> Self {
        Self {
            canceled: true,
            path: None,
            paths: None,
            backend,
        }
    }

    #[cfg(target_os = "macos")]
    fn selected(path: impl Into<std::path::PathBuf>, backend: DialogBackendKind) -> Self {
        let path = path.into();
        Self {
            canceled: false,
            path: Some(path),
            paths: None,
            backend,
        }
    }

    #[cfg(target_os = "macos")]
    fn selected_multiple(paths: Vec<std::path::PathBuf>, backend: DialogBackendKind) -> Self {
        let path = paths.first().cloned();
        Self {
            canceled: false,
            path,
            paths: Some(paths),
            backend,
        }
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"canceled\":{},\"path\":{},\"paths\":{},\"backend\":{}}}",
            self.canceled,
            self.path
                .as_ref()
                .and_then(|path| path.to_str())
                .map(json_string_literal)
                .unwrap_or_else(|| "null".to_owned()),
            self.paths
                .as_ref()
                .map(|paths| {
                    let entries = paths
                        .iter()
                        .filter_map(|path| path.to_str())
                        .map(json_string_literal)
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("[{entries}]")
                })
                .unwrap_or_else(|| "null".to_owned()),
            json_string_literal(self.backend.as_str()),
        )
    }
}

pub fn execute_dialog_request(
    backend: DialogBackendKind,
    request: DialogRequest,
) -> DialogResponse {
    match backend {
        DialogBackendKind::Headless => DialogResponse::canceled(DialogBackendKind::Headless),
        DialogBackendKind::SystemUnavailable => {
            DialogResponse::canceled(DialogBackendKind::SystemUnavailable)
        }
        DialogBackendKind::System => execute_system_dialog_request(request),
    }
}

fn execute_system_dialog_request(request: DialogRequest) -> DialogResponse {
    #[cfg(target_os = "macos")]
    {
        execute_macos_dialog_request(request)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = request;
        DialogResponse::canceled(DialogBackendKind::SystemUnavailable)
    }
}

#[cfg(target_os = "macos")]
fn execute_macos_dialog_request(request: DialogRequest) -> DialogResponse {
    let script = macos_dialog_script(&request);
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output();
    let Ok(output) = output else {
        return DialogResponse::canceled(DialogBackendKind::SystemUnavailable);
    };

    if !output.status.success() {
        return DialogResponse::canceled(DialogBackendKind::System);
    }

    let paths = parse_macos_dialog_paths(&output.stdout);
    if paths.is_empty() {
        DialogResponse::canceled(DialogBackendKind::System)
    } else if request.multiple {
        DialogResponse::selected_multiple(paths, DialogBackendKind::System)
    } else {
        DialogResponse::selected(paths[0].clone(), DialogBackendKind::System)
    }
}

#[cfg(target_os = "macos")]
fn macos_dialog_script(request: &DialogRequest) -> String {
    let prompt = request
        .title
        .as_deref()
        .map(applescript_string_literal)
        .map(|title| format!(" with prompt {title}"))
        .unwrap_or_default();
    let default_location = request
        .default_path
        .as_ref()
        .and_then(|path| path.parent())
        .and_then(|path| path.to_str())
        .map(applescript_string_literal)
        .map(|path| format!(" default location POSIX file {path}"))
        .unwrap_or_default();

    let command = match request.kind {
        DialogRequestKind::Open if request.directory => {
            let multiple = if request.multiple {
                " with multiple selections allowed"
            } else {
                ""
            };
            format!("my axionJoinPaths(choose folder{prompt}{default_location}{multiple})")
        }
        DialogRequestKind::Open => {
            let multiple = if request.multiple {
                " with multiple selections allowed"
            } else {
                ""
            };
            format!("my axionJoinPaths(choose file{prompt}{default_location}{multiple})")
        }
        DialogRequestKind::Save => {
            let default_name = request
                .default_path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|file_name| file_name.to_str())
                .map(applescript_string_literal)
                .map(|name| format!(" default name {name}"))
                .unwrap_or_default();
            format!("my axionJoinPaths(choose file name{prompt}{default_name}{default_location})")
        }
    };

    format!(
        "{command}\n\
        on axionJoinPaths(selectionResult)\n\
            if class of selectionResult is list then\n\
                set joinedPaths to \"\"\n\
                repeat with selectedItem in selectionResult\n\
                    set joinedPaths to joinedPaths & POSIX path of selectedItem & linefeed\n\
                end repeat\n\
                return joinedPaths\n\
            end if\n\
            return POSIX path of selectionResult\n\
        end axionJoinPaths"
    )
}

#[cfg(target_os = "macos")]
fn parse_macos_dialog_paths(stdout: &[u8]) -> Vec<std::path::PathBuf> {
    String::from_utf8_lossy(stdout)
        .trim()
        .split('\n')
        .filter_map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(entry))
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn applescript_string_literal(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn register_builtin_events(builder: &mut BridgeBindingsBuilder, allowed_events: &[String]) {
    if allowed_events.iter().any(|event| event == "app.log") {
        builder.register_event("app.log", |_context, _request| Ok(()));
    }
}

fn execute_window_control_json(
    window_control: &WindowControlHandle,
    target_window_id: Option<&str>,
    request: WindowControlRequest,
) -> Result<String, String> {
    match window_control.execute(target_window_id, request)? {
        WindowControlResponse::AppExit { .. } => {
            Err("window control backend returned an unexpected app exit response".to_owned())
        }
        WindowControlResponse::CloseRequested { request_id, window } => Ok(format!(
            "{{\"pending\":true,\"requestId\":{},\"window\":{}}}",
            json_string_literal(&request_id),
            window_state_json(&window)
        )),
        WindowControlResponse::ClosePrevented {
            request_id,
            window_id,
        } => Ok(format!(
            "{{\"prevented\":true,\"requestId\":{},\"windowId\":{}}}",
            json_string_literal(&request_id),
            json_string_literal(&window_id)
        )),
        WindowControlResponse::State(state) => Ok(window_state_json(&state)),
        WindowControlResponse::List(states) => Ok(window_state_list_json(&states)),
    }
}

fn execute_app_exit_json(window_control: &WindowControlHandle) -> Result<String, String> {
    match window_control.execute(None, WindowControlRequest::ExitApp)? {
        WindowControlResponse::AppExit {
            request_id,
            window_count,
            request_count,
        } => Ok(format!(
            "{{\"pending\":true,\"requestId\":{},\"windowCount\":{window_count},\"requestCount\":{request_count}}}",
            json_string_literal(&request_id)
        )),
        WindowControlResponse::CloseRequested { .. }
        | WindowControlResponse::ClosePrevented { .. }
        | WindowControlResponse::State(_)
        | WindowControlResponse::List(_) => {
            Err("window control backend returned an unexpected non-exit response".to_owned())
        }
    }
}

fn current_window_state(
    window_control: &WindowControlHandle,
    context: &CommandContext,
    target_window_id: Option<&str>,
) -> Result<WindowStateSnapshot, String> {
    match window_control.execute(target_window_id, WindowControlRequest::GetState) {
        Ok(WindowControlResponse::State(state)) => Ok(state),
        Ok(WindowControlResponse::List(_)) => {
            Err("window control backend returned an unexpected list response".to_owned())
        }
        Ok(
            WindowControlResponse::AppExit { .. }
            | WindowControlResponse::CloseRequested { .. }
            | WindowControlResponse::ClosePrevented { .. },
        ) => Err("window control backend returned an unexpected app exit response".to_owned()),
        Err(_) if target_window_id.is_none_or(|target| target == context.window.id) => {
            Ok(WindowStateSnapshot {
                id: context.window.id.clone(),
                title: context.window.title.clone(),
                width: context.window.width,
                height: context.window.height,
                resizable: context.window.resizable,
                visible: context.window.visible,
                focused: false,
            })
        }
        Err(error) => Err(error),
    }
}

fn window_state_list_json(states: &[WindowStateSnapshot]) -> String {
    let entries = states
        .iter()
        .map(window_state_json)
        .collect::<Vec<_>>()
        .join(",");

    format!("{{\"windows\":[{entries}]}}")
}

fn window_state_json(state: &WindowStateSnapshot) -> String {
    format!(
        "{{\"id\":{},\"title\":{},\"width\":{},\"height\":{},\"resizable\":{},\"visible\":{},\"focused\":{}}}",
        json_string_literal(&state.id),
        json_string_literal(&state.title),
        state.width,
        state.height,
        state.resizable,
        state.visible,
        state.focused,
    )
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
        app.config().capabilities.get(window_id),
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
    run_launch_request(launch_request)
}

pub fn run_launch_request(launch_request: RuntimeLaunchRequest) -> Result<(), RuntimeError> {
    #[cfg(not(feature = "servo-runtime"))]
    {
        let _ = launch_request;
        Err(RuntimeError::ServoRuntimeDisabled)
    }

    #[cfg(feature = "servo-runtime")]
    {
        let window_bindings = launch_request
            .window_bindings
            .into_iter()
            .map(|binding| axion_window_winit::WindowBridgeBinding {
                window_id: binding.window_id,
                bridge_token: binding.bridge_token,
                command_context: binding.command_context,
                bridge_bindings: binding.bridge_bindings,
                security_policy: binding.security_policy,
                window_control: binding.window_control,
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
                launch_request.close_timeout_ms,
                url,
            )
            .map_err(RuntimeError::from),
            RuntimeLaunchTarget::AppProtocol(app_protocol) => axion_window_winit::run_app_protocol(
                launch_request.app_name,
                launch_request.identifier,
                launch_request.mode,
                launch_request.windows,
                window_bindings,
                launch_request.close_timeout_ms,
                app_protocol.initial_url,
                app_protocol.resolver,
            )
            .map_err(RuntimeError::from),
        }
    }
}

pub fn reload_window(
    window_control: &WindowControlHandle,
    target_window_id: Option<&str>,
) -> Result<(), String> {
    window_control
        .execute(target_window_id, WindowControlRequest::Reload)
        .map(|_| ())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use std::fs;
    use std::future::Future;
    use std::path::PathBuf;
    use std::pin::pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use std::time::{SystemTime, UNIX_EPOCH};

    use axion_core::{
        BuildConfig, Builder, CapabilityConfig, DevServerConfig, DialogConfig, LifecycleConfig,
        NativeConfig, WindowConfig,
    };
    use axion_protocol::ProtocolError;
    use url::Url;

    use super::{
        AppProtocolLaunch, BridgeRequest, DiagnosticSeverity, DiagnosticsReport,
        DiagnosticsWindowReport, DialogBackendKind, DialogRequest, DialogRequestKind,
        PanicReportConfig, RuntimeBridgeBindingsBuilder, RuntimeError, RuntimeLaunchTarget,
        RuntimePlugin, WindowControlHandle, WindowControlRequest, WindowControlResponse,
        WindowStateSnapshot, current_window_state, diagnostic_report, execute_dialog_request,
        format_panic_report_body, launch_request, launch_request_with_plugins, panic_report_path,
        reload_window,
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

    fn block_on<F>(future: F) -> F::Output
    where
        F: Future,
    {
        fn noop_raw_waker() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                noop_raw_waker()
            }
            fn wake(_: *const ()) {}
            fn wake_by_ref(_: *const ()) {}
            fn drop(_: *const ()) {}

            RawWaker::new(
                std::ptr::null(),
                &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
            )
        }

        let waker = unsafe { Waker::from_raw(noop_raw_waker()) };
        let mut context = Context::from_waker(&waker);
        let mut future = pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
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
                command: None,
                cwd: None,
                timeout_ms: None,
            });
        }

        builder = builder.with_capability(
            "main",
            CapabilityConfig {
                profiles: Vec::new(),
                commands: vec!["app.ping".to_owned()],
                events: vec!["app.log".to_owned()],
                protocols: vec!["axion".to_owned()],
                allowed_navigation_origins: Vec::new(),
                allow_remote_navigation: false,
                ..Default::default()
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
                    profiles: Vec::new(),
                    commands: vec!["app.ping".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                    ..Default::default()
                },
            )
            .with_capability(
                "settings",
                CapabilityConfig {
                    profiles: Vec::new(),
                    commands: vec!["window.info".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: vec!["https://docs.example".to_owned()],
                    allow_remote_navigation: true,
                    ..Default::default()
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
                    profiles: Vec::new(),
                    commands: vec!["app.ping".to_owned()],
                    events: vec!["app.log".to_owned()],
                    protocols: Vec::new(),
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                    ..Default::default()
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
                    profiles: Vec::new(),
                    commands: vec!["app.ping".to_owned(), command.to_owned()],
                    events: vec!["app.log".to_owned(), "plugin.event".to_owned()],
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                    ..Default::default()
                },
            )
            .build()
            .expect("test app should build")
    }

    fn app_with_native_commands() -> axion_core::App {
        let (frontend_dist, entry) = frontend_fixture("native-commands");
        Builder::new()
            .with_name("axion-runtime-test")
            .with_window(WindowConfig::main("Runtime Test"))
            .with_build(BuildConfig::new(frontend_dist, entry))
            .with_capability(
                "main",
                CapabilityConfig {
                    profiles: Vec::new(),
                    commands: vec![
                        "app.version".to_owned(),
                        "clipboard.read_text".to_owned(),
                        "clipboard.write_text".to_owned(),
                        "fs.read_text".to_owned(),
                        "fs.write_text".to_owned(),
                        "dialog.open".to_owned(),
                        "dialog.save".to_owned(),
                    ],
                    events: Vec::new(),
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                    ..Default::default()
                },
            )
            .build()
            .expect("test app should build")
    }

    fn app_with_system_dialog_backend() -> axion_core::App {
        let (frontend_dist, entry) = frontend_fixture("system-dialog");
        Builder::new()
            .with_name("axion-runtime-test")
            .with_window(WindowConfig::main("Runtime Test"))
            .with_build(BuildConfig::new(frontend_dist, entry))
            .with_native(NativeConfig::new().with_dialog(DialogConfig::system()))
            .with_capability(
                "main",
                CapabilityConfig {
                    profiles: Vec::new(),
                    commands: vec!["dialog.open".to_owned()],
                    events: Vec::new(),
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                    ..Default::default()
                },
            )
            .build()
            .expect("test app should build")
    }

    fn app_with_window_control_commands() -> axion_core::App {
        let (frontend_dist, entry) = frontend_fixture("window-controls");
        Builder::new()
            .with_name("axion-runtime-test")
            .with_window(WindowConfig::main("Runtime Test"))
            .with_build(BuildConfig::new(frontend_dist, entry))
            .with_capability(
                "main",
                CapabilityConfig {
                    profiles: Vec::new(),
                    commands: vec![
                        "app.exit".to_owned(),
                        "window.list".to_owned(),
                        "window.info".to_owned(),
                        "window.show".to_owned(),
                        "window.hide".to_owned(),
                        "window.close".to_owned(),
                        "window.confirm_close".to_owned(),
                        "window.prevent_close".to_owned(),
                        "window.reload".to_owned(),
                        "window.focus".to_owned(),
                        "window.set_title".to_owned(),
                        "window.set_size".to_owned(),
                    ],
                    events: Vec::new(),
                    protocols: vec!["axion".to_owned()],
                    allowed_navigation_origins: Vec::new(),
                    allow_remote_navigation: false,
                    ..Default::default()
                },
            )
            .build()
            .expect("test app should build")
    }

    #[derive(Clone)]
    struct FakeWindowControlExecutor;

    impl axion_bridge::WindowControlExecutor for FakeWindowControlExecutor {
        fn execute(
            &self,
            target_window_id: Option<&str>,
            request: WindowControlRequest,
        ) -> Result<WindowControlResponse, String> {
            if matches!(request, WindowControlRequest::ListStates) {
                return Ok(WindowControlResponse::List(vec![
                    fake_window_state("main").expect("main fake state should exist"),
                    fake_window_state("settings").expect("settings fake state should exist"),
                ]));
            }
            if matches!(request, WindowControlRequest::ExitApp) {
                return Ok(WindowControlResponse::AppExit {
                    request_id: "test-exit".to_owned(),
                    window_count: 2,
                    request_count: 2,
                });
            }

            let target_window_id = target_window_id.unwrap_or("main");
            let mut state = fake_window_state(target_window_id)
                .ok_or_else(|| format!("window '{target_window_id}' is unavailable"))?;
            match request {
                WindowControlRequest::GetState
                | WindowControlRequest::Show
                | WindowControlRequest::Close
                | WindowControlRequest::ConfirmClose { .. }
                | WindowControlRequest::Reload => {}
                WindowControlRequest::PreventClose { request_id } => {
                    return Ok(WindowControlResponse::ClosePrevented {
                        request_id,
                        window_id: target_window_id.to_owned(),
                    });
                }
                WindowControlRequest::Hide => state.visible = false,
                WindowControlRequest::Focus => state.focused = true,
                WindowControlRequest::SetTitle { title } => state.title = title,
                WindowControlRequest::SetSize { width, height } => {
                    state.width = width;
                    state.height = height;
                }
                WindowControlRequest::ListStates | WindowControlRequest::ExitApp => unreachable!(),
            }

            Ok(WindowControlResponse::State(state))
        }
    }

    fn fake_window_state(window_id: &str) -> Option<WindowStateSnapshot> {
        match window_id {
            "main" => Some(WindowStateSnapshot {
                id: "main".to_owned(),
                title: "Runtime Test".to_owned(),
                width: 960,
                height: 720,
                resizable: true,
                visible: true,
                focused: false,
            }),
            "settings" => Some(WindowStateSnapshot {
                id: "settings".to_owned(),
                title: "Settings".to_owned(),
                width: 720,
                height: 540,
                resizable: true,
                visible: true,
                focused: false,
            }),
            _ => None,
        }
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
                command: None,
                cwd: None,
                timeout_ms: None,
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
        assert_eq!(binding.bridge_bindings.startup_events.len(), 3);
        assert!(
            binding
                .bridge_bindings
                .startup_events
                .iter()
                .any(|event| event.name == "window.created")
        );
        assert!(
            binding
                .bridge_bindings
                .startup_events
                .iter()
                .any(|event| event.name == "window.ready")
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
        assert_eq!(binding.bridge_bindings.startup_events.len(), 3);
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
    fn builtin_native_commands_are_registered_by_capability() {
        let request = launch_request(&app_with_native_commands(), axion_core::RunMode::Production)
            .expect("launch request should build");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");

        assert_eq!(
            binding.bridge_bindings.command_registry.command_names(),
            vec![
                "app.version".to_owned(),
                "clipboard.read_text".to_owned(),
                "clipboard.write_text".to_owned(),
                "dialog.open".to_owned(),
                "dialog.save".to_owned(),
                "fs.read_text".to_owned(),
                "fs.write_text".to_owned(),
            ]
        );

        let version = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("app.version", "null"),
        ))
        .expect("app.version should dispatch");
        assert!(version.contains("\"framework\":\"axion\""));
        assert!(version.contains("\"release\":\"v0.1.25.0\""));

        let dialog_open = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("dialog.open", "{\"title\":\"Open fixture\"}"),
        ))
        .expect("dialog.open should dispatch");
        assert_eq!(
            dialog_open,
            "{\"canceled\":true,\"path\":null,\"paths\":null,\"backend\":\"headless\"}"
        );

        let clipboard_write = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("clipboard.write_text", "{\"text\":\"hello clipboard\"}"),
        ))
        .expect("clipboard.write_text should dispatch");
        assert!(clipboard_write.contains("\"bytes\":15"));
        assert!(clipboard_write.contains("\"backend\":\"memory\""));

        let clipboard_read = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("clipboard.read_text", "null"),
        ))
        .expect("clipboard.read_text should dispatch");
        assert!(clipboard_read.contains("\"text\":\"hello clipboard\""));
    }

    #[test]
    fn dialog_command_rejects_invalid_save_multiple_payload() {
        let request = launch_request(&app_with_native_commands(), axion_core::RunMode::Production)
            .expect("launch request should build");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");

        let error = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("dialog.save", "{\"multiple\":true}"),
        ))
        .expect_err("dialog.save should reject multiple=true");

        assert!(format!("{error:?}").contains("does not support 'multiple=true'"));
    }

    #[test]
    fn json_string_literal_escapes_custom_command_output() {
        assert_eq!(
            super::json_string_literal("hello \"axion\"\n"),
            "\"hello \\\"axion\\\"\\n\""
        );
        assert_eq!(super::json_string_literal("a\u{0007}b"), "\"a\\u0007b\"");
    }

    #[test]
    fn diagnostics_report_serializes_stable_schema() {
        let report = DiagnosticsReport {
            source: "unit-test".to_owned(),
            exported_at_unix_seconds: Some(42),
            manifest_path: Some(std::path::PathBuf::from("axion.toml")),
            app_name: "diagnostics-test".to_owned(),
            identifier: Some("dev.axion.diagnostics-test".to_owned()),
            version: Some("0.1.0".to_owned()),
            description: None,
            authors: vec!["Axion".to_owned()],
            homepage: None,
            mode: Some("production".to_owned()),
            window_count: 1,
            windows: vec![DiagnosticsWindowReport {
                id: "main".to_owned(),
                title: "Main".to_owned(),
                bridge_enabled: true,
                configured_profiles: vec!["app-info".to_owned()],
                configured_commands: vec!["app.ping".to_owned()],
                configured_events: vec!["app.ready".to_owned()],
                configured_protocols: vec!["axion".to_owned()],
                runtime_command_count: 1,
                runtime_event_count: 1,
                host_events: vec!["app.ready".to_owned()],
                trusted_origins: Vec::new(),
                allowed_navigation_origins: Vec::new(),
                allow_remote_navigation: false,
            }],
            frontend_dist: Some(std::path::PathBuf::from("frontend")),
            entry: Some(std::path::PathBuf::from("frontend/index.html")),
            configured_dialog_backend: Some("headless".to_owned()),
            dialog_backend: Some("headless".to_owned()),
            configured_clipboard_backend: Some("memory".to_owned()),
            clipboard_backend: Some("memory".to_owned()),
            close_timeout_ms: Some(3000),
            icon: None,
            host_events: vec!["app.ready".to_owned()],
            staged_app_dir: Some(std::path::PathBuf::from("target/axion/app")),
            asset_manifest_path: Some(std::path::PathBuf::from("target/axion/app/assets.json")),
            artifacts_removed: Some(true),
            diagnostics: Some("{\"kind\":\"unit\"}".to_owned()),
            result: "ok".to_owned(),
        };
        let json = report.to_json();

        assert!(json.contains("\"schema\":\"axion.diagnostics-report.v1\""));
        assert!(json.contains("\"source\":\"unit-test\""));
        assert!(json.contains("\"manifest_path\":\"axion.toml\""));
        assert!(json.contains("\"close_timeout_ms\":3000"));
        assert!(json.contains("\"configured_profiles\":[\"app-info\"]"));
        assert!(json.contains("\"configured_commands\":[\"app.ping\"]"));
        assert!(json.contains("\"artifacts_removed\":true"));
        assert!(json.contains("\"diagnostics\":{\"kind\":\"unit\"}"));
    }

    #[test]
    fn builtin_fs_commands_round_trip_app_data_text() {
        let request = launch_request(&app_with_native_commands(), axion_core::RunMode::Production)
            .expect("launch request should build");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");

        let write = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new(
                "fs.write_text",
                "{\"path\":\"notes/hello.txt\",\"contents\":\"hello from axion\"}",
            ),
        ))
        .expect("fs.write_text should dispatch");
        assert!(write.contains("\"bytes\":16"));

        let read = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("fs.read_text", "{\"path\":\"notes/hello.txt\"}"),
        ))
        .expect("fs.read_text should dispatch");
        assert!(read.contains("\"contents\":\"hello from axion\""));
    }

    #[test]
    fn builtin_fs_commands_reject_path_escape() {
        let request = launch_request(&app_with_native_commands(), axion_core::RunMode::Production)
            .expect("launch request should build");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");

        let error = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new(
                "fs.write_text",
                "{\"path\":\"../escape.txt\",\"contents\":\"x\"}",
            ),
        ))
        .expect_err("path escape should be rejected");

        assert!(format!("{error:?}").contains("parent or root components"));
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
        assert_eq!(
            report.configured_dialog_backend,
            DialogBackendKind::Headless
        );
        assert_eq!(report.dialog_backend, DialogBackendKind::Headless);
        assert_eq!(report.close_timeout_ms, 3000);
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
                "window.ready".to_owned(),
                "window.close_requested".to_owned(),
                "window.close_prevented".to_owned(),
                "window.close_completed".to_owned(),
                "window.close_timed_out".to_owned(),
                "window.closed".to_owned(),
                "window.resized".to_owned(),
                "window.focused".to_owned(),
                "window.blurred".to_owned(),
                "window.moved".to_owned(),
                "window.redraw_failed".to_owned(),
                "app.exit_requested".to_owned(),
                "app.exit_prevented".to_owned(),
                "app.exit_completed".to_owned(),
            ]
        );
        assert_eq!(settings.startup_event_count, 3);
        assert_eq!(
            settings.lifecycle_events,
            vec![
                "window.created".to_owned(),
                "window.ready".to_owned(),
                "window.close_requested".to_owned(),
                "window.close_prevented".to_owned(),
                "window.close_completed".to_owned(),
                "window.close_timed_out".to_owned(),
                "window.closed".to_owned(),
                "window.resized".to_owned(),
                "window.focused".to_owned(),
                "window.blurred".to_owned(),
                "window.moved".to_owned(),
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
    fn launch_request_preserves_lifecycle_close_timeout() {
        let (frontend_dist, entry) = frontend_fixture("lifecycle-timeout");
        let app = Builder::new()
            .with_name("axion-runtime-test")
            .with_window(WindowConfig::main("Runtime Test"))
            .with_build(BuildConfig::new(frontend_dist, entry))
            .with_native(
                NativeConfig::new()
                    .with_lifecycle(LifecycleConfig::new().with_close_timeout_ms(1750)),
            )
            .build()
            .expect("test app should build");

        let request =
            launch_request(&app, axion_core::RunMode::Production).expect("request should build");
        let report = diagnostic_report(&app, axion_core::RunMode::Production);

        assert_eq!(request.close_timeout_ms, 1750);
        assert_eq!(report.close_timeout_ms, 1750);
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
                "window.ready".to_owned(),
                "window.close_requested".to_owned(),
                "window.close_prevented".to_owned(),
                "window.close_completed".to_owned(),
                "window.close_timed_out".to_owned(),
                "window.closed".to_owned(),
                "window.resized".to_owned(),
                "window.focused".to_owned(),
                "window.blurred".to_owned(),
                "window.moved".to_owned(),
                "window.redraw_failed".to_owned(),
                "app.exit_requested".to_owned(),
                "app.exit_prevented".to_owned(),
                "app.exit_completed".to_owned(),
            ]
        );
    }

    #[test]
    fn dialog_headless_backend_returns_canceled_response() {
        let response = execute_dialog_request(
            DialogBackendKind::Headless,
            DialogRequest {
                kind: DialogRequestKind::Save,
                title: Some("Save fixture".to_owned()),
                default_path: Some(PathBuf::from("notes.txt")),
                directory: false,
                multiple: false,
                filters: Vec::new(),
            },
        );

        assert!(response.canceled);
        assert_eq!(response.path, None);
        assert_eq!(response.paths, None);
        assert_eq!(response.backend, DialogBackendKind::Headless);
    }

    #[test]
    fn dialog_request_parses_multiple_directory_and_filters() {
        let request = DialogRequest::from_payload(
            DialogRequestKind::Open,
            r#"{
                "title":"Select fixtures",
                "defaultPath":"fixtures",
                "directory":true,
                "multiple":true,
                "filters":[{"name":"Images","extensions":["png","jpg"]}]
            }"#,
        )
        .expect("dialog request should parse");

        assert_eq!(request.title.as_deref(), Some("Select fixtures"));
        assert_eq!(request.default_path, Some(PathBuf::from("fixtures")));
        assert!(request.directory);
        assert!(request.multiple);
        assert_eq!(request.filters.len(), 1);
        assert_eq!(request.filters[0].name, "Images");
        assert_eq!(request.filters[0].extensions, vec!["png", "jpg"]);
    }

    #[test]
    fn dialog_request_rejects_invalid_filter_shape() {
        let error = DialogRequest::from_payload(
            DialogRequestKind::Open,
            r#"{"filters":[{"name":"Images","extensions":"png"}]}"#,
        )
        .expect_err("invalid filters should fail");

        assert!(error.to_string().contains("string array 'extensions'"));
    }

    #[test]
    fn system_dialog_backend_resolves_per_platform() {
        let report = diagnostic_report(
            &app_with_system_dialog_backend(),
            axion_core::RunMode::Production,
        );

        assert_eq!(report.configured_dialog_backend, DialogBackendKind::System);
        #[cfg(target_os = "macos")]
        assert_eq!(report.dialog_backend, DialogBackendKind::System);
        #[cfg(not(target_os = "macos"))]
        {
            assert_eq!(report.dialog_backend, DialogBackendKind::SystemUnavailable);
            assert!(
                report
                    .issues
                    .iter()
                    .any(|issue| issue.message.contains("resolves to 'system-unavailable'"))
            );
        }
    }

    #[test]
    fn window_control_commands_dispatch_through_control_handle() {
        let request = launch_request(
            &app_with_window_control_commands(),
            axion_core::RunMode::Production,
        )
        .expect("launch request should build");
        let binding = request
            .window_bindings
            .first()
            .expect("main window binding should exist");
        binding
            .window_control
            .install_executor(Arc::new(FakeWindowControlExecutor));

        let info = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.info", "null"),
        ))
        .expect("window.info should dispatch");
        assert!(info.contains("\"id\":\"main\""));
        assert!(info.contains("\"focused\":false"));

        let windows = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.list", "null"),
        ))
        .expect("window.list should dispatch");
        assert!(windows.contains("\"windows\":["));
        assert!(windows.contains("\"id\":\"main\""));
        assert!(windows.contains("\"id\":\"settings\""));

        let targeted_info = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.info", "{\"target\":\"settings\"}"),
        ))
        .expect("targeted window.info should dispatch");
        assert!(targeted_info.contains("\"id\":\"settings\""));

        let hidden = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.hide", "null"),
        ))
        .expect("window.hide should dispatch");
        assert!(hidden.contains("\"visible\":false"));

        let closed = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.close", "{\"target\":\"settings\"}"),
        ))
        .expect("window.close should dispatch");
        assert!(closed.contains("\"id\":\"settings\""));

        let renamed = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new(
                "window.set_title",
                "{\"target\":\"settings\",\"title\":\"Renamed\"}",
            ),
        ))
        .expect("window.set_title should dispatch");
        assert!(renamed.contains("\"id\":\"settings\""));
        assert!(renamed.contains("\"title\":\"Renamed\""));

        let resized = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.set_size", "{\"width\":640,\"height\":480}"),
        ))
        .expect("window.set_size should dispatch");
        assert!(resized.contains("\"width\":640"));
        assert!(resized.contains("\"height\":480"));

        let reloaded = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.reload", "null"),
        ))
        .expect("window.reload should dispatch");
        assert!(reloaded.contains("\"id\":\"main\""));

        reload_window(&binding.window_control, Some("main"))
            .expect("reload helper should dispatch");

        let exit = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("app.exit", "null"),
        ))
        .expect("app.exit should dispatch");
        assert_eq!(
            exit,
            "{\"pending\":true,\"requestId\":\"test-exit\",\"windowCount\":2,\"requestCount\":2}"
        );

        let prevented = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.prevent_close", "{\"requestId\":\"test-close\"}"),
        ))
        .expect("window.prevent_close should dispatch");
        assert!(prevented.contains("\"prevented\":true"));

        let confirmed = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.confirm_close", "{\"requestId\":\"test-close\"}"),
        ))
        .expect("window.confirm_close should dispatch");
        assert!(confirmed.contains("\"id\":\"main\""));

        let missing = block_on(binding.bridge_bindings.command_registry.dispatch(
            &binding.command_context,
            &BridgeRequest::new("window.info", "{\"target\":\"missing\"}"),
        ))
        .expect_err("missing target should fail");
        assert!(matches!(
            missing,
            axion_bridge::CommandDispatchError::Handler(message)
                if message.contains("unavailable")
        ));
    }

    #[test]
    fn window_info_falls_back_to_command_context_without_backend() {
        let state = current_window_state(
            &WindowControlHandle::new(),
            &axion_bridge::CommandContext {
                app_name: "axion-runtime-test".to_owned(),
                identifier: None,
                version: None,
                description: None,
                authors: Vec::new(),
                homepage: None,
                mode: axion_bridge::BridgeRunMode::Production,
                window: axion_bridge::WindowCommandContext {
                    id: "main".to_owned(),
                    title: "Runtime Test".to_owned(),
                    width: 960,
                    height: 720,
                    resizable: true,
                    visible: true,
                },
            },
            None,
        );
        let state = state.expect("window.info fallback should succeed");

        assert_eq!(state.id, "main");
        assert_eq!(state.title, "Runtime Test");
        assert_eq!(state.width, 960);
        assert!(state.visible);
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
                "window.ready".to_owned(),
                "window.close_requested".to_owned(),
                "window.close_prevented".to_owned(),
                "window.close_completed".to_owned(),
                "window.close_timed_out".to_owned(),
                "window.closed".to_owned(),
                "window.resized".to_owned(),
                "window.focused".to_owned(),
                "window.blurred".to_owned(),
                "window.moved".to_owned(),
                "window.redraw_failed".to_owned(),
            ]
        );
    }

    #[test]
    fn app_lifecycle_event_names_are_stable() {
        assert_eq!(
            super::app_lifecycle_event_names(),
            vec![
                "app.exit_requested".to_owned(),
                "app.exit_prevented".to_owned(),
                "app.exit_completed".to_owned(),
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

/// Escape a Rust string as a JSON string literal for command responses.
///
/// Custom command handlers return JSON text today. Use this helper when building
/// small JSON responses without adding an application-level JSON dependency.
pub fn json_string_literal(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() + 2);
    encoded.push('"');
    for character in value.chars() {
        match character {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            character if character.is_control() => {
                encoded.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => encoded.push(character),
        }
    }
    encoded.push('"');
    encoded
}

fn optional_json_string_literal(value: Option<&str>) -> String {
    value
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_owned())
}

fn optional_json_path(path: Option<&std::path::Path>) -> String {
    path.map(|path| json_string_literal(&path.display().to_string()))
        .unwrap_or_else(|| "null".to_owned())
}

fn optional_json_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_owned())
}

fn optional_json_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
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

fn json_string_array_literal(values: &[String]) -> String {
    let entries = values
        .iter()
        .map(|value| json_string_literal(value))
        .collect::<Vec<_>>()
        .join(",");

    format!("[{entries}]")
}

fn json_bool_field(payload: &str, field: &str) -> Option<bool> {
    let value = json_field_value(payload, field)?;
    if value.starts_with("true") {
        Some(true)
    } else if value.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn json_u32_field(payload: &str, field: &str) -> Option<u32> {
    let value = json_field_value(payload, field)?;
    let end = value
        .find(|character: char| !(character.is_ascii_digit()))
        .unwrap_or(value.len());
    value[..end].parse().ok()
}

fn json_string_field(payload: &str, field: &str) -> Option<String> {
    let mut search_start = 0;

    while let Some(after_colon) = json_field_value_from(payload, field, &mut search_start) {
        if let Some(value) = parse_json_string(after_colon) {
            return Some(value);
        }
    }

    None
}

fn json_string_array_field(payload: &str, field: &str) -> Option<Vec<String>> {
    let array = extract_json_array(json_field_value(payload, field)?)?;
    let entries = split_top_level_json_array(array)?;
    let mut values = Vec::new();
    for entry in entries {
        values.push(parse_json_string(entry.trim())?);
    }
    Some(values)
}

fn dialog_filters_field(
    payload: &str,
    field: &str,
) -> Result<Vec<DialogFilter>, DialogRequestError> {
    let Some(value) = json_field_value(payload, field) else {
        return Ok(Vec::new());
    };
    if value.starts_with("null") {
        return Ok(Vec::new());
    }

    let Some(array) = extract_json_array(value) else {
        return Err(DialogRequestError::InvalidPayload {
            message: "dialog filters must be a JSON array".to_owned(),
        });
    };
    let Some(entries) = split_top_level_json_array(array) else {
        return Err(DialogRequestError::InvalidPayload {
            message: "dialog filters must be a valid JSON array".to_owned(),
        });
    };

    let mut filters = Vec::new();
    for entry in entries {
        let entry = entry.trim();
        if !entry.starts_with('{') {
            return Err(DialogRequestError::InvalidPayload {
                message: "dialog filters must be objects with 'name' and 'extensions'".to_owned(),
            });
        }
        let name =
            json_string_field(entry, "name").ok_or_else(|| DialogRequestError::InvalidPayload {
                message: "dialog filters require a string 'name'".to_owned(),
            })?;
        let extensions = json_string_array_field(entry, "extensions").ok_or_else(|| {
            DialogRequestError::InvalidPayload {
                message: "dialog filters require a string array 'extensions'".to_owned(),
            }
        })?;
        filters.push(DialogFilter { name, extensions });
    }

    Ok(filters)
}

fn json_field_value<'a>(payload: &'a str, field: &str) -> Option<&'a str> {
    let mut search_start = 0;
    json_field_value_from(payload, field, &mut search_start)
}

fn json_field_value_from<'a>(
    payload: &'a str,
    field: &str,
    search_start: &mut usize,
) -> Option<&'a str> {
    let field_pattern = format!("\"{}\"", field);

    let relative_position = payload[*search_start..].find(&field_pattern)?;
    let field_start = *search_start + relative_position + field_pattern.len();
    let after_field = payload[field_start..].trim_start();
    let after_colon = after_field.strip_prefix(':')?.trim_start();
    *search_start = field_start;
    Some(after_colon)
}

fn extract_json_array(input: &str) -> Option<&str> {
    extract_json_enclosed(input, '[', ']')
}

fn extract_json_enclosed(input: &str, open: char, close: char) -> Option<&str> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with(open) {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, character) in trimmed.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match character {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            value if value == open => depth += 1,
            value if value == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&trimmed[..=index]);
                }
            }
            _ => {}
        }
    }

    None
}

fn split_top_level_json_array(input: &str) -> Option<Vec<&str>> {
    let trimmed = input.trim();
    if trimmed == "[]" {
        return Some(Vec::new());
    }
    let inner = trimmed.strip_prefix('[')?.strip_suffix(']')?;
    let inner = inner.trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }

    let mut values = Vec::new();
    let mut start = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, character) in inner.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match character {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ',' if bracket_depth == 0 && brace_depth == 0 => {
                values.push(inner[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }

    values.push(inner[start..].trim());
    Some(values)
}

fn parse_json_string(input: &str) -> Option<String> {
    let mut chars = input.chars();
    if chars.next()? != '"' {
        return None;
    }

    let mut value = String::new();
    let mut escaped = false;
    for character in chars {
        if escaped {
            match character {
                '"' => value.push('"'),
                '\\' => value.push('\\'),
                '/' => value.push('/'),
                'b' => value.push('\u{0008}'),
                'f' => value.push('\u{000c}'),
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                'u' => return None,
                other => value.push(other),
            }
            escaped = false;
            continue;
        }

        match character {
            '\\' => escaped = true,
            '"' => return Some(value),
            other => value.push(other),
        }
    }

    None
}

fn resolve_app_data_path(
    app_data_dir: &std::path::Path,
    relative_path: &str,
    create_parent: bool,
) -> Result<std::path::PathBuf, String> {
    let relative = std::path::Path::new(relative_path);
    if relative_path.trim().is_empty() || relative.is_absolute() {
        return Err("app data path must be a non-empty relative path".to_owned());
    }

    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("app data path must not contain parent or root components".to_owned());
        }
    }

    if create_parent {
        std::fs::create_dir_all(app_data_dir)
            .map_err(|error| format!("failed to create app data directory: {error}"))?;
    }
    let canonical_base = app_data_dir
        .canonicalize()
        .map_err(|error| format!("failed to access app data directory: {error}"))?;
    let path = app_data_dir.join(relative);

    if create_parent {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create app data parent directory: {error}"))?;
            let canonical_parent = parent
                .canonicalize()
                .map_err(|error| format!("failed to access app data parent directory: {error}"))?;
            if !canonical_parent.starts_with(&canonical_base) {
                return Err("app data path escapes the app data directory".to_owned());
            }
        }
    }

    if path
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err("app data path must not be a symlink".to_owned());
    }

    if !create_parent {
        let canonical_path = path
            .canonicalize()
            .map_err(|error| format!("failed to access app data file: {error}"))?;
        if !canonical_path.starts_with(&canonical_base) {
            return Err("app data path escapes the app data directory".to_owned());
        }
    }

    Ok(path)
}
