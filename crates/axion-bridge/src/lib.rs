mod blocking;
mod error;
pub mod lifecycle;
pub use blocking::{
    WINDOW_CONTROL_TIMEOUT, run_blocking, run_blocking_control, run_blocking_control_with_deadline,
};
pub use error::BridgeError;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Debug, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub const BRIDGE_MAX_NAME_BYTES: usize = 128;
pub const BRIDGE_MAX_PAYLOAD_BYTES: usize = 64 * 1024;
pub const BRIDGE_MAX_REQUEST_ID_BYTES: usize = 128;
pub const BRIDGE_MAX_JSON_DEPTH: usize = 64;
const AXION_RELEASE_VERSION: &str = "v0.6.2";
const AXION_DIAGNOSTICS_REPORT_SCHEMA: &str = "axion.diagnostics-report.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeRequest {
    pub id: String,
    pub command: String,
    pub payload: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeEmitRequest {
    pub id: String,
    pub event: String,
    pub payload: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeEvent {
    pub name: String,
    pub payload_json: String,
}

impl BridgeEvent {
    pub fn try_new(
        name: impl Into<String>,
        payload_json: impl Into<String>,
    ) -> Result<Self, BridgeEventError> {
        let name = normalize_event_name(name.into())?;
        let payload_json =
            normalize_json_payload(payload_json.into()).map_err(BridgeEventError::from)?;
        Ok(Self { name, payload_json })
    }

    pub fn new(name: impl Into<String>, payload_json: impl Into<String>) -> Self {
        Self::try_new(name, payload_json)
            .expect("Axion bridge event must use a valid event name and JSON payload")
    }
}

impl BridgeEmitRequest {
    pub fn try_new(
        event: impl Into<String>,
        payload: impl Into<String>,
    ) -> Result<Self, BridgePayloadError> {
        Ok(Self {
            id: String::new(),
            event: event.into(),
            payload: normalize_json_payload(payload.into())?,
            metadata: BTreeMap::new(),
        })
    }

    pub fn new(event: impl Into<String>, payload: impl Into<String>) -> Self {
        Self::try_new(event, payload).expect("Axion emit request must use a valid JSON payload")
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = normalize_request_id(id.into())
            .expect("Axion emit request id must use a valid bridge request id");
        self
    }

    pub fn try_with_id(mut self, id: impl Into<String>) -> Result<Self, BridgeRequestIdError> {
        self.id = normalize_request_id(id.into())?;
        Ok(self)
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl BridgeRequest {
    pub fn try_new(
        command: impl Into<String>,
        payload: impl Into<String>,
    ) -> Result<Self, BridgePayloadError> {
        Ok(Self {
            id: String::new(),
            command: command.into(),
            payload: normalize_json_payload(payload.into())?,
            metadata: BTreeMap::new(),
        })
    }

    pub fn new(command: impl Into<String>, payload: impl Into<String>) -> Self {
        Self::try_new(command, payload)
            .expect("Axion command request must use a valid JSON payload")
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = normalize_request_id(id.into())
            .expect("Axion command request id must use a valid bridge request id");
        self
    }

    pub fn try_with_id(mut self, id: impl Into<String>) -> Result<Self, BridgeRequestIdError> {
        self.id = normalize_request_id(id.into())?;
        Ok(self)
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeRunMode {
    Development,
    Production,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowCommandContext {
    pub id: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandContext {
    pub app_name: String,
    pub identifier: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub homepage: Option<String>,
    pub mode: BridgeRunMode,
    pub window: WindowCommandContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowStateSnapshot {
    pub id: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub visible: bool,
    pub focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowControlRequest {
    ListStates,
    ExitApp,
    GetState,
    Show,
    Hide,
    Close,
    ConfirmClose { request_id: String },
    PreventClose { request_id: String },
    Focus,
    Reload,
    SetTitle { title: String },
    SetSize { width: u32, height: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowControlResponse {
    AppExit {
        request_id: String,
        window_count: usize,
        request_count: usize,
    },
    CloseRequested {
        request_id: String,
        window: WindowStateSnapshot,
    },
    ClosePrevented {
        request_id: String,
        window_id: String,
    },
    State(WindowStateSnapshot),
    List(Vec<WindowStateSnapshot>),
}

pub trait WindowControlExecutor: Send + Sync {
    fn execute(
        &self,
        target_window_id: Option<&str>,
        request: WindowControlRequest,
    ) -> Result<WindowControlResponse, String>;

    fn execute_with_deadline(
        &self,
        target_window_id: Option<&str>,
        request: WindowControlRequest,
        deadline: Instant,
    ) -> Result<WindowControlResponse, String> {
        if Instant::now() >= deadline {
            return Err("window.control-timeout: request expired before execution".to_owned());
        }
        self.execute(target_window_id, request)
    }
}

#[derive(Clone, Default)]
pub struct WindowControlHandle {
    inner: Arc<Mutex<Option<Arc<dyn WindowControlExecutor>>>>,
    deadline: Option<Instant>,
}

impl Debug for WindowControlHandle {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WindowControlHandle")
            .field("installed", &self.is_installed())
            .finish()
    }
}

impl WindowControlHandle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind this handle clone to a deadline without changing other callers.
    pub fn with_deadline(mut self, deadline: Instant) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn install_executor(&self, executor: Arc<dyn WindowControlExecutor>) {
        if let Ok(mut slot) = self.inner.lock() {
            *slot = Some(executor);
        }
    }

    pub fn is_installed(&self) -> bool {
        self.inner
            .lock()
            .map(|slot| slot.is_some())
            .unwrap_or(false)
    }

    pub fn execute(
        &self,
        target_window_id: Option<&str>,
        request: WindowControlRequest,
    ) -> Result<WindowControlResponse, String> {
        let executor = self
            .inner
            .lock()
            .map_err(|_| "window control state lock was poisoned".to_owned())?
            .clone()
            .ok_or_else(|| "window control backend is unavailable".to_owned())?;
        match self.deadline {
            Some(deadline) => executor.execute_with_deadline(target_window_id, request, deadline),
            None => executor.execute(target_window_id, request),
        }
    }
}

#[cfg(test)]
mod window_control_deadline_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    struct CountingExecutor(Arc<AtomicUsize>);
    impl WindowControlExecutor for CountingExecutor {
        fn execute(
            &self,
            _: Option<&str>,
            _: WindowControlRequest,
        ) -> Result<WindowControlResponse, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(WindowControlResponse::List(Vec::new()))
        }
    }

    #[test]
    fn expired_handle_skips_execution_and_does_not_change_other_clones() {
        let count = Arc::new(AtomicUsize::new(0));
        let handle = WindowControlHandle::new();
        handle.install_executor(Arc::new(CountingExecutor(count.clone())));
        let expired = handle.clone().with_deadline(Instant::now());
        assert!(
            expired
                .execute(None, WindowControlRequest::ListStates)
                .unwrap_err()
                .starts_with("window.control-timeout:")
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
        handle
            .execute(None, WindowControlRequest::ListStates)
            .unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    struct DeadlineExecutor(Arc<Mutex<Option<Instant>>>);
    impl WindowControlExecutor for DeadlineExecutor {
        fn execute(
            &self,
            _: Option<&str>,
            _: WindowControlRequest,
        ) -> Result<WindowControlResponse, String> {
            panic!("the bounded handle must pass its deadline")
        }
        fn execute_with_deadline(
            &self,
            _: Option<&str>,
            _: WindowControlRequest,
            deadline: Instant,
        ) -> Result<WindowControlResponse, String> {
            *self.0.lock().unwrap() = Some(deadline);
            Ok(WindowControlResponse::List(Vec::new()))
        }
    }

    #[test]
    fn handle_forwards_the_original_deadline_to_the_backend() {
        let captured = Arc::new(Mutex::new(None));
        let handle = WindowControlHandle::new();
        handle.install_executor(Arc::new(DeadlineExecutor(captured.clone())));
        let deadline = Instant::now() + Duration::from_secs(5);
        handle
            .with_deadline(deadline)
            .execute(None, WindowControlRequest::ListStates)
            .unwrap();
        assert_eq!(*captured.lock().unwrap(), Some(deadline));
    }
}

type CommandFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
type CommandHandler = Arc<dyn Fn(CommandContext, BridgeRequest) -> CommandFuture + Send + Sync>;
type EventFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
type EventHandler = Arc<dyn Fn(CommandContext, BridgeEmitRequest) -> EventFuture + Send + Sync>;

#[derive(Clone, Default)]
pub struct CommandRegistry {
    handlers: BTreeMap<String, CommandHandler>,
}

#[derive(Clone, Default)]
pub struct EventRegistry {
    handlers: BTreeMap<String, EventHandler>,
}

impl Debug for CommandRegistry {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandRegistry")
            .field("commands", &self.command_names())
            .finish()
    }
}

impl Debug for EventRegistry {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventRegistry")
            .field("events", &self.event_names())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandDispatchError {
    NotFound,
    InvalidRequestCommand,
    InvalidRequestPayload,
    InvalidResponsePayload,
    Handler(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventDispatchError {
    NotFound,
    InvalidEvent,
    InvalidPayload,
    Handler(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRegistryError {
    InvalidCommandName { command: String },
}

impl Display for CommandRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCommandName { command } => {
                write!(formatter, "invalid Axion command name '{command}'")
            }
        }
    }
}

impl std::error::Error for CommandRegistryError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgePayloadError {
    InvalidJsonPayload {
        payload: String,
    },
    PayloadTooLarge {
        actual_bytes: usize,
        max_bytes: usize,
    },
}

impl Display for BridgePayloadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJsonPayload { payload } => {
                write!(formatter, "invalid Axion bridge JSON payload '{payload}'")
            }
            Self::PayloadTooLarge {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "Axion bridge JSON payload is too large ({actual_bytes} bytes; max {max_bytes})"
            ),
        }
    }
}

impl std::error::Error for BridgePayloadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeRequestIdError {
    InvalidRequestId {
        id: String,
    },
    RequestIdTooLong {
        actual_bytes: usize,
        max_bytes: usize,
    },
}

impl Display for BridgeRequestIdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequestId { id } => {
                write!(formatter, "invalid Axion bridge request id '{id}'")
            }
            Self::RequestIdTooLong {
                actual_bytes,
                max_bytes,
            } => write!(
                formatter,
                "Axion bridge request id is too large ({actual_bytes} bytes; max {max_bytes})"
            ),
        }
    }
}

impl std::error::Error for BridgeRequestIdError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeEventError {
    InvalidEventName { event: String },
    InvalidPayloadJson { payload: String },
}

impl Display for BridgeEventError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEventName { event } => {
                write!(formatter, "invalid Axion bridge event name '{event}'")
            }
            Self::InvalidPayloadJson { payload } => {
                write!(
                    formatter,
                    "invalid Axion bridge event JSON payload '{payload}'"
                )
            }
        }
    }
}

impl std::error::Error for BridgeEventError {}

impl From<BridgePayloadError> for BridgeEventError {
    fn from(error: BridgePayloadError) -> Self {
        match error {
            BridgePayloadError::InvalidJsonPayload { payload } => {
                Self::InvalidPayloadJson { payload }
            }
            BridgePayloadError::PayloadTooLarge {
                actual_bytes,
                max_bytes,
            } => Self::InvalidPayloadJson {
                payload: format!("payload too large ({actual_bytes} bytes; max {max_bytes})"),
            },
        }
    }
}

impl CommandRegistry {
    pub fn try_register(
        &mut self,
        command: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeRequest) -> Result<String, String>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), CommandRegistryError> {
        let command = normalize_command_name(command.into())?;
        let handler = Arc::new(handler);
        self.handlers.insert(
            command,
            Arc::new(move |context, request| {
                let handler = handler.clone();
                Box::pin(async move { handler(&context, &request) })
            }),
        );
        Ok(())
    }

    pub fn register(
        &mut self,
        command: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeRequest) -> Result<String, String>
        + Send
        + Sync
        + 'static,
    ) {
        self.try_register(command, handler)
            .expect("Axion command registration must use a valid command name");
    }

    pub fn try_register_async<F, H>(
        &mut self,
        command: impl Into<String>,
        handler: H,
    ) -> Result<(), CommandRegistryError>
    where
        F: Future<Output = Result<String, String>> + Send + 'static,
        H: Fn(CommandContext, BridgeRequest) -> F + Send + Sync + 'static,
    {
        let command = normalize_command_name(command.into())?;
        self.handlers.insert(
            command,
            Arc::new(move |context, request| Box::pin(handler(context, request))),
        );
        Ok(())
    }

    pub fn register_async<F, H>(&mut self, command: impl Into<String>, handler: H)
    where
        F: Future<Output = Result<String, String>> + Send + 'static,
        H: Fn(CommandContext, BridgeRequest) -> F + Send + Sync + 'static,
    {
        self.try_register_async(command, handler)
            .expect("Axion command registration must use a valid command name");
    }

    pub fn command_names(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    pub fn retain_commands(
        &mut self,
        allowed_commands: impl IntoIterator<Item = impl Into<String>>,
    ) {
        let allowed_commands = allowed_commands
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        self.handlers
            .retain(|command, _handler| allowed_commands.contains(command));
    }

    pub fn merge(&mut self, other: Self) {
        self.handlers.extend(other.handlers);
    }

    pub async fn dispatch(
        &self,
        context: &CommandContext,
        request: &BridgeRequest,
    ) -> Result<String, CommandDispatchError> {
        if !is_valid_command_name(&request.command) {
            return Err(CommandDispatchError::InvalidRequestCommand);
        }

        if request.payload.len() > BRIDGE_MAX_PAYLOAD_BYTES {
            return Err(CommandDispatchError::InvalidRequestPayload);
        }

        if !is_valid_json_value(&request.payload) {
            return Err(CommandDispatchError::InvalidRequestPayload);
        }

        let Some(handler) = self.handlers.get(&request.command) else {
            return Err(CommandDispatchError::NotFound);
        };

        let payload = handler(context.clone(), request.clone())
            .await
            .map_err(CommandDispatchError::Handler)?;
        normalize_json_payload(payload).map_err(|_| CommandDispatchError::InvalidResponsePayload)
    }
}

impl EventRegistry {
    pub fn try_register(
        &mut self,
        event: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeEmitRequest) -> Result<(), String>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), BridgeEventError> {
        let event = normalize_event_name(event.into())?;
        let handler = Arc::new(handler);
        self.handlers.insert(
            event,
            Arc::new(move |context, request| {
                let handler = handler.clone();
                Box::pin(async move { handler(&context, &request) })
            }),
        );
        Ok(())
    }

    pub fn register(
        &mut self,
        event: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeEmitRequest) -> Result<(), String>
        + Send
        + Sync
        + 'static,
    ) {
        self.try_register(event, handler)
            .expect("Axion event registration must use a valid event name");
    }

    pub fn try_register_async<F, H>(
        &mut self,
        event: impl Into<String>,
        handler: H,
    ) -> Result<(), BridgeEventError>
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        H: Fn(CommandContext, BridgeEmitRequest) -> F + Send + Sync + 'static,
    {
        let event = normalize_event_name(event.into())?;
        self.handlers.insert(
            event,
            Arc::new(move |context, request| Box::pin(handler(context, request))),
        );
        Ok(())
    }

    pub fn register_async<F, H>(&mut self, event: impl Into<String>, handler: H)
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        H: Fn(CommandContext, BridgeEmitRequest) -> F + Send + Sync + 'static,
    {
        self.try_register_async(event, handler)
            .expect("Axion event registration must use a valid event name");
    }

    pub fn event_names(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    pub fn retain_events(&mut self, allowed_events: impl IntoIterator<Item = impl Into<String>>) {
        let allowed_events = allowed_events
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        self.handlers
            .retain(|event, _handler| allowed_events.contains(event));
    }

    pub fn merge(&mut self, other: Self) {
        self.handlers.extend(other.handlers);
    }

    pub async fn dispatch(
        &self,
        context: &CommandContext,
        request: &BridgeEmitRequest,
    ) -> Result<(), EventDispatchError> {
        if !is_valid_event_name(&request.event) {
            return Err(EventDispatchError::InvalidEvent);
        }

        if request.payload.len() > BRIDGE_MAX_PAYLOAD_BYTES {
            return Err(EventDispatchError::InvalidPayload);
        }

        if !is_valid_json_value(&request.payload) {
            return Err(EventDispatchError::InvalidPayload);
        }

        let Some(handler) = self.handlers.get(&request.event) else {
            return Err(EventDispatchError::NotFound);
        };

        handler(context.clone(), request.clone())
            .await
            .map_err(EventDispatchError::Handler)
    }
}

pub fn is_valid_command_name(value: &str) -> bool {
    if value.is_empty() || value.len() > BRIDGE_MAX_NAME_BYTES {
        return false;
    }

    value.split('.').all(|segment| {
        !segment.is_empty()
            && segment.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
    })
}

fn normalize_command_name(command: String) -> Result<String, CommandRegistryError> {
    let command = command.trim().to_owned();
    if is_valid_command_name(&command) {
        Ok(command)
    } else {
        Err(CommandRegistryError::InvalidCommandName { command })
    }
}

pub fn is_valid_event_name(value: &str) -> bool {
    is_valid_command_name(value)
}

pub fn is_valid_request_id(value: &str) -> bool {
    value.len() <= BRIDGE_MAX_REQUEST_ID_BYTES
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

fn normalize_event_name(event: String) -> Result<String, BridgeEventError> {
    let event = event.trim().to_owned();
    if is_valid_event_name(&event) {
        Ok(event)
    } else {
        Err(BridgeEventError::InvalidEventName { event })
    }
}

pub fn is_valid_json_value(value: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(value)
        .is_ok_and(|value| json_depth_is_allowed(&value, BRIDGE_MAX_JSON_DEPTH))
}

fn json_depth_is_allowed(value: &serde_json::Value, remaining: usize) -> bool {
    match value {
        serde_json::Value::Array(values) => {
            remaining > 0
                && values
                    .iter()
                    .all(|value| json_depth_is_allowed(value, remaining - 1))
        }
        serde_json::Value::Object(values) => {
            remaining > 0
                && values
                    .values()
                    .all(|value| json_depth_is_allowed(value, remaining - 1))
        }
        _ => true,
    }
}

fn normalize_json_payload(payload: String) -> Result<String, BridgePayloadError> {
    let payload = payload.trim().to_owned();
    if payload.len() > BRIDGE_MAX_PAYLOAD_BYTES {
        return Err(BridgePayloadError::PayloadTooLarge {
            actual_bytes: payload.len(),
            max_bytes: BRIDGE_MAX_PAYLOAD_BYTES,
        });
    }

    if is_valid_json_value(&payload) {
        Ok(payload)
    } else {
        Err(BridgePayloadError::InvalidJsonPayload { payload })
    }
}

fn normalize_request_id(id: String) -> Result<String, BridgeRequestIdError> {
    let id = id.trim().to_owned();
    if id.len() > BRIDGE_MAX_REQUEST_ID_BYTES {
        return Err(BridgeRequestIdError::RequestIdTooLong {
            actual_bytes: id.len(),
            max_bytes: BRIDGE_MAX_REQUEST_ID_BYTES,
        });
    }

    if is_valid_request_id(&id) {
        Ok(id)
    } else {
        Err(BridgeRequestIdError::InvalidRequestId { id })
    }
}

#[derive(Debug, Clone, Default)]
pub struct BridgeBindings {
    pub command_registry: CommandRegistry,
    pub event_registry: EventRegistry,
    pub startup_events: Vec<BridgeEvent>,
}

impl BridgeBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_register_command(
        &mut self,
        command: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeRequest) -> Result<String, String>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), CommandRegistryError> {
        self.command_registry.try_register(command, handler)
    }

    pub fn register_command(
        &mut self,
        command: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeRequest) -> Result<String, String>
        + Send
        + Sync
        + 'static,
    ) {
        self.try_register_command(command, handler)
            .expect("Axion command registration must use a valid command name");
    }

    pub fn try_register_command_async<F, H>(
        &mut self,
        command: impl Into<String>,
        handler: H,
    ) -> Result<(), CommandRegistryError>
    where
        F: Future<Output = Result<String, String>> + Send + 'static,
        H: Fn(CommandContext, BridgeRequest) -> F + Send + Sync + 'static,
    {
        self.command_registry.try_register_async(command, handler)
    }

    pub fn register_command_async<F, H>(&mut self, command: impl Into<String>, handler: H)
    where
        F: Future<Output = Result<String, String>> + Send + 'static,
        H: Fn(CommandContext, BridgeRequest) -> F + Send + Sync + 'static,
    {
        self.try_register_command_async(command, handler)
            .expect("Axion command registration must use a valid command name");
    }

    pub fn try_register_event(
        &mut self,
        event: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeEmitRequest) -> Result<(), String>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), BridgeEventError> {
        self.event_registry.try_register(event, handler)
    }

    pub fn register_event(
        &mut self,
        event: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeEmitRequest) -> Result<(), String>
        + Send
        + Sync
        + 'static,
    ) {
        self.try_register_event(event, handler)
            .expect("Axion event registration must use a valid event name");
    }

    pub fn try_register_event_async<F, H>(
        &mut self,
        event: impl Into<String>,
        handler: H,
    ) -> Result<(), BridgeEventError>
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        H: Fn(CommandContext, BridgeEmitRequest) -> F + Send + Sync + 'static,
    {
        self.event_registry.try_register_async(event, handler)
    }

    pub fn register_event_async<F, H>(&mut self, event: impl Into<String>, handler: H)
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        H: Fn(CommandContext, BridgeEmitRequest) -> F + Send + Sync + 'static,
    {
        self.try_register_event_async(event, handler)
            .expect("Axion event registration must use a valid event name");
    }

    pub fn try_push_startup_event(&mut self, event: BridgeEvent) -> Result<(), BridgeEventError> {
        let event = BridgeEvent::try_new(event.name, event.payload_json)?;
        self.startup_events.push(event);
        Ok(())
    }

    pub fn push_startup_event(&mut self, event: BridgeEvent) {
        self.try_push_startup_event(event)
            .expect("Axion startup event must use a valid event name");
    }

    pub fn merge(&mut self, other: Self) {
        self.command_registry.merge(other.command_registry);
        self.event_registry.merge(other.event_registry);
        self.startup_events.extend(other.startup_events);
    }

    pub fn retain_commands(
        &mut self,
        allowed_commands: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.command_registry.retain_commands(allowed_commands);
    }

    pub fn retain_events(&mut self, allowed_events: impl IntoIterator<Item = impl Into<String>>) {
        self.event_registry.retain_events(allowed_events);
    }
}

pub trait BridgeBindingsPlugin: Send + Sync {
    fn register(&self, builder: &mut BridgeBindingsBuilder);
}

#[derive(Debug, Clone)]
pub struct BridgeBindingsBuilder {
    command_context: CommandContext,
    bindings: BridgeBindings,
}

impl BridgeBindingsBuilder {
    pub fn new(command_context: CommandContext) -> Self {
        Self {
            command_context,
            bindings: BridgeBindings::new(),
        }
    }

    pub fn command_context(&self) -> &CommandContext {
        &self.command_context
    }

    pub fn try_register_command(
        &mut self,
        command: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeRequest) -> Result<String, String>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), CommandRegistryError> {
        self.bindings.try_register_command(command, handler)
    }

    pub fn register_command(
        &mut self,
        command: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeRequest) -> Result<String, String>
        + Send
        + Sync
        + 'static,
    ) {
        self.try_register_command(command, handler)
            .expect("Axion command registration must use a valid command name");
    }

    pub fn try_register_command_async<F, H>(
        &mut self,
        command: impl Into<String>,
        handler: H,
    ) -> Result<(), CommandRegistryError>
    where
        F: Future<Output = Result<String, String>> + Send + 'static,
        H: Fn(CommandContext, BridgeRequest) -> F + Send + Sync + 'static,
    {
        self.bindings.try_register_command_async(command, handler)
    }

    pub fn register_command_async<F, H>(&mut self, command: impl Into<String>, handler: H)
    where
        F: Future<Output = Result<String, String>> + Send + 'static,
        H: Fn(CommandContext, BridgeRequest) -> F + Send + Sync + 'static,
    {
        self.try_register_command_async(command, handler)
            .expect("Axion command registration must use a valid command name");
    }

    pub fn try_register_event(
        &mut self,
        event: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeEmitRequest) -> Result<(), String>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), BridgeEventError> {
        self.bindings.try_register_event(event, handler)
    }

    pub fn register_event(
        &mut self,
        event: impl Into<String>,
        handler: impl Fn(&CommandContext, &BridgeEmitRequest) -> Result<(), String>
        + Send
        + Sync
        + 'static,
    ) {
        self.try_register_event(event, handler)
            .expect("Axion event registration must use a valid event name");
    }

    pub fn try_register_event_async<F, H>(
        &mut self,
        event: impl Into<String>,
        handler: H,
    ) -> Result<(), BridgeEventError>
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        H: Fn(CommandContext, BridgeEmitRequest) -> F + Send + Sync + 'static,
    {
        self.bindings.try_register_event_async(event, handler)
    }

    pub fn register_event_async<F, H>(&mut self, event: impl Into<String>, handler: H)
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        H: Fn(CommandContext, BridgeEmitRequest) -> F + Send + Sync + 'static,
    {
        self.try_register_event_async(event, handler)
            .expect("Axion event registration must use a valid event name");
    }

    pub fn try_push_startup_event(&mut self, event: BridgeEvent) -> Result<(), BridgeEventError> {
        self.bindings.try_push_startup_event(event)
    }

    pub fn push_startup_event(&mut self, event: BridgeEvent) {
        self.try_push_startup_event(event)
            .expect("Axion startup event must use a valid event name");
    }

    pub fn apply_plugin(&mut self, plugin: &dyn BridgeBindingsPlugin) {
        plugin.register(self);
    }

    pub fn finish(self) -> BridgeBindings {
        self.bindings
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapConfig {
    pub app_name: String,
    pub bridge_token: String,
    pub commands: Vec<String>,
    pub events: Vec<String>,
    pub host_events: Vec<String>,
    pub trusted_origins: Vec<String>,
}

impl BootstrapConfig {
    pub fn new(app_name: impl Into<String>, bridge_token: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            bridge_token: bridge_token.into(),
            commands: Vec::new(),
            events: Vec::new(),
            host_events: Vec::new(),
            trusted_origins: Vec::new(),
        }
    }

    pub fn try_with_commands(
        mut self,
        commands: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, CommandRegistryError> {
        let mut normalized = Vec::new();
        for command in commands {
            let command = normalize_command_name(command.into())?;
            if !normalized.contains(&command) {
                normalized.push(command);
            }
        }
        self.commands = normalized;
        Ok(self)
    }

    pub fn with_commands(self, commands: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.try_with_commands(commands)
            .expect("Axion bootstrap commands must use valid command names")
    }

    pub fn try_with_events(
        mut self,
        events: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, BridgeEventError> {
        let mut normalized = Vec::new();
        for event in events {
            let event = normalize_event_name(event.into())?;
            if !normalized.contains(&event) {
                normalized.push(event);
            }
        }
        self.events = normalized;
        Ok(self)
    }

    pub fn with_events(self, events: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.try_with_events(events)
            .expect("Axion bootstrap events must use valid event names")
    }

    pub fn try_with_host_events(
        mut self,
        host_events: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, BridgeEventError> {
        let mut normalized = Vec::new();
        for event in host_events {
            let event = normalize_event_name(event.into())?;
            if !normalized.contains(&event) {
                normalized.push(event);
            }
        }
        self.host_events = normalized;
        Ok(self)
    }

    pub fn with_host_events(
        self,
        host_events: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.try_with_host_events(host_events)
            .expect("Axion bootstrap host events must use valid event names")
    }

    pub fn with_trusted_origins(
        mut self,
        trusted_origins: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.trusted_origins = trusted_origins.into_iter().map(Into::into).collect();
        self
    }

    pub fn script_source(&self) -> String {
        let configuration = serde_json::json!({
            "appName": self.app_name,
            "bridgeToken": self.bridge_token,
            "commands": self.commands,
            "events": self.events,
            "hostEvents": self.host_events,
            "trustedOrigins": self.trusted_origins,
            "protocol": "axion",
            "version": format!("{AXION_RELEASE_VERSION}-bootstrap"),
            "diagnosticsReportSchema": AXION_DIAGNOSTICS_REPORT_SCHEMA,
        });
        // Assemble trusted static code first. Insert configuration only once, so
        // marker-like text in app names or tokens is never interpreted as code.
        let template = include_str!("assets/bootstrap.js")
            .replace(
                "/* AXION_COMPAT_HELPERS */",
                include_str!("assets/compat.js"),
            )
            .replace(
                "/* AXION_DIAGNOSTICS_HELPERS */",
                include_str!("assets/diagnostics.js"),
            );
        template.replacen("/* AXION_CONFIG */ null", &configuration.to_string(), 1)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    use super::{
        BRIDGE_MAX_NAME_BYTES, BRIDGE_MAX_PAYLOAD_BYTES, BRIDGE_MAX_REQUEST_ID_BYTES,
        BootstrapConfig, BridgeBindings, BridgeBindingsBuilder, BridgeBindingsPlugin,
        BridgeEmitRequest, BridgeEvent, BridgeEventError, BridgePayloadError, BridgeRequest,
        BridgeRequestIdError, BridgeRunMode, CommandContext, CommandDispatchError, CommandRegistry,
        CommandRegistryError, EventDispatchError, EventRegistry, WindowCommandContext,
        is_valid_command_name, is_valid_event_name, is_valid_json_value, is_valid_request_id,
    };

    fn context() -> CommandContext {
        CommandContext {
            app_name: "hello-axion".to_owned(),
            identifier: Some("dev.axion.hello".to_owned()),
            version: Some("1.0.0".to_owned()),
            description: Some("Hello Axion".to_owned()),
            authors: vec!["Axion Maintainers".to_owned()],
            homepage: Some("https://example.dev".to_owned()),
            mode: BridgeRunMode::Development,
            window: WindowCommandContext {
                id: "main".to_owned(),
                title: "Hello Axion".to_owned(),
                width: 960,
                height: 720,
                resizable: true,
                visible: true,
            },
        }
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
        let mut future = pin!(future);
        let mut context = Context::from_waker(&waker);

        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn command_registry_dispatches_registered_command() {
        let mut registry = CommandRegistry::default();
        registry.register("app.ping", |context, request| {
            Ok(format!(
                "{{\"appName\":\"{}\",\"payload\":{}}}",
                context.app_name, request.payload
            ))
        });

        let payload =
            block_on(registry.dispatch(&context(), &BridgeRequest::new("app.ping", "null")))
                .expect("command should dispatch");

        assert!(payload.contains("hello-axion"));
    }

    #[test]
    fn command_registry_rejects_invalid_command_names() {
        for command in ["", "../secret", "app ping", ".hidden", "app.", "app..ping"] {
            let mut registry = CommandRegistry::default();
            let error = registry
                .try_register(command, |_context, _request| Ok("{}".to_owned()))
                .expect_err("invalid command name should fail");

            assert_eq!(
                error,
                CommandRegistryError::InvalidCommandName {
                    command: command.trim().to_owned()
                }
            );
            assert!(registry.command_names().is_empty());
        }
    }

    #[test]
    #[should_panic(expected = "Axion command registration must use a valid command name")]
    fn command_registry_panics_for_invalid_command_names_on_infallible_api() {
        let mut registry = CommandRegistry::default();
        registry.register("app ping", |_context, _request| Ok("{}".to_owned()));
    }

    #[test]
    fn command_name_validation_matches_manifest_format() {
        assert!(is_valid_command_name("app.ping"));
        assert!(is_valid_command_name("fs.read_text"));
        assert!(is_valid_command_name("plugin-v1.echo_2"));
        assert!(!is_valid_command_name(""));
        assert!(!is_valid_command_name("app ping"));
        assert!(!is_valid_command_name("app..ping"));
        assert!(!is_valid_command_name("../secret"));
        assert!(!is_valid_command_name(
            &"a".repeat(BRIDGE_MAX_NAME_BYTES + 1)
        ));
    }

    #[test]
    fn request_id_validation_allows_bridge_generated_ids() {
        assert!(is_valid_request_id(""));
        assert!(is_valid_request_id("axion_mh9x_deadbeef"));
        assert!(is_valid_request_id("axion-1.2_3"));
        assert!(!is_valid_request_id("axion id"));
        assert!(!is_valid_request_id("../secret"));
        assert!(!is_valid_request_id("id/with/slash"));
        assert!(!is_valid_request_id(
            &"a".repeat(BRIDGE_MAX_REQUEST_ID_BYTES + 1)
        ));
    }

    #[test]
    fn json_payload_validation_accepts_json_values() {
        for payload in [
            "null",
            "true",
            "false",
            "\"hello\\nworld\"",
            "0",
            "-12.5e+2",
            "[]",
            "[1, true, null]",
            "{}",
            "{\"nested\":[{\"value\":\"ok\"}]}",
            "  {\"trimmed\":true}  ",
        ] {
            assert!(
                is_valid_json_value(payload),
                "payload should be valid: {payload}"
            );
        }
    }

    #[test]
    fn json_payload_validation_rejects_invalid_json() {
        for payload in [
            "",
            "undefined",
            "{",
            "{\"missing\":}",
            "{\"trailing\":true,}",
            "[1,]",
            "01",
            "\"unterminated",
            "\"bad\\xescape\"",
            "\"bad\ncontrol\"",
            "true false",
        ] {
            assert!(
                !is_valid_json_value(payload),
                "payload should be invalid: {payload}"
            );
        }
    }

    #[test]
    fn json_payload_validation_enforces_the_public_container_depth_limit() {
        let depth = super::BRIDGE_MAX_JSON_DEPTH;
        let allowed = format!("{}null{}", "[".repeat(depth), "]".repeat(depth));
        let rejected = format!("{}null{}", "[".repeat(depth + 1), "]".repeat(depth + 1));
        assert!(is_valid_json_value(&allowed));
        assert!(!is_valid_json_value(&rejected));
        assert!(BridgeRequest::try_new("app.ping", rejected).is_err());
        let empty = format!("{}{}", "[".repeat(depth + 1), "]".repeat(depth + 1));
        assert!(!is_valid_json_value(&empty));
    }

    #[test]
    fn bridge_requests_reject_invalid_payload_json() {
        let error = BridgeRequest::try_new("app.ping", "{bad")
            .expect_err("invalid command payload should fail");

        assert_eq!(
            error,
            BridgePayloadError::InvalidJsonPayload {
                payload: "{bad".to_owned()
            }
        );
    }

    #[test]
    fn bridge_requests_reject_oversized_payload_json() {
        let payload = format!("\"{}\"", "x".repeat(BRIDGE_MAX_PAYLOAD_BYTES));
        let error = BridgeRequest::try_new("app.ping", payload)
            .expect_err("oversized command payload should fail");

        assert_eq!(
            error,
            BridgePayloadError::PayloadTooLarge {
                actual_bytes: BRIDGE_MAX_PAYLOAD_BYTES + 2,
                max_bytes: BRIDGE_MAX_PAYLOAD_BYTES
            }
        );
    }

    #[test]
    fn bridge_requests_reject_invalid_request_ids() {
        let error = BridgeRequest::new("app.ping", "null")
            .try_with_id("bad id")
            .expect_err("invalid request id should fail");

        assert_eq!(
            error,
            BridgeRequestIdError::InvalidRequestId {
                id: "bad id".to_owned()
            }
        );
    }

    #[test]
    fn bridge_requests_reject_oversized_request_ids() {
        let id = "a".repeat(BRIDGE_MAX_REQUEST_ID_BYTES + 1);
        let error = BridgeEmitRequest::new("app.log", "null")
            .try_with_id(id)
            .expect_err("oversized request id should fail");

        assert_eq!(
            error,
            BridgeRequestIdError::RequestIdTooLong {
                actual_bytes: BRIDGE_MAX_REQUEST_ID_BYTES + 1,
                max_bytes: BRIDGE_MAX_REQUEST_ID_BYTES
            }
        );
    }

    #[test]
    fn bridge_event_rejects_invalid_event_names() {
        for event in ["", "../ready", "app ready", ".ready", "app.", "app..ready"] {
            let error =
                BridgeEvent::try_new(event, "{}").expect_err("invalid event name should fail");

            assert_eq!(
                error,
                BridgeEventError::InvalidEventName {
                    event: event.trim().to_owned()
                }
            );
        }
    }

    #[test]
    fn bridge_event_rejects_invalid_payload_json() {
        let error = BridgeEvent::try_new("app.ready", "{bad")
            .expect_err("invalid event payload should fail");

        assert_eq!(
            error,
            BridgeEventError::InvalidPayloadJson {
                payload: "{bad".to_owned()
            }
        );
    }

    #[test]
    #[should_panic(expected = "Axion bridge event must use a valid event name")]
    fn bridge_event_panics_for_invalid_event_names_on_infallible_api() {
        let _ = BridgeEvent::new("app ready", "{}");
    }

    #[test]
    fn bridge_bindings_rejects_manually_constructed_invalid_startup_event() {
        let mut bindings = BridgeBindings::new();
        let error = bindings
            .try_push_startup_event(BridgeEvent {
                name: "app ready".to_owned(),
                payload_json: "{}".to_owned(),
            })
            .expect_err("invalid event should fail");

        assert_eq!(
            error,
            BridgeEventError::InvalidEventName {
                event: "app ready".to_owned()
            }
        );
        assert!(bindings.startup_events.is_empty());
    }

    #[test]
    fn bridge_bindings_exposes_fallible_command_registration() {
        let mut bindings = BridgeBindings::new();

        bindings
            .try_register_command("app.ping", |_context, _request| Ok("{}".to_owned()))
            .expect("valid command should register");
        let error = bindings
            .try_register_command("app ping", |_context, _request| Ok("{}".to_owned()))
            .expect_err("invalid command should fail");

        assert!(matches!(
            error,
            CommandRegistryError::InvalidCommandName { .. }
        ));
        assert_eq!(
            bindings.command_registry.command_names(),
            vec!["app.ping".to_owned()]
        );
    }

    #[test]
    fn bridge_bindings_builder_exposes_fallible_registration() {
        let mut builder = BridgeBindingsBuilder::new(context());

        builder
            .try_register_command("plugin.echo", |_context, request| {
                Ok(request.payload.clone())
            })
            .expect("valid command should register");
        let command_error = builder
            .try_register_command("plugin echo", |_context, _request| Ok("{}".to_owned()))
            .expect_err("invalid command should fail");
        let event_error = builder
            .try_push_startup_event(BridgeEvent {
                name: "plugin ready".to_owned(),
                payload_json: "{}".to_owned(),
            })
            .expect_err("invalid event should fail");

        let bindings = builder.finish();
        assert!(matches!(
            command_error,
            CommandRegistryError::InvalidCommandName { .. }
        ));
        assert!(matches!(
            event_error,
            BridgeEventError::InvalidEventName { .. }
        ));
        assert_eq!(
            bindings.command_registry.command_names(),
            vec!["plugin.echo".to_owned()]
        );
        assert!(bindings.startup_events.is_empty());
    }

    #[test]
    fn bridge_bindings_exposes_fallible_async_command_registration() {
        let mut bindings = BridgeBindings::new();

        bindings
            .try_register_command_async("app.echo", |_context, request| async move {
                Ok(request.payload)
            })
            .expect("valid async command should register");
        let error = bindings
            .try_register_command_async("app echo", |_context, _request| async move {
                Ok("{}".to_owned())
            })
            .expect_err("invalid async command should fail");

        assert!(matches!(
            error,
            CommandRegistryError::InvalidCommandName { .. }
        ));
        assert_eq!(
            bindings.command_registry.command_names(),
            vec!["app.echo".to_owned()]
        );
    }

    #[test]
    fn event_name_validation_matches_command_format() {
        assert!(is_valid_event_name("app.ready"));
        assert!(is_valid_event_name("plugin-v1.ready_2"));
        assert!(!is_valid_event_name(""));
        assert!(!is_valid_event_name("app ready"));
        assert!(!is_valid_event_name("app..ready"));
        assert!(!is_valid_event_name(&"a".repeat(BRIDGE_MAX_NAME_BYTES + 1)));
    }

    #[test]
    fn event_registry_dispatches_registered_event() {
        let mut registry = EventRegistry::default();
        registry.register("app.log", |context, request| {
            if context.window.id != "main" || !request.payload.contains("hello") {
                return Err("unexpected event context".to_owned());
            }
            Ok(())
        });

        block_on(registry.dispatch(
            &context(),
            &BridgeEmitRequest::new("app.log", "{\"message\":\"hello\"}").with_id("evt_123"),
        ))
        .expect("event should dispatch");
    }

    #[test]
    fn event_registry_reports_not_found() {
        let registry = EventRegistry::default();
        let error = block_on(
            registry.dispatch(&context(), &BridgeEmitRequest::new("missing.event", "null")),
        )
        .expect_err("missing event should fail");

        assert_eq!(error, EventDispatchError::NotFound);
    }

    #[test]
    fn command_registry_reports_not_found() {
        let registry = CommandRegistry::default();
        let error = block_on(registry.dispatch(&context(), &BridgeRequest::new("missing", "null")))
            .expect_err("missing command should fail");

        assert_eq!(error, CommandDispatchError::NotFound);
    }

    #[test]
    fn command_registry_rejects_invalid_request_payload() {
        let mut registry = CommandRegistry::default();
        registry.register("app.ping", |_context, _request| Ok("{}".to_owned()));
        let request = BridgeRequest {
            id: String::new(),
            command: "app.ping".to_owned(),
            payload: "{bad".to_owned(),
            metadata: BTreeMap::new(),
        };

        let error = block_on(registry.dispatch(&context(), &request))
            .expect_err("invalid request payload should fail");

        assert_eq!(error, CommandDispatchError::InvalidRequestPayload);
    }

    #[test]
    fn command_registry_rejects_invalid_request_command() {
        let mut registry = CommandRegistry::default();
        registry.register("app.ping", |_context, _request| Ok("{}".to_owned()));
        let request = BridgeRequest {
            id: String::new(),
            command: "../secret".to_owned(),
            payload: "null".to_owned(),
            metadata: BTreeMap::new(),
        };

        let error = block_on(registry.dispatch(&context(), &request))
            .expect_err("invalid request command should fail");

        assert_eq!(error, CommandDispatchError::InvalidRequestCommand);
    }

    #[test]
    fn command_registry_rejects_invalid_response_payload() {
        let mut registry = CommandRegistry::default();
        registry.register("app.bad", |_context, _request| Ok("{bad".to_owned()));

        let error = block_on(registry.dispatch(&context(), &BridgeRequest::new("app.bad", "null")))
            .expect_err("invalid response payload should fail");

        assert_eq!(error, CommandDispatchError::InvalidResponsePayload);
    }

    #[test]
    fn event_registry_rejects_invalid_payload() {
        let mut registry = EventRegistry::default();
        registry.register("app.log", |_context, _request| Ok(()));
        let request = BridgeEmitRequest {
            id: String::new(),
            event: "app.log".to_owned(),
            payload: "{bad".to_owned(),
            metadata: BTreeMap::new(),
        };

        let error = block_on(registry.dispatch(&context(), &request))
            .expect_err("invalid event payload should fail");

        assert_eq!(error, EventDispatchError::InvalidPayload);
    }

    #[test]
    fn event_registry_rejects_invalid_event_name() {
        let mut registry = EventRegistry::default();
        registry.register("app.log", |_context, _request| Ok(()));
        let request = BridgeEmitRequest {
            id: String::new(),
            event: "../log".to_owned(),
            payload: "null".to_owned(),
            metadata: BTreeMap::new(),
        };

        let error = block_on(registry.dispatch(&context(), &request))
            .expect_err("invalid event name should fail");

        assert_eq!(error, EventDispatchError::InvalidEvent);
    }

    #[test]
    fn bootstrap_script_contains_bridge_primitives() {
        let script = BootstrapConfig::new("hello-axion", "token-123")
            .with_commands(["app.ping", "window.show"])
            .with_events(["app.log"])
            .with_host_events(["app.ready", "window.resized"])
            .with_trusted_origins(["axion://app", "http://127.0.0.1:3000"])
            .script_source();

        assert!(script.contains("window.__AXION__"));
        assert!(script.contains("hello-axion"));
        assert!(script.contains("app.ping"));
        assert!(script.contains("window.show"));
        assert!(script.contains("app.log"));
        assert!(script.contains("app.ready"));
        assert!(script.contains("window.resized"));
        assert!(script.contains("__axion__/${kind}"));
        assert!(script.contains("bridgeFetch('invoke'"));
        assert!(script.contains("bridgeFetch('emit'"));
        assert!(script.contains("X-Axion-Bridge-Token"));
        assert!(script.contains("nextRequestId"));
        assert!(script.contains("envelope.id"));
        assert!(script.contains("token-123"));
        assert!(script.contains("const listeners = new Map()"));
        assert!(script.contains("events: state.events"));
        assert!(script.contains("hostEvents: state.hostEvents"));
        assert!(script.contains("isListenableEvent(event)"));
        assert!(script.contains("compat: Object.freeze"));
        assert!(script.contains("installTextInputSelectionPatch"));
        assert!(script.contains("manualPointerSelection"));
        assert!(script.contains("diagnostics: Object.freeze"));
        assert!(script.contains("reportSchema: state.diagnosticsReportSchema"));
        assert!(script.contains("diagnosticsReportSchema"));
        assert!(script.contains("axion.diagnostics-report.v1"));
        assert!(script.contains("describeBridge"));
        assert!(script.contains("snapshotTextControl"));
        assert!(script.contains("normalizeError"));
        assert!(script.contains("createBridgeError"));
        assert!(script.contains("toPrettyJson"));
        assert!(script.contains("__dispatchFromHost"));
        assert!(script.contains("__dispatchFromHost(token, event, payload)"));
        assert!(script.contains("token !== state.bridgeToken"));
        assert!(script.contains("!state.hostEvents.includes(event)"));
    }

    #[test]
    fn bootstrap_configuration_is_serialized_once_without_replacing_data_markers() {
        let app_name = "quoted \"name\"; /* AXION_COMPAT_HELPERS */\n\0\u{2028}中文";
        let token = "/* AXION_CONFIG */ null /* AXION_DIAGNOSTICS_HELPERS */";
        let script = BootstrapConfig::new(app_name, token)
            .with_commands(["app.ping"])
            .with_trusted_origins(["axion://app"])
            .script_source();
        let configuration_line = script
            .lines()
            .find_map(|line| line.trim().strip_prefix("const configuration = "))
            .unwrap();
        let configuration: serde_json::Value =
            serde_json::from_str(configuration_line.strip_suffix(';').unwrap()).unwrap();
        assert_eq!(configuration["appName"], app_name);
        assert_eq!(configuration["bridgeToken"], token);
        assert_eq!(script.matches("function normalizeError(").count(), 1);
        assert_eq!(
            script
                .matches("function installTextInputSelectionPatch(")
                .count(),
            1
        );
    }

    #[test]
    fn bootstrap_config_normalizes_and_deduplicates_commands() {
        let config = BootstrapConfig::new("hello-axion", "token-123")
            .try_with_commands([" app.ping ", "app.ping", "window.info"])
            .expect("valid commands should configure");

        assert_eq!(
            config.commands,
            vec!["app.ping".to_owned(), "window.info".to_owned()]
        );
    }

    #[test]
    fn bootstrap_config_rejects_invalid_commands() {
        let error = BootstrapConfig::new("hello-axion", "token-123")
            .try_with_commands(["app.ping", "app ping"])
            .expect_err("invalid command should fail");

        assert_eq!(
            error,
            CommandRegistryError::InvalidCommandName {
                command: "app ping".to_owned()
            }
        );
    }

    #[test]
    fn bootstrap_config_normalizes_and_deduplicates_events() {
        let config = BootstrapConfig::new("hello-axion", "token-123")
            .try_with_events([" app.log ", "app.log", "window.resized"])
            .expect("valid events should configure");

        assert_eq!(
            config.events,
            vec!["app.log".to_owned(), "window.resized".to_owned()]
        );
    }

    #[test]
    fn bootstrap_config_normalizes_and_deduplicates_host_events() {
        let config = BootstrapConfig::new("hello-axion", "token-123")
            .try_with_host_events([" app.ready ", "app.ready", "window.resized"])
            .expect("valid host events should configure");

        assert_eq!(
            config.host_events,
            vec!["app.ready".to_owned(), "window.resized".to_owned()]
        );
    }

    #[test]
    fn bootstrap_config_rejects_invalid_events() {
        let error = BootstrapConfig::new("hello-axion", "token-123")
            .try_with_events(["app.log", "app log"])
            .expect_err("invalid event should fail");

        assert_eq!(
            error,
            BridgeEventError::InvalidEventName {
                event: "app log".to_owned()
            }
        );
    }

    #[test]
    fn bootstrap_config_rejects_invalid_host_events() {
        let error = BootstrapConfig::new("hello-axion", "token-123")
            .try_with_host_events(["app.ready", "app ready"])
            .expect_err("invalid host event should fail");

        assert_eq!(
            error,
            BridgeEventError::InvalidEventName {
                event: "app ready".to_owned()
            }
        );
    }

    #[test]
    #[should_panic(expected = "Axion bootstrap commands must use valid command names")]
    fn bootstrap_config_panics_for_invalid_commands_on_infallible_api() {
        let _ = BootstrapConfig::new("hello-axion", "token-123").with_commands(["app ping"]);
    }

    #[test]
    fn bridge_bindings_merge_commands_and_events() {
        let mut base = BridgeBindings::new();
        base.register_command("app.ping", |_context, _request| Ok("{}".to_owned()));
        base.register_event("app.log", |_context, _request| Ok(()));
        base.push_startup_event(BridgeEvent::new("app.ready", "{}"));

        let mut extra = BridgeBindings::new();
        extra.register_command("window.info", |_context, _request| Ok("{}".to_owned()));
        extra.register_event("window.event", |_context, _request| Ok(()));
        extra.push_startup_event(BridgeEvent::new("window.ready", "{}"));

        base.merge(extra);

        assert_eq!(
            base.command_registry.command_names(),
            vec!["app.ping".to_owned(), "window.info".to_owned()]
        );
        assert_eq!(
            base.event_registry.event_names(),
            vec!["app.log".to_owned(), "window.event".to_owned()]
        );
        assert_eq!(base.startup_events.len(), 2);
    }

    #[test]
    fn bridge_bindings_can_retain_allowed_commands() {
        let mut bindings = BridgeBindings::new();
        bindings.register_command("app.ping", |_context, _request| Ok("{}".to_owned()));
        bindings.register_command("plugin.echo", |_context, request| {
            Ok(request.payload.clone())
        });
        bindings.register_event("app.log", |_context, _request| Ok(()));
        bindings.register_event("plugin.event", |_context, _request| Ok(()));
        bindings.push_startup_event(BridgeEvent::new("plugin.ready", "{}"));

        bindings.retain_commands(["plugin.echo"]);
        bindings.retain_events(["plugin.event"]);

        assert_eq!(
            bindings.command_registry.command_names(),
            vec!["plugin.echo".to_owned()]
        );
        assert_eq!(
            bindings.event_registry.event_names(),
            vec!["plugin.event".to_owned()]
        );
        assert_eq!(bindings.startup_events.len(), 1);
    }

    struct TestPlugin;

    impl BridgeBindingsPlugin for TestPlugin {
        fn register(&self, builder: &mut BridgeBindingsBuilder) {
            builder.register_command("plugin.echo", |_context, request| {
                Ok(request.payload.clone())
            });
            builder.register_event("plugin.event", |_context, _request| Ok(()));
            builder.push_startup_event(BridgeEvent::new("plugin.ready", "{}"));
        }
    }

    #[test]
    fn bindings_builder_applies_plugin() {
        let mut builder = BridgeBindingsBuilder::new(context());
        builder.apply_plugin(&TestPlugin);
        let bindings = builder.finish();

        assert_eq!(
            bindings.command_registry.command_names(),
            vec!["plugin.echo".to_owned()]
        );
        assert_eq!(
            bindings.event_registry.event_names(),
            vec!["plugin.event".to_owned()]
        );
        assert_eq!(bindings.startup_events.len(), 1);
    }

    #[test]
    fn command_registry_dispatches_async_command() {
        let mut registry = CommandRegistry::default();
        registry.register_async("app.echo", |context, request| async move {
            Ok(format!(
                "{{\"appName\":\"{}\",\"requestId\":\"{}\",\"payload\":{}}}",
                context.app_name, request.id, request.payload
            ))
        });

        let payload = block_on(registry.dispatch(
            &context(),
            &BridgeRequest::new("app.echo", "{\"value\":1}").with_id("req_123"),
        ))
        .expect("async command should dispatch");

        assert!(payload.contains("\"requestId\":\"req_123\""));
        assert!(payload.contains("\"value\":1"));
    }
}
