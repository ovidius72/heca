use super::*;

#[test]
fn a_key_is_named_in_the_backends_terms_with_what_was_held() {
    let ctrl = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    let event = key_event(GridKey::Char('c'), ctrl);
    assert_eq!(event.code, BackendKeyCode::Char('c'));
    assert!(event.modifiers.ctrl && !event.modifiers.shift && !event.modifiers.alt);
}

#[test]
fn the_command_key_is_the_backends_super() {
    let meta = Modifiers {
        meta: true,
        ..Default::default()
    };
    assert!(key_event(GridKey::Enter, meta).modifiers.super_);
}

#[test]
fn every_key_the_tree_can_deliver_has_a_backend_name() {
    assert_eq!(key_code(GridKey::Space), BackendKeyCode::Char(' '));
    assert_eq!(key_code(GridKey::PageUp), BackendKeyCode::PageUp);
    assert_eq!(key_code(GridKey::Function(5)), BackendKeyCode::Function(5));
    assert_eq!(key_code(GridKey::ArrowLeft), BackendKeyCode::LeftArrow);
}

/// A terminal that is either at the live bottom or back in its history.
struct Scrolled {
    inner: heca_core::backend::FakeBackend,
    back: bool,
    heard: Vec<u8>,
}

impl PaneBackend for Scrolled {
    fn pane_type(&self) -> heca_core::backend::PaneType {
        self.inner.pane_type()
    }
    fn title(&self) -> &str {
        self.inner.title()
    }
    fn set_size(&mut self, cols: usize, rows: usize) {
        self.inner.set_size(cols, rows);
    }
    fn process_input(&mut self, data: &[u8]) {
        self.heard.extend_from_slice(data);
    }
    fn update(&mut self) -> bool {
        self.inner.update()
    }
    fn render_data(&self) -> heca_core::backend::BackendRenderData {
        self.inner.render_data()
    }
    fn should_close(&self) -> bool {
        self.inner.should_close()
    }
    fn at_bottom(&self) -> bool {
        !self.back
    }
    fn scroll_to_bottom(&mut self) {
        self.back = false;
    }
}

/// **Typing asks for a frame only when the view moved.** At the live bottom, what a key changes on
/// screen is the program's echo, which arrives as output and asks for its own frame — drawing on the
/// key itself repainted the same picture and queued it ahead of the echo. Back in the history, the
/// view snaps down, and that is a change to draw.
#[test]
fn typing_asks_for_a_frame_only_when_the_view_moved() {
    let at = |back: bool| Scrolled {
        inner: heca_core::backend::FakeBackend::new(80, 24),
        back,
        heard: Vec::new(),
    };
    let mut bottom = at(false);
    assert!(
        !typed(&mut bottom, |b| b.process_input(b"a")),
        "at the bottom: no frame, the echo brings one"
    );
    assert_eq!(bottom.heard, b"a", "the process still hears it");

    let mut history = at(true);
    assert!(
        typed(&mut history, |b| b.process_input(b"a")),
        "back in the history: the view snaps down, a frame"
    );
    assert!(history.at_bottom(), "and it is at the bottom now");
}
