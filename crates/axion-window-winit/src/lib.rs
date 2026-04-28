use thiserror::Error;

#[cfg(feature = "servo-runtime")]
pub use enabled::WindowBridgeBinding;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WinitWindowBackend;

impl WinitWindowBackend {
    pub const fn name(self) -> &'static str {
        "winit"
    }
}

#[derive(Debug, Error)]
pub enum WinitRunError {
    #[error("the Servo desktop runtime is disabled; rebuild with `--features servo-runtime`")]
    ServoRuntimeDisabled,
    #[cfg(feature = "servo-runtime")]
    #[error("app must define at least one window before the winit backend can run")]
    MissingWindow,
    #[cfg(feature = "servo-runtime")]
    #[error("missing bridge binding for window '{window_id}'")]
    MissingWindowBinding { window_id: String },
    #[cfg(feature = "servo-runtime")]
    #[error("failed to convert packaged entry '{path}' into a file URL")]
    InvalidPackagedEntry { path: std::path::PathBuf },
    #[cfg(feature = "servo-runtime")]
    #[error("failed to create winit event loop: {0}")]
    EventLoop(#[from] winit::error::EventLoopError),
    #[cfg(feature = "servo-runtime")]
    #[error("failed to create the native window: {0}")]
    CreateWindow(String),
    #[cfg(feature = "servo-runtime")]
    #[error("failed to access the native display handle: {0}")]
    DisplayHandle(String),
    #[cfg(feature = "servo-runtime")]
    #[error("failed to access the native window handle: {0}")]
    WindowHandle(String),
    #[cfg(feature = "servo-runtime")]
    #[error("failed to create the window rendering context: {0}")]
    RenderingContext(String),
    #[cfg(feature = "servo-runtime")]
    #[error("failed to make the rendering context current: {0}")]
    MakeCurrent(String),
    #[cfg(feature = "servo-runtime")]
    #[error("failed to register the Axion protocol: {0}")]
    RegisterProtocol(String),
}

#[cfg(feature = "servo-runtime")]
mod enabled {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::fs::File;
    use std::future::Future;
    use std::io::BufReader;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use axion_bridge::{
        BootstrapConfig, BridgeBindings, BridgeEmitRequest, BridgeEvent, BridgePayloadError,
        BridgeRequest, BridgeRequestIdError, CommandContext, CommandDispatchError,
        EventDispatchError, WindowControlHandle, WindowControlRequest, WindowControlResponse,
        WindowStateSnapshot, is_valid_command_name, is_valid_event_name,
    };
    use axion_core::WindowLaunchConfig;
    use axion_protocol::{AXION_SCHEME, AppAssetResolver, ResourcePolicy};
    use axion_security::SecurityPolicy;
    use embedder_traits::user_contents::UserScript;
    use euclid::Scale;
    use http::StatusCode;
    use http::header::{
        CONTENT_SECURITY_POLICY, CONTENT_TYPE, HeaderName, HeaderValue, ORIGIN, REFERER,
    };
    use net_traits::request::{Origin as RequestOrigin, Referrer};
    use servo::protocol_handler::{
        DoneChannel, FILE_CHUNK_SIZE, FetchContext, NetworkError, ProtocolHandler,
        ProtocolRegistry, RelativePos, Request, ResourceFetchTiming, Response, ResponseBody,
    };
    use servo::{
        Code, DevicePoint, InputEvent, Key, KeyState, KeyboardEvent, Location, Modifiers,
        MouseButton as ServoMouseButton, MouseButtonAction, MouseButtonEvent,
        MouseLeftViewportEvent, MouseMoveEvent, NamedKey, RenderingContext, Servo, ServoBuilder,
        TouchEvent, TouchEventType, TouchId, UserContentManager, WebView, WebViewBuilder,
        WheelDelta, WheelEvent, WheelMode, WindowRenderingContext,
    };
    use tokio::sync::mpsc::unbounded_channel;
    use url::Url;
    use winit::application::ApplicationHandler;
    use winit::dpi::LogicalSize;
    use winit::event::{
        ElementState, KeyEvent, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
    };
    use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
    use winit::keyboard::{
        Key as WinitKey, KeyCode, KeyLocation as WinitKeyLocation, ModifiersState,
        NamedKey as WinitNamedKey, PhysicalKey,
    };
    use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
    use winit::window::{Window, WindowAttributes};

    use crate::WinitRunError;

    const WINDOW_CLOSE_REQUESTED_EVENT: &str = "window.close_requested";
    const WINDOW_CLOSED_EVENT: &str = "window.closed";
    const WINDOW_RESIZED_EVENT: &str = "window.resized";
    const WINDOW_FOCUSED_EVENT: &str = "window.focused";
    const WINDOW_BLURRED_EVENT: &str = "window.blurred";
    const WINDOW_MOVED_EVENT: &str = "window.moved";
    const WINDOW_REDRAW_FAILED_EVENT: &str = "window.redraw_failed";
    const DEFAULT_SELF_TEST_TIMEOUT: Duration = Duration::from_secs(10);
    const DEFAULT_CLOSE_CONFIRM_TIMEOUT: Duration = Duration::from_secs(3);

    pub fn run_dev_server(
        app_name: String,
        _identifier: Option<String>,
        _mode: axion_core::RunMode,
        resolver: AppAssetResolver,
        windows: Vec<WindowLaunchConfig>,
        window_bindings: Vec<WindowBridgeBinding>,
        close_timeout_ms: u64,
        url: Url,
    ) -> Result<(), WinitRunError> {
        let launch = LaunchRequest::new(
            app_name,
            resolver,
            windows,
            window_bindings,
            StartupTarget::DirectUrl(url),
            close_timeout_ms,
        )?;
        run_launch(launch)
    }

    pub fn run_app_protocol(
        app_name: String,
        _identifier: Option<String>,
        _mode: axion_core::RunMode,
        windows: Vec<WindowLaunchConfig>,
        window_bindings: Vec<WindowBridgeBinding>,
        close_timeout_ms: u64,
        initial_url: Url,
        resolver: AppAssetResolver,
    ) -> Result<(), WinitRunError> {
        let launch = LaunchRequest::new(
            app_name,
            resolver.clone(),
            windows,
            window_bindings,
            StartupTarget::AppProtocol { initial_url },
            close_timeout_ms,
        )?;
        run_launch(launch)
    }

    fn run_launch(launch: LaunchRequest) -> Result<(), WinitRunError> {
        let event_loop = EventLoop::with_user_event().build()?;
        let failure = Rc::new(RefCell::new(None));
        let mut runner = WinitApp::new(&event_loop, launch, failure.clone());

        event_loop.run_app(&mut runner)?;

        if let Some(error) = failure.borrow_mut().take() {
            return Err(error);
        }

        Ok(())
    }

    #[derive(Clone, Debug)]
    struct LaunchRequest {
        app_name: String,
        resolver: AppAssetResolver,
        windows: Vec<WindowLaunchConfig>,
        window_bindings: Vec<WindowBridgeBinding>,
        startup_target: StartupTarget,
        exit_after_startup: bool,
        self_test_bridge: bool,
        gui_smoke: bool,
        self_test_timeout: Duration,
        close_timeout: Duration,
    }

    #[derive(Clone, Debug)]
    pub struct WindowBridgeBinding {
        pub window_id: String,
        pub bridge_token: String,
        pub command_context: CommandContext,
        pub bridge_bindings: BridgeBindings,
        pub security_policy: SecurityPolicy,
        pub window_control: WindowControlHandle,
    }

    #[derive(Clone, Debug)]
    enum StartupTarget {
        DirectUrl(Url),
        AppProtocol { initial_url: Url },
    }

    impl LaunchRequest {
        fn new(
            app_name: String,
            resolver: AppAssetResolver,
            windows: Vec<WindowLaunchConfig>,
            window_bindings: Vec<WindowBridgeBinding>,
            startup_target: StartupTarget,
            close_timeout_ms: u64,
        ) -> Result<Self, WinitRunError> {
            if windows.is_empty() {
                return Err(WinitRunError::MissingWindow);
            }
            for window in &windows {
                if !window_bindings
                    .iter()
                    .any(|binding| binding.window_id == window.id)
                {
                    return Err(WinitRunError::MissingWindowBinding {
                        window_id: window.id.clone(),
                    });
                }
            }

            let self_test_bridge = std::env::var_os("AXION_SELFTEST_BRIDGE").is_some();
            let gui_smoke = std::env::var_os("AXION_GUI_SMOKE").is_some();
            let self_test_timeout = launch_self_test_timeout(gui_smoke);
            let close_timeout = if close_timeout_ms == 0 {
                DEFAULT_CLOSE_CONFIRM_TIMEOUT
            } else {
                Duration::from_millis(close_timeout_ms)
            };

            Ok(Self {
                app_name,
                resolver,
                windows,
                window_bindings,
                startup_target,
                exit_after_startup: std::env::var_os("AXION_EXIT_AFTER_STARTUP").is_some()
                    && !self_test_bridge
                    && !gui_smoke,
                self_test_bridge,
                gui_smoke,
                self_test_timeout,
                close_timeout,
            })
        }

        fn window_attributes(window: &WindowLaunchConfig) -> WindowAttributes {
            Window::default_attributes()
                .with_title(window.title.clone())
                .with_inner_size(LogicalSize::new(window.width as f64, window.height as f64))
                .with_resizable(window.resizable)
                .with_visible(window.visible)
        }

        fn initial_url(&self) -> Url {
            match &self.startup_target {
                StartupTarget::DirectUrl(url) => url.clone(),
                StartupTarget::AppProtocol { initial_url } => initial_url.clone(),
            }
        }

        fn protocol_registry(&self) -> Result<ProtocolRegistry, WinitRunError> {
            let mut registry = ProtocolRegistry::default();

            registry
                .register(
                    AXION_SCHEME,
                    AxionProtocolHandler::new(self.resolver.clone(), self.window_bindings.clone()),
                )
                .map_err(|error| WinitRunError::RegisterProtocol(format!("{error:?}")))?;

            Ok(registry)
        }
    }

    fn launch_self_test_timeout(gui_smoke: bool) -> Duration {
        let override_name = if gui_smoke {
            "AXION_GUI_SMOKE_TIMEOUT_MS"
        } else {
            "AXION_SELFTEST_TIMEOUT_MS"
        };

        std::env::var(override_name)
            .ok()
            .and_then(|value| parse_timeout_ms(&value))
            .or_else(|| {
                std::env::var("AXION_SELFTEST_TIMEOUT_MS")
                    .ok()
                    .and_then(|value| parse_timeout_ms(&value))
            })
            .unwrap_or(DEFAULT_SELF_TEST_TIMEOUT)
    }

    fn parse_timeout_ms(value: &str) -> Option<Duration> {
        let millis = value.trim().parse::<u64>().ok()?;
        (millis > 0).then(|| Duration::from_millis(millis))
    }

    struct RuntimeWindow {
        window_id: String,
        bridge_token: String,
        security_policy: SecurityPolicy,
        startup_events: Vec<BridgeEvent>,
        startup_events_dispatched: std::cell::Cell<bool>,
        title: RefCell<String>,
        resizable: bool,
        visible: std::cell::Cell<bool>,
        focused: std::cell::Cell<bool>,
        cursor_position: std::cell::Cell<DevicePoint>,
        window: Window,
        rendering_context: Rc<WindowRenderingContext>,
        webview: WebView,
    }

    struct AppState {
        event_loop_proxy: EventLoopProxy<WakerEvent>,
        servo: Servo,
        self_test_bridge: bool,
        gui_smoke: bool,
        self_test_finished: std::cell::Cell<bool>,
        self_test_poll_in_flight: std::cell::Cell<bool>,
        self_test_started: std::cell::Cell<bool>,
        self_test_started_at: std::cell::Cell<Option<Instant>>,
        self_test_timeout: Duration,
        close_timeout: Duration,
        modifiers_state: std::cell::Cell<ModifiersState>,
        window_registry: Arc<Mutex<BTreeMap<String, winit::window::WindowId>>>,
        close_request_counter: std::cell::Cell<u64>,
        pending_close_requests: RefCell<BTreeMap<String, winit::window::WindowId>>,
        windows: RefCell<Vec<RuntimeWindow>>,
    }

    impl servo::WebViewDelegate for AppState {
        fn notify_new_frame_ready(&self, webview: WebView) {
            self.request_redraw_for_webview(webview.id());
        }

        fn notify_load_status_changed(
            &self,
            webview: WebView,
            status: embedder_traits::LoadStatus,
        ) {
            if status != embedder_traits::LoadStatus::Complete {
                return;
            }

            if (self.self_test_bridge || self.gui_smoke) && !self.self_test_started.replace(true) {
                self.start_self_test(webview);
            } else {
                self.queue_startup_events(&webview);
            }
        }

        fn request_navigation(
            &self,
            webview: WebView,
            navigation_request: servo::NavigationRequest,
        ) {
            if self.allows_navigation(webview.id(), &navigation_request.url) {
                navigation_request.allow();
            } else {
                navigation_request.deny();
            }
        }
    }

    impl AppState {
        fn request_redraw_for_webview(&self, webview_id: servo::WebViewId) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.webview.id() == webview_id)
            {
                runtime_window.window.request_redraw();
            }
        }

        fn first_webview(&self) -> Option<WebView> {
            self.windows
                .borrow()
                .first()
                .map(|runtime_window| runtime_window.webview.clone())
        }

        fn binding_for_webview(
            &self,
            webview_id: servo::WebViewId,
        ) -> Option<(String, Vec<BridgeEvent>)> {
            self.windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.webview.id() == webview_id)
                .map(|runtime_window| {
                    (
                        runtime_window.bridge_token.clone(),
                        runtime_window.startup_events.clone(),
                    )
                })
        }

        fn allows_navigation(&self, webview_id: servo::WebViewId, url: &Url) -> bool {
            self.windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.webview.id() == webview_id)
                .is_some_and(|runtime_window| runtime_window.security_policy.allows_navigation(url))
        }

        fn webview_for_bridge_token(&self, bridge_token: &str) -> Option<WebView> {
            self.windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.bridge_token == bridge_token)
                .map(|runtime_window| runtime_window.webview.clone())
        }

        fn dispatch_window_event(
            &self,
            window_id: winit::window::WindowId,
            event_name: &str,
            payload_json: String,
        ) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                dispatch_to_webview(
                    &runtime_window.webview,
                    &runtime_window.bridge_token,
                    event_name,
                    &payload_json,
                );
            }
        }

        fn request_close_window(
            &self,
            window_id: winit::window::WindowId,
            reason: &str,
        ) -> Result<WindowControlResponse, String> {
            if let Some((request_id, _)) = self
                .pending_close_requests
                .borrow()
                .iter()
                .find(|(_, pending_window_id)| **pending_window_id == window_id)
            {
                let state = self
                    .window_state_for_id(window_id)
                    .ok_or_else(|| "window control target is unavailable".to_owned())?;
                return Ok(WindowControlResponse::CloseRequested {
                    request_id: request_id.clone(),
                    window: state,
                });
            }

            let state = self
                .window_state_for_id(window_id)
                .ok_or_else(|| "window control target is unavailable".to_owned())?;
            let counter = self.close_request_counter.get().saturating_add(1);
            self.close_request_counter.set(counter);
            let request_id = format!("axion-close-{counter}");
            self.pending_close_requests
                .borrow_mut()
                .insert(request_id.clone(), window_id);
            self.dispatch_window_event(
                window_id,
                WINDOW_CLOSE_REQUESTED_EVENT,
                self.close_requested_payload(window_id, &request_id, reason),
            );

            let proxy = self.event_loop_proxy.clone();
            let timeout_request_id = request_id.clone();
            let close_timeout = self.close_timeout;
            std::thread::spawn(move || {
                std::thread::sleep(close_timeout);
                let _ = proxy.send_event(WakerEvent::CloseTimeout {
                    request_id: timeout_request_id,
                });
            });

            Ok(WindowControlResponse::CloseRequested {
                request_id,
                window: state,
            })
        }

        fn confirm_close_request(&self, request_id: &str) -> Result<WindowControlResponse, String> {
            let window_id = self
                .pending_close_requests
                .borrow_mut()
                .remove(request_id)
                .ok_or_else(|| format!("close request '{request_id}' is unavailable"))?;
            let state = self
                .window_state_for_id(window_id)
                .ok_or_else(|| "window control target is unavailable".to_owned())?;
            let _ = self.close_window(window_id);
            Ok(WindowControlResponse::State(state))
        }

        fn prevent_close_request(&self, request_id: &str) -> Result<WindowControlResponse, String> {
            let window_id = self
                .pending_close_requests
                .borrow_mut()
                .remove(request_id)
                .ok_or_else(|| format!("close request '{request_id}' is unavailable"))?;
            let window_id = self
                .window_id_for_winit(window_id)
                .ok_or_else(|| "window control target is unavailable".to_owned())?;
            Ok(WindowControlResponse::ClosePrevented {
                request_id: request_id.to_owned(),
                window_id,
            })
        }

        fn close_timeout(&self, request_id: &str) -> bool {
            let Some(window_id) = self.pending_close_requests.borrow_mut().remove(request_id)
            else {
                return self.windows.borrow().is_empty();
            };
            self.close_window(window_id)
        }

        fn close_window(&self, window_id: winit::window::WindowId) -> bool {
            self.dispatch_window_event(
                window_id,
                WINDOW_CLOSED_EVENT,
                self.window_payload(window_id),
            );
            self.remove_window(window_id)
        }

        fn window_state_for_id(
            &self,
            window_id: winit::window::WindowId,
        ) -> Option<WindowStateSnapshot> {
            self.windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
                .map(window_state_snapshot)
        }

        fn window_id_for_winit(&self, window_id: winit::window::WindowId) -> Option<String> {
            self.windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
                .map(|runtime_window| runtime_window.window_id.clone())
        }

        fn remove_window(&self, window_id: winit::window::WindowId) -> bool {
            let mut windows = self.windows.borrow_mut();
            let removed_window_id = if let Some(position) = windows
                .iter()
                .position(|runtime_window| runtime_window.window.id() == window_id)
            {
                Some(windows.remove(position).window_id)
            } else {
                None
            };

            let is_empty = windows.is_empty();
            drop(windows);

            if let Some(window_id) = removed_window_id {
                if let Ok(mut registry) = self.window_registry.lock() {
                    registry.remove(&window_id);
                }
            }

            is_empty
        }

        fn redraw_window(&self, window_id: winit::window::WindowId) -> Result<(), WinitRunError> {
            let windows = self.windows.borrow();
            let Some(runtime_window) = windows
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            else {
                return Ok(());
            };

            runtime_window
                .rendering_context
                .make_current()
                .map_err(|error| WinitRunError::MakeCurrent(format!("{error:?}")))?;
            runtime_window.webview.paint();
            runtime_window.rendering_context.present();
            Ok(())
        }

        fn window_payload(&self, window_id: winit::window::WindowId) -> String {
            self.windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
                .map(window_payload)
                .unwrap_or_else(|| "{\"windowId\":null}".to_owned())
        }

        fn close_requested_payload(
            &self,
            window_id: winit::window::WindowId,
            request_id: &str,
            reason: &str,
        ) -> String {
            let base_payload = self.window_payload(window_id);
            let trimmed = base_payload.trim_end_matches('}');
            format!(
                "{trimmed},\"requestId\":{},\"reason\":{},\"defaultAction\":\"allow\",\"timeoutMs\":{}}}",
                json_string_literal(request_id),
                json_string_literal(reason),
                self.close_timeout.as_millis(),
            )
        }

        fn redraw_failed_payload(
            &self,
            window_id: winit::window::WindowId,
            message: &str,
        ) -> String {
            let base_payload = self.window_payload(window_id);
            let trimmed = base_payload.trim_end_matches('}');
            format!("{trimmed},\"message\":{}}}", json_string_literal(message))
        }

        fn resize_window(
            &self,
            window_id: winit::window::WindowId,
            new_size: winit::dpi::PhysicalSize<u32>,
        ) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                runtime_window.rendering_context.resize(new_size);
                runtime_window.webview.resize(new_size);
                let payload_json = window_payload_with_size(runtime_window, new_size);
                dispatch_to_webview(
                    &runtime_window.webview,
                    &runtime_window.bridge_token,
                    WINDOW_RESIZED_EVENT,
                    &payload_json,
                );
            }
        }

        fn focus_window_event(&self, window_id: winit::window::WindowId, focused: bool) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                runtime_window.focused.set(focused);
                let event_name = if focused {
                    WINDOW_FOCUSED_EVENT
                } else {
                    WINDOW_BLURRED_EVENT
                };
                dispatch_to_webview(
                    &runtime_window.webview,
                    &runtime_window.bridge_token,
                    event_name,
                    &window_payload(runtime_window),
                );
            }
        }

        fn move_window_event(
            &self,
            window_id: winit::window::WindowId,
            position: winit::dpi::PhysicalPosition<i32>,
        ) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                let payload_json = format!(
                    "{{\"windowId\":{},\"x\":{},\"y\":{},\"scaleFactor\":{}}}",
                    json_string_literal(&runtime_window.window_id),
                    position.x,
                    position.y,
                    runtime_window.window.scale_factor(),
                );
                dispatch_to_webview(
                    &runtime_window.webview,
                    &runtime_window.bridge_token,
                    WINDOW_MOVED_EVENT,
                    &payload_json,
                );
            }
        }

        fn cursor_moved(
            &self,
            window_id: winit::window::WindowId,
            position: winit::dpi::PhysicalPosition<f64>,
        ) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                let point = DevicePoint::new(position.x as f32, position.y as f32);
                runtime_window.cursor_position.set(point);
                runtime_window
                    .webview
                    .notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(point.into())));
            }
        }

        fn cursor_left(&self, window_id: winit::window::WindowId) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                runtime_window
                    .webview
                    .notify_input_event(InputEvent::MouseLeftViewport(
                        MouseLeftViewportEvent::default(),
                    ));
            }
        }

        fn mouse_input(
            &self,
            window_id: winit::window::WindowId,
            state: ElementState,
            button: MouseButton,
        ) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                let button = match button {
                    MouseButton::Left => ServoMouseButton::Left,
                    MouseButton::Right => ServoMouseButton::Right,
                    MouseButton::Middle => ServoMouseButton::Middle,
                    MouseButton::Back => ServoMouseButton::Back,
                    MouseButton::Forward => ServoMouseButton::Forward,
                    MouseButton::Other(value) => ServoMouseButton::Other(value),
                };
                let action = match state {
                    ElementState::Pressed => MouseButtonAction::Down,
                    ElementState::Released => MouseButtonAction::Up,
                };
                runtime_window
                    .webview
                    .notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
                        action,
                        button,
                        runtime_window.cursor_position.get().into(),
                    )));
            }
        }

        fn mouse_wheel(&self, window_id: winit::window::WindowId, delta: MouseScrollDelta) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                let (x, y, mode) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        ((x * 76.0) as f64, (y * 76.0) as f64, WheelMode::DeltaLine)
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        (delta.x, delta.y, WheelMode::DeltaPixel)
                    }
                };
                runtime_window
                    .webview
                    .notify_input_event(InputEvent::Wheel(WheelEvent::new(
                        WheelDelta { x, y, z: 0.0, mode },
                        runtime_window.cursor_position.get().into(),
                    )));
            }
        }

        fn touch_event(&self, window_id: winit::window::WindowId, touch: winit::event::Touch) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                let event_type = match touch.phase {
                    TouchPhase::Started => TouchEventType::Down,
                    TouchPhase::Moved => TouchEventType::Move,
                    TouchPhase::Ended => TouchEventType::Up,
                    TouchPhase::Cancelled => TouchEventType::Cancel,
                };
                let point = DevicePoint::new(touch.location.x as f32, touch.location.y as f32);
                runtime_window.cursor_position.set(point);
                runtime_window
                    .webview
                    .notify_input_event(InputEvent::Touch(TouchEvent::new(
                        event_type,
                        TouchId(touch.id as i32),
                        point.into(),
                    )));
            }
        }

        fn set_modifiers(&self, modifiers: ModifiersState) {
            self.modifiers_state.set(modifiers);
        }

        fn keyboard_input(&self, window_id: winit::window::WindowId, event: KeyEvent) {
            if let Some(runtime_window) = self
                .windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.window.id() == window_id)
            {
                let keyboard_event = keyboard_event_from_winit(&event, self.modifiers_state.get());
                runtime_window
                    .webview
                    .notify_input_event(InputEvent::Keyboard(keyboard_event));
            }
        }

        fn apply_window_control(
            &self,
            window_id: Option<winit::window::WindowId>,
            request: WindowControlRequest,
        ) -> Result<WindowControlResponse, String> {
            if matches!(request, WindowControlRequest::ExitApp) {
                let window_ids = self
                    .windows
                    .borrow()
                    .iter()
                    .map(|runtime_window| runtime_window.window.id())
                    .collect::<Vec<_>>();
                let window_count = window_ids.len();
                let mut request_count = 0;
                for window_id in window_ids {
                    if self.request_close_window(window_id, "app-exit").is_ok() {
                        request_count += 1;
                    }
                }
                return Ok(WindowControlResponse::AppExit {
                    window_count,
                    request_count,
                });
            }

            if matches!(request, WindowControlRequest::ListStates) {
                let states = self
                    .windows
                    .borrow()
                    .iter()
                    .map(window_state_snapshot)
                    .collect();
                return Ok(WindowControlResponse::List(states));
            }

            if let WindowControlRequest::ConfirmClose { request_id } = &request {
                return self.confirm_close_request(request_id);
            }
            if let WindowControlRequest::PreventClose { request_id } = &request {
                return self.prevent_close_request(request_id);
            }

            let window_id =
                window_id.ok_or_else(|| "window control target is unavailable".to_owned())?;
            let response = {
                let windows = self.windows.borrow();
                let runtime_window = windows
                    .iter()
                    .find(|runtime_window| runtime_window.window.id() == window_id)
                    .ok_or_else(|| "window control target is unavailable".to_owned())?;

                match request {
                    WindowControlRequest::ListStates => WindowControlResponse::List(Vec::new()),
                    WindowControlRequest::ExitApp => WindowControlResponse::AppExit {
                        window_count: 0,
                        request_count: 0,
                    },
                    WindowControlRequest::ConfirmClose { .. }
                    | WindowControlRequest::PreventClose { .. } => {
                        unreachable!("close decisions are handled before window lookup")
                    }
                    WindowControlRequest::GetState => {
                        WindowControlResponse::State(window_state_snapshot(runtime_window))
                    }
                    WindowControlRequest::Show => {
                        runtime_window.window.set_visible(true);
                        runtime_window.visible.set(true);
                        WindowControlResponse::State(window_state_snapshot(runtime_window))
                    }
                    WindowControlRequest::Hide => {
                        runtime_window.window.set_visible(false);
                        runtime_window.visible.set(false);
                        WindowControlResponse::State(window_state_snapshot(runtime_window))
                    }
                    WindowControlRequest::Close => {
                        return self.request_close_window(window_id, "command");
                    }
                    WindowControlRequest::Focus => {
                        runtime_window.window.focus_window();
                        runtime_window.focused.set(true);
                        WindowControlResponse::State(window_state_snapshot(runtime_window))
                    }
                    WindowControlRequest::Reload => {
                        runtime_window.startup_events_dispatched.set(false);
                        runtime_window.webview.evaluate_javascript(
                            "window.location.reload(); 'axion-reload-requested';",
                            |_| {},
                        );
                        runtime_window.window.request_redraw();
                        WindowControlResponse::State(window_state_snapshot(runtime_window))
                    }
                    WindowControlRequest::SetTitle { title } => {
                        runtime_window.window.set_title(&title);
                        *runtime_window.title.borrow_mut() = title;
                        WindowControlResponse::State(window_state_snapshot(runtime_window))
                    }
                    WindowControlRequest::SetSize { width, height } => {
                        let _ = runtime_window
                            .window
                            .request_inner_size(LogicalSize::new(width as f64, height as f64));
                        WindowControlResponse::State(window_state_snapshot(runtime_window))
                    }
                }
            };

            Ok(response)
        }

        fn queue_startup_events(&self, webview: &WebView) {
            let Some((bridge_token, startup_events)) = self.binding_for_webview(webview.id())
            else {
                return;
            };

            if self.mark_startup_events_dispatched(&bridge_token) {
                for event in &startup_events {
                    let _ = self.event_loop_proxy.send_event(WakerEvent::DispatchEvent {
                        bridge_token: bridge_token.clone(),
                        event_name: event.name.clone(),
                        payload_json: event.payload_json.clone(),
                    });
                }
            }
        }

        fn mark_startup_events_dispatched(&self, bridge_token: &str) -> bool {
            self.windows
                .borrow()
                .iter()
                .find(|runtime_window| runtime_window.bridge_token == bridge_token)
                .is_some_and(|runtime_window| {
                    !runtime_window.startup_events_dispatched.replace(true)
                })
        }

        fn start_self_test(&self, webview: WebView) {
            self.self_test_started_at.set(Some(Instant::now()));
            let event_loop_proxy = self.event_loop_proxy.clone();
            let startup_events = self.binding_for_webview(webview.id()).and_then(
                |(bridge_token, startup_events)| {
                    self.mark_startup_events_dispatched(&bridge_token)
                        .then_some((bridge_token, startup_events))
                },
            );
            let script = if self.gui_smoke {
                "Promise.resolve()\n  .then(() => {\n    if (typeof window.__AXION_GUI_SMOKE__ !== 'function') {\n      throw new Error('window.__AXION_GUI_SMOKE__ is not available');\n    }\n    return window.__AXION_GUI_SMOKE__();\n  })\n  .then(payload => {\n    const result = payload ?? { result: 'ok' };\n    if (result.result && result.result !== 'ok') {\n      window.__AXION_SELFTEST__ = 'ERROR:' + JSON.stringify(result);\n      return;\n    }\n    window.__AXION_SELFTEST__ = JSON.stringify(result);\n  })\n  .catch(error => {\n    window.__AXION_SELFTEST__ = 'ERROR:' + error.message;\n  });\n'started';"
            } else {
                "Promise.all([\n  new Promise(resolve => window.__AXION__.listen('app.ready', payload => resolve(payload))),\n  window.__AXION__.invoke('app.ping', { from: 'axion-selftest' })\n]).then(([eventPayload, pingPayload]) => {\n  window.__AXION_SELFTEST__ = JSON.stringify({ eventPayload, pingPayload });\n}).catch(error => {\n  window.__AXION_SELFTEST__ = 'ERROR:' + error.message;\n});\n'started';"
            };
            webview.evaluate_javascript(script, move |_| {
                if let Some((bridge_token, startup_events)) = startup_events {
                    for event in startup_events {
                        let _ = event_loop_proxy.send_event(WakerEvent::DispatchEvent {
                            bridge_token: bridge_token.clone(),
                            event_name: event.name,
                            payload_json: event.payload_json,
                        });
                    }
                }
                let _ = event_loop_proxy.send_event(WakerEvent::Wake);
            });
        }

        fn poll_self_test(self: &Rc<Self>, webview: &WebView) {
            if !(self.self_test_bridge || self.gui_smoke)
                || !self.self_test_started.get()
                || self.self_test_poll_in_flight.replace(true)
            {
                return;
            }

            if self.self_test_timed_out() {
                let _ = self
                    .event_loop_proxy
                    .send_event(WakerEvent::SelfTestFailed(format!(
                        "timed out after {}ms",
                        self.self_test_timeout.as_millis()
                    )));
                return;
            }

            let app_state = self.clone();
            let event_loop_proxy = self.event_loop_proxy.clone();
            webview.evaluate_javascript("window.__AXION_SELFTEST__ ?? null", move |result| {
                app_state.self_test_poll_in_flight.set(false);
                match result {
                    Ok(embedder_traits::JSValue::String(value)) if value.starts_with("ERROR:") => {
                        let _ = event_loop_proxy.send_event(WakerEvent::SelfTestFailed(
                            value.trim_start_matches("ERROR:").to_owned(),
                        ));
                    }
                    Ok(embedder_traits::JSValue::String(value)) => {
                        let _ = event_loop_proxy.send_event(WakerEvent::SelfTestPassed(value));
                    }
                    Ok(embedder_traits::JSValue::Null | embedder_traits::JSValue::Undefined) => {
                        let _ = event_loop_proxy.send_event(WakerEvent::Wake);
                    }
                    Ok(other) => {
                        let _ = event_loop_proxy.send_event(WakerEvent::SelfTestFailed(format!(
                            "unexpected self-test value: {other:?}"
                        )));
                    }
                    Err(error) => {
                        let _ = event_loop_proxy.send_event(WakerEvent::SelfTestFailed(format!(
                            "javascript evaluation failed: {error:?}"
                        )));
                    }
                }
            });
        }

        fn self_test_timed_out(&self) -> bool {
            self.self_test_started_at
                .get()
                .is_some_and(|started_at| started_at.elapsed() >= self.self_test_timeout)
        }
    }

    enum Lifecycle {
        Initial { launch: LaunchRequest, waker: Waker },
        Running(Rc<AppState>),
        Failed,
    }

    struct WinitApp {
        lifecycle: Lifecycle,
        failure: Rc<RefCell<Option<WinitRunError>>>,
    }

    impl WinitApp {
        fn new(
            event_loop: &EventLoop<WakerEvent>,
            launch: LaunchRequest,
            failure: Rc<RefCell<Option<WinitRunError>>>,
        ) -> Self {
            Self {
                lifecycle: Lifecycle::Initial {
                    launch,
                    waker: Waker::new(event_loop),
                },
                failure,
            }
        }

        fn fail(&mut self, event_loop: &ActiveEventLoop, error: WinitRunError) {
            *self.failure.borrow_mut() = Some(error);
            self.lifecycle = Lifecycle::Failed;
            event_loop.exit();
        }
    }

    impl ApplicationHandler<WakerEvent> for WinitApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let (launch, waker) = match &self.lifecycle {
                Lifecycle::Initial { launch, waker } => (launch.clone(), waker.clone()),
                Lifecycle::Running(_) | Lifecycle::Failed => return,
            };

            match build_app_state(event_loop, launch.clone(), waker) {
                Ok(app_state) => {
                    self.lifecycle = Lifecycle::Running(app_state.clone());
                    let _ = app_state.event_loop_proxy.send_event(WakerEvent::Wake);
                    if launch.exit_after_startup {
                        event_loop.exit();
                    }
                }
                Err(error) => self.fail(event_loop, error),
            }
        }

        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: WakerEvent) {
            if let Lifecycle::Running(state) = &self.lifecycle {
                state.servo.spin_event_loop();

                match event {
                    WakerEvent::Wake => {
                        if let Some(webview) = state.first_webview() {
                            state.poll_self_test(&webview);
                        }
                    }
                    WakerEvent::DispatchEvent {
                        bridge_token,
                        event_name,
                        payload_json,
                    } => {
                        if let Some(webview) = state.webview_for_bridge_token(&bridge_token) {
                            dispatch_to_webview(
                                &webview,
                                &bridge_token,
                                &event_name,
                                &payload_json,
                            );
                        }
                    }
                    WakerEvent::WindowControl(control) => {
                        let result =
                            state.apply_window_control(control.window_id, control.request.clone());
                        let windows_empty = state.windows.borrow().is_empty();
                        let _ = control.response.send(result);
                        if windows_empty {
                            event_loop.exit();
                        }
                    }
                    WakerEvent::CloseTimeout { request_id } => {
                        if state.close_timeout(&request_id) {
                            event_loop.exit();
                        }
                    }
                    WakerEvent::SelfTestPassed(value) => {
                        if state.self_test_finished.replace(true) {
                            return;
                        }
                        if state.gui_smoke {
                            println!("Axion GUI smoke passed: {value}");
                        } else {
                            println!("Axion self-test passed: {value}");
                        }
                        event_loop.exit();
                    }
                    WakerEvent::SelfTestFailed(message) => {
                        if state.self_test_finished.replace(true) {
                            return;
                        }
                        let message = if state.gui_smoke {
                            format!("GUI smoke failed: {message}")
                        } else {
                            format!("bridge self-test failed: {message}")
                        };
                        self.fail(event_loop, WinitRunError::RegisterProtocol(message));
                    }
                }
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            let Lifecycle::Running(state) = &self.lifecycle else {
                return;
            };
            let state = state.clone();
            state.servo.spin_event_loop();
            let mut spin_after_event = false;

            match event {
                WindowEvent::CloseRequested => {
                    let _ = state.request_close_window(window_id, "system");
                }
                WindowEvent::RedrawRequested => {
                    if let Err(error) = state.redraw_window(window_id) {
                        state.dispatch_window_event(
                            window_id,
                            WINDOW_REDRAW_FAILED_EVENT,
                            state.redraw_failed_payload(window_id, &error.to_string()),
                        );
                        self.fail(event_loop, error);
                    }
                }
                WindowEvent::Resized(new_size) => {
                    state.resize_window(window_id, new_size);
                    spin_after_event = true;
                }
                WindowEvent::Focused(focused) => {
                    state.focus_window_event(window_id, focused);
                }
                WindowEvent::Moved(position) => {
                    state.move_window_event(window_id, position);
                }
                WindowEvent::CursorMoved { position, .. } => {
                    state.cursor_moved(window_id, position);
                    spin_after_event = true;
                }
                WindowEvent::CursorLeft { .. } => {
                    state.cursor_left(window_id);
                    spin_after_event = true;
                }
                WindowEvent::MouseInput {
                    state: button_state,
                    button,
                    ..
                } => {
                    state.mouse_input(window_id, button_state, button);
                    spin_after_event = true;
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    state.mouse_wheel(window_id, delta);
                    spin_after_event = true;
                }
                WindowEvent::Touch(touch) => {
                    state.touch_event(window_id, touch);
                    spin_after_event = true;
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    state.set_modifiers(modifiers.state());
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    state.keyboard_input(window_id, event);
                    spin_after_event = true;
                }
                _ => {}
            }

            if spin_after_event {
                state.servo.spin_event_loop();
            }
        }
    }

    fn build_app_state(
        event_loop: &ActiveEventLoop,
        launch: LaunchRequest,
        waker: Waker,
    ) -> Result<Rc<AppState>, WinitRunError> {
        let protocol_registry = launch.protocol_registry()?;

        let servo = ServoBuilder::default()
            .protocol_registry(protocol_registry)
            .event_loop_waker(Box::new(waker.clone()))
            .build();

        let app_state = Rc::new(AppState {
            event_loop_proxy: waker.0.clone(),
            servo,
            self_test_bridge: launch.self_test_bridge,
            gui_smoke: launch.gui_smoke,
            self_test_finished: std::cell::Cell::new(false),
            self_test_poll_in_flight: std::cell::Cell::new(false),
            self_test_started: std::cell::Cell::new(false),
            self_test_started_at: std::cell::Cell::new(None),
            self_test_timeout: launch.self_test_timeout,
            close_timeout: launch.close_timeout,
            modifiers_state: std::cell::Cell::new(ModifiersState::empty()),
            window_registry: Arc::new(Mutex::new(BTreeMap::new())),
            close_request_counter: std::cell::Cell::new(0),
            pending_close_requests: RefCell::new(BTreeMap::new()),
            windows: RefCell::new(Vec::new()),
        });

        for window_config in &launch.windows {
            let binding = launch
                .window_bindings
                .iter()
                .find(|binding| binding.window_id == window_config.id)
                .ok_or_else(|| WinitRunError::MissingWindowBinding {
                    window_id: window_config.id.clone(),
                })?;
            let display_handle = event_loop
                .display_handle()
                .map_err(|error| WinitRunError::DisplayHandle(error.to_string()))?;
            let window = event_loop
                .create_window(LaunchRequest::window_attributes(window_config))
                .map_err(|error| WinitRunError::CreateWindow(error.to_string()))?;
            let window_handle = window
                .window_handle()
                .map_err(|error| WinitRunError::WindowHandle(error.to_string()))?;

            let rendering_context = Rc::new(
                WindowRenderingContext::new(display_handle, window_handle, window.inner_size())
                    .map_err(|error| WinitRunError::RenderingContext(format!("{error:?}")))?,
            );
            rendering_context
                .make_current()
                .map_err(|error| WinitRunError::MakeCurrent(format!("{error:?}")))?;

            let user_content_manager = Rc::new(UserContentManager::new(&app_state.servo));
            if binding.security_policy.allows_protocol(AXION_SCHEME) {
                let host_events = host_event_names(&binding.bridge_bindings);
                user_content_manager.add_script(Rc::new(UserScript::new(
                    BootstrapConfig::new(launch.app_name.clone(), binding.bridge_token.clone())
                        .with_commands(binding.bridge_bindings.command_registry.command_names())
                        .with_events(binding.bridge_bindings.event_registry.event_names())
                        .with_host_events(host_events)
                        .with_trusted_origins(binding.security_policy.trusted_origins())
                        .script_source(),
                    Some(PathBuf::from("axion-bootstrap.js")),
                )));
            }

            let webview = WebViewBuilder::new(&app_state.servo, rendering_context.clone())
                .url(launch.initial_url())
                .hidpi_scale_factor(Scale::new(window.scale_factor() as f32))
                .user_content_manager(user_content_manager.clone())
                .delegate(app_state.clone())
                .build();

            if let Ok(mut registry) = app_state.window_registry.lock() {
                registry.insert(binding.window_id.clone(), window.id());
            }

            binding
                .window_control
                .install_executor(Arc::new(WinitWindowControlExecutor {
                    event_loop_proxy: app_state.event_loop_proxy.clone(),
                    window_registry: app_state.window_registry.clone(),
                    current_window_id: binding.window_id.clone(),
                }));

            app_state.windows.borrow_mut().push(RuntimeWindow {
                window_id: binding.window_id.clone(),
                bridge_token: binding.bridge_token.clone(),
                security_policy: binding.security_policy.clone(),
                startup_events: binding.bridge_bindings.startup_events.clone(),
                startup_events_dispatched: std::cell::Cell::new(false),
                title: RefCell::new(window_config.title.clone()),
                resizable: window_config.resizable,
                visible: std::cell::Cell::new(window_config.visible),
                focused: std::cell::Cell::new(false),
                cursor_position: std::cell::Cell::new(DevicePoint::new(0.0, 0.0)),
                window,
                rendering_context,
                webview,
            });
        }

        Ok(app_state)
    }

    fn dispatch_to_webview(
        webview: &WebView,
        bridge_token: &str,
        event_name: &str,
        payload_json: &str,
    ) {
        let script = format!(
            "window.__AXION__?.__dispatchFromHost?.({}, {}, {});",
            json_string_literal(bridge_token),
            json_string_literal(event_name),
            payload_json,
        );
        webview.evaluate_javascript(script, |_| {});
    }

    fn host_event_names(bindings: &BridgeBindings) -> Vec<String> {
        let mut events = Vec::new();
        for event in &bindings.startup_events {
            if !events.contains(&event.name) {
                events.push(event.name.clone());
            }
        }
        for event in [
            WINDOW_CLOSE_REQUESTED_EVENT,
            WINDOW_CLOSED_EVENT,
            WINDOW_RESIZED_EVENT,
            WINDOW_FOCUSED_EVENT,
            WINDOW_BLURRED_EVENT,
            WINDOW_MOVED_EVENT,
            WINDOW_REDRAW_FAILED_EVENT,
        ] {
            if !events.iter().any(|existing| existing == event) {
                events.push(event.to_owned());
            }
        }
        events
    }

    fn window_payload(runtime_window: &RuntimeWindow) -> String {
        window_payload_with_size(runtime_window, runtime_window.window.inner_size())
    }

    fn window_state_snapshot(runtime_window: &RuntimeWindow) -> WindowStateSnapshot {
        let size = runtime_window.window.inner_size();
        WindowStateSnapshot {
            id: runtime_window.window_id.clone(),
            title: runtime_window.title.borrow().clone(),
            width: size.width,
            height: size.height,
            resizable: runtime_window.resizable,
            visible: runtime_window.visible.get(),
            focused: runtime_window.focused.get(),
        }
    }

    fn window_payload_with_size(
        runtime_window: &RuntimeWindow,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> String {
        format!(
            "{{\"windowId\":{},\"width\":{},\"height\":{},\"scaleFactor\":{}}}",
            json_string_literal(&runtime_window.window_id),
            size.width,
            size.height,
            runtime_window.window.scale_factor(),
        )
    }

    #[derive(Clone)]
    struct Waker(EventLoopProxy<WakerEvent>);

    #[derive(Debug)]
    struct WindowControlEvent {
        window_id: Option<winit::window::WindowId>,
        request: WindowControlRequest,
        response: std::sync::mpsc::SyncSender<Result<WindowControlResponse, String>>,
    }

    #[derive(Debug, Clone)]
    enum WakerEvent {
        Wake,
        DispatchEvent {
            bridge_token: String,
            event_name: String,
            payload_json: String,
        },
        WindowControl(Arc<WindowControlEvent>),
        CloseTimeout {
            request_id: String,
        },
        SelfTestPassed(String),
        SelfTestFailed(String),
    }

    impl Waker {
        fn new(event_loop: &EventLoop<WakerEvent>) -> Self {
            Self(event_loop.create_proxy())
        }
    }

    impl embedder_traits::EventLoopWaker for Waker {
        fn clone_box(&self) -> Box<dyn embedder_traits::EventLoopWaker> {
            Box::new(self.clone())
        }

        fn wake(&self) {
            let _ = self.0.send_event(WakerEvent::Wake);
        }
    }

    fn keyboard_event_from_winit(event: &KeyEvent, modifiers: ModifiersState) -> KeyboardEvent {
        KeyboardEvent::new_without_event(
            match event.state {
                ElementState::Pressed => KeyState::Down,
                ElementState::Released => KeyState::Up,
            },
            key_from_winit(event),
            code_from_winit(event),
            location_from_winit(event),
            modifiers_from_winit(modifiers),
            event.repeat,
            false,
        )
    }

    fn key_from_winit(event: &KeyEvent) -> Key {
        match &event.logical_key {
            WinitKey::Character(value) => Key::Character(value.to_string()),
            WinitKey::Named(named) => match named {
                WinitNamedKey::Space => Key::Character(" ".to_owned()),
                WinitNamedKey::Backspace => Key::Named(NamedKey::Backspace),
                WinitNamedKey::Tab => Key::Named(NamedKey::Tab),
                WinitNamedKey::Enter => Key::Named(NamedKey::Enter),
                WinitNamedKey::Escape => Key::Named(NamedKey::Escape),
                WinitNamedKey::ArrowLeft => Key::Named(NamedKey::ArrowLeft),
                WinitNamedKey::ArrowRight => Key::Named(NamedKey::ArrowRight),
                WinitNamedKey::ArrowUp => Key::Named(NamedKey::ArrowUp),
                WinitNamedKey::ArrowDown => Key::Named(NamedKey::ArrowDown),
                WinitNamedKey::Delete => Key::Named(NamedKey::Delete),
                WinitNamedKey::Home => Key::Named(NamedKey::Home),
                WinitNamedKey::End => Key::Named(NamedKey::End),
                _ => Key::Named(NamedKey::Unidentified),
            },
            WinitKey::Dead(_) | WinitKey::Unidentified(_) => Key::Named(NamedKey::Unidentified),
        }
    }

    fn code_from_winit(event: &KeyEvent) -> Code {
        match event.physical_key {
            PhysicalKey::Code(code) => match code {
                KeyCode::Backspace => Code::Backspace,
                KeyCode::Tab => Code::Tab,
                KeyCode::Enter => Code::Enter,
                KeyCode::Escape => Code::Escape,
                KeyCode::Space => Code::Space,
                KeyCode::Delete => Code::Delete,
                KeyCode::ArrowLeft => Code::ArrowLeft,
                KeyCode::ArrowRight => Code::ArrowRight,
                KeyCode::ArrowUp => Code::ArrowUp,
                KeyCode::ArrowDown => Code::ArrowDown,
                KeyCode::Home => Code::Home,
                KeyCode::End => Code::End,
                KeyCode::KeyA => Code::KeyA,
                KeyCode::KeyB => Code::KeyB,
                KeyCode::KeyC => Code::KeyC,
                KeyCode::KeyD => Code::KeyD,
                KeyCode::KeyE => Code::KeyE,
                KeyCode::KeyF => Code::KeyF,
                KeyCode::KeyG => Code::KeyG,
                KeyCode::KeyH => Code::KeyH,
                KeyCode::KeyI => Code::KeyI,
                KeyCode::KeyJ => Code::KeyJ,
                KeyCode::KeyK => Code::KeyK,
                KeyCode::KeyL => Code::KeyL,
                KeyCode::KeyM => Code::KeyM,
                KeyCode::KeyN => Code::KeyN,
                KeyCode::KeyO => Code::KeyO,
                KeyCode::KeyP => Code::KeyP,
                KeyCode::KeyQ => Code::KeyQ,
                KeyCode::KeyR => Code::KeyR,
                KeyCode::KeyS => Code::KeyS,
                KeyCode::KeyT => Code::KeyT,
                KeyCode::KeyU => Code::KeyU,
                KeyCode::KeyV => Code::KeyV,
                KeyCode::KeyW => Code::KeyW,
                KeyCode::KeyX => Code::KeyX,
                KeyCode::KeyY => Code::KeyY,
                KeyCode::KeyZ => Code::KeyZ,
                KeyCode::Digit0 => Code::Digit0,
                KeyCode::Digit1 => Code::Digit1,
                KeyCode::Digit2 => Code::Digit2,
                KeyCode::Digit3 => Code::Digit3,
                KeyCode::Digit4 => Code::Digit4,
                KeyCode::Digit5 => Code::Digit5,
                KeyCode::Digit6 => Code::Digit6,
                KeyCode::Digit7 => Code::Digit7,
                KeyCode::Digit8 => Code::Digit8,
                KeyCode::Digit9 => Code::Digit9,
                _ => Code::Unidentified,
            },
            PhysicalKey::Unidentified(_) => Code::Unidentified,
        }
    }

    fn location_from_winit(event: &KeyEvent) -> Location {
        match event.location {
            WinitKeyLocation::Standard => Location::Standard,
            WinitKeyLocation::Left => Location::Left,
            WinitKeyLocation::Right => Location::Right,
            WinitKeyLocation::Numpad => Location::Numpad,
        }
    }

    fn modifiers_from_winit(modifiers: ModifiersState) -> Modifiers {
        let mut result = Modifiers::empty();
        if modifiers.alt_key() {
            result.insert(Modifiers::ALT);
        }
        if modifiers.control_key() {
            result.insert(Modifiers::CONTROL);
        }
        if modifiers.shift_key() {
            result.insert(Modifiers::SHIFT);
        }
        if modifiers.super_key() {
            result.insert(Modifiers::META);
        }
        result
    }

    #[derive(Clone)]
    struct WinitWindowControlExecutor {
        event_loop_proxy: EventLoopProxy<WakerEvent>,
        window_registry: Arc<Mutex<BTreeMap<String, winit::window::WindowId>>>,
        current_window_id: String,
    }

    impl axion_bridge::WindowControlExecutor for WinitWindowControlExecutor {
        fn execute(
            &self,
            target_window_id: Option<&str>,
            request: WindowControlRequest,
        ) -> Result<WindowControlResponse, String> {
            let window_id = match &request {
                WindowControlRequest::ListStates
                | WindowControlRequest::ExitApp
                | WindowControlRequest::ConfirmClose { .. }
                | WindowControlRequest::PreventClose { .. } => None,
                _ => {
                    let target_window_id =
                        target_window_id.unwrap_or(self.current_window_id.as_str());
                    let registry = self
                        .window_registry
                        .lock()
                        .map_err(|_| "window control registry lock was poisoned".to_owned())?;
                    Some(
                        *registry
                            .get(target_window_id)
                            .ok_or_else(|| format!("window '{target_window_id}' is unavailable"))?,
                    )
                }
            };
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            self.event_loop_proxy
                .send_event(WakerEvent::WindowControl(Arc::new(WindowControlEvent {
                    window_id,
                    request,
                    response: sender,
                })))
                .map_err(|_| {
                    "failed to send window control request to the event loop".to_owned()
                })?;
            receiver
                .recv()
                .map_err(|_| "window control response channel was closed".to_owned())?
        }
    }
    #[derive(Clone)]
    struct AxionProtocolHandler {
        resolver: AppAssetResolver,
        window_bindings: Vec<WindowBridgeBinding>,
        content_security_policy: String,
    }

    impl AxionProtocolHandler {
        fn new(resolver: AppAssetResolver, window_bindings: Vec<WindowBridgeBinding>) -> Self {
            let content_security_policy = window_bindings
                .first()
                .map(|binding| binding.security_policy.content_security_policy())
                .unwrap_or_else(|| {
                    "default-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'".to_owned()
                });
            Self {
                resolver,
                window_bindings,
                content_security_policy,
            }
        }

        fn binding_for_request(&self, request: &Request) -> Option<&WindowBridgeBinding> {
            let token = request
                .headers
                .get("X-Axion-Bridge-Token")
                .and_then(|value| value.to_str().ok())?;

            self.window_bindings
                .iter()
                .find(|binding| binding.bridge_token == token)
        }

        async fn response_for_invoke(&self, request: &Request) -> Response {
            let Some(binding) = self.binding_for_request(request) else {
                return json_error_response(
                    request,
                    StatusCode::FORBIDDEN,
                    None,
                    "Axion invoke token is invalid",
                );
            };

            if !binding.security_policy.allows_protocol(AXION_SCHEME) {
                return json_error_response(
                    request,
                    StatusCode::FORBIDDEN,
                    None,
                    "Axion protocol is not allowed",
                );
            }

            if !request_origin_is_trusted(request, &binding.security_policy)
                && !bridge_token_matches(request, &binding.bridge_token)
            {
                return json_error_response(
                    request,
                    StatusCode::FORBIDDEN,
                    None,
                    "Axion invoke origin is not trusted",
                );
            }

            let current_url = request.current_url();
            let path = current_url.path().trim_start_matches('/');
            let command = path.trim_start_matches("__axion__/invoke/");
            if command == path {
                return json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    None,
                    "Invalid Axion invoke path",
                );
            }
            if !is_valid_command_name(command) {
                return json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    None,
                    "Invalid Axion invoke command",
                );
            }

            let query_pairs = request
                .current_url()
                .as_url()
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect::<Vec<_>>();
            let payload = query_pairs
                .iter()
                .find(|(key, _)| key == "payload")
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| "null".to_owned());
            let request_id = query_pairs
                .iter()
                .find(|(key, _)| key == "id")
                .map(|(_, value)| value.clone());

            let mut bridge_request = match BridgeRequest::try_new(command.to_owned(), payload) {
                Ok(bridge_request) => {
                    match bridge_request.try_with_id(request_id.clone().unwrap_or_default()) {
                        Ok(bridge_request) => bridge_request,
                        Err(error) => {
                            return json_error_response(
                                request,
                                StatusCode::BAD_REQUEST,
                                None,
                                request_id_error_message(&error),
                            );
                        }
                    }
                }
                Err(BridgePayloadError::InvalidJsonPayload { .. }) => {
                    return json_error_response(
                        request,
                        StatusCode::BAD_REQUEST,
                        request_id.as_deref(),
                        "Axion invoke payload must be valid JSON",
                    );
                }
                Err(BridgePayloadError::PayloadTooLarge { .. }) => {
                    return json_error_response(
                        request,
                        StatusCode::PAYLOAD_TOO_LARGE,
                        request_id.as_deref(),
                        "Axion invoke payload is too large",
                    );
                }
            };
            if let Some(origin) = request_origin_value(request) {
                bridge_request = bridge_request.with_metadata("origin", origin);
            }
            if let Some(referrer) = request_referrer_value(request) {
                bridge_request = bridge_request.with_metadata("referrer", referrer);
            }
            bridge_request = bridge_request.with_metadata("window", binding.window_id.clone());

            match binding
                .bridge_bindings
                .command_registry
                .dispatch(&binding.command_context, &bridge_request)
                .await
            {
                Ok(payload) => json_ok_response(
                    request,
                    request_id.as_deref(),
                    format!(
                        "{{\"ok\":true,\"id\":{},\"payload\":{payload},\"error\":null}}",
                        optional_json_string_literal(request_id.as_deref())
                    ),
                ),
                Err(CommandDispatchError::NotFound) => json_error_response(
                    request,
                    StatusCode::NOT_FOUND,
                    request_id.as_deref(),
                    "Axion command not found",
                ),
                Err(CommandDispatchError::InvalidRequestCommand) => json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    request_id.as_deref(),
                    "Invalid Axion invoke command",
                ),
                Err(CommandDispatchError::InvalidRequestPayload) => json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    request_id.as_deref(),
                    "Axion invoke payload must be valid JSON",
                ),
                Err(CommandDispatchError::InvalidResponsePayload) => json_error_response(
                    request,
                    StatusCode::INTERNAL_SERVER_ERROR,
                    request_id.as_deref(),
                    "Axion command returned invalid JSON payload",
                ),
                Err(CommandDispatchError::Handler(error)) => json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    request_id.as_deref(),
                    &error,
                ),
            }
        }

        async fn response_for_emit(&self, request: &Request) -> Response {
            let Some(binding) = self.binding_for_request(request) else {
                return json_error_response(
                    request,
                    StatusCode::FORBIDDEN,
                    None,
                    "Axion emit token is invalid",
                );
            };

            if !binding.security_policy.allows_protocol(AXION_SCHEME) {
                return json_error_response(
                    request,
                    StatusCode::FORBIDDEN,
                    None,
                    "Axion protocol is not allowed",
                );
            }

            if !request_origin_is_trusted(request, &binding.security_policy)
                && !bridge_token_matches(request, &binding.bridge_token)
            {
                return json_error_response(
                    request,
                    StatusCode::FORBIDDEN,
                    None,
                    "Axion emit origin is not trusted",
                );
            }

            let current_url = request.current_url();
            let path = current_url.path().trim_start_matches('/');
            let event = path.trim_start_matches("__axion__/emit/");
            if event == path {
                return json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    None,
                    "Invalid Axion emit path",
                );
            }
            if !is_valid_event_name(event) {
                return json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    None,
                    "Invalid Axion emit event",
                );
            }

            let query_pairs = request
                .current_url()
                .as_url()
                .query_pairs()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect::<Vec<_>>();
            let payload = query_pairs
                .iter()
                .find(|(key, _)| key == "payload")
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| "null".to_owned());
            let request_id = query_pairs
                .iter()
                .find(|(key, _)| key == "id")
                .map(|(_, value)| value.clone());

            let mut emit_request = match BridgeEmitRequest::try_new(event.to_owned(), payload) {
                Ok(emit_request) => {
                    match emit_request.try_with_id(request_id.clone().unwrap_or_default()) {
                        Ok(emit_request) => emit_request,
                        Err(error) => {
                            return json_error_response(
                                request,
                                StatusCode::BAD_REQUEST,
                                None,
                                request_id_error_message(&error),
                            );
                        }
                    }
                }
                Err(BridgePayloadError::InvalidJsonPayload { .. }) => {
                    return json_error_response(
                        request,
                        StatusCode::BAD_REQUEST,
                        request_id.as_deref(),
                        "Axion emit payload must be valid JSON",
                    );
                }
                Err(BridgePayloadError::PayloadTooLarge { .. }) => {
                    return json_error_response(
                        request,
                        StatusCode::PAYLOAD_TOO_LARGE,
                        request_id.as_deref(),
                        "Axion emit payload is too large",
                    );
                }
            };
            if let Some(origin) = request_origin_value(request) {
                emit_request = emit_request.with_metadata("origin", origin);
            }
            if let Some(referrer) = request_referrer_value(request) {
                emit_request = emit_request.with_metadata("referrer", referrer);
            }
            emit_request = emit_request.with_metadata("window", binding.window_id.clone());

            match binding
                .bridge_bindings
                .event_registry
                .dispatch(&binding.command_context, &emit_request)
                .await
            {
                Ok(()) => json_ok_response(
                    request,
                    request_id.as_deref(),
                    format!(
                        "{{\"ok\":true,\"id\":{},\"payload\":null,\"error\":null}}",
                        optional_json_string_literal(request_id.as_deref())
                    ),
                ),
                Err(EventDispatchError::NotFound) => json_error_response(
                    request,
                    StatusCode::NOT_FOUND,
                    request_id.as_deref(),
                    "Axion event not found",
                ),
                Err(EventDispatchError::InvalidEvent) => json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    request_id.as_deref(),
                    "Invalid Axion emit event",
                ),
                Err(EventDispatchError::InvalidPayload) => json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    request_id.as_deref(),
                    "Axion emit payload must be valid JSON",
                ),
                Err(EventDispatchError::Handler(error)) => json_error_response(
                    request,
                    StatusCode::BAD_REQUEST,
                    request_id.as_deref(),
                    &error,
                ),
            }
        }

        fn response_for_asset(
            &self,
            request: &mut Request,
            done_chan: &mut DoneChannel,
            context: &FetchContext,
        ) -> Pin<Box<dyn Future<Output = Response> + Send>> {
            let resolved = match self
                .resolver
                .resolve_existing_request_path(request.current_url().path())
            {
                Ok(resolved) => resolved,
                Err(error) => {
                    return Box::pin(std::future::ready(Response::network_error(
                        NetworkError::ResourceLoadError(error.to_string()),
                    )));
                }
            };

            let response = if let Ok(file) = File::open(&resolved.file_path) {
                let mut response = Response::new(
                    request.current_url(),
                    ResourceFetchTiming::new(request.timing_type()),
                );
                let reader = BufReader::with_capacity(FILE_CHUNK_SIZE, file);

                let resource_policy = ResourcePolicy::for_asset(&resolved);
                for (name, value) in resource_policy.headers {
                    if let (Ok(name), Ok(value)) = (
                        HeaderName::from_bytes(name.as_bytes()),
                        HeaderValue::from_str(&value),
                    ) {
                        response.headers.insert(name, value);
                    }
                }
                if let Ok(header_value) = HeaderValue::from_str(&self.content_security_policy) {
                    response
                        .headers
                        .insert(CONTENT_SECURITY_POLICY, header_value);
                }

                let (mut done_sender, done_receiver) = unbounded_channel();
                *done_chan = Some((done_sender.clone(), done_receiver));
                *response.body.lock() = ResponseBody::Receiving(vec![]);

                context.filemanager.fetch_file_in_chunks(
                    &mut done_sender,
                    reader,
                    response.body.clone(),
                    context.cancellation_listener.clone(),
                    RelativePos::full_range(),
                );

                response
            } else {
                Response::network_error(NetworkError::ResourceLoadError(
                    "Opening Axion asset failed".to_owned(),
                ))
            };

            Box::pin(std::future::ready(response))
        }
    }

    impl ProtocolHandler for AxionProtocolHandler {
        fn load<'a>(
            &'a self,
            request: &'a mut Request,
            done_chan: &mut DoneChannel,
            context: &FetchContext,
        ) -> Pin<Box<dyn Future<Output = Response> + Send + 'a>> {
            if request
                .current_url()
                .path()
                .starts_with("/__axion__/invoke/")
            {
                return Box::pin(self.response_for_invoke(request));
            }
            if request.current_url().path().starts_with("/__axion__/emit/") {
                return Box::pin(self.response_for_emit(request));
            }

            self.response_for_asset(request, done_chan, context)
        }

        fn is_fetchable(&self) -> bool {
            true
        }

        fn is_secure(&self) -> bool {
            true
        }
    }

    fn json_ok_response(request: &Request, _request_id: Option<&str>, body: String) -> Response {
        json_response(request, StatusCode::OK, body)
    }

    fn request_id_error_message(error: &BridgeRequestIdError) -> &'static str {
        match error {
            BridgeRequestIdError::InvalidRequestId { .. } => "Axion request id is invalid",
            BridgeRequestIdError::RequestIdTooLong { .. } => "Axion request id is too large",
        }
    }

    fn json_error_response(
        request: &Request,
        status: StatusCode,
        request_id: Option<&str>,
        message: &str,
    ) -> Response {
        json_response(
            request,
            status,
            format!(
                "{{\"ok\":false,\"id\":{},\"payload\":null,\"error\":{}}}",
                optional_json_string_literal(request_id),
                json_string_literal(message),
            ),
        )
    }

    fn json_response(request: &Request, status: StatusCode, body: String) -> Response {
        let mut response = Response::new(
            request.current_url(),
            ResourceFetchTiming::new(request.timing_type()),
        );
        response.status = status.into();
        response.headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        *response.body.lock() = ResponseBody::Done(body.into_bytes());
        response
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
            .filter(|value| !value.is_empty())
            .map(json_string_literal)
            .unwrap_or_else(|| "null".to_owned())
    }

    fn request_origin_value(request: &Request) -> Option<String> {
        request
            .headers
            .get(ORIGIN)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .or_else(|| match &request.origin {
                RequestOrigin::Origin(origin) => Some(origin.ascii_serialization()),
                RequestOrigin::Client => None,
            })
    }

    fn request_referrer_value(request: &Request) -> Option<String> {
        request
            .headers
            .get(REFERER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .or_else(|| match &request.referrer {
                Referrer::Client(url) | Referrer::ReferrerUrl(url) => Some(url.to_string()),
                Referrer::NoReferrer => None,
            })
    }

    fn request_origin_is_trusted(request: &Request, security_policy: &SecurityPolicy) -> bool {
        let request_origin = match &request.origin {
            RequestOrigin::Origin(origin) => Some(origin.ascii_serialization()),
            RequestOrigin::Client => None,
        };

        if request_origin
            .as_ref()
            .is_some_and(|origin| security_policy.is_trusted_origin(origin))
        {
            return true;
        }

        let referrer_origin = match &request.referrer {
            Referrer::Client(url) | Referrer::ReferrerUrl(url) => {
                Some(url.origin().ascii_serialization())
            }
            Referrer::NoReferrer => None,
        };

        if referrer_origin
            .as_ref()
            .is_some_and(|origin| security_policy.is_trusted_origin(origin))
        {
            return true;
        }

        let origin_header = request
            .headers
            .get(ORIGIN)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);

        if let Some(origin) = origin_header {
            return security_policy.is_trusted_origin(&origin);
        }

        let referer_origin = request
            .headers
            .get(REFERER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| Url::parse(value).ok())
            .map(|url| SecurityPolicy::origin_string(&url));

        referer_origin
            .as_ref()
            .is_some_and(|origin| security_policy.is_trusted_origin(origin))
    }

    fn bridge_token_matches(request: &Request, expected_token: &str) -> bool {
        request
            .headers
            .get("X-Axion-Bridge-Token")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == expected_token)
    }

    #[cfg(test)]
    mod tests {
        use std::time::Duration;

        use super::parse_timeout_ms;

        #[test]
        fn parse_timeout_ms_accepts_positive_values() {
            assert_eq!(parse_timeout_ms("1"), Some(Duration::from_millis(1)));
            assert_eq!(
                parse_timeout_ms(" 2500 "),
                Some(Duration::from_millis(2500))
            );
        }

        #[test]
        fn parse_timeout_ms_rejects_invalid_values() {
            assert_eq!(parse_timeout_ms("0"), None);
            assert_eq!(parse_timeout_ms("-1"), None);
            assert_eq!(parse_timeout_ms("abc"), None);
        }
    }
}

#[cfg(feature = "servo-runtime")]
pub use enabled::{run_app_protocol, run_dev_server};
