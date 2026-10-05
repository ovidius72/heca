//! The header's tree: its buttons, its identity key, and the widget built from them.

use super::*;

/// Per-pane context the header buttons need to build their (parameterized) actions
/// and emit them through the app event loop.
pub(crate) struct PaneHeaderCtx {
    pub(crate) pane_id: PaneId,
    pub(crate) ws_idx: usize,
    pub(crate) col_idx: usize,
    pub(crate) event_proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
}

/// What a pane header *renders* — the projection inputs shared by the rebuild key
/// and the tree builder (groups args so neither fn explodes).
pub(crate) struct PaneHeaderContent<'a> {
    /// What is true of this pane right now — what every chip is worked out from.
    pub(crate) facts: &'a super::pane_items::PaneFacts,
    /// The chips to show, in order — resolved from the user's `title_segments` by name.
    pub(crate) segments: &'a [&'a super::pane_items::PaneChipDef],
    /// Where an added chip's answer is kept between frames.
    pub(crate) chips: &'a super::pane_items::PaneChips,
    /// The buttons to show, in order — resolved from the user's `title_actions` by name.
    pub(crate) actions: &'a [&'a super::pane_items::PaneButtonDef],
    /// The pane's workspace + column — baked into the split action and tracked in
    /// the rebuild key so the header re-bakes when the pane changes column.
    pub(crate) ws_idx: usize,
    pub(crate) col_idx: usize,
    /// The pane's column is zoomed or full-width → the `zoom` button shows active.
    pub(crate) zoomed: bool,
    /// The pane is in the floating domain → the `float` button shows active and
    /// non-floating buttons (split/zoom/move) are hidden (per the action policy).
    pub(crate) floating: bool,
    /// Tooltip keybind hints (tracked in the key so a config reload rebuilds tips).
    pub(crate) shortcuts: &'a ActionShortcuts,
    /// Runtime action-metadata catalog — the single source of each button's icon (and, later,
    /// plugin-contributed action metadata). Threaded alongside `shortcuts`.
    pub(crate) catalog: &'a crate::actions::ActionCatalog,
}

/// One resolved pane-header action button — the generic unit the header renders, worked out for one
/// pane from a [`PaneButtonDef`](super::pane_items::PaneButtonDef).
///
/// The buttons come from the **named registry** in `chrome/pane_items`: heca's own six and the ones a
/// program built on heca added (`pro.pane_button(..)`) are the same kind of entry, and the user's
/// `[appearance.pane] title_actions` picks which show and in what order. The loop in
/// [`build_pane_header`] reads only these fields — an icon, words, what a click sends, whether it
/// reads as held — and never names a button, so a new source of buttons needs no change here.
pub(crate) struct PaneHeaderButton {
    glyph: Glyph,
    /// Canonical action name — the stable key for the tooltip shortcut lookup (the
    /// emitted `WmAction` may be a button-only variant, so the name is the identity).
    action_name: String,
    /// What the button sends (click) and the KeyHint fires (picker).
    emit: super::pane_items::ButtonEmit,
    label: String,
    /// Active-targeted (zoom/float): must focus the owning pane before the action so it
    /// lands on this pane, not whatever is active. Cleared while the pane is floating.
    needs_focus: bool,
    /// Held-on status (zoomed column / floating pane) → the icon paints as toggled-on.
    is_active: bool,
    /// **Destructive** → the button reads in the danger hue. Declared by the action itself and read
    /// from the catalog, never decided here: which acts cannot be undone is not a fact about pane
    /// headers.
    destructive: bool,
}

/// Resolve the header's action buttons for `content` into the generic
/// [`PaneHeaderButton`] vector the render loop consumes. Floating panes drop the
/// tiled-only buttons (per the shared action policy). This is the single place the
/// button *set* is decided — config-driven today, plugin-extensible later (see
/// [`PaneHeaderButton`]).
pub(crate) fn pane_header_buttons(
    content: &PaneHeaderContent,
    ctx: &PaneHeaderCtx,
) -> Vec<PaneHeaderButton> {
    use super::pane_items::PaneIds;
    let ids = PaneIds {
        pane_id: ctx.pane_id,
        ws_idx: ctx.ws_idx,
        col_idx: ctx.col_idx,
    };
    content
        .actions
        .iter()
        .filter_map(|def| {
            let resolved = def.resolve(content.catalog, ids);
            // Floating panes drop the buttons their action's policy refuses there.
            if content.floating && !resolved.emit.allowed_when_floating(content.catalog) {
                return None;
            }
            // An added button's words come from its action's declaration in the catalog.
            let label = content
                .catalog
                .label(&def.action_name)
                .filter(|_| def.from_extension)
                .map(str::to_string)
                .unwrap_or(resolved.label);
            Some(PaneHeaderButton {
                glyph: resolved.glyph,
                action_name: def.action_name.clone(),
                emit: resolved.emit,
                label,
                // Don't focus-first when floating: the floating pane is already active,
                // and a `FocusPane` from MouseContent is blocked in the floating domain.
                needs_focus: resolved.needs_focus && !content.floating,
                is_active: def.is_held(content.zoomed, content.floating),
                destructive: content.catalog.destructive(&def.action_name),
            })
        })
        .collect()
}

/// A content key identifying everything the header *renders* — used to decide when
/// the retained tree must be rebuilt (vs. just re-laid-out). Cheap per-frame string
/// build (≤20 panes); avoids deriving `Hash` on the projection enums.
pub(crate) fn pane_header_key(content: &PaneHeaderContent, font: f32, avail_w: f32) -> String {
    // ⚠️ **The width is deliberately NOT part of this key.**
    //
    // A pane's width is a per-frame layout input, not part of what the header *is* — the same rule
    // the pane shell states for its own rect. It was bucketed in here so a resize would re-run the
    // bar's own width arithmetic; `Label` truncates itself (`Ellipsis::End` is its default), so
    // there is nothing left that needs re-running. Keying on it meant every step of a drag threw the
    // whole header away and built a new one — a burst of CPU and a visible flicker in the buttons
    // (Antonio, driving, 2026-09-03). A resize now re-lays out the retained tree instead.
    let _ = avail_w;
    let w_bucket = 0;
    // Tooltip hints for the configured actions (so a rebind rebuilds the tips).
    let hints: Vec<String> = content
        .actions
        .iter()
        .map(|def| content.shortcuts.get(&def.action_name).unwrap_or_default())
        .collect();
    let button_names: Vec<&str> = content.actions.iter().map(|d| d.name.as_str()).collect();
    // **The words are NOT part of this key** (F003/P097/T500). What a header *shows* — the
    // foreground program, the branch, the working directory — changes constantly and changes
    // nothing about the header's shape, so it is written into the retained tree each frame
    // (`refresh_pane_header_text`) instead. Keying on it rebuilt the whole bar twice per command,
    // and a rebuilt widget paints nothing until the layout walk reaches it, so the buttons blinked
    // out and back both times.
    //
    // ⚠️ **A process status never belonged here at all** — it is rendered nowhere in this bar, so
    // Idle → Running → Success churned the identity while changing nothing a user could see.
    //
    // What stays is what changes the SHAPE: which chips have data at all (a pane outside a
    // repository has no branch chip), and the icon, since a glyph is not text.
    // Which chips have data at all — the shape. `max_width` is passed as unbounded because
    // truncation changes a chip's *words*, never whether it exists, and the words are not here.
    let present: Vec<_> = segment_items(
        content.chips,
        content.segments,
        content.facts,
        f32::INFINITY,
        font,
    )
    .into_iter()
    .map(|(name, glyph, _)| (name, glyph))
    .collect();
    let chip_names: Vec<&str> = content.segments.iter().map(|d| d.name.as_str()).collect();
    format!(
        "{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{}|{}|{:?}",
        content.facts.icon,
        present,
        chip_names,
        button_names,
        content.ws_idx,
        content.col_idx,
        content.zoomed,
        content.floating,
        font.to_bits(),
        w_bucket,
        hints,
    )
}

/// Build the retained header tree: the segment `Tag` (left) + an `IconButton`
/// cluster (right), each button wrapped in a `Tooltip` and wired to emit its
/// (parameterized) WM action through `app.on`-style `ChromeIntent`. Returns `None`
/// when there are neither segments-with-data nor actions.
pub(crate) fn build_pane_header(
    content: &PaneHeaderContent,
    theme: &GuiTheme,
    band: heca_grid_ui::Color,
    font: f32,
    avail_w: f32,
    ctx: PaneHeaderCtx,
) -> Option<Surface> {
    // The button set is a dynamic vector of descriptors (config-driven today,
    // plugin-extensible later) — the loop below never matches on a concrete action.
    let specs = pane_header_buttons(content, &ctx);
    // Build the action-button cluster first: each button self-sizes from its
    // `WidgetSize::Header` variant (emphasized glyph + snug cluster padding), so its
    // width is owned by the widget, not hand-computed here.
    let buttons = if specs.is_empty() {
        None
    } else {
        // **A `ButtonGroup`, not a hand-built row.** It owns what happens when the pane is too
        // narrow for its actions: the words come off first (they become what each button says on
        // hover), then whatever still does not fit moves into a menu behind a trailing ⋮. Before
        // this the cluster was a plain row of icon buttons, and a narrow pane squashed every one of
        // them to a seven-pixel sliver while the title beside them ellipsed correctly (Antonio,
        // driving, 2026-09-02).
        // **Icons, always.** A pane's header is a strip, not a toolbar with room for words — the
        // labels are still carried, and they are what each button says on hover and what its row
        // reads once the group has to put it in the menu.
        let mut group = ButtonGroup::new()
            .size(WidgetSize::Header)
            .variant(ButtonVariant::Ghost)
            .display(heca_grid_ui::widgets::Display::IconOnly);
        for spec in specs {
            // Close is destructive → its glyph + hover/press use the theme danger
            // hue; the rest use the foreground glyph with an accent hover. The danger
            // glyph is softened toward the header surface so the red reads as a cue,
            // not an alarm (full-intensity danger was too vibrant).
            // **A destructive action says so as a variant** — the semantic the library already has,
            // not a colour chosen here. Which actions are destructive is the action's own
            // declaration, read from the catalog; what a destructive button looks like is the
            // widget's business and the theme's.
            let variant = if spec.destructive {
                ButtonVariant::Destructive
            } else {
                ButtonVariant::Primary
            };
            let proxy = ctx.event_proxy.clone();
            let pane_id = ctx.pane_id;
            let emit = spec.emit.clone();
            let needs_focus = spec.needs_focus;
            // **One gesture, written once**: the click and the `prefix+/` pick both run this.
            // Active-targeted actions (zoom/float) act on the focused pane, so it focuses this pane
            // first — the events are queued and processed in order on the UI thread, so the action
            // lands on this pane.
            let fire = move || {
                use crate::app::interaction::{InteractionIntent, InteractionSource};
                if needs_focus {
                    let _ = proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                        source: InteractionSource::MouseContent,
                        intent: InteractionIntent::FocusPane { pane_id },
                    });
                }
                let intent = match &emit {
                    super::pane_items::ButtonEmit::Wm(wm) => {
                        InteractionIntent::ActivateAction(wm.clone())
                    }
                    super::pane_items::ButtonEmit::Intent(intent) => {
                        InteractionIntent::View(intent.clone())
                    }
                };
                let _ = proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                    source: InteractionSource::MouseContent,
                    intent,
                });
            };
            // **Its words are carried even while only its icon shows** — they are what it says on
            // hover, and what its row reads in the menu once the pane is too narrow to hold it.
            // Every `Button` constructor takes them, which is what makes a collapsed group
            // readable with nothing extra written here.
            // `Primary` is the group's cue that this button named nothing of its own, so the
            // group's variant applies; `Destructive` is a button naming one, and it keeps it.
            let button = Button::new(spec.label.clone())
                .icon(spec.glyph)
                .variant(variant)
                .active(spec.is_active)
                .on_click(fire);
            // **The button's identity, from its data — the action it runs.**
            //
            // Without it the identity is DERIVED from the button's content, and a derived identity
            // moves when the content does: zooming changes what the cluster renders, so a button's
            // name or its index among identically-named siblings shifts, and the picker can no
            // longer tell it is the same button — so its letter changes under you
            // (Antonio, driving, 2026-08-19). `docs/widgets.md` § Identity states the limit:
            // derived identity is fine for a remembered letter until the label changes.
            //
            // **Nothing is declared about picking.** These buttons used to repeat their own click
            // as a hint, purely to win back a letter the picker was withholding from anything
            // inside a pane. The picker counts things now, so a button gets its letter for being a
            // button (Antonio, 2026-09-04: *"Users/Developers MUST not think where a widget is"*).
            // **Named for which pane's control it is**, not for what it does. `zoom` is every
            // pane's zoom; `pane:7-zoom` is this one's, so the picker can offer all of them at
            // once. `target_identity` used to add the pane in front of it, which meant the name
            // was only right while that prefixing lasted.
            let button = button.key(crate::chrome::pane_control_key(
                ctx.pane_id,
                &spec.action_name,
            ));
            // Tooltip = label + the action's current keybind(s), resolved centrally
            // by name (never hand-picked here); the leader renders via PREFIX_SYMBOL. It is a
            // property now, so what comes back is still a `Button` and the group will take it.
            group = group.child(action_tooltip(
                button,
                &spec.action_name,
                &spec.label,
                content.shortcuts,
            ));
        }
        Some(group)
    };

    // **Nothing measures the buttons to work out the title's budget any more.** That was a second
    // layout pass whose only job was to feed an estimate — and the estimate then guessed the title's
    // width from a character count and two font multiples, with the per-pane render clip as its
    // stated backstop. That clip is gone (the bar is a child of its pane now), so the guess had
    // nothing catching it. The row divides the space instead: the group keeps its buttons at their
    // own size and gives them up when it must, and the title chip ellipses itself in what is left.
    let bar_max = avail_w;
    let bar = build_pane_info_bar(
        segment_items(
            content.chips,
            content.segments,
            content.facts,
            bar_max,
            font,
        ),
        theme,
    );

    // **The bar carries no size of its own.** It fills the width and the height it is given and
    // centres its content in them — it is a child of the pane, so the pane's layout owns its box.
    //
    // Neither a width nor a height belongs here. It used to carry a pixel width left over from
    // being positioned by hand, and a height computed from the font is the same mistake one step
    // further on: a measurement standing in for "as tall as the space I am in"
    // (Antonio, driving, 2026-09-02). The pane is a column of two — this bar at its natural height,
    // the content taking everything left — so the bar is exactly as tall as what is in it.
    let row = Flex::row()
        .width(Length::FULL)
        // **Air between the title and the actions**, as a token — it resolves against the inherited
        // font, so it holds at every font size and UI zoom instead of being tuned for one.
        .gap(heca_grid_ui::Spacing::Sm)
        // **The bar's own breathing room, which is what makes the strip the height it is.** The
        // host reserves `title_bar_reserve` at the pane top — the bar's content height plus this
        // margin twice over — so carrying the margin here is what makes the bar exactly fill the
        // strip that was reserved for it, instead of sitting at the top of it with dead space
        // below (Antonio, driving, 2026-09-02).
        .align("center");
    let row = match (bar, buttons) {
        (Some(bar), Some(buttons)) => row.justify("space-between").child(bar).child(buttons),
        (Some(bar), None) => row.justify("start").child(bar),
        (None, Some(buttons)) => row.justify("end").child(buttons),
        (None, None) => return None,
    };
    // **The band is the bar's own surface**, so the strip is exactly as tall as what is in it —
    // rather than a rect the host drew at a height it had worked out separately, which anything
    // else placed in a pane's header slot would have had to match. `Flex` carries no background on
    // purpose: it lays out, a `Surface` decorates.
    Some(
        Surface::new()
            .background(band)
            .width(Length::FULL)
            // **The strip's own inset, on both axes.** It replaces a hand-subtracted 6px margin
            // that the host used to take off the pane width before handing the bar a budget — the
            // container holds its own padding, and nothing outside it has to know the number.
            .padding_x(heca_grid_ui::Spacing::Xs)
            // **A theme token, not a pixel count.** `Spacing` resolves against the inherited font
            // at layout, so the bar's breathing room scales with the font, the size variant and UI
            // zoom. A raw px value is tuned for one font size and wrong at every other
            // (`docs/widgets.md` § Flex).
            .padding_y(heca_grid_ui::Spacing::Xs)
            .child(row),
    )
}
