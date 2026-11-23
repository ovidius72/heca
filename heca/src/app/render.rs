//! Render and viewport helpers.
//!
//! These helpers keep low-level pane rendering and viewport synchronization out
//! of `main.rs` while preserving the current render pipeline behavior.

use crate::app::frame;
use crate::app_state::{AppState, InputMode};
use crate::chrome::ChromeConfig;

/// Human-readable status mode label and suffix for the status bar.
///
/// Chrome focus deliberately says **nothing** here: this bar is temporary and is
/// being replaced, and a dock's name sitting where an input mode's word goes reads as a mode when it
/// is not one. The affordance for "the keys are going there" is the **focus ring**, which is on the
/// thing itself rather than in a corner.
pub(crate) fn status_mode_parts(
    input_mode: &InputMode,
    catalog: &crate::actions::ActionCatalog,
    shortcuts: &crate::chrome::ActionShortcuts,
) -> (&'static str, String) {
    // For pick modes the prompt suffix is sourced from the action's `ActionDescriptor`
    // (via `pending_pick`) so the text lives in one place — the action catalog.
    let pick_suffix = || {
        input_mode
            .pending_pick(catalog)
            .map(|p| format!(" — {}", p.prompt))
            .unwrap_or_default()
    };
    match input_mode {
        InputMode::Normal => ("NORMAL", String::new()),
        InputMode::Prefix => ("PREFIX", String::new()),
        InputMode::PaneSelect { .. } => ("SELECT", pick_suffix()),
        // The link picker is the one picker over the links' set; it keeps its own words.
        InputMode::HintPick {
            scope: Some(scope), ..
        } if scope == crate::chrome::panes::terminal::LINK_SCOPE => {
            ("FOLLOW", " — press a letter to open the link".to_string())
        }
        InputMode::HintPick { .. } => {
            ("HINT", " — press a letter to activate a target".to_string())
        }
        InputMode::Search { .. } => ("SEARCH", search_prompt(shortcuts)),
        InputMode::PaneSwap { focus_after, .. } => (
            if *focus_after { "SWAP+FOCUS" } else { "SWAP" },
            pick_suffix(),
        ),
        InputMode::Chord { sequence } => ("CHORD", format!(" w→{}", sequence.join("→"))),
        InputMode::Mode { name } => ("MODE", format!(" {} → ?", name)),
        // The confirm now lives entirely in the Modal dialog; the status bar only
        // shows the mode word, no duplicated prompt.
        InputMode::ConfirmDelete => ("CONFIRM", String::new()),
        InputMode::PaneTake { focus_after, .. } => {
            (if *focus_after { "TAKE+" } else { "TAKE" }, pick_suffix())
        }
        InputMode::WorkspacePick { target, .. } => {
            let label = match target {
                crate::app_state::WorkspacePickTarget::Column { .. } => "MOVE COL",
                crate::app_state::WorkspacePickTarget::Pane(_) => "MOVE PANE",
            };
            (label, pick_suffix())
        }
        InputMode::ColumnPick { .. } => ("MOVE PANE", pick_suffix()),
        InputMode::DockPick { .. } => ("FOCUS DOCK", pick_suffix()),
        InputMode::Selection => ("SELECTION", String::new()),
    }
}

/// Render the full frame for the current app state: the phases of [`frame`], in the order they are
/// drawn.
pub(crate) fn render_frame(state: &mut AppState) {
    use crate::slow_steps::step;
    let _slow = step("frame");
    let Some(v) = ({
        let _s = step("frame.begin");
        frame::begin(state)
    }) else {
        return;
    };
    let scene = {
        let _s = step("frame.paint_window");
        frame::paint_window(state, &v)
    };
    let terminals = {
        let _s = step("frame.collect_panes");
        frame::collect_panes(state, &v)
    };
    {
        let _s = step("frame.sync_terminal_layers");
        frame::sync_terminal_layers(state, &v, &terminals)
    };
    let Some(mut frame) = ({
        let _s = step("frame.open");
        frame::Frame::open(state, v)
    }) else {
        return;
    };
    {
        let _s = step("frame.clear_and_background");
        frame.clear_and_background(state)
    };
    {
        let _s = step("frame.flush_window");
        frame.flush_window(state, &scene, &terminals)
    };
    {
        let _s = step("frame.finish");
        frame.finish(state, &terminals)
    };
}

/// **The search field's prompt, in the user's own keys** — each step names what `[keys.widgets]`
/// binds it to (`menu_down` next, `menu_up` previous, `clear_input` clear, `dismiss` close), so
/// rebinding one changes the prompt; a step bound to no key is left out rather than named wrongly.
fn search_prompt(shortcuts: &crate::chrome::ActionShortcuts) -> String {
    let mut prompt = " — type to search".to_string();
    for (name, says) in [
        ("menu_down", "next"),
        ("menu_up", "previous"),
        ("clear_input", "clear"),
        ("dismiss", "close"),
    ] {
        if let Some(keys) = shortcuts.get(name) {
            prompt.push_str(&format!(", {keys} {says}"));
        }
    }
    prompt
}

/// Update session viewport to match current chrome/content area size.
pub(crate) fn update_session_viewport(state: &mut AppState) {
    let pane_area = ChromeConfig::of(state).content_rect();
    let new_size = heca_core::layout::types::Size::new(pane_area.size.w, pane_area.size.h);
    state.layout_mut().update_viewport(new_size);
}

#[cfg(test)]
mod tests {
    use super::status_mode_parts;
    use crate::actions::ActionCatalog;
    use crate::app_state::InputMode;
    use heca_core::layout::PaneId;

    #[test]
    fn status_mode_parts_formats_take_and_confirm() {
        let catalog = ActionCatalog::with_builtins();
        assert_eq!(
            status_mode_parts(
                &InputMode::PaneTake {
                    candidates: vec![("a".chars().next().expect("candidate label"), PaneId(1))],
                    focus_after: true,
                },
                &catalog,
                &Default::default(),
            ),
            (
                "TAKE+",
                " — Select a pane to pull into the active column, then focus it.".to_string()
            )
        );

        assert_eq!(
            status_mode_parts(&InputMode::ConfirmDelete, &catalog, &Default::default()),
            ("CONFIRM", String::new()),
            "the prompt lives in the Modal now — the status bar shows only the mode word"
        );
    }

    /// **The search prompt names the user's own keys**: by default the arrows and Ctrl+j/k and
    /// Escape, and after `[keys.widgets] dismiss = "Ctrl+g"` it says Ctrl+g — the prompt follows the
    /// binding instead of naming a key itself.
    #[test]
    fn the_search_prompt_names_the_keys_the_user_bound() {
        let shortcuts_of = |config: &heca_config::theme::Config| {
            let keymaps = crate::app::registry::build_keymaps(
                config,
                &mut crate::app::conflicts::Conflicts::default(),
            );
            crate::chrome::ActionShortcuts::from_index(
                &keymaps.by_action,
                crate::shortcut::KeyStyle::Compact,
            )
        };
        let catalog = ActionCatalog::with_builtins();
        let search = InputMode::Search {
            terminal: crate::chrome::panes::terminal::TerminalId(1),
        };

        let defaults = heca_config::theme::Config::default();
        let (_, prompt) = status_mode_parts(&search, &catalog, &shortcuts_of(&defaults));
        for says in ["next", "previous", "close"] {
            assert!(prompt.contains(says), "{prompt}");
        }
        let escape = shortcuts_of(&defaults)
            .get("dismiss")
            .expect("dismiss is bound");
        assert!(prompt.contains(&format!("{escape} close")), "{prompt}");

        let rebound = heca_config::theme::Config {
            keys: toml::from_str("[widgets]\ndismiss = \"Ctrl+g\"\n").expect("keys parse"),
            ..heca_config::theme::Config::default()
        };
        let ctrl_g = shortcuts_of(&rebound)
            .get("dismiss")
            .expect("rebound dismiss");
        let (_, prompt) = status_mode_parts(&search, &catalog, &shortcuts_of(&rebound));
        assert!(prompt.contains(&format!("{ctrl_g} close")), "{prompt}");
        assert!(
            !prompt.contains(&format!("{escape} close")),
            "the old key is gone: {prompt}"
        );
    }
}
