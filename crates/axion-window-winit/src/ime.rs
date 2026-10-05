use servo::{
    CompositionEvent, CompositionState, DeviceIntRect, EmbedderControlId, ImeEvent, InputEvent,
    KeyboardEvent,
};
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::Ime;

#[derive(Default)]
pub(crate) struct ImeState {
    visible_control: Option<EmbedderControlId>,
    enabled: bool,
    composing: bool,
    awaiting_disabled: bool,
    pending_end_control: Option<EmbedderControlId>,
}

impl ImeState {
    pub(crate) fn show(&mut self, control_id: EmbedderControlId) {
        if self.visible_control != Some(control_id) {
            self.composing = false;
            self.pending_end_control = None;
        }
        self.visible_control = Some(control_id);
    }

    pub(crate) fn hide(&mut self, control_id: EmbedderControlId) -> bool {
        if self.visible_control != Some(control_id) {
            return false;
        }
        self.visible_control = None;
        self.composing = false;
        self.pending_end_control = None;
        // winit reports Disabled after the native IME was enabled. Remember
        // this acknowledgement even if Servo focuses another field meanwhile.
        self.awaiting_disabled |= self.enabled;
        self.enabled = false;
        true
    }

    pub(crate) fn input_events(&mut self, event: Ime) -> Vec<InputEvent> {
        if event == Ime::Disabled && self.awaiting_disabled {
            self.awaiting_disabled = false;
            return Vec::new();
        }
        if self.visible_control.is_none() {
            return Vec::new();
        }
        let mut events = Vec::new();
        let event = match event {
            Ime::Enabled => {
                // Native IME availability is not a composition boundary: macOS
                // can keep it enabled across several preedit/commit cycles.
                self.enabled = true;
                return events;
            }
            Ime::Preedit(text, _) => {
                let composing = !text.is_empty();
                if composing {
                    if let Some(event) = self.finish_cleared_preedit() {
                        events.push(event);
                    }
                } else if self.composing {
                    // winit clears preedit immediately before Commit as well as
                    // on cancellation. Wait until this native batch completes.
                    self.pending_end_control = self.visible_control;
                }
                if composing && !self.composing {
                    events.push(InputEvent::Ime(ImeEvent::Composition(CompositionEvent {
                        state: CompositionState::Start,
                        data: String::new(),
                    })));
                }
                self.composing = composing;
                ImeEvent::Composition(CompositionEvent {
                    state: CompositionState::Update,
                    data: text,
                })
            }
            Ime::Commit(text) => {
                self.pending_end_control = None;
                self.composing = false;
                // Committed text is delivered only through composition, never
                // also synthesized as a character KeyboardEvent.
                ImeEvent::Composition(CompositionEvent {
                    state: CompositionState::End,
                    data: text,
                })
            }
            Ime::Disabled => {
                // macOS also disables IME when switching keyboard input sources.
                // Keep Servo's focused control; its hide callback handles blur.
                // A backend can disable IME without first clearing preedit.
                if self.composing {
                    self.pending_end_control = self.visible_control;
                }
                if let Some(event) = self.finish_cleared_preedit() {
                    events.push(event);
                }
                self.enabled = false;
                self.composing = false;
                return events;
            }
        };
        events.push(InputEvent::Ime(event));
        events
    }

    pub(crate) fn finish_cleared_preedit(&mut self) -> Option<InputEvent> {
        let control_id = self.pending_end_control.take()?;
        if self.visible_control != Some(control_id) {
            return None;
        }
        Some(InputEvent::Ime(ImeEvent::Composition(CompositionEvent {
            state: CompositionState::End,
            data: String::new(),
        })))
    }

    pub(crate) fn mark_keyboard_event(&self, event: &mut KeyboardEvent) {
        event.event.is_composing = self.composing;
    }
}

pub(crate) fn cursor_area(position: DeviceIntRect) -> (LogicalPosition<i32>, LogicalSize<i32>) {
    // InputMethodControl's rectangle follows servoshell's logical window
    // coordinate convention. Axion has no toolbar offset to add.
    (
        LogicalPosition::new(position.min.x, position.min.y),
        LogicalSize::new(
            position.max.x - position.min.x,
            position.max.y - position.min.y,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control_id(index: u32) -> EmbedderControlId {
        serde_json::from_value(serde_json::json!({
            "webview_id": [9999, { "namespace_id": 0, "index": [1, null] }],
            "pipeline_id": { "namespace_id": 0, "index": [1, null] },
            "index": index,
        }))
        .unwrap()
    }

    fn assert_composition(events: Vec<InputEvent>, state: CompositionState, data: &str) {
        assert_eq!(events.len(), 1);
        let Some(InputEvent::Ime(ImeEvent::Composition(event))) = events.into_iter().next() else {
            panic!("expected one native composition event");
        };
        assert_eq!(event.state, state);
        assert_eq!(event.data, data);
    }

    fn assert_start_and_update(events: Vec<InputEvent>, data: &str) {
        assert_eq!(events.len(), 2);
        let mut events = events.into_iter();
        assert_composition(vec![events.next().unwrap()], CompositionState::Start, "");
        assert_composition(vec![events.next().unwrap()], CompositionState::Update, data);
    }

    #[test]
    fn preedit_and_commit_use_one_native_composition_path() {
        let mut state = ImeState::default();
        state.show(control_id(1));
        assert!(state.input_events(Ime::Enabled).is_empty());
        assert_start_and_update(
            state.input_events(Ime::Preedit("ni".to_owned(), Some((2, 2)))),
            "ni",
        );
        let mut keyboard = KeyboardEvent::from_state_and_key(
            servo::KeyState::Down,
            servo::Key::Character("n".to_owned()),
        );
        state.mark_keyboard_event(&mut keyboard);
        assert!(keyboard.event.is_composing);
        assert_composition(
            state.input_events(Ime::Preedit(String::new(), None)),
            CompositionState::Update,
            "",
        );
        assert_composition(
            state.input_events(Ime::Commit("你好🙂".to_owned())),
            CompositionState::End,
            "你好🙂",
        );
        state.mark_keyboard_event(&mut keyboard);
        assert!(!keyboard.event.is_composing);
    }

    #[test]
    fn each_commit_cycle_starts_without_another_native_enable() {
        let mut state = ImeState::default();
        state.show(control_id(1));
        assert!(state.input_events(Ime::Enabled).is_empty());
        for (preedit, committed) in [("ni", "你"), ("hao", "好")] {
            assert_start_and_update(
                state.input_events(Ime::Preedit(preedit.to_owned(), None)),
                preedit,
            );
            assert_composition(
                state.input_events(Ime::Preedit(String::new(), None)),
                CompositionState::Update,
                "",
            );
            assert_composition(
                state.input_events(Ime::Commit(committed.to_owned())),
                CompositionState::End,
                committed,
            );
        }
    }

    #[test]
    fn changing_nonempty_preedit_keeps_one_composition_start() {
        let mut state = ImeState::default();
        state.show(control_id(1));
        assert_start_and_update(state.input_events(Ime::Preedit("n".to_owned(), None)), "n");
        assert_composition(
            state.input_events(Ime::Preedit("ni".to_owned(), None)),
            CompositionState::Update,
            "ni",
        );
    }

    #[test]
    fn new_preedit_after_a_clear_starts_again() {
        let mut state = ImeState::default();
        state.show(control_id(1));
        state.input_events(Ime::Preedit("zh".to_owned(), None));
        assert_composition(
            state.input_events(Ime::Preedit(String::new(), None)),
            CompositionState::Update,
            "",
        );
        assert!(!state.composing);
        let mut events = state.input_events(Ime::Preedit("n".to_owned(), None));
        assert_eq!(events.len(), 3);
        assert_composition(vec![events.remove(0)], CompositionState::End, "");
        assert_start_and_update(events, "n");
        assert!(state.finish_cleared_preedit().is_none());
    }

    #[test]
    fn cancelled_preedit_ends_once_at_the_native_batch_boundary() {
        let mut state = ImeState::default();
        state.show(control_id(1));
        state.input_events(Ime::Enabled);
        state.input_events(Ime::Preedit("zh".to_owned(), None));
        assert_composition(
            state.input_events(Ime::Preedit(String::new(), None)),
            CompositionState::Update,
            "",
        );
        assert_composition(
            vec![state.finish_cleared_preedit().unwrap()],
            CompositionState::End,
            "",
        );
        assert!(state.finish_cleared_preedit().is_none());
        assert!(!state.composing);
    }

    #[test]
    fn commit_after_empty_preedit_cancels_the_pending_empty_end() {
        let mut state = ImeState::default();
        state.show(control_id(1));
        state.input_events(Ime::Enabled);
        state.input_events(Ime::Preedit("ni".to_owned(), None));
        assert_composition(
            state.input_events(Ime::Preedit(String::new(), None)),
            CompositionState::Update,
            "",
        );
        assert_composition(
            state.input_events(Ime::Commit("你".to_owned())),
            CompositionState::End,
            "你",
        );
        assert!(state.finish_cleared_preedit().is_none());
    }

    #[test]
    fn focus_change_and_hide_drop_the_previous_pending_end() {
        let mut state = ImeState::default();
        let previous = control_id(1);
        let next = control_id(2);
        state.show(previous);
        state.input_events(Ime::Preedit("zh".to_owned(), None));
        state.input_events(Ime::Preedit(String::new(), None));
        state.show(next);
        assert!(state.finish_cleared_preedit().is_none());
        state.input_events(Ime::Preedit("ni".to_owned(), None));
        state.input_events(Ime::Preedit(String::new(), None));
        assert!(!state.hide(previous));
        assert!(state.hide(next));
        assert!(state.finish_cleared_preedit().is_none());
    }

    #[test]
    fn source_disable_finishes_a_cleared_preedit_without_blurring() {
        let mut state = ImeState::default();
        let current = control_id(1);
        state.show(current);
        state.input_events(Ime::Enabled);
        state.input_events(Ime::Preedit("zh".to_owned(), None));
        state.input_events(Ime::Preedit(String::new(), None));
        assert_composition(state.input_events(Ime::Disabled), CompositionState::End, "");
        assert_eq!(state.visible_control, Some(current));
        assert!(state.finish_cleared_preedit().is_none());
    }

    #[test]
    fn backend_hide_does_not_dismiss_a_newly_focused_control() {
        let mut state = ImeState::default();
        let previous = control_id(1);
        let next = control_id(2);
        state.show(previous);
        state.input_events(Ime::Enabled);
        assert!(state.hide(previous));
        state.show(next);
        assert!(state.input_events(Ime::Disabled).is_empty());
        assert_eq!(state.visible_control, Some(next));
        assert!(state.input_events(Ime::Enabled).is_empty());
        assert!(state.input_events(Ime::Disabled).is_empty());
        assert_eq!(state.visible_control, Some(next));
        assert!(!state.enabled);
    }

    #[test]
    fn hide_without_native_enable_does_not_swallow_source_disable() {
        let mut state = ImeState::default();
        state.show(control_id(1));
        assert!(state.hide(control_id(1)));
        state.show(control_id(2));
        state.input_events(Ime::Enabled);
        assert!(state.input_events(Ime::Disabled).is_empty());
        assert!(!state.enabled);
        assert_eq!(state.visible_control, Some(control_id(2)));
    }

    #[test]
    fn switching_input_sources_preserves_the_focused_control() {
        let mut state = ImeState::default();
        let current = control_id(1);
        state.show(current);
        state.input_events(Ime::Enabled);
        state.input_events(Ime::Preedit("ni".to_owned(), None));
        assert_composition(state.input_events(Ime::Disabled), CompositionState::End, "");
        assert!(state.input_events(Ime::Disabled).is_empty());
        assert!(state.finish_cleared_preedit().is_none());
        assert_eq!(state.visible_control, Some(current));
        assert!(!state.enabled);
        let mut keyboard = KeyboardEvent::from_state_and_key(
            servo::KeyState::Down,
            servo::Key::Character("a".to_owned()),
        );
        state.mark_keyboard_event(&mut keyboard);
        assert!(!keyboard.event.is_composing);
        assert!(state.input_events(Ime::Enabled).is_empty());
        assert_start_and_update(
            state.input_events(Ime::Preedit("hao".to_owned(), None)),
            "hao",
        );
    }

    #[test]
    fn stale_hide_cannot_disable_the_current_control() {
        let mut state = ImeState::default();
        state.show(control_id(2));
        state.input_events(Ime::Preedit("中文".to_owned(), Some((6, 6))));
        assert!(!state.hide(control_id(1)));
        assert_eq!(state.visible_control, Some(control_id(2)));
        assert!(state.composing);
    }

    #[test]
    fn inactive_or_hidden_controls_ignore_late_input() {
        let mut state = ImeState::default();
        assert!(
            state
                .input_events(Ime::Commit("late".to_owned()))
                .is_empty()
        );
        let current = control_id(1);
        state.show(current);
        state.input_events(Ime::Enabled);
        assert!(state.hide(current));
        assert!(state.input_events(Ime::Disabled).is_empty());
        assert!(state.visible_control.is_none());
        assert!(
            state
                .input_events(Ime::Commit("late".to_owned()))
                .is_empty()
        );
    }

    #[test]
    fn candidate_area_keeps_logical_position_and_size() {
        let rectangle = DeviceIntRect::new(
            servo::DeviceIntPoint::new(12, 24),
            servo::DeviceIntPoint::new(112, 64),
        );
        let (position, size) = cursor_area(rectangle);
        assert_eq!(position, LogicalPosition::new(12, 24));
        assert_eq!(size, LogicalSize::new(100, 40));
    }
}
