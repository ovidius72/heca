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
    theme: GuiTheme,
    band: heca_grid_ui::Color,
    segment_names: Vec<String>,
    action_names: Vec<String>,
    chips: super::pane_items::PaneChips,
    buttons: super::pane_items::PaneButtons,
    programs: std::rc::Rc<ProgramsConfig>,
    shortcuts: ActionShortcuts,
    catalog: crate::actions::ActionCatalog,
    event_proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
}

impl HeaderEnv {
    /// Take what a header is built from, as it is now.
    pub(crate) fn of(state: &crate::app_state::AppState) -> std::rc::Rc<Self> {
        std::rc::Rc::new(Self {
            theme: super::chrome_gui_theme(state),
            band: crate::chrome::theme::top_bottom_pane_background_color(&state.theme),
            segment_names: state.appearance.pane.title_segments.clone(),
            action_names: state.appearance.pane.title_actions.clone(),
            chips: state.pane_chips.clone(),
            buttons: state.pane_buttons.clone(),
            programs: state.programs.clone(),
            shortcuts: state.action_shortcuts.clone(),
            catalog: state.action_catalog.clone(),
            event_proxy: state.event_proxy.clone(),
        })
    }
}

/// **A pane's header, held by the pane that shows it.** The pane seats the same node as a child and
/// answers [`PaneHeaderInput`] props by showing them in it, so whoever owns the pane hands it
/// facts and never touches the header.
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

    /// The node to seat in the pane. A clone is the same node.
    pub(crate) fn seat(&self) -> Box<dyn Component> {
        Box::new(self.node.clone())
    }

    /// Teach `pane` to show the facts it is handed in this header.
    pub(crate) fn answer_props<C: heca_grid_ui::builders::ComponentExt>(self, pane: C) -> C {
        pane.on_props(move |input: &PaneHeaderInput, _| (self.show)(input, &self.node))
    }
}

/// **Hand a pane the facts its header shows.** A pane that does not take them is said once, so a
/// header that stops updating is never silent.
pub(crate) fn give_header_facts(pane: &mut dyn Component, input: &PaneHeaderInput) {
    if !pane.set_props(input) {
        super::identity::warn_author(
            "[heca] a pane did not take the facts its header shows, so the header will not update"
                .to_string(),
        );
    }
}

/// **Tell a pane's header what is true of it now.** Cheap when nothing changed: the shape is summed
/// up in a key, and the header builds a tree only when the key is not the one it already shows.
fn show_pane_header(env: &HeaderEnv, input: &PaneHeaderInput, header: &heca_grid_ui::widgets::Keyed) {
    let segments = env.chips.shown(&env.segment_names);
    let actions = env.buttons.shown(&env.action_names);
    let font = env.theme.font_size;
    let facts = super::pane_items::PaneFacts::of(
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
    let key = pane_header_key(&content, font, input.avail_w);
    // **The words travel beside the tree, not inside its identity**: they are written onto the tree
    // by the key of the chip that shows each, so a command changing the program it shows moves the
    // words and nothing else.
    let words: Vec<_> = segment_items(
        content.chips,
        content.segments,
        content.facts,
        input.avail_w,
        font,
    )
    .into_iter()
    .map(|(name, _, text)| (segment_text_key(&name), text))
    .collect();
    header.texts(words);
    header.show(key, || {
        let ctx = PaneHeaderCtx {
            pane_id: input.pane_id,
            ws_idx: input.ws_idx,
            col_idx: input.col_idx,
            event_proxy: env.event_proxy.clone(),
        };
        let root = build_pane_header(&content, &env.theme, env.band, font, input.avail_w, ctx)?;
        // The identity rule's warning half: each control carries a key naming which pane's
        // control it is, and this says so if that ever goes.
        super::identity::report_ambiguous_widgets("pane-header", &root);
        Some(Box::new(root) as Box<dyn Component>)
    });
}
