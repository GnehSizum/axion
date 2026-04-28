use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Debug, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

pub const BRIDGE_MAX_NAME_BYTES: usize = 128;
pub const BRIDGE_MAX_PAYLOAD_BYTES: usize = 64 * 1024;
pub const BRIDGE_MAX_REQUEST_ID_BYTES: usize = 128;
const AXION_RELEASE_VERSION: &str = "v0.1.18.0";
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
}

#[derive(Clone, Default)]
pub struct WindowControlHandle {
    inner: Arc<Mutex<Option<Arc<dyn WindowControlExecutor>>>>,
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
        executor.execute(target_window_id, request)
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
    JsonValueParser::new(value).parse()
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

struct JsonValueParser<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> JsonValueParser<'a> {
    const MAX_DEPTH: usize = 64;

    fn new(value: &'a str) -> Self {
        Self {
            bytes: value.as_bytes(),
            cursor: 0,
        }
    }

    fn parse(mut self) -> bool {
        self.skip_whitespace();
        if self.cursor == self.bytes.len() || !self.parse_value(0) {
            return false;
        }
        self.skip_whitespace();
        self.cursor == self.bytes.len()
    }

    fn parse_value(&mut self, depth: usize) -> bool {
        if depth > Self::MAX_DEPTH {
            return false;
        }
        self.skip_whitespace();
        match self.peek() {
            Some(b'n') => self.consume_literal(b"null"),
            Some(b't') => self.consume_literal(b"true"),
            Some(b'f') => self.consume_literal(b"false"),
            Some(b'"') => self.parse_string(),
            Some(b'[') => self.parse_array(depth + 1),
            Some(b'{') => self.parse_object(depth + 1),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            _ => false,
        }
    }

    fn parse_array(&mut self, depth: usize) -> bool {
        if !self.consume_byte(b'[') {
            return false;
        }
        self.skip_whitespace();
        if self.consume_byte(b']') {
            return true;
        }
        loop {
            if !self.parse_value(depth) {
                return false;
            }
            self.skip_whitespace();
            if self.consume_byte(b']') {
                return true;
            }
            if !self.consume_byte(b',') {
                return false;
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> bool {
        if !self.consume_byte(b'{') {
            return false;
        }
        self.skip_whitespace();
        if self.consume_byte(b'}') {
            return true;
        }
        loop {
            self.skip_whitespace();
            if !self.parse_string() {
                return false;
            }
            self.skip_whitespace();
            if !self.consume_byte(b':') {
                return false;
            }
            if !self.parse_value(depth) {
                return false;
            }
            self.skip_whitespace();
            if self.consume_byte(b'}') {
                return true;
            }
            if !self.consume_byte(b',') {
                return false;
            }
        }
    }

    fn parse_string(&mut self) -> bool {
        if !self.consume_byte(b'"') {
            return false;
        }
        while let Some(byte) = self.next() {
            match byte {
                b'"' => return true,
                b'\\' => {
                    let Some(escaped) = self.next() else {
                        return false;
                    };
                    match escaped {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {}
                        b'u' => {
                            for _ in 0..4 {
                                if !self.next().is_some_and(|byte| byte.is_ascii_hexdigit()) {
                                    return false;
                                }
                            }
                        }
                        _ => return false,
                    }
                }
                0x00..=0x1f => return false,
                _ => {}
            }
        }
        false
    }

    fn parse_number(&mut self) -> bool {
        let start = self.cursor;
        let _ = self.consume_byte(b'-');

        match self.peek() {
            Some(b'0') => {
                self.cursor += 1;
                if self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                    return false;
                }
            }
            Some(b'1'..=b'9') => {
                self.cursor += 1;
                while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                    self.cursor += 1;
                }
            }
            _ => return false,
        }

        if self.consume_byte(b'.') {
            if !self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                return false;
            }
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.cursor += 1;
            }
        }

        if self.peek().is_some_and(|byte| matches!(byte, b'e' | b'E')) {
            self.cursor += 1;
            if self.peek().is_some_and(|byte| matches!(byte, b'+' | b'-')) {
                self.cursor += 1;
            }
            if !self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                return false;
            }
            while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                self.cursor += 1;
            }
        }

        self.cursor > start
    }

    fn consume_literal(&mut self, literal: &[u8]) -> bool {
        if self.bytes[self.cursor..].starts_with(literal) {
            self.cursor += literal.len();
            true
        } else {
            false
        }
    }

    fn consume_byte(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.cursor += 1;
        Some(byte)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.cursor).copied()
    }

    fn skip_whitespace(&mut self) {
        while self
            .peek()
            .is_some_and(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.cursor += 1;
        }
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
        let app_name = javascript_string_literal(&self.app_name);
        let bridge_token = javascript_string_literal(&self.bridge_token);
        let commands = self
            .commands
            .iter()
            .map(|command| javascript_string_literal(command))
            .collect::<Vec<_>>()
            .join(", ");
        let events = self
            .events
            .iter()
            .map(|event| javascript_string_literal(event))
            .collect::<Vec<_>>()
            .join(", ");
        let host_events = self
            .host_events
            .iter()
            .map(|event| javascript_string_literal(event))
            .collect::<Vec<_>>()
            .join(", ");
        let trusted_origins = self
            .trusted_origins
            .iter()
            .map(|origin| javascript_string_literal(origin))
            .collect::<Vec<_>>()
            .join(", ");
        let bridge_version =
            javascript_string_literal(&format!("{AXION_RELEASE_VERSION}-bootstrap"));
        let diagnostics_report_schema = javascript_string_literal(AXION_DIAGNOSTICS_REPORT_SCHEMA);
        let compat_helpers = bootstrap_compat_helpers();
        let diagnostics_helpers = bootstrap_diagnostics_helpers();

        format!(
            "(function() {{\n  if (window.__AXION__) return;\n  const state = Object.freeze({{\n    appName: {app_name},\n    bridgeToken: {bridge_token},\n    commands: Object.freeze([{commands}]),\n    events: Object.freeze([{events}]),\n    hostEvents: Object.freeze([{host_events}]),\n    trustedOrigins: Object.freeze([{trusted_origins}]),\n    protocol: 'axion',\n    version: {bridge_version},\n    diagnosticsReportSchema: {diagnostics_report_schema}\n  }});\n  const listeners = new Map();\n  const lastEvents = new Map();\n\n  function currentOrigin() {{\n    const url = new URL(window.location.href);\n    return `${{url.protocol}}//${{url.host}}`;\n  }}\n\n  if (!state.trustedOrigins.includes(currentOrigin())) return;\n\n  function isListenableEvent(event) {{\n    return state.events.includes(event) || state.hostEvents.includes(event);\n  }}\n\n  function dispatch(event, payload) {{\n    if (!isListenableEvent(event)) return false;\n    lastEvents.set(event, payload);\n    const handlers = listeners.get(event);\n    if (handlers) {{\n      for (const handler of handlers) {{\n        try {{ handler(payload); }} catch (error) {{ console.error('Axion listener error', error); }}\n      }}\n    }}\n    window.dispatchEvent(new CustomEvent(`axion:${{event}}`, {{ detail: payload }}));\n    return true;\n  }}\n\n  function nextRequestId() {{\n    return `axion_${{Date.now().toString(36)}}_${{Math.random().toString(16).slice(2)}}`;\n  }}\n\n  function bridgeUrl(kind, name, payload, requestId) {{\n    const encodedName = encodeURIComponent(name);\n    const payloadJson = encodeURIComponent(JSON.stringify(payload ?? null));\n    const encodedId = encodeURIComponent(requestId);\n    return `${{state.protocol}}://app/__axion__/${{kind}}/${{encodedName}}?payload=${{payloadJson}}&id=${{encodedId}}`;\n  }}\n\n  async function bridgeFetch(kind, name, payload) {{\n    const requestId = nextRequestId();\n    const response = await fetch(bridgeUrl(kind, name, payload, requestId), {{\n      headers: {{ 'X-Axion-Bridge-Token': state.bridgeToken }}\n    }});\n    const envelope = await response.json();\n    if (envelope.id && envelope.id !== requestId) {{\n      throw new Error(`Axion bridge returned an unexpected request id for ${{name}}`);\n    }}\n    if (!response.ok || envelope.ok === false) {{\n      throw new Error(envelope.error || `Axion bridge request failed: ${{name}}`);\n    }}\n    return envelope.payload;\n  }}\n\n  async function invoke(command, payload) {{\n    if (!state.commands.includes(command)) {{\n      throw new Error(`Axion command is not allowed: ${{command}}`);\n    }}\n\n    return bridgeFetch('invoke', command, payload);\n  }}\n\n  async function emit(event, payload) {{\n    if (!state.events.includes(event)) {{\n      throw new Error(`Axion event is not allowed: ${{event}}`);\n    }}\n\n    await bridgeFetch('emit', event, payload);\n    dispatch(event, payload);\n    return true;\n  }}\n\n  function listen(event, handler) {{\n    if (!isListenableEvent(event)) {{\n      throw new Error(`Axion event is not listenable: ${{event}}`);\n    }}\n    if (typeof handler !== 'function') {{\n      throw new Error('Axion listen() requires a function handler');\n    }}\n    const handlers = listeners.get(event) || new Set();\n    handlers.add(handler);\n    listeners.set(event, handlers);\n    if (lastEvents.has(event)) {{\n      handler(lastEvents.get(event));\n    }}\n    return () => {{\n      const current = listeners.get(event);\n      if (!current) return;\n      current.delete(handler);\n      if (current.size === 0) listeners.delete(event);\n    }};\n  }}\n\n{compat_helpers}\n{diagnostics_helpers}\n  window.__AXION__ = Object.freeze({{\n    ready: true,\n    appName: state.appName,\n    commands: state.commands,\n    events: state.events,\n    hostEvents: state.hostEvents,\n    trustedOrigins: state.trustedOrigins,\n    protocol: state.protocol,\n    version: state.version,\n    compat: Object.freeze({{\n      installTextInputSelectionPatch\n    }}),\n    diagnostics: Object.freeze({{\n      reportSchema: state.diagnosticsReportSchema,\n      currentOrigin,\n      describeBridge,\n      snapshotTextControl,\n      toPrettyJson\n    }}),\n    invoke,\n    emit,\n    listen,\n    __dispatchFromHost(token, event, payload) {{\n      if (token !== state.bridgeToken || !state.hostEvents.includes(event)) return false;\n      return dispatch(event, payload);\n    }}\n  }});\n}})();\n"
        )
    }
}

fn javascript_string_literal(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn bootstrap_compat_helpers() -> &'static str {
    r#"  function isTextControl(element) {
    return element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement;
  }

  function resolveLineHeight(style) {
    const raw = style.lineHeight;
    if (!raw || raw === 'normal') {
      const fontSize = Number.parseFloat(style.fontSize);
      return Number.isFinite(fontSize) ? fontSize * 1.4 : 18;
    }

    if (raw.endsWith('px')) {
      const value = Number.parseFloat(raw);
      return Number.isFinite(value) ? value : 18;
    }

    const unitless = Number.parseFloat(raw);
    if (!Number.isFinite(unitless)) {
      return 18;
    }

    const fontSize = Number.parseFloat(style.fontSize);
    return Number.isFinite(fontSize) ? unitless * fontSize : unitless;
  }

  function textMetrics(element) {
    const style = window.getComputedStyle(element);
    const canvas = document.createElement('canvas');
    const context = canvas.getContext('2d');
    const font = [
      style.fontStyle,
      style.fontVariant,
      style.fontWeight,
      style.fontSize,
      style.fontFamily
    ].filter(Boolean).join(' ');

    if (context && font) {
      context.font = font;
    }

    return {
      charWidth: context?.measureText('M').width || 8,
      lineHeight: resolveLineHeight(style),
      paddingLeft: Number.parseFloat(style.paddingLeft) || 0,
      paddingTop: Number.parseFloat(style.paddingTop) || 0,
      borderLeft: Number.parseFloat(style.borderLeftWidth) || 0,
      borderTop: Number.parseFloat(style.borderTopWidth) || 0
    };
  }

  function clamp(value, min, max) {
    return Math.max(min, Math.min(max, value));
  }

  function caretIndexFromPoint(element, point) {
    if (!isTextControl(element) || typeof point?.clientX !== 'number' || typeof point?.clientY !== 'number') {
      return null;
    }

    const rect = element.getBoundingClientRect();
    const {
      charWidth,
      lineHeight,
      paddingLeft,
      paddingTop,
      borderLeft,
      borderTop
    } = textMetrics(element);
    const relativeX = point.clientX - rect.left - borderLeft - paddingLeft + element.scrollLeft;
    const relativeY = point.clientY - rect.top - borderTop - paddingTop + element.scrollTop;

    if (element instanceof HTMLInputElement) {
      const index = clamp(Math.round(relativeX / charWidth), 0, element.value.length);
      return {
        index,
        detail: {
          kind: 'input',
          relativeX,
          charWidth,
          paddingLeft,
          borderLeft,
          scrollLeft: element.scrollLeft,
          correctedIndex: index
        }
      };
    }

    const lines = element.value.split('\n');
    const lineIndex = clamp(Math.floor(relativeY / lineHeight), 0, Math.max(lines.length - 1, 0));
    const line = lines[lineIndex] ?? '';
    const column = clamp(Math.round(relativeX / charWidth), 0, line.length);
    let absoluteIndex = column;
    for (let index = 0; index < lineIndex; index += 1) {
      absoluteIndex += lines[index].length + 1;
    }

    return {
      index: absoluteIndex,
      detail: {
        kind: 'textarea',
        relativeX,
        relativeY,
        charWidth,
        lineHeight,
        paddingLeft,
        paddingTop,
        borderLeft,
        borderTop,
        scrollLeft: element.scrollLeft,
        scrollTop: element.scrollTop,
        lineIndex,
        column,
        correctedIndex: absoluteIndex
      }
    };
  }

  function normalizeCompatOptions(options) {
    return {
      manualPointerSelection: options?.manualPointerSelection === true,
      onUpdate: typeof options?.onUpdate === 'function' ? options.onUpdate : null,
      onStatus: typeof options?.onStatus === 'function' ? options.onStatus : null
    };
  }

  function reportCompatUpdate(options, element, detail) {
    if (!options.onUpdate) {
      return;
    }

    options.onUpdate({
      targetId: element.id || null,
      selectionStart: typeof element.selectionStart === 'number' ? element.selectionStart : null,
      selectionEnd: typeof element.selectionEnd === 'number' ? element.selectionEnd : null,
      valueLength: typeof element.value === 'string' ? element.value.length : null,
      scrollLeft: typeof element.scrollLeft === 'number' ? element.scrollLeft : null,
      scrollTop: typeof element.scrollTop === 'number' ? element.scrollTop : null,
      detail
    });
  }

  function reportCompatStatus(options, message) {
    if (options.onStatus) {
      options.onStatus(message);
    }
  }

  function setCaretFromPoint(element, point, options, source) {
    const result = caretIndexFromPoint(element, point);
    if (!result) {
      return null;
    }

    element.setSelectionRange(result.index, result.index);
    reportCompatUpdate(options, element, {
      ...result.detail,
      source
    });
    return result.index;
  }

  function setSelectionFromPoint(element, anchorIndex, point, options, source) {
    const result = caretIndexFromPoint(element, point);
    if (!result) {
      return null;
    }

    const currentIndex = result.index;
    const start = Math.min(anchorIndex, currentIndex);
    const end = Math.max(anchorIndex, currentIndex);
    element.setSelectionRange(start, end);
    reportCompatUpdate(options, element, {
      ...result.detail,
      source,
      anchorIndex,
      currentIndex,
      selectionStart: start,
      selectionEnd: end
    });
    return currentIndex;
  }

  function installTextInputSelectionPatch(element, rawOptions) {
    if (!isTextControl(element)) {
      throw new Error('Axion compat patch requires an input or textarea element');
    }

    const options = normalizeCompatOptions(rawOptions);
    const listeners = [];
    const drag = {
      pointerId: null,
      anchorIndex: null,
      pendingPoint: null,
      rafId: null
    };

    function addListener(type, handler) {
      element.addEventListener(type, handler);
      listeners.push(() => element.removeEventListener(type, handler));
    }

    function clearDrag(pointerId = null) {
      if (pointerId !== null && drag.pointerId !== pointerId) {
        return;
      }

      if (drag.rafId !== null) {
        window.cancelAnimationFrame(drag.rafId);
      }

      drag.pointerId = null;
      drag.anchorIndex = null;
      drag.pendingPoint = null;
      drag.rafId = null;
    }

    function queueManualDragSelection(event) {
      drag.pendingPoint = {
        clientX: event.clientX,
        clientY: event.clientY
      };

      if (drag.rafId !== null) {
        return;
      }

      drag.rafId = window.requestAnimationFrame(() => {
        drag.rafId = null;
        if (drag.anchorIndex === null || !drag.pendingPoint) {
          return;
        }

        const currentIndex = setSelectionFromPoint(
          element,
          drag.anchorIndex,
          drag.pendingPoint,
          options,
          'drag-selection-correction'
        );
        if (currentIndex !== null) {
          reportCompatStatus(
            options,
            `Selection corrected: ${element.id || element.tagName}@${drag.anchorIndex}→${currentIndex}`
          );
        }
      });
    }

    if (!options.manualPointerSelection) {
      addListener('click', (event) => {
        window.setTimeout(() => {
          const correctedIndex = setCaretFromPoint(element, event, options, 'caret-correction');
          if (correctedIndex !== null) {
            reportCompatStatus(
              options,
              `Caret corrected after click: ${element.id || element.tagName}@${correctedIndex}`
            );
          }
        }, 0);
      });
      addListener('pointerdown', (event) => {
        window.setTimeout(() => {
          const anchorIndex = setCaretFromPoint(
            element,
            event,
            options,
            'caret-correction'
          );
          if (anchorIndex !== null) {
            drag.pointerId = event.pointerId;
            drag.anchorIndex = anchorIndex;
            reportCompatStatus(
              options,
              `Selection anchor set: ${element.id || element.tagName}@${anchorIndex}`
            );
          }
        }, 0);
      });
      addListener('pointermove', (event) => {
        if (drag.anchorIndex === null || drag.pointerId !== event.pointerId || event.buttons === 0) {
          return;
        }

        window.setTimeout(() => {
          const currentIndex = setSelectionFromPoint(
            element,
            drag.anchorIndex,
            event,
            options,
            'drag-selection-correction'
          );
          if (currentIndex !== null) {
            reportCompatStatus(
              options,
              `Selection corrected: ${element.id || element.tagName}@${drag.anchorIndex}→${currentIndex}`
            );
          }
        }, 0);
      });
      addListener('pointerup', () => clearDrag());
      addListener('pointercancel', () => clearDrag());

      return () => {
        clearDrag();
        for (const dispose of listeners.splice(0)) {
          dispose();
        }
      };
    }

    addListener('pointerdown', (event) => {
      if (event.button !== 0) {
        return;
      }

      event.preventDefault();
      element.focus();

      const anchorIndex = setCaretFromPoint(element, event, options, 'caret-correction');
      if (anchorIndex === null) {
        return;
      }

      drag.pointerId = event.pointerId;
      drag.anchorIndex = anchorIndex;

      if (typeof element.setPointerCapture === 'function') {
        try {
          element.setPointerCapture(event.pointerId);
        } catch (_error) {
        }
      }

      reportCompatStatus(
        options,
        `Manual selection anchor set: ${element.id || element.tagName}@${anchorIndex}`
      );
    });

    addListener('mousedown', (event) => {
      event.preventDefault();
    });

    addListener('mouseup', (event) => {
      event.preventDefault();
    });

    addListener('click', (event) => {
      event.preventDefault();
      event.stopPropagation();

      queueMicrotask(() => {
        const correctedIndex = setCaretFromPoint(element, event, options, 'caret-correction');
        if (correctedIndex !== null) {
          reportCompatStatus(
            options,
            `Caret corrected after click: ${element.id || element.tagName}@${correctedIndex}`
          );
        }
      });
    });

    addListener('pointermove', (event) => {
      if (drag.anchorIndex === null || drag.pointerId !== event.pointerId || event.buttons === 0) {
        return;
      }

      event.preventDefault();
      queueManualDragSelection(event);
    });

    addListener('pointerup', (event) => {
      if (drag.anchorIndex === null || drag.pointerId !== event.pointerId) {
        return;
      }

      event.preventDefault();
      const currentIndex = setSelectionFromPoint(
        element,
        drag.anchorIndex,
        event,
        options,
        'drag-selection-correction'
      );
      if (currentIndex !== null) {
        reportCompatStatus(
          options,
          `Selection corrected: ${element.id || element.tagName}@${drag.anchorIndex}→${currentIndex}`
        );
      }

      if (typeof element.releasePointerCapture === 'function') {
        try {
          element.releasePointerCapture(event.pointerId);
        } catch (_error) {
        }
      }

      clearDrag(event.pointerId);
    });

    addListener('pointercancel', (event) => {
      clearDrag(event.pointerId);
    });

    return () => {
      clearDrag();
      for (const dispose of listeners.splice(0)) {
        dispose();
      }
    };
  }
"#
}

fn bootstrap_diagnostics_helpers() -> &'static str {
    r#"  function toPrettyJson(value) {
    return JSON.stringify(value, null, 2);
  }

  function snapshotTextControl(element, detail = null) {
    const active = document.activeElement;
    return {
      targetId: element?.id ?? null,
      activeElementId: active instanceof HTMLElement ? active.id || active.tagName : null,
      selectionStart: typeof element?.selectionStart === 'number' ? element.selectionStart : null,
      selectionEnd: typeof element?.selectionEnd === 'number' ? element.selectionEnd : null,
      valueLength: typeof element?.value === 'string' ? element.value.length : null,
      scrollLeft: typeof element?.scrollLeft === 'number' ? element.scrollLeft : null,
      scrollTop: typeof element?.scrollTop === 'number' ? element.scrollTop : null,
      detail,
      devicePixelRatio: window.devicePixelRatio ?? null
    };
  }

  function describeBridge() {
    return {
      ready: true,
      appName: state.appName,
      commands: [...state.commands],
      events: [...state.events],
      hostEvents: [...state.hostEvents],
      trustedOrigins: [...state.trustedOrigins],
      protocol: state.protocol,
      version: state.version,
      diagnosticsReportSchema: state.diagnosticsReportSchema,
      currentOrigin: currentOrigin(),
      locationHref: window.location.href
    };
  }
"#
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
        assert!(script.contains("toPrettyJson"));
        assert!(script.contains("__dispatchFromHost"));
        assert!(script.contains("__dispatchFromHost(token, event, payload)"));
        assert!(script.contains("token !== state.bridgeToken"));
        assert!(script.contains("!state.hostEvents.includes(event)"));
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
