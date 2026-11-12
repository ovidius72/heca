//! **What a pane's header is built from, and how a pane is handed the facts it shows.**
//!
//! The header's content and look live in the parent module; this is the seam between a pane and it.

use super::*;

/// **Everything a pane's header is built from that does not change from frame to frame**, taken
/// when the pane is built.
///
/// It lives as long as the pane does, and a config reload rebuilds every pane (and with it this), so
/// nothing in it is refreshed per frame — **do not add a per-frame field**. What is true of the pane
/// right now travels beside it, as the [`PaneHeaderInput`] the pane is handed.
pub(crate) struct HeaderEnv {
    pub(super) theme: GuiTheme,
    pub(super) band: heca_grid_ui::Color,
    pub(super) segment_names: Vec<String>,
    pub(super) action_names: Vec<String>,
    pub(super) chips: crate::chrome::panes::pane_items::PaneChips,
    pub(super) buttons: crate::chrome::panes::pane_items::PaneButtons,
    pub(super) programs: std::rc::Rc<ProgramsConfig>,
    pub(super) shortcuts: ActionShortcuts,
    pub(super) catalog: crate::actions::ActionCatalog,
    pub(super) event_proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
}

/// The terminal content's pane-info header. Its retained node is independent of the terminal body.
pub(crate) struct PaneHeader {
    node: heca_grid_ui::widgets::Keyed,
    show: Box<Show>,
}

/// Shows a pane's facts in its header node.
type Show = dyn Fn(&PaneHeaderInput, &heca_grid_ui::widgets::Keyed);

impl PaneHeader {
    /// A header built from `env`.
    pub(crate) fn new(env: std::rc::Rc<HeaderEnv>) -> Self {
        Self::showing(move |input, node| show_pane_header(&env, input, node))
    }

    /// A header that shows facts by `show` — what `new` builds on, and what a test hands in.
    pub(crate) fn showing(
        show: impl Fn(&PaneHeaderInput, &heca_grid_ui::widgets::Keyed) + 'static,
    ) -> Self {
        Self {
            node: heca_grid_ui::widgets::Keyed::new(),
            show: Box::new(show),
        }
    }

    /// The node to compose above the terminal. A clone is the same retained node.
    pub(crate) fn seat(&self) -> heca_grid_ui::WidgetModel {
        heca_grid_ui::WidgetModel::new(self.node.clone())
    }

    /// The terminal composition, not its enclosing frame, receives these facts.
    pub(crate) fn answer_props<C: heca_grid_ui::builders::IdentityExt>(self, content: C) -> C {
        content.on_props(move |input: &PaneHeaderInput, _| (self.show)(input, &self.node))
    }
}

/// **Hand a pane the facts its header shows.** A pane that does not take them is said once, so a
/// header that stops updating is never silent.
pub(crate) fn give_header_facts(pane: &mut dyn Component, input: &PaneHeaderInput) {
    if !pane.set_props(input) {
        crate::chrome::input::identity::warn_author(
            "[heca] terminal content did not take its header facts, so the header will not update"
                .to_string(),
        );
    }
}

/// **Tell a pane's header what is true of it now.** Cheap when nothing changed: the shape is summed
/// up in a key, and the header builds a tree only when the key is not the one it already shows.
fn show_pane_header(
    env: &HeaderEnv,
    input: &PaneHeaderInput,
    header: &heca_grid_ui::widgets::Keyed,
) {
    let segments = env.chips.shown(&env.segment_names);
    let actions = env.buttons.shown(&env.action_names);
    let font = env.theme.font_size;
    let facts = crate::chrome::panes::pane_items::PaneFacts::of(
        input.pane_id,
        &env.programs,
        &input.name,
        input.custom_name.as_deref(),
        input.runtime.as_ref(),
    );
    let content = PaneHeaderContent {
        facts: &facts,
        segments: &segments,
        chips: &env.chips,
        actions: &actions,
        ws_idx: input.ws_idx,
        col_idx: input.col_idx,
        zoomed: input.zoomed,
        floating: input.floating,
        shortcuts: &env.shortcuts,
        catalog: &env.catalog,
    };
    let key = pane_header_key(&content, font);
    // **The words travel beside the tree, not inside its identity**: they are written onto the tree
    // by the key of the chip that shows each, so a command changing the program it shows moves the
    // words and nothing else.
    let words: Vec<_> = segment_items(
        content.chips,
        content.segments,
        content.facts,
    )
    .into_iter()
    .map(|item| (segment_text_key(&item.name), item.chip.text))
    .collect();
    header.texts(words);
    header.show(key, || {
        let ctx = PaneHeaderCtx {
            pane_id: input.pane_id,
            ws_idx: input.ws_idx,
            col_idx: input.col_idx,
            event_proxy: env.event_proxy.clone(),
        };
        let root = build_pane_header(&content, env.band, ctx)?;
        // The identity rule's warning half: each control carries a key naming which pane's
        // control it is, and this says so if that ever goes.
        crate::chrome::input::identity::report_ambiguous_widgets("pane-header", &root);
        Some(heca_grid_ui::WidgetModel::new(root))
    });
}
