//! **Which terminal the keyboard acts on** — the one answer every terminal key reads: search,
//! selection, the scroll keys and the font size.

use super::*;

impl AppState {
    /// A selected pane is not observed while another window, overlay or dock owns input.
    pub(crate) fn input_focused_pane(&self) -> Option<heca_core::layout::PaneId> {
        effective_pane(self.window_focused, self.layers.top_modal_id(&self.window_root).is_some(),
            self.window_keyboard_terminal().and_then(|id| self.client.backends.pane_of(id)))
    }

    /// The terminal the keyboard acts on: while selecting or searching, the terminal being selected
    /// in or searched; otherwise the terminal holding the window's keyboard — in a pane, a dock, an
    /// overlay, anywhere. When no terminal holds it (a sidebar list does, say), the focused pane's.
    ///
    /// A selection left on screen does not hold the keyboard: once its mode is left, the next key
    /// goes where the keyboard is, not to the terminal that still shows a highlight.
    ///
    /// One definition, used by every terminal key. They must agree: a search started for one
    /// terminal while its steps went to another would find nothing to step.
    pub fn keyboard_terminal(&self) -> Option<crate::chrome::panes::terminal::TerminalId> {
        match self.input_mode {
            InputMode::Search { terminal } => return Some(terminal),
            InputMode::Selection => {
                if let Some(crate::app::terminal_host::selection::SelectionOwner(terminal)) =
                    self.selection.owner()
                {
                    return Some(terminal);
                }
            }
            _ => {}
        }
        self.window_keyboard_terminal()
            .or_else(|| self.client.backends.identity_of(self.focused_pane?))
    }

    /// **The terminal holding the window's keyboard**, wherever it is placed — asked of the window,
    /// the one record of where the keyboard is. `None` when the keyboard is on something else, or
    /// on nothing.
    pub fn window_keyboard_terminal(&self) -> Option<crate::chrome::panes::terminal::TerminalId> {
        let holder = heca_grid_ui::keyboard_holder(&self.window_root)?;
        self.terminals
            .values()
            .find(|t| t.node() == Some(holder))
            .and_then(|t| t.id())
    }

    /// The process of the terminal the keyboard acts on ([`keyboard_terminal`](Self::keyboard_terminal)).
    pub fn keyboard_backend_mut(&mut self) -> Option<&mut dyn heca_core::backend::PaneBackend> {
        let terminal = self.keyboard_terminal()?;
        self.client.backends.get_mut_by_id(terminal)
    }
}

fn effective_pane(active_window: bool, blocked: bool,
    input_pane: Option<heca_core::layout::PaneId>) -> Option<heca_core::layout::PaneId> {
    if active_window && !blocked { input_pane } else { None }
}

#[cfg(test)]
mod feedback_focus_tests {
    use super::effective_pane;
    use heca_core::layout::PaneId;

    #[test]
    fn selection_behind_a_palette_or_inactive_window_is_not_input_focus() {
        assert_eq!(effective_pane(true, false, Some(PaneId(7))), Some(PaneId(7)));
        assert_eq!(effective_pane(false, false, Some(PaneId(7))), None);
        assert_eq!(effective_pane(true, true, Some(PaneId(7))), None);
        assert_eq!(effective_pane(true, false, None), None, "a dock or nonterminal control owns input");
    }
}
