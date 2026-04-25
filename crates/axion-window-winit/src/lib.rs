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
    use std::fs::File;
    use std::future::Future;
    use std::io::BufReader;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::rc::Rc;

    use axion_bridge::{
        BootstrapConfig, BridgeBindings, BridgeEmitRequest, BridgeEvent, BridgePayloadError,
        BridgeRequest, BridgeRequestIdError, CommandContext, CommandDispatchError,
        EventDispatchError, is_valid_command_name, is_valid_event_name,
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
        RenderingContext, Servo, ServoBuilder, UserContentManager, WebView, WebViewBuilder,
        WindowRenderingContext,
    };
    use tokio::sync::mpsc::unbounded_channel;
    use url::Url;
    use winit::application::ApplicationHandler;
    use winit::dpi::LogicalSize;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
    use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
    use winit::window::{Window, WindowAttributes};

    use crate::WinitRunError;

    const WINDOW_CLOSE_REQUESTED_EVENT: &str = "window.close_requested";
    const WINDOW_CLOSED_EVENT: &str = "window.closed";
    const WINDOW_RESIZED_EVENT: &str = "window.resized";
    const WINDOW_REDRAW_FAILED_EVENT: &str = "window.redraw_failed";

    pub fn run_dev_server(
        app_name: String,
        _identifier: Option<String>,
        _mode: axion_core::RunMode,
        resolver: AppAssetResolver,
        windows: Vec<WindowLaunchConfig>,
        window_bindings: Vec<WindowBridgeBinding>,
        url: Url,
    ) -> Result<(), WinitRunError> {
        let launch = LaunchRequest::new(
            app_name,
            resolver,
            windows,
            window_bindings,
            StartupTarget::DirectUrl(url),
        )?;
        run_launch(launch)
    }

    pub fn run_app_protocol(
        app_name: String,
        _identifier: Option<String>,
        _mode: axion_core::RunMode,
        windows: Vec<WindowLaunchConfig>,
        window_bindings: Vec<WindowBridgeBinding>,
        initial_url: Url,
        resolver: AppAssetResolver,
    ) -> Result<(), WinitRunError> {
        let launch = LaunchRequest::new(
            app_name,
            resolver.clone(),
            windows,
            window_bindings,
            StartupTarget::AppProtocol { initial_url },
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
    }

    #[derive(Clone, Debug)]
    pub struct WindowBridgeBinding {
        pub window_id: String,
        pub bridge_token: String,
        pub command_context: CommandContext,
        pub bridge_bindings: BridgeBindings,
        pub security_policy: SecurityPolicy,
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

            Ok(Self {
                app_name,
                resolver,
                windows,
                window_bindings,
                startup_target,
                exit_after_startup: std::env::var_os("AXION_EXIT_AFTER_STARTUP").is_some()
                    && std::env::var_os("AXION_SELFTEST_BRIDGE").is_none(),
                self_test_bridge: std::env::var_os("AXION_SELFTEST_BRIDGE").is_some(),
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

    struct RuntimeWindow {
        window_id: String,
        bridge_token: String,
        security_policy: SecurityPolicy,
        startup_events: Vec<BridgeEvent>,
        startup_events_dispatched: std::cell::Cell<bool>,
        window: Window,
        rendering_context: Rc<WindowRenderingContext>,
        webview: WebView,
    }

    struct AppState {
        event_loop_proxy: EventLoopProxy<WakerEvent>,
        servo: Servo,
        self_test_bridge: bool,
        self_test_finished: std::cell::Cell<bool>,
        self_test_poll_in_flight: std::cell::Cell<bool>,
        self_test_started: std::cell::Cell<bool>,
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

            if self.self_test_bridge && !self.self_test_started.replace(true) {
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

        fn close_window(&self, window_id: winit::window::WindowId) -> bool {
            self.dispatch_window_event(
                window_id,
                WINDOW_CLOSE_REQUESTED_EVENT,
                self.window_payload(window_id),
            );
            self.dispatch_window_event(
                window_id,
                WINDOW_CLOSED_EVENT,
                self.window_payload(window_id),
            );
            self.remove_window(window_id)
        }

        fn remove_window(&self, window_id: winit::window::WindowId) -> bool {
            let mut windows = self.windows.borrow_mut();
            if let Some(position) = windows
                .iter()
                .position(|runtime_window| runtime_window.window.id() == window_id)
            {
                windows.remove(position);
            }

            windows.is_empty()
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
            let event_loop_proxy = self.event_loop_proxy.clone();
            let startup_events = self.binding_for_webview(webview.id()).and_then(
                |(bridge_token, startup_events)| {
                    self.mark_startup_events_dispatched(&bridge_token)
                        .then_some((bridge_token, startup_events))
                },
            );
            webview.evaluate_javascript(
                "Promise.all([\n  new Promise(resolve => window.__AXION__.listen('app.ready', payload => resolve(payload))),\n  window.__AXION__.invoke('app.ping', { from: 'axion-selftest' })\n]).then(([eventPayload, pingPayload]) => {\n  window.__AXION_SELFTEST__ = JSON.stringify({ eventPayload, pingPayload });\n}).catch(error => {\n  window.__AXION_SELFTEST__ = 'ERROR:' + error.message;\n});\n'started';",
                move |_| {
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
                },
            );
        }

        fn poll_self_test(self: &Rc<Self>, webview: &WebView) {
            if !self.self_test_bridge
                || !self.self_test_started.get()
                || self.self_test_poll_in_flight.replace(true)
            {
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
                    WakerEvent::SelfTestPassed(value) => {
                        if state.self_test_finished.replace(true) {
                            return;
                        }
                        println!("Axion self-test passed: {value}");
                        event_loop.exit();
                    }
                    WakerEvent::SelfTestFailed(message) => {
                        if state.self_test_finished.replace(true) {
                            return;
                        }
                        self.fail(
                            event_loop,
                            WinitRunError::RegisterProtocol(format!(
                                "bridge self-test failed: {message}"
                            )),
                        );
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
            if let Lifecycle::Running(state) = &self.lifecycle {
                state.servo.spin_event_loop();

                match event {
                    WindowEvent::CloseRequested => {
                        if state.close_window(window_id) {
                            event_loop.exit();
                        }
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
                    }
                    _ => {}
                }
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
            self_test_finished: std::cell::Cell::new(false),
            self_test_poll_in_flight: std::cell::Cell::new(false),
            self_test_started: std::cell::Cell::new(false),
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

            app_state.windows.borrow_mut().push(RuntimeWindow {
                window_id: binding.window_id.clone(),
                bridge_token: binding.bridge_token.clone(),
                security_policy: binding.security_policy.clone(),
                startup_events: binding.bridge_bindings.startup_events.clone(),
                startup_events_dispatched: std::cell::Cell::new(false),
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

    #[derive(Debug, Clone)]
    enum WakerEvent {
        Wake,
        DispatchEvent {
            bridge_token: String,
            event_name: String,
            payload_json: String,
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
}

#[cfg(feature = "servo-runtime")]
pub use enabled::{run_app_protocol, run_dev_server};
