use crate::BridgeEvent;

pub const WINDOW_CREATED_EVENT: &str = "window.created";
pub const WINDOW_READY_EVENT: &str = "window.ready";
pub const WINDOW_CLOSE_REQUESTED_EVENT: &str = "window.close_requested";
pub const WINDOW_CLOSE_PREVENTED_EVENT: &str = "window.close_prevented";
pub const WINDOW_CLOSE_COMPLETED_EVENT: &str = "window.close_completed";
pub const WINDOW_CLOSE_TIMED_OUT_EVENT: &str = "window.close_timed_out";
pub const WINDOW_CLOSED_EVENT: &str = "window.closed";
pub const WINDOW_RESIZED_EVENT: &str = "window.resized";
pub const WINDOW_FOCUSED_EVENT: &str = "window.focused";
pub const WINDOW_BLURRED_EVENT: &str = "window.blurred";
pub const WINDOW_MOVED_EVENT: &str = "window.moved";
pub const WINDOW_REDRAW_FAILED_EVENT: &str = "window.redraw_failed";
pub const APP_EXIT_REQUESTED_EVENT: &str = "app.exit_requested";
pub const APP_EXIT_PREVENTED_EVENT: &str = "app.exit_prevented";
pub const APP_EXIT_COMPLETED_EVENT: &str = "app.exit_completed";

pub const WINDOW_EVENTS: &[&str] = &[
    WINDOW_CREATED_EVENT,
    WINDOW_READY_EVENT,
    WINDOW_CLOSE_REQUESTED_EVENT,
    WINDOW_CLOSE_PREVENTED_EVENT,
    WINDOW_CLOSE_COMPLETED_EVENT,
    WINDOW_CLOSE_TIMED_OUT_EVENT,
    WINDOW_CLOSED_EVENT,
    WINDOW_RESIZED_EVENT,
    WINDOW_FOCUSED_EVENT,
    WINDOW_BLURRED_EVENT,
    WINDOW_MOVED_EVENT,
    WINDOW_REDRAW_FAILED_EVENT,
];
pub const APP_EVENTS: &[&str] = &[
    APP_EXIT_REQUESTED_EVENT,
    APP_EXIT_PREVENTED_EVENT,
    APP_EXIT_COMPLETED_EVENT,
];

pub fn window_event_names() -> Vec<String> {
    WINDOW_EVENTS.iter().map(|s| (*s).to_owned()).collect()
}
pub fn app_event_names() -> Vec<String> {
    APP_EVENTS.iter().map(|s| (*s).to_owned()).collect()
}
pub fn host_event_names(startup: &[BridgeEvent]) -> Vec<String> {
    let mut names = Vec::new();
    for name in startup
        .iter()
        .map(|event| event.name.as_str())
        .chain(WINDOW_EVENTS.iter().copied())
        .chain(APP_EVENTS.iter().copied())
    {
        if !names.iter().any(|existing| existing == name) {
            names.push(name.to_owned());
        }
    }
    names
}
