//! **One window's share of the library's thread state.**
//!
//! A window installs its hooks once — where a frame request goes, where a menu, an action and an
//! unclaimed drop are handed — and a layout leaves behind the room and the theme it used. Each of
//! these is one per thread, because the UI runs on one thread. A program with **several windows on
//! that thread** keeps one [`WindowContext`] per window and enters it before the window handles
//! anything, so a widget in the second window never asks the first one for a frame or opens its menu
//! there.
//!
//! ```ignore
//! install_frame_request(..); install_menu_sink(..);   // while building window two
//! let two = WindowContext::capture();
//! one.enter();   /* window one handles its event */   one.leave();
//! two.enter();   /* window two handles its event */   two.leave();
//! ```
//!
//! A program with one window needs none of this: what it installed stays installed.

use crate::component::{FrameWake, frame_waker};
use heca_core::layout::Size;
use std::rc::Rc;

/// What one window installed in the library, and what its last layout left behind.
#[derive(Clone)]
pub struct WindowContext {
    frame: FrameWake,
    menu: Option<crate::menu::MenuSink>,
    intent: Option<crate::intent::IntentSink>,
    drop: Option<crate::drag::sink::DropSink>,
    viewport: Size,
    theme: Rc<heca_theme::Theme>,
}

impl WindowContext {
    /// **What is installed now** — call it right after building a window, while its hooks are the
    /// ones in place.
    pub fn capture() -> Self {
        let (viewport, theme) = crate::layout::laid_out();
        Self {
            frame: frame_waker(),
            menu: crate::menu::menu_sink(),
            intent: crate::intent::intent_sink(),
            drop: crate::drag::sink::drop_sink(),
            viewport,
            theme,
        }
    }

    /// **This window's turn**: put its hooks and its last layout back in place.
    pub fn enter(&self) {
        crate::component::set_frame_waker(self.frame.clone());
        crate::menu::set_menu_sink(self.menu.clone());
        crate::intent::set_intent_sink(self.intent.clone());
        crate::drag::sink::set_drop_sink(self.drop.clone());
        crate::layout::set_laid_out(self.viewport, self.theme.clone());
    }

    /// **The turn is over**: keep what its layout left behind (the room, the theme) for the next.
    pub fn leave(&mut self) {
        let (viewport, theme) = crate::layout::laid_out();
        self.viewport = viewport;
        self.theme = theme;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// **Each window's requests reach that window** — a frame asked for, a menu shown and an action
    /// sent while window two has its turn go to window two's hooks, and window one's again on its
    /// turn; and each keeps the room its own last layout had.
    #[test]
    fn each_window_hears_only_what_was_asked_during_its_own_turn() {
        let frames = [Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0))];
        let actions: [Rc<RefCell<Vec<String>>>; 2] = Default::default();
        let mut windows = Vec::new();
        for (i, room) in [(0usize, 300.0), (1, 900.0)] {
            let frame = frames[i].clone();
            crate::install_frame_request(move || {
                frame.fetch_add(1, Ordering::Relaxed);
            });
            let said = actions[i].clone();
            crate::intent::install_intent_sink(move |intent| said.borrow_mut().push(intent.action));
            crate::layout::set_laid_out(
                Size::new(room, room),
                Rc::new(heca_theme::Theme::default()),
            );
            windows.push(WindowContext::capture());
        }
        for (i, window) in windows.iter_mut().enumerate() {
            window.enter();
            crate::component::frame_served();
            crate::request_frame();
            crate::intent::dispatch(heca_view::Intent::new(format!("from-{i}")));
            assert_eq!(
                crate::layout::viewport().w,
                [300.0, 900.0][i],
                "its own room"
            );
            window.leave();
        }
        assert_eq!(frames[0].load(Ordering::Relaxed), 1);
        assert_eq!(frames[1].load(Ordering::Relaxed), 1);
        assert_eq!(*actions[0].borrow(), ["from-0"]);
        assert_eq!(*actions[1].borrow(), ["from-1"]);
    }
}
