use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use axion_core::{App, AppConfig, Builder, RunMode};
use axion_runtime::json_string_literal;

use crate::cli::DevArgs;
use crate::error::AxionCliError;

const DEFAULT_DEV_SERVER_TIMEOUT_MS: u64 = 15_000;
const DEV_SERVER_POLL_INTERVAL_MS: u64 = 100;
const WATCH_POLL_INTERVAL_MS: u64 = 500;
const WATCH_DEBOUNCE_MS: u64 = 250;

pub fn run(args: DevArgs) -> Result<(), AxionCliError> {
    let config = axion_manifest::load_app_config_from_path(&args.manifest_path)?;
    let app = Builder::new().apply_config(config).build()?;
    let frontend_command = frontend_command_plan(&args, app.config(), &args.manifest_path)?;
    let dev_events = DevEventOutput::new(&args)?;
    let mut frontend_process = None;
    let mut frontend_command_lines = Vec::new();
    let mut frontend_wait_result = None;

    if let Some(plan) = frontend_command {
        let mut process = FrontendProcess::spawn(&plan)?;
        let wait_result = wait_for_dev_server(app.config(), &mut process, plan.timeout_ms);
        frontend_command_lines.extend(process.diagnostic_lines(&wait_result));
        frontend_process = Some(process);
        frontend_wait_result = Some(wait_result);
    }

    let dev_server_status = dev_server_status(app.config());
    let packaged_fallback_status = packaged_fallback_status(&app);
    let launch_mode = select_launch_mode_with_packaged_fallback(
        &dev_server_status,
        &packaged_fallback_status,
        args.fallback_packaged,
    );

    println!("Axion development diagnostics");
    for line in dev_diagnostic_lines(
        &app,
        &args.manifest_path,
        &dev_server_status,
        &packaged_fallback_status,
        args.fallback_packaged,
    ) {
        println!("{line}");
    }
    if frontend_command_lines.is_empty() {
        println!("frontend_command: not configured");
    } else {
        for line in frontend_command_lines {
            println!("{line}");
        }
    }
    for line in dev_option_lines(&args) {
        println!("{line}");
    }

    if let Some(wait_result) = &frontend_wait_result {
        if !args.fallback_packaged && !matches!(wait_result, DevServerWaitResult::Reachable) {
            let _ = std::io::stdout().flush();
            return Err(frontend_wait_error(wait_result).into());
        }
    }

    if args.launch {
        let launch_mode = launch_mode?;
        let mut launch_count = 1usize;
        loop {
            println!("launch_summary:");
            for line in launch_summary_lines(
                &app,
                &dev_server_status,
                &packaged_fallback_status,
                launch_mode,
            ) {
                println!("{line}");
            }

            let launch_request = axion_runtime::launch_request(&app, launch_mode)?;
            let reload_targets = reload_targets_from_launch_request(&launch_request);
            let watch_guard =
                DevWatchGuard::spawn(&args, app.config(), reload_targets, dev_events.clone())?;
            axion_runtime::run_launch_request(launch_request)?;
            let restart_requested = watch_guard
                .as_ref()
                .is_some_and(DevWatchGuard::restart_requested);
            drop(watch_guard);
            if args.restart_on_change && restart_requested {
                launch_count = launch_count.saturating_add(1);
                println!("restart_applied: launch={launch_count}; reason=frontend assets changed");
                dev_events.emit(&DevEvent::RestartApplied {
                    launch: launch_count,
                    reason: "frontend assets changed".to_owned(),
                });
                continue;
            }
            break;
        }
        drop(frontend_process);
        return Ok(());
    }

    let plan_mode = launch_mode
        .as_ref()
        .copied()
        .unwrap_or(RunMode::Development);
    let plan = app.runtime_plan(plan_mode);

    println!("runtime_plan:");
    println!("{plan}");

    run_foreground_watch_if_requested(&args, app.config(), dev_events)?;

    drop(frontend_process);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DevServerStatus {
    Unconfigured,
    InvalidEndpoint { url: String },
    Unreachable { url: String },
    Reachable { url: String },
}

impl DevServerStatus {
    fn summary(&self) -> String {
        match self {
            Self::Unconfigured => "unconfigured".to_owned(),
            Self::InvalidEndpoint { url } => format!("invalid endpoint ({url})"),
            Self::Unreachable { url } => format!("unreachable ({url})"),
            Self::Reachable { url } => format!("reachable ({url})"),
        }
    }

    fn launch_error(&self) -> std::io::Error {
        let message = match self {
            Self::Unconfigured => {
                "dev server is not configured; add [dev] url = \"http://127.0.0.1:3000\", start your frontend server, or pass --fallback-packaged to launch packaged assets".to_owned()
            }
            Self::InvalidEndpoint { url } => {
                format!(
                    "dev server URL does not include a usable host and port: {url}; fix [dev].url, or pass --fallback-packaged to launch packaged assets"
                )
            }
            Self::Unreachable { url } => {
                format!(
                    "dev server is not reachable at {url}; start the frontend dev server, check [dev].url, or pass --fallback-packaged to launch packaged assets"
                )
            }
            Self::Reachable { .. } => "dev server is reachable".to_owned(),
        };

        std::io::Error::other(message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PackagedFallbackStatus {
    Available { url: String },
    Unavailable { reason: String },
}

impl PackagedFallbackStatus {
    fn summary(&self, fallback_enabled: bool, selected: bool) -> String {
        match self {
            Self::Available { url } if selected => {
                format!("selected ({url})")
            }
            Self::Available { url } if fallback_enabled => {
                format!("enabled ({url})")
            }
            Self::Available { url } => {
                format!("disabled; available with --fallback-packaged ({url})")
            }
            Self::Unavailable { reason } if fallback_enabled => {
                format!("enabled but unavailable ({reason})")
            }
            Self::Unavailable { reason } => {
                format!("disabled; unavailable ({reason})")
            }
        }
    }

    fn launch_error(&self) -> std::io::Error {
        let message = match self {
            Self::Available { .. } => "packaged fallback is available".to_owned(),
            Self::Unavailable { reason } => {
                format!("packaged fallback is unavailable: {reason}")
            }
        };

        std::io::Error::other(message)
    }

    fn entry_url(&self) -> Option<&str> {
        match self {
            Self::Available { url } => Some(url),
            Self::Unavailable { .. } => None,
        }
    }

    fn selected_summary(&self, selected: bool) -> String {
        match self {
            Self::Available { url } if selected => format!("selected ({url})"),
            Self::Available { url } => format!("available ({url})"),
            Self::Unavailable { reason } => format!("unavailable ({reason})"),
        }
    }
}

#[cfg(test)]
fn select_launch_mode(
    dev_server_status: &DevServerStatus,
    fallback_packaged: bool,
) -> Result<RunMode, AxionCliError> {
    match dev_server_status {
        DevServerStatus::Reachable { .. } => Ok(RunMode::Development),
        _ if fallback_packaged => Ok(RunMode::Production),
        _ => Err(dev_server_status.launch_error().into()),
    }
}

fn select_launch_mode_with_packaged_fallback(
    dev_server_status: &DevServerStatus,
    packaged_fallback_status: &PackagedFallbackStatus,
    fallback_packaged: bool,
) -> Result<RunMode, AxionCliError> {
    match dev_server_status {
        DevServerStatus::Reachable { .. } => Ok(RunMode::Development),
        _ if fallback_packaged => match packaged_fallback_status {
            PackagedFallbackStatus::Available { .. } => Ok(RunMode::Production),
            PackagedFallbackStatus::Unavailable { .. } => {
                Err(packaged_fallback_status.launch_error().into())
            }
        },
        _ => Err(dev_server_status.launch_error().into()),
    }
}

fn dev_server_status(config: &AppConfig) -> DevServerStatus {
    dev_server_status_with(config, |host, port| {
        let Ok(addresses) = (host, port).to_socket_addrs() else {
            return None;
        };

        let timeout = Duration::from_millis(300);
        Some(
            addresses
                .into_iter()
                .any(|address| TcpStream::connect_timeout(&address, timeout).is_ok()),
        )
    })
}

fn packaged_fallback_status(app: &App) -> PackagedFallbackStatus {
    match axion_runtime::launch_request(app, RunMode::Production) {
        Ok(request) => match request.target {
            axion_runtime::RuntimeLaunchTarget::AppProtocol(app_protocol) => {
                PackagedFallbackStatus::Available {
                    url: app_protocol.initial_url.to_string(),
                }
            }
            axion_runtime::RuntimeLaunchTarget::DevServer(url) => {
                PackagedFallbackStatus::Unavailable {
                    reason: format!("production launch unexpectedly resolved to {url}"),
                }
            }
        },
        Err(error) => PackagedFallbackStatus::Unavailable {
            reason: error.to_string(),
        },
    }
}

fn dev_server_status_with(
    config: &AppConfig,
    is_reachable: impl Fn(&str, u16) -> Option<bool>,
) -> DevServerStatus {
    let Some(dev_server) = &config.dev else {
        return DevServerStatus::Unconfigured;
    };
    let url = dev_server.url.to_string();

    let Some(host) = dev_server.url.host_str() else {
        return DevServerStatus::InvalidEndpoint { url };
    };
    let Some(port) = dev_server.url.port_or_known_default() else {
        return DevServerStatus::InvalidEndpoint { url };
    };

    match is_reachable(host, port) {
        Some(true) => DevServerStatus::Reachable { url },
        Some(false) => DevServerStatus::Unreachable { url },
        None => DevServerStatus::InvalidEndpoint { url },
    }
}

pub(crate) fn dev_server_is_reachable(config: &AppConfig) -> bool {
    matches!(dev_server_status(config), DevServerStatus::Reachable { .. })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FrontendCommandPlan {
    command: String,
    cwd: PathBuf,
    timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DevServerWaitResult {
    Reachable,
    Timeout,
    ExitedEarly {
        status: Option<i32>,
        stderr_summary: String,
    },
}

struct FrontendProcess {
    child: Child,
    stderr: StderrCollector,
    command: String,
    cwd: PathBuf,
}

#[derive(Debug, Default)]
struct StderrCollector {
    last_line: Arc<Mutex<String>>,
}

impl StderrCollector {
    fn spawn(stderr: Option<ChildStderr>) -> Self {
        let collector = Self::default();
        let Some(stderr) = stderr else {
            return collector;
        };

        let last_line = Arc::clone(&collector.last_line);
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(mut summary) = last_line.lock() {
                    *summary = line.chars().take(240).collect();
                }
            }
        });

        collector
    }

    fn summary(&self) -> String {
        self.last_line
            .lock()
            .map(|summary| summary.clone())
            .unwrap_or_default()
    }
}

impl FrontendProcess {
    fn spawn(plan: &FrontendCommandPlan) -> Result<Self, AxionCliError> {
        let mut command = shell_command(&plan.command);
        command
            .current_dir(&plan.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|source| {
            std::io::Error::new(
                source.kind(),
                format!(
                    "failed to start frontend command {:?} in {}: {source}",
                    plan.command,
                    plan.cwd.display()
                ),
            )
        })?;
        let stderr = StderrCollector::spawn(child.stderr.take());

        Ok(Self {
            child,
            stderr,
            command: plan.command.clone(),
            cwd: plan.cwd.clone(),
        })
    }

    fn diagnostic_lines(&self, wait_result: &DevServerWaitResult) -> Vec<String> {
        frontend_diagnostic_lines(&self.command, &self.cwd, wait_result)
    }
}

impl Drop for FrontendProcess {
    fn drop(&mut self) {
        if let Ok(Some(_status)) = self.child.try_wait() {
            return;
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn frontend_command_plan(
    args: &DevArgs,
    config: &AppConfig,
    manifest_path: &Path,
) -> Result<Option<FrontendCommandPlan>, AxionCliError> {
    let Some(command) = args
        .frontend_command
        .clone()
        .or_else(|| config.dev.as_ref().and_then(|dev| dev.command.clone()))
    else {
        return Ok(None);
    };

    if config.dev.is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--frontend-command requires [dev] url in axion.toml",
        )
        .into());
    }

    let command = command.trim().to_owned();
    if command.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "frontend command must not be empty",
        )
        .into());
    }

    let cwd = args
        .frontend_cwd
        .clone()
        .or_else(|| config.dev.as_ref().and_then(|dev| dev.cwd.clone()))
        .unwrap_or_else(|| {
            manifest_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        });
    let timeout_ms = args
        .dev_server_timeout_ms
        .or_else(|| config.dev.as_ref().and_then(|dev| dev.timeout_ms))
        .unwrap_or(DEFAULT_DEV_SERVER_TIMEOUT_MS);

    Ok(Some(FrontendCommandPlan {
        command,
        cwd,
        timeout_ms,
    }))
}

fn frontend_diagnostic_lines(
    command: &str,
    cwd: &Path,
    wait_result: &DevServerWaitResult,
) -> Vec<String> {
    let mut lines = vec![
        format!("frontend_command: started ({command})"),
        format!("frontend_cwd: {}", cwd.display()),
    ];

    match wait_result {
        DevServerWaitResult::Reachable => {
            lines.push("dev_server_wait: reachable".to_owned());
        }
        DevServerWaitResult::Timeout => {
            lines.push("dev_server_wait: timeout".to_owned());
        }
        DevServerWaitResult::ExitedEarly {
            status,
            stderr_summary,
        } => {
            lines.push(format!(
                "dev_server_wait: exited early (status={})",
                status
                    .map(|status| status.to_string())
                    .unwrap_or_else(|| "unknown".to_owned())
            ));
            if !stderr_summary.is_empty() {
                lines.push(format!("frontend_stderr: {stderr_summary}"));
            }
        }
    }

    lines
}

fn frontend_wait_error(wait_result: &DevServerWaitResult) -> std::io::Error {
    let message = match wait_result {
        DevServerWaitResult::Reachable => "frontend dev server is reachable".to_owned(),
        DevServerWaitResult::Timeout => {
            "frontend command started but dev server did not become reachable before timeout"
                .to_owned()
        }
        DevServerWaitResult::ExitedEarly {
            status,
            stderr_summary,
        } => {
            let status = status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "unknown".to_owned());
            if stderr_summary.is_empty() {
                format!(
                    "frontend command exited before dev server became reachable (status={status})"
                )
            } else {
                format!(
                    "frontend command exited before dev server became reachable (status={status}): {stderr_summary}"
                )
            }
        }
    };

    std::io::Error::other(message)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchedFile {
    modified_millis: u128,
    len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchSnapshot {
    files: BTreeMap<PathBuf, WatchedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WatchChangeKind {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchChange {
    path: PathBuf,
    kind: WatchChangeKind,
}

struct DevWatchGuard {
    stop: Arc<AtomicBool>,
    restart_requested: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Clone)]
struct DevEventOutput {
    json_stdout: bool,
    log: Option<Arc<Mutex<fs::File>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DevEvent {
    WatchChange {
        kind: WatchChangeKind,
        path: PathBuf,
    },
    WatchError {
        message: String,
    },
    ReloadRequested {
        reason: String,
    },
    ReloadApplied {
        window_id: String,
    },
    ReloadDeferred {
        window_id: Option<String>,
        reason: String,
    },
    RestartRequired {
        window_id: String,
        reason: String,
    },
    RestartRequested {
        reason: String,
    },
    RestartExitRequested {
        window_count: usize,
        request_count: usize,
    },
    RestartDeferred {
        reason: String,
    },
    RestartApplied {
        launch: usize,
        reason: String,
    },
}

#[derive(Clone)]
struct ReloadTarget {
    window_id: String,
    window_control: axion_runtime::WindowControlHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReloadOutcome {
    Applied {
        window_id: String,
    },
    Deferred {
        window_id: Option<String>,
        reason: String,
    },
    RestartRequired {
        window_id: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RestartOutcome {
    Requested {
        window_count: usize,
        request_count: usize,
    },
    Deferred {
        reason: String,
    },
}

impl DevEventOutput {
    fn new(args: &DevArgs) -> Result<Self, AxionCliError> {
        let log = if let Some(path) = &args.event_log {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            Some(Arc::new(Mutex::new(fs::File::create(path)?)))
        } else {
            None
        };

        Ok(Self {
            json_stdout: args.json_events,
            log,
        })
    }

    fn emit(&self, event: &DevEvent) {
        let json = event.to_json();
        if self.json_stdout {
            println!("{json}");
        }
        if let Some(log) = &self.log {
            if let Ok(mut file) = log.lock() {
                let _ = writeln!(file, "{json}");
            }
        }
    }

    fn emit_many(&self, events: &[DevEvent]) {
        for event in events {
            self.emit(event);
        }
    }
}

impl DevEvent {
    fn event_name(&self) -> &'static str {
        match self {
            Self::WatchChange { .. } => "watch_change",
            Self::WatchError { .. } => "watch_error",
            Self::ReloadRequested { .. } => "reload_requested",
            Self::ReloadApplied { .. } => "reload_applied",
            Self::ReloadDeferred { .. } => "reload_deferred",
            Self::RestartRequired { .. } => "restart_required",
            Self::RestartRequested { .. } => "restart_requested",
            Self::RestartExitRequested { .. } => "restart_exit_requested",
            Self::RestartDeferred { .. } => "restart_deferred",
            Self::RestartApplied { .. } => "restart_applied",
        }
    }

    fn to_json(&self) -> String {
        let fields = match self {
            Self::WatchChange { kind, path } => format!(
                ",\"kind\":{},\"path\":{}",
                json_string_literal(watch_change_kind_label(kind)),
                json_string_literal(&path.display().to_string())
            ),
            Self::WatchError { message } => {
                format!(",\"message\":{}", json_string_literal(message))
            }
            Self::ReloadRequested { reason } | Self::RestartRequested { reason } => {
                format!(",\"reason\":{}", json_string_literal(reason))
            }
            Self::ReloadApplied { window_id } => {
                format!(",\"windowId\":{}", json_string_literal(window_id))
            }
            Self::ReloadDeferred { window_id, reason } => format!(
                ",\"windowId\":{},\"reason\":{}",
                optional_json_string_literal(window_id.as_deref()),
                json_string_literal(reason)
            ),
            Self::RestartRequired { window_id, reason } => format!(
                ",\"windowId\":{},\"reason\":{}",
                json_string_literal(window_id),
                json_string_literal(reason)
            ),
            Self::RestartExitRequested {
                window_count,
                request_count,
            } => format!(",\"windowCount\":{window_count},\"requestCount\":{request_count}"),
            Self::RestartDeferred { reason } => {
                format!(",\"reason\":{}", json_string_literal(reason))
            }
            Self::RestartApplied { launch, reason } => {
                format!(
                    ",\"launch\":{launch},\"reason\":{}",
                    json_string_literal(reason)
                )
            }
        };
        format!(
            "{{\"schema\":\"axion.dev-event.v1\",\"event\":{}{}}}",
            json_string_literal(self.event_name()),
            fields
        )
    }
}

fn optional_json_string_literal(value: Option<&str>) -> String {
    value
        .map(json_string_literal)
        .unwrap_or_else(|| "null".to_owned())
}

impl DevWatchGuard {
    fn spawn(
        args: &DevArgs,
        config: &AppConfig,
        reload_targets: Vec<ReloadTarget>,
        event_output: DevEventOutput,
    ) -> Result<Option<Self>, AxionCliError> {
        if !args.watch {
            return Ok(None);
        }

        let root = config.build.frontend_dist.clone();
        let mut snapshot = scan_watch_root(&root)?;
        let reload = args.reload;
        let restart_on_change = args.restart_on_change;
        println!(
            "watch: watching {} (poll={}ms, debounce={}ms, files={})",
            root.display(),
            WATCH_POLL_INTERVAL_MS,
            WATCH_DEBOUNCE_MS,
            snapshot.files.len()
        );
        println!("{}", reload_mode_line(reload));

        let stop = Arc::new(AtomicBool::new(false));
        let restart_requested = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_restart_requested = Arc::clone(&restart_requested);
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(WATCH_POLL_INTERVAL_MS));
                if thread_stop.load(Ordering::Relaxed) {
                    break;
                }

                match scan_watch_root(&root) {
                    Ok(next_snapshot) => {
                        let (changes, debounced_snapshot) =
                            debounce_watch_changes(&root, &snapshot, next_snapshot);
                        if !changes.is_empty() {
                            let reload_outcomes = if reload {
                                reload_targets
                                    .iter()
                                    .map(apply_reload_target)
                                    .collect::<Vec<_>>()
                            } else {
                                Vec::new()
                            };
                            let restart_outcome = (restart_on_change
                                && should_restart_after_change(reload, &reload_outcomes))
                            .then(|| request_restart(&reload_targets));
                            for line in watch_change_lines(
                                &changes,
                                reload,
                                &reload_outcomes,
                                restart_outcome.as_ref(),
                            ) {
                                println!("{line}");
                            }
                            event_output.emit_many(&watch_change_events(
                                &changes,
                                reload,
                                &reload_outcomes,
                                restart_outcome.as_ref(),
                            ));
                            if matches!(restart_outcome, Some(RestartOutcome::Requested { .. })) {
                                thread_restart_requested.store(true, Ordering::Relaxed);
                                break;
                            }
                        }
                        snapshot = debounced_snapshot;
                    }
                    Err(error) => {
                        println!("watch_error: {error}");
                        event_output.emit(&DevEvent::WatchError {
                            message: error.to_string(),
                        });
                    }
                }
            }
        });

        Ok(Some(Self {
            stop,
            restart_requested,
            handle: Some(handle),
        }))
    }

    fn restart_requested(&self) -> bool {
        self.restart_requested.load(Ordering::Relaxed)
    }
}

impl Drop for DevWatchGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn reload_targets_from_launch_request(
    launch_request: &axion_runtime::RuntimeLaunchRequest,
) -> Vec<ReloadTarget> {
    launch_request
        .window_bindings
        .iter()
        .map(|binding| ReloadTarget {
            window_id: binding.window_id.clone(),
            window_control: binding.window_control.clone(),
        })
        .collect()
}

fn apply_reload_target(target: &ReloadTarget) -> ReloadOutcome {
    match axion_runtime::reload_window(&target.window_control, Some(target.window_id.as_str())) {
        Ok(()) => ReloadOutcome::Applied {
            window_id: target.window_id.clone(),
        },
        Err(error) => ReloadOutcome::RestartRequired {
            window_id: target.window_id.clone(),
            reason: error,
        },
    }
}

fn request_restart(reload_targets: &[ReloadTarget]) -> RestartOutcome {
    let Some(target) = reload_targets.first() else {
        return RestartOutcome::Deferred {
            reason: "no live window control targets are available; launch the app with --launch to restart on changes".to_owned(),
        };
    };

    match target
        .window_control
        .execute(None, axion_runtime::WindowControlRequest::ExitApp)
    {
        Ok(axion_runtime::WindowControlResponse::AppExit {
            window_count,
            request_count,
            ..
        }) => RestartOutcome::Requested {
            window_count,
            request_count,
        },
        Ok(_) => RestartOutcome::Deferred {
            reason: "window control backend returned an unexpected restart response".to_owned(),
        },
        Err(error) => RestartOutcome::Deferred { reason: error },
    }
}

fn should_restart_after_change(reload: bool, reload_outcomes: &[ReloadOutcome]) -> bool {
    !reload
        || reload_outcomes.is_empty()
        || reload_outcomes
            .iter()
            .any(|outcome| !matches!(outcome, ReloadOutcome::Applied { .. }))
}

fn run_foreground_watch_if_requested(
    args: &DevArgs,
    config: &AppConfig,
    event_output: DevEventOutput,
) -> Result<(), AxionCliError> {
    let Some(_watch_guard) = DevWatchGuard::spawn(args, config, Vec::new(), event_output)? else {
        return Ok(());
    };

    println!("watch: press Ctrl+C to stop.");
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn scan_watch_root(root: &Path) -> std::io::Result<WatchSnapshot> {
    let mut files = BTreeMap::new();
    scan_watch_dir(root, root, &mut files)?;
    Ok(WatchSnapshot { files })
}

fn scan_watch_dir(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<PathBuf, WatchedFile>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;

        if should_ignore_watch_entry(&path, metadata.is_dir()) {
            continue;
        }

        if metadata.is_dir() {
            scan_watch_dir(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            files.insert(
                relative,
                WatchedFile {
                    modified_millis: modified_millis(metadata.modified().ok()),
                    len: metadata.len(),
                },
            );
        }
    }

    Ok(())
}

fn should_ignore_watch_entry(path: &Path, is_dir: bool) -> bool {
    let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
        return false;
    };

    if is_dir {
        return matches!(
            file_name,
            ".git" | ".next" | ".turbo" | ".vite" | "node_modules" | "target"
        );
    }

    file_name == ".DS_Store"
        || file_name.starts_with(".#")
        || file_name.ends_with('~')
        || file_name.ends_with(".log")
        || file_name.ends_with(".swp")
        || file_name.ends_with(".swo")
        || file_name.ends_with(".tmp")
        || file_name.ends_with(".temp")
}

fn modified_millis(modified: Option<SystemTime>) -> u128 {
    modified
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn watch_changes(previous: &WatchSnapshot, next: &WatchSnapshot) -> Vec<WatchChange> {
    let mut changes = Vec::new();

    for (path, next_file) in &next.files {
        match previous.files.get(path) {
            Some(previous_file) if previous_file == next_file => {}
            Some(_) => changes.push(WatchChange {
                path: path.clone(),
                kind: WatchChangeKind::Modified,
            }),
            None => changes.push(WatchChange {
                path: path.clone(),
                kind: WatchChangeKind::Created,
            }),
        }
    }

    for path in previous.files.keys() {
        if !next.files.contains_key(path) {
            changes.push(WatchChange {
                path: path.clone(),
                kind: WatchChangeKind::Deleted,
            });
        }
    }

    changes
}

fn debounce_watch_changes(
    root: &Path,
    previous: &WatchSnapshot,
    next: WatchSnapshot,
) -> (Vec<WatchChange>, WatchSnapshot) {
    if watch_changes(previous, &next).is_empty() {
        return (Vec::new(), next);
    }

    thread::sleep(Duration::from_millis(WATCH_DEBOUNCE_MS));
    match scan_watch_root(root) {
        Ok(debounced_snapshot) => (
            watch_changes(previous, &debounced_snapshot),
            debounced_snapshot,
        ),
        Err(_) => (watch_changes(previous, &next), next),
    }
}

fn watch_change_lines(
    changes: &[WatchChange],
    reload: bool,
    reload_outcomes: &[ReloadOutcome],
    restart_outcome: Option<&RestartOutcome>,
) -> Vec<String> {
    let mut lines = Vec::new();
    for change in changes {
        lines.push(format!(
            "watch_change: {} {}",
            watch_change_kind_label(&change.kind),
            change.path.display()
        ));
    }
    if reload {
        lines.push("reload_requested: frontend assets changed.".to_owned());
        if reload_outcomes.is_empty() {
            lines.push(reload_outcome_line(&ReloadOutcome::Deferred {
                window_id: None,
                reason: "no live window control targets are available; launch the app with --launch to apply reloads".to_owned(),
            }));
        } else {
            for outcome in reload_outcomes {
                lines.push(reload_outcome_line(outcome));
            }
        }
    }
    if let Some(restart_outcome) = restart_outcome {
        lines.push("restart_requested: frontend assets changed.".to_owned());
        lines.push(restart_outcome_line(restart_outcome));
    }
    lines
}

fn watch_change_events(
    changes: &[WatchChange],
    reload: bool,
    reload_outcomes: &[ReloadOutcome],
    restart_outcome: Option<&RestartOutcome>,
) -> Vec<DevEvent> {
    let mut events = changes
        .iter()
        .map(|change| DevEvent::WatchChange {
            kind: change.kind.clone(),
            path: change.path.clone(),
        })
        .collect::<Vec<_>>();
    if reload {
        events.push(DevEvent::ReloadRequested {
            reason: "frontend assets changed".to_owned(),
        });
        if reload_outcomes.is_empty() {
            events.push(DevEvent::ReloadDeferred {
                window_id: None,
                reason: "no live window control targets are available; launch the app with --launch to apply reloads".to_owned(),
            });
        } else {
            events.extend(reload_outcomes.iter().map(reload_outcome_event));
        }
    }
    if let Some(restart_outcome) = restart_outcome {
        events.push(DevEvent::RestartRequested {
            reason: "frontend assets changed".to_owned(),
        });
        events.push(restart_outcome_event(restart_outcome));
    }
    events
}

fn reload_outcome_event(outcome: &ReloadOutcome) -> DevEvent {
    match outcome {
        ReloadOutcome::Applied { window_id } => DevEvent::ReloadApplied {
            window_id: window_id.clone(),
        },
        ReloadOutcome::Deferred { window_id, reason } => DevEvent::ReloadDeferred {
            window_id: window_id.clone(),
            reason: reason.clone(),
        },
        ReloadOutcome::RestartRequired { window_id, reason } => DevEvent::RestartRequired {
            window_id: window_id.clone(),
            reason: reason.clone(),
        },
    }
}

fn restart_outcome_event(outcome: &RestartOutcome) -> DevEvent {
    match outcome {
        RestartOutcome::Requested {
            window_count,
            request_count,
        } => DevEvent::RestartExitRequested {
            window_count: *window_count,
            request_count: *request_count,
        },
        RestartOutcome::Deferred { reason } => DevEvent::RestartDeferred {
            reason: reason.clone(),
        },
    }
}

fn reload_outcome_line(outcome: &ReloadOutcome) -> String {
    match outcome {
        ReloadOutcome::Applied { window_id } => {
            format!("reload_applied: window={window_id}")
        }
        ReloadOutcome::Deferred { window_id, reason } => match window_id {
            Some(window_id) => format!("reload_deferred: window={window_id}; reason={reason}"),
            None => format!("reload_deferred: {reason}."),
        },
        ReloadOutcome::RestartRequired { window_id, reason } => {
            format!("restart_required: window={window_id}; reason={reason}")
        }
    }
}

fn restart_outcome_line(outcome: &RestartOutcome) -> String {
    match outcome {
        RestartOutcome::Requested {
            window_count,
            request_count,
        } => {
            format!(
                "restart_exit_requested: windows={window_count}; close_requests={request_count}"
            )
        }
        RestartOutcome::Deferred { reason } => format!("restart_deferred: {reason}."),
    }
}

fn watch_change_kind_label(kind: &WatchChangeKind) -> &'static str {
    match kind {
        WatchChangeKind::Created => "created",
        WatchChangeKind::Modified => "modified",
        WatchChangeKind::Deleted => "deleted",
    }
}

fn reload_mode_line(reload: bool) -> String {
    if reload {
        "reload: enabled; file changes emit reload_requested and attempt window reload when --launch is active.".to_owned()
    } else {
        "reload: disabled; use --reload to request live window reloads when watched files change."
            .to_owned()
    }
}

fn wait_for_dev_server(
    config: &AppConfig,
    process: &mut FrontendProcess,
    timeout_ms: u64,
) -> DevServerWaitResult {
    let timeout = Duration::from_millis(timeout_ms);
    let started = Instant::now();

    while started.elapsed() <= timeout {
        if dev_server_is_reachable(config) {
            return DevServerWaitResult::Reachable;
        }

        match process.child.try_wait() {
            Ok(Some(status)) => {
                return DevServerWaitResult::ExitedEarly {
                    status: status.code(),
                    stderr_summary: process.stderr.summary(),
                };
            }
            Ok(None) => {}
            Err(error) => {
                return DevServerWaitResult::ExitedEarly {
                    status: None,
                    stderr_summary: error.to_string(),
                };
            }
        }

        std::thread::sleep(Duration::from_millis(DEV_SERVER_POLL_INTERVAL_MS));
    }

    DevServerWaitResult::Timeout
}

#[cfg(unix)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("sh");
    shell.arg("-c").arg(format!("exec {command}"));
    shell
}

#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    let mut shell = Command::new("cmd");
    shell.arg("/C").arg(command);
    shell
}

fn dev_diagnostic_lines(
    app: &App,
    manifest_path: &std::path::Path,
    dev_server_status: &DevServerStatus,
    packaged_fallback_status: &PackagedFallbackStatus,
    fallback_packaged: bool,
) -> Vec<String> {
    let launch_mode = select_launch_mode_with_packaged_fallback(
        dev_server_status,
        packaged_fallback_status,
        fallback_packaged,
    );
    let selected_packaged_fallback = matches!(launch_mode, Ok(RunMode::Production));

    let mut lines = vec![
        format!("manifest: {}", manifest_path.display()),
        format!(
            "launch_mode: {}",
            launch_mode
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|error| format!("blocked ({error})"))
        ),
        format!("dev_server: {}", dev_server_status.summary()),
        format!(
            "packaged_fallback: {}",
            packaged_fallback_status.summary(fallback_packaged, selected_packaged_fallback)
        ),
        "window_entries:".to_owned(),
    ];

    lines.extend(window_entry_lines(
        app,
        dev_server_status,
        packaged_fallback_status,
        launch_mode.ok(),
    ));
    lines.extend(next_step_lines(
        dev_server_status,
        packaged_fallback_status,
        fallback_packaged,
    ));
    lines
}

fn window_entry_lines(
    app: &App,
    dev_server_status: &DevServerStatus,
    packaged_fallback_status: &PackagedFallbackStatus,
    launch_mode: Option<RunMode>,
) -> Vec<String> {
    let entry = match launch_mode {
        Some(RunMode::Development) => match dev_server_status {
            DevServerStatus::Reachable { url } => {
                format!("{url} (development)")
            }
            _ => "unavailable (development server is not reachable)".to_owned(),
        },
        Some(RunMode::Production) => packaged_fallback_status
            .entry_url()
            .map(|url| format!("{url} (packaged fallback)"))
            .unwrap_or_else(|| "unavailable (packaged fallback is invalid)".to_owned()),
        None => "unavailable (launch blocked)".to_owned(),
    };

    app.config()
        .windows
        .iter()
        .map(|window| format!("- {}: {entry}", window.id.as_str()))
        .collect()
}

fn next_step_lines(
    dev_server_status: &DevServerStatus,
    packaged_fallback_status: &PackagedFallbackStatus,
    fallback_packaged: bool,
) -> Vec<String> {
    match (dev_server_status, fallback_packaged, packaged_fallback_status) {
        (DevServerStatus::Reachable { .. }, _, _) => {
            vec!["next_steps: run with --launch to open the reachable dev server.".to_owned()]
        }
        (_, true, PackagedFallbackStatus::Available { .. }) => vec![
            "next_steps: packaged fallback is selected; start the dev server and remove --fallback-packaged to use live frontend assets.".to_owned(),
        ],
        (_, true, PackagedFallbackStatus::Unavailable { reason }) => vec![format!(
            "next_steps: packaged fallback was requested but is unavailable ({reason}); fix [build].frontend_dist and [build].entry."
        )],
        (DevServerStatus::Unconfigured, false, _) => vec![
            "next_steps: add [dev] url = \"http://127.0.0.1:3000\" or pass --fallback-packaged.".to_owned(),
        ],
        (DevServerStatus::InvalidEndpoint { .. }, false, _) => {
            vec!["next_steps: fix [dev].url so it includes a host and port, or pass --fallback-packaged.".to_owned()]
        }
        (DevServerStatus::Unreachable { url }, false, _) => vec![format!(
            "next_steps: start the frontend dev server at {url}, check [dev].url, or pass --fallback-packaged."
        )],
    }
}

fn dev_option_lines(args: &DevArgs) -> Vec<String> {
    let mut lines = Vec::new();
    if args.watch {
        lines.push("watch: enabled for frontend asset polling.".to_owned());
    }
    if args.reload {
        if args.watch {
            lines.push(
                "reload: enabled; window reload is attempted when --launch is active.".to_owned(),
            );
        } else {
            lines.push(
                "reload: requested without --watch; no file changes will be observed.".to_owned(),
            );
        }
    }
    if args.restart_on_change {
        if args.watch {
            lines.push(
                "restart_on_change: enabled; app restart is requested after watched file changes."
                    .to_owned(),
            );
        } else {
            lines.push(
                "restart_on_change: requested without --watch; no file changes will be observed."
                    .to_owned(),
            );
        }
    }
    if args.json_events {
        lines.push("json_events: enabled; dev events are printed as JSON lines.".to_owned());
    }
    if let Some(path) = &args.event_log {
        lines.push(format!(
            "event_log: enabled; writing JSON lines to {}",
            path.display()
        ));
    }
    if args.open_devtools {
        lines.push(
            "devtools: requested but unsupported by the current Servo backend; continuing without opening devtools.".to_owned(),
        );
    }
    lines
}

fn launch_summary_lines(
    app: &App,
    dev_server_status: &DevServerStatus,
    packaged_fallback_status: &PackagedFallbackStatus,
    launch_mode: RunMode,
) -> Vec<String> {
    let selected_packaged_fallback = matches!(launch_mode, RunMode::Production);
    let entry_lines = window_entry_lines(
        app,
        dev_server_status,
        packaged_fallback_status,
        Some(launch_mode),
    );
    let mut lines = vec![
        format!("- mode: {launch_mode}"),
        format!(
            "- packaged_fallback: {}",
            packaged_fallback_status.selected_summary(selected_packaged_fallback)
        ),
        "- windows:".to_owned(),
    ];
    lines.extend(entry_lines.into_iter().map(|line| format!("  {line}")));
    lines
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use axion_core::{AppConfig, AppIdentity, BuildConfig, RunMode, WindowConfig, WindowId};
    use url::Url;

    use super::{
        DevServerStatus, DevServerWaitResult, FrontendCommandPlan, PackagedFallbackStatus,
        ReloadOutcome, dev_diagnostic_lines, dev_option_lines, dev_server_status,
        dev_server_status_with, frontend_command_plan, frontend_diagnostic_lines,
        frontend_wait_error, launch_summary_lines, packaged_fallback_status, reload_mode_line,
        reload_targets_from_launch_request, scan_watch_root, select_launch_mode,
        select_launch_mode_with_packaged_fallback, should_ignore_watch_entry, watch_change_lines,
        watch_changes,
    };
    use crate::cli::DevArgs;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

    fn temp_dir() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let serial = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("axion-dev-test-{unique}-{serial}"))
    }

    fn config_with_dev_url(dev_url: Option<&str>) -> AppConfig {
        AppConfig {
            identity: AppIdentity::new("axion-cli-test"),
            windows: vec![WindowConfig::main("CLI Test")],
            dev: dev_url.map(|value| axion_core::DevServerConfig {
                url: Url::parse(value).expect("test URL must parse"),
                command: None,
                cwd: None,
                timeout_ms: None,
            }),
            build: BuildConfig::new("frontend", "frontend/index.html"),
            bundle: Default::default(),
            native: Default::default(),
            capabilities: Default::default(),
        }
    }

    fn dev_args() -> DevArgs {
        DevArgs {
            manifest_path: "axion.toml".into(),
            launch: false,
            fallback_packaged: false,
            watch: false,
            reload: false,
            restart_on_change: false,
            json_events: false,
            event_log: None,
            open_devtools: false,
            frontend_command: None,
            frontend_cwd: None,
            dev_server_timeout_ms: None,
        }
    }

    fn app_with_frontend(dev_url: Option<&str>, window_count: usize) -> axion_core::App {
        let root = temp_dir();
        let frontend = root.join("frontend");
        let entry = frontend.join("index.html");
        fs::create_dir_all(&frontend).unwrap();
        fs::write(&entry, "<html></html>").unwrap();

        let windows = if window_count == 1 {
            vec![WindowConfig::main("CLI Test")]
        } else {
            vec![
                WindowConfig::main("CLI Test"),
                WindowConfig::new(WindowId::new("settings"), "Settings", 480, 360),
            ]
        };

        axion_core::Builder::new()
            .apply_config(AppConfig {
                identity: AppIdentity::new("axion-cli-test"),
                windows,
                dev: dev_url.map(|value| axion_core::DevServerConfig {
                    url: Url::parse(value).expect("test URL must parse"),
                    command: None,
                    cwd: None,
                    timeout_ms: None,
                }),
                build: BuildConfig::new(&frontend, &entry),
                bundle: Default::default(),
                native: Default::default(),
                capabilities: Default::default(),
            })
            .build()
            .unwrap()
    }

    #[test]
    fn launch_mode_errors_without_dev_server_by_default() {
        let status = dev_server_status(&config_with_dev_url(None));

        assert_eq!(status, DevServerStatus::Unconfigured);
        assert!(select_launch_mode(&status, false).is_err());
        assert_eq!(
            select_launch_mode(&status, true).expect("fallback should select production"),
            RunMode::Production
        );
    }

    #[test]
    fn launch_mode_uses_reachable_dev_server() {
        let config = config_with_dev_url(Some("http://127.0.0.1:3000"));
        let status = dev_server_status_with(&config, |_host, _port| Some(true));

        assert!(matches!(status, DevServerStatus::Reachable { .. }));
        assert_eq!(
            select_launch_mode(&status, false).expect("reachable dev server should launch dev"),
            RunMode::Development
        );
    }

    #[test]
    fn launch_mode_requires_available_packaged_fallback() {
        let dev_status = DevServerStatus::Unreachable {
            url: "http://127.0.0.1:3000/".to_owned(),
        };
        let fallback_status = PackagedFallbackStatus::Unavailable {
            reason: "missing index.html".to_owned(),
        };

        assert!(
            select_launch_mode_with_packaged_fallback(&dev_status, &fallback_status, true).is_err()
        );
    }

    #[test]
    fn dev_server_status_reports_unreachable_server() {
        let config = config_with_dev_url(Some("http://127.0.0.1:3000"));

        assert!(matches!(
            dev_server_status_with(&config, |_host, _port| Some(false)),
            DevServerStatus::Unreachable { .. }
        ));
    }

    #[test]
    fn dev_server_status_reports_invalid_endpoint() {
        let config = config_with_dev_url(Some("http://127.0.0.1:3000"));

        assert!(matches!(
            dev_server_status_with(&config, |_host, _port| None),
            DevServerStatus::InvalidEndpoint { .. }
        ));
    }

    #[test]
    fn dev_diagnostics_report_reachable_development_entries() {
        let app = app_with_frontend(Some("http://127.0.0.1:3000"), 2);
        let dev_status = DevServerStatus::Reachable {
            url: "http://127.0.0.1:3000/".to_owned(),
        };
        let fallback_status = packaged_fallback_status(&app);

        let lines = dev_diagnostic_lines(
            &app,
            std::path::Path::new("axion.toml"),
            &dev_status,
            &fallback_status,
            false,
        );

        assert!(lines.iter().any(|line| line == "launch_mode: development"));
        assert!(
            lines
                .iter()
                .any(|line| line == "dev_server: reachable (http://127.0.0.1:3000/)")
        );
        assert!(
            lines.iter().any(|line| {
                line == "packaged_fallback: disabled; available with --fallback-packaged (axion://app/index.html)"
            })
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "- main: http://127.0.0.1:3000/ (development)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "- settings: http://127.0.0.1:3000/ (development)")
        );
        assert!(lines.iter().any(|line| line.contains("run with --launch")));
    }

    #[test]
    fn dev_diagnostics_report_blocked_launch_without_fallback() {
        let app = app_with_frontend(Some("http://127.0.0.1:3000"), 1);
        let dev_status = DevServerStatus::Unreachable {
            url: "http://127.0.0.1:3000/".to_owned(),
        };
        let fallback_status = packaged_fallback_status(&app);

        let lines = dev_diagnostic_lines(
            &app,
            std::path::Path::new("axion.toml"),
            &dev_status,
            &fallback_status,
            false,
        );

        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("launch_mode: blocked"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "- main: unavailable (launch blocked)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("start the frontend dev server"))
        );
    }

    #[test]
    fn dev_diagnostics_report_selected_packaged_fallback_entries() {
        let app = app_with_frontend(Some("http://127.0.0.1:3000"), 1);
        let dev_status = DevServerStatus::Unreachable {
            url: "http://127.0.0.1:3000/".to_owned(),
        };
        let fallback_status = packaged_fallback_status(&app);

        let lines = dev_diagnostic_lines(
            &app,
            std::path::Path::new("axion.toml"),
            &dev_status,
            &fallback_status,
            true,
        );

        assert!(lines.iter().any(|line| line == "launch_mode: production"));
        assert!(
            lines
                .iter()
                .any(|line| line == "packaged_fallback: selected (axion://app/index.html)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "- main: axion://app/index.html (packaged fallback)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("packaged fallback is selected"))
        );
    }

    #[test]
    fn dev_option_lines_report_reserved_flags() {
        let lines = dev_option_lines(&DevArgs {
            watch: true,
            reload: true,
            restart_on_change: true,
            json_events: true,
            event_log: Some("target/dev-events.jsonl".into()),
            open_devtools: true,
            ..dev_args()
        });

        assert!(
            lines
                .iter()
                .any(|line| line == "watch: enabled for frontend asset polling.")
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("reload: enabled; window reload is attempted"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("restart_on_change: enabled"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "json_events: enabled; dev events are printed as JSON lines.")
        );
        assert!(lines.iter().any(
            |line| line == "event_log: enabled; writing JSON lines to target/dev-events.jsonl"
        ));
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("devtools: requested"))
        );
    }

    #[test]
    fn dev_option_lines_report_reload_without_watch() {
        let lines = dev_option_lines(&DevArgs {
            reload: true,
            restart_on_change: true,
            ..dev_args()
        });

        assert!(
            lines.iter().any(|line| line
                == "reload: requested without --watch; no file changes will be observed.")
        );
        assert!(lines.iter().any(|line| line
            == "restart_on_change: requested without --watch; no file changes will be observed."));
    }

    #[test]
    fn watch_snapshot_reports_created_modified_and_deleted_files() {
        let root = temp_dir();
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("index.html"), "one").unwrap();
        fs::write(root.join("nested").join("app.js"), "one").unwrap();
        let first = scan_watch_root(&root).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(2));
        fs::write(root.join("index.html"), "two").unwrap();
        fs::write(root.join("style.css"), "body{}").unwrap();
        fs::remove_file(root.join("nested").join("app.js")).unwrap();
        let second = scan_watch_root(&root).unwrap();
        let changes = watch_changes(&first, &second);
        let lines = watch_change_lines(&changes, true, &[], None);

        assert!(
            lines
                .iter()
                .any(|line| line == "watch_change: modified index.html")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "watch_change: created style.css")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "watch_change: deleted nested/app.js")
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("reload_requested: frontend assets changed"))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("reload_deferred: no live window control targets"))
        );
    }

    #[test]
    fn watch_change_lines_report_applied_and_deferred_reload_outcomes() {
        let changes = vec![super::WatchChange {
            path: "app.js".into(),
            kind: super::WatchChangeKind::Modified,
        }];
        let lines = watch_change_lines(
            &changes,
            true,
            &[
                ReloadOutcome::Applied {
                    window_id: "main".to_owned(),
                },
                ReloadOutcome::Deferred {
                    window_id: Some("settings".to_owned()),
                    reason: "no live target".to_owned(),
                },
                ReloadOutcome::RestartRequired {
                    window_id: "tools".to_owned(),
                    reason: "window control backend is unavailable".to_owned(),
                },
            ],
            None,
        );

        assert!(
            lines
                .iter()
                .any(|line| line == "reload_applied: window=main")
        );
        assert!(
            lines
                .iter()
                .any(|line| { line == "reload_deferred: window=settings; reason=no live target" })
        );
        assert!(lines.iter().any(|line| {
            line == "restart_required: window=tools; reason=window control backend is unavailable"
        }));
    }

    #[test]
    fn watch_change_lines_report_restart_on_change_outcomes() {
        let changes = vec![super::WatchChange {
            path: "app.js".into(),
            kind: super::WatchChangeKind::Modified,
        }];
        let lines = watch_change_lines(
            &changes,
            false,
            &[],
            Some(&super::RestartOutcome::Requested {
                window_count: 2,
                request_count: 2,
            }),
        );

        assert!(
            lines
                .iter()
                .any(|line| line == "restart_requested: frontend assets changed.")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "restart_exit_requested: windows=2; close_requests=2")
        );

        let deferred = watch_change_lines(
            &changes,
            false,
            &[],
            Some(&super::RestartOutcome::Deferred {
                reason: "no live target".to_owned(),
            }),
        );
        assert!(
            deferred
                .iter()
                .any(|line| line == "restart_deferred: no live target.")
        );
    }

    #[test]
    fn watch_change_events_serialize_stable_json() {
        let changes = vec![super::WatchChange {
            path: "app.js".into(),
            kind: super::WatchChangeKind::Modified,
        }];
        let events = super::watch_change_events(
            &changes,
            true,
            &[
                ReloadOutcome::Applied {
                    window_id: "main".to_owned(),
                },
                ReloadOutcome::RestartRequired {
                    window_id: "settings".to_owned(),
                    reason: "window control backend is unavailable".to_owned(),
                },
            ],
            Some(&super::RestartOutcome::Requested {
                window_count: 2,
                request_count: 2,
            }),
        );
        let json = events
            .iter()
            .map(super::DevEvent::to_json)
            .collect::<Vec<_>>();

        assert!(json[0].contains("\"schema\":\"axion.dev-event.v1\""));
        assert!(json[0].contains("\"event\":\"watch_change\""));
        assert!(json[0].contains("\"kind\":\"modified\""));
        assert!(json[0].contains("\"path\":\"app.js\""));
        assert!(
            json.iter()
                .any(|line| line.contains("\"event\":\"reload_applied\""))
        );
        assert!(
            json.iter()
                .any(|line| line.contains("\"event\":\"restart_required\""))
        );
        assert!(
            json.iter()
                .any(|line| line.contains("\"event\":\"restart_exit_requested\""))
        );
    }

    #[test]
    fn dev_event_output_writes_json_lines_report() {
        let root = temp_dir();
        let log_path = root.join("dev-events.jsonl");
        let output = super::DevEventOutput::new(&DevArgs {
            event_log: Some(log_path.clone()),
            ..dev_args()
        })
        .expect("event output should create log");

        output.emit(&super::DevEvent::RestartApplied {
            launch: 2,
            reason: "frontend assets changed".to_owned(),
        });
        drop(output);

        let body = fs::read_to_string(log_path).expect("event log should be readable");
        assert!(body.contains("\"schema\":\"axion.dev-event.v1\""));
        assert!(body.contains("\"event\":\"restart_applied\""));
        assert!(body.contains("\"launch\":2"));
    }

    #[test]
    fn restart_on_change_is_only_required_when_reload_cannot_cover_change() {
        assert!(super::should_restart_after_change(false, &[]));
        assert!(super::should_restart_after_change(true, &[]));
        assert!(!super::should_restart_after_change(
            true,
            &[ReloadOutcome::Applied {
                window_id: "main".to_owned(),
            }]
        ));
        assert!(super::should_restart_after_change(
            true,
            &[
                ReloadOutcome::Applied {
                    window_id: "main".to_owned(),
                },
                ReloadOutcome::RestartRequired {
                    window_id: "settings".to_owned(),
                    reason: "window control backend is unavailable".to_owned(),
                },
            ]
        ));
    }

    #[test]
    fn reload_targets_cover_each_launch_window() {
        let app = app_with_frontend(None, 2);
        let launch_request = axion_runtime::launch_request(&app, RunMode::Production)
            .expect("production launch request should build");
        let reload_targets = reload_targets_from_launch_request(&launch_request);
        let window_ids = reload_targets
            .iter()
            .map(|target| target.window_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(window_ids, vec!["main", "settings"]);
    }

    #[test]
    fn watch_snapshot_ignores_common_temporary_files_and_dirs() {
        let root = temp_dir();
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::create_dir_all(root.join(".vite")).unwrap();
        fs::write(root.join("index.html"), "ok").unwrap();
        fs::write(root.join(".DS_Store"), "ignored").unwrap();
        fs::write(root.join("debug.log"), "ignored").unwrap();
        fs::write(root.join("index.html.swp"), "ignored").unwrap();
        fs::write(root.join("node_modules").join("dep.js"), "ignored").unwrap();
        fs::write(root.join(".vite").join("cache.js"), "ignored").unwrap();

        let snapshot = scan_watch_root(&root).unwrap();

        assert!(
            snapshot
                .files
                .contains_key(std::path::Path::new("index.html"))
        );
        assert!(
            !snapshot
                .files
                .contains_key(std::path::Path::new(".DS_Store"))
        );
        assert!(
            !snapshot
                .files
                .contains_key(std::path::Path::new("debug.log"))
        );
        assert!(
            !snapshot
                .files
                .contains_key(std::path::Path::new("index.html.swp"))
        );
        assert!(
            !snapshot
                .files
                .contains_key(std::path::Path::new("node_modules/dep.js"))
        );
        assert!(
            !snapshot
                .files
                .contains_key(std::path::Path::new(".vite/cache.js"))
        );
        assert!(should_ignore_watch_entry(&root.join(".DS_Store"), false));
        assert!(should_ignore_watch_entry(&root.join("node_modules"), true));
    }

    #[test]
    fn reload_mode_lines_describe_live_reload_behavior() {
        assert!(reload_mode_line(true).contains("attempt window reload"));
        assert!(reload_mode_line(false).contains("use --reload"));
    }

    #[test]
    fn launch_summary_reports_final_window_entries() {
        let app = app_with_frontend(Some("http://127.0.0.1:3000"), 2);
        let dev_status = DevServerStatus::Reachable {
            url: "http://127.0.0.1:3000/".to_owned(),
        };
        let fallback_status = packaged_fallback_status(&app);

        let lines = launch_summary_lines(&app, &dev_status, &fallback_status, RunMode::Development);

        assert!(lines.iter().any(|line| line == "- mode: development"));
        assert!(
            lines
                .iter()
                .any(|line| line == "  - main: http://127.0.0.1:3000/ (development)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "  - settings: http://127.0.0.1:3000/ (development)")
        );
    }

    #[test]
    fn frontend_command_plan_uses_cli_over_manifest() {
        let mut config = config_with_dev_url(Some("http://127.0.0.1:3000"));
        let manifest_cwd = temp_dir();
        config.dev.as_mut().unwrap().command = Some("manifest command".to_owned());
        config.dev.as_mut().unwrap().cwd = Some(manifest_cwd);
        config.dev.as_mut().unwrap().timeout_ms = Some(1000);

        let cli_cwd = temp_dir();
        let plan = frontend_command_plan(
            &DevArgs {
                frontend_command: Some("cli command".to_owned()),
                frontend_cwd: Some(cli_cwd.clone()),
                dev_server_timeout_ms: Some(42),
                ..dev_args()
            },
            &config,
            std::path::Path::new("axion.toml"),
        )
        .expect("frontend command should plan")
        .expect("plan should exist");

        assert_eq!(
            plan,
            FrontendCommandPlan {
                command: "cli command".to_owned(),
                cwd: cli_cwd,
                timeout_ms: 42,
            }
        );
    }

    #[test]
    fn frontend_command_plan_uses_manifest_defaults() {
        let mut config = config_with_dev_url(Some("http://127.0.0.1:3000"));
        let manifest_cwd = temp_dir();
        config.dev.as_mut().unwrap().command = Some("manifest command".to_owned());
        config.dev.as_mut().unwrap().cwd = Some(manifest_cwd.clone());
        config.dev.as_mut().unwrap().timeout_ms = Some(1000);

        let plan = frontend_command_plan(&dev_args(), &config, std::path::Path::new("axion.toml"))
            .expect("frontend command should plan")
            .expect("plan should exist");

        assert_eq!(
            plan,
            FrontendCommandPlan {
                command: "manifest command".to_owned(),
                cwd: manifest_cwd,
                timeout_ms: 1000,
            }
        );
    }

    #[test]
    fn frontend_command_plan_requires_dev_url() {
        let mut args = dev_args();
        args.frontend_command = Some("echo test".to_owned());

        assert!(
            frontend_command_plan(
                &args,
                &config_with_dev_url(None),
                std::path::Path::new("axion.toml")
            )
            .is_err()
        );
    }

    #[test]
    fn frontend_process_diagnostics_report_wait_results() {
        let lines = frontend_diagnostic_lines(
            "test server",
            std::path::Path::new("/tmp"),
            &DevServerWaitResult::ExitedEarly {
                status: Some(2),
                stderr_summary: "failed to bind".to_owned(),
            },
        );

        assert!(
            lines
                .iter()
                .any(|line| line == "frontend_command: started (test server)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "dev_server_wait: exited early (status=2)")
        );
        assert!(
            lines
                .iter()
                .any(|line| line == "frontend_stderr: failed to bind")
        );
    }

    #[test]
    fn frontend_wait_error_reports_early_exit() {
        let error = frontend_wait_error(&DevServerWaitResult::ExitedEarly {
            status: Some(7),
            stderr_summary: "frontend boom".to_owned(),
        });

        assert!(error.to_string().contains("status=7"));
        assert!(error.to_string().contains("frontend boom"));
    }
}
