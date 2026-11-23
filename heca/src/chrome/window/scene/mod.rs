//! **Building and painting the chrome scene** — the root tree, the sidebar shell, and every
//! drawing pass the host runs over it (bell flash, link hints, scrollback search).
//!
//! ⚠️ A drag is **not** one of these passes. The insertion line, the swap outline and the picture
//! of the thing under the pointer are drawn by the widgets the drag passes through, inside
//! `heca_grid_ui::paint_child` — so nothing opts in and a plugin's row gets them free.
//!
//! ⚠️ The app must **never** draw a hint letter itself. A widget carrying a letter has it drawn by
//! the framework inside `heca_grid_ui::paint_child`; a host pass that stamps its own keycap is the
//! mistake that was deleted once already, and it is the reason the passes below draw everything
//! except letters.
//!
//! Split out of `chrome/mod.rs`. Nothing here is new; the passes are unchanged.

use crate::chrome::*;
use heca_grid_ui::builders::DragExt;
use heca_grid_ui::widgets::DockFrame;

/// Make a node take the box its parent gives it instead of the size of its own content.
///
/// A wrapper that hugs its child measures the child's *content*, and a flex item's automatic minimum
/// size then stops it shrinking back — so a dock 1214px tall keeps all 1214px inside the 296px slot
/// its share won and overflows the frame. The fix is CSS's `flex: 1 1 0`: a zero base size plus
/// permission to shrink, so the parent's box is what there is to divide.
///
/// It is `.flex(1.0)` on the node — the library's one builder for "n parts of my parent" — so the
/// engine applies CSS `flex: 1 1 0` in a flex parent and nothing in a grid.
pub(super) fn pass_box_down(node: &mut dyn Component) {
    node.base_mut().style.layout.flex = Some(1.0);
}

/// Put a container's part on the **outermost** node the host built around it.
///
/// The container said its part on its own body (`.flex(n)`), but the host wraps that body — focus
/// ring, dock letter — and a part set only on the inner body would leave the wrapper content-sized
/// and divide nothing. So the same number is repeated on the wrapper. `0.0` is content-sized.
pub(super) fn with_flex(mut body: WidgetModel, parts: f32) -> WidgetModel {
    body.base_mut().style.layout.flex = Some(parts);
    body
}

/// Wrap a container body in the two things the **host** owns about it: the region it names itself
/// as (so the keyboard can be given to it, and a press inside it resolves back to it), and its
/// letter while a dock pick is open.
///
/// Both are host state, not container state — a container cannot know which letter it was given
/// among its siblings — so they are applied here rather than left to each provider to remember. Both
/// wrappers are transparent: they hug the body and route events, focus and drag straight through, so
/// the container behaves exactly as it does unwrapped.
///
/// The focus ring is not wired: the region draws it itself while the keyboard is anywhere inside it
/// (`FocusScope`), so the ring and the keys can never disagree about which dock has focus.
fn focus_and_pick(
    mut body: WidgetModel,
    container: &str,
    share: f32,
    has_cursor: bool,
    movable: bool,
    headed: bool,
    ctx: &crate::providers::ChromeCtx<'_>,
) -> WidgetModel {
    let move_key = ctx
        .move_key()
        .unwrap_or(heca_config::theme::ModifierKey::Super);
    // The share lands on the outermost node, so every level below it has to pass the box down or the
    // dock keeps its content height inside the slot its share won — measured 1214px inside 296px, the
    // same failure as one level up.
    //
    // Only when there *is* a share to pass: a container that asked for `0.0` is saying "size me to my
    // content", and a zero base size inside a content-sized parent would collapse it to nothing.
    if share > 0.0 {
        pass_box_down(body.as_mut());
    }
    let mut picked = KeyHint::new(body)
        // Top-centre over a tall dock.
        .placement(HintPlacement::TopCenter)
        // The `warning` tone, so a dock letter reads distinctly from a pane pick (accent) and a
        // column pick (success) — a meaning, resolved against the theme when the letter is drawn.
        .hint_tone(heca_grid_ui::Tone::Warning);
    if share > 0.0 {
        pass_box_down(&mut picked);
    }
    // Stamp the placement id on the outermost wrapper, so a press anywhere inside — including on a
    // widget that consumes it — resolves back to this container (`nav::scope_at`).
    // It goes here because this is the one place the host already wraps every
    // mount, so a container gets click-to-focus with nothing declared, a plugin's included.
    // **A click anywhere inside this container focuses it** — declared as a handler on the wrapper
    // the host already puts round every mount, so a container gets click-to-focus with nothing
    // written, a plugin's included. It bubbles: a click on a row runs the row's handler first and
    // then this one, and a click on the container's padding reaches only this one — which is the
    // rule "clicking a container's padding is not a request to move the cursor", free, instead of
    // read off a second geometric hit-test (AGENTS.md § 0c).
    //
    // No guard: `FocusDock` only focuses, and focusing the dock that already has the keyboard is a
    // no-op. `ToggleDock` carries the toggle.
    //
    // **Whatever a widget inside names goes out as this dock's own** — seated here and judged as a
    // gesture in this region, exactly what `fires` / `picks` do. So a row writes
    // `.on_activate(Intent::new(..))` or `ev.dispatch(..)`, a plugin's included, and never carries
    // the mount or the emitter to do it.
    let focus_scope = match ctx.emit_intent() {
        Some(emit) => {
            let focus = emit.clone();
            let route = emit.clone();
            let id = container.to_string();
            let seat = id.clone();
            FocusScope::new(picked)
                .on_click(move |_| {
                    focus.fire(crate::app::interaction::InteractionIntent::ActivateAction(
                        crate::input::WmAction::FocusDock {
                            dock: Some(id.clone()),
                        },
                    ));
                })
                .on_dispatch(move |intent| {
                    route.fire_intent(crate::chrome::input::emitter::seated(&seat, intent))
                })
        }
        None => FocusScope::new(picked),
    };
    heca_grid_ui::WidgetModel::new(
        focus_scope
            // **The dock's own letter belongs to the dock pick, not the ordinary one.** Clicking
            // this wrapper focuses the container, which is what makes it actionable and therefore
            // lettered — but that is a keyboard destination `prefix+Shift+e` already offers, so in
            // `prefix+/` it was a letter per placement pointing at something with its own key.
            .hint_scope([crate::chrome::DOCK_PICK_SCOPE])
            // **A dock with its own cursor keeps the keyboard itself**, so `Enter` and `j`/`k` act
            // on its cursor, never on whichever row was clicked last.
            .keeps_keyboard(has_cursor)
            // Still declared: `offer_hint_by_key` matches it so the DOCK PICK can letter this
            // container (`chrome/hint/letters.rs`). It is no longer read by any hit-test.
            .scope_key(container)
            // **A dock is carried by the window's move key and lands beside another** — the same
            // key that carries a pane, so moving a container is one gesture wherever it is. Held,
            // it wins over the rows inside (`DragExt::draggable_when`). A dock that says it does
            // not move is still a place to land beside. What the drop means is the host's
            // (`chrome::dock_drop_action`), posted as a `chrome.container.*` action.
            .draggable_as(movable.then_some(crate::chrome::DOCK_DRAG_KIND))
            .draggable_when(move |held| crate::modifier::held(move_key, held))
            // With a header, its grip picks the dock up with no key as well.
            .drag_by_handle(headed)
            .accepts_beside([crate::chrome::DOCK_DRAG_KIND]),
    )
}

/// Build the body of a chrome **region** from whatever the [`ChromeHost`] has seated in
/// it — the render half of the pluggable-chrome contract.
///
/// For each mounted container, in the host's order: ask its provider for a
/// [`Contribution`] and call the container's `build` seam. The other contribution kinds
/// belong to other hosts (bars take `ToolbarGroup`/`StatusSegment`, overlays go to the
/// overlay host, §3.1.1), so a region ignores them rather than guessing.
///
/// `None` — not an empty widget — when nothing is mounted, so the shell can tell "no
/// provider here" from "a provider that built an empty body".
///
/// Takes the three registries rather than a ready-made [`BuildCx`] because the context is **per
/// container**, not per region: each build hook is told which mount it is building, so it can ask
/// for state that is per mount (its own scroll offset). One `BuildCx` for a whole region could not
/// carry that.
pub(in crate::chrome) fn build_region_content(
    host: &ChromeHost,
    region: RegionId,
    ctx: &crate::providers::ChromeCtx<'_>,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
) -> Option<WidgetModel> {
    // What each dock built, before the host wraps it: whether there are headers depends on how
    // many there are, so they are all built first.
    struct Built {
        order: heca_grid_ui::order::Order,
        id: String,
        title: String,
        body: WidgetModel,
        parts: f32,
        navigable: bool,
        movable: bool,
    }
    let mut built = host
        .contributions(region)
        .iter()
        .filter_map(|mounted| match mounted.provider().build_contribution(ctx) {
            Contribution::Container(c) => {
                drag.register(c.id.clone(), ChromeDragItem::Dock(c.id.clone()));
                let mut bx = BuildCx::new(&c.id, signals, drag);
                let body = (c.build)(ctx, &mut bx);
                // **How big it is and where it sits.** Whoever added the dock may have said
                // (`.flex` / `.order` on the dock as it was appended), and that wins; otherwise the
                // dock's own body says (`.flex(n)` / `.order(..)`, natively or as a described
                // property); otherwise one equal part, in the order added. The region never writes
                // a size of its own: it is a list anyone may append to.
                let placed = mounted.placement();
                let own = &body.base().style.layout;
                // A share the user gave by hand (resizing) wins over both.
                let parts = mounted
                    .hand_share()
                    .or(placed.flex)
                    .or(own.flex)
                    .unwrap_or(1.0);
                let order = placed.order.or(own.order).unwrap_or_default();
                Some(Built {
                    order,
                    id: c.id.clone(),
                    title: c.title.clone(),
                    body,
                    parts,
                    navigable: mounted.keyboard_navigable(),
                    movable: c.movable,
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    // Lower first; ties — and every dock that said nothing — keep the order they were added.
    built.sort_by_key(|dock| dock.order);
    // **Every dock has a header**: its name, the grip it is dragged by, and the fold — alone in its
    // region too, so a dock looks and moves the same whatever sits beside it.
    let headed = !built.is_empty();
    let bodies = built
        .into_iter()
        .map(|dock| {
            let folded = headed && ctx.state().is_container_collapsed(&dock.id);
            // A folded dock is its header alone, sized to it; the others share what is left.
            let parts = if folded { 0.0 } else { dock.parts };
            let body = match headed {
                true => with_header(dock.body, &dock.id, &dock.title, folded, parts),
                false => dock.body,
            };
            // The part goes on the OUTERMOST node, so it has to be applied after the wrappers:
            // a part set only on the body would leave the wrapper content-sized and divide
            // nothing (one level up).
            (
                dock.order,
                dock.id.clone(),
                with_flex(
                    focus_and_pick(
                        body,
                        &dock.id,
                        parts,
                        dock.navigable,
                        dock.movable,
                        headed,
                        ctx,
                    ),
                    parts,
                ),
            )
        })
        .collect::<Vec<_>>();
    let ids: Vec<String> = bodies.iter().map(|(_, id, _)| id.clone()).collect();
    let mut bodies: Vec<WidgetModel> = bodies.into_iter().map(|(_, _, body)| body).collect();
    match bodies.len() {
        0 => None,
        // One container owns the region: its own share already makes it fill the region, so
        // there is nothing to wrap it in.
        1 => bodies.pop(),
        // Several containers share a region: a column of them in the host's order (the order
        // `reorder`/`move_container` maintain), **each taking the part its own `.flex` asked for**.
        // A list of like things, so a `Flex` — never a grid, whose template would be a size list
        // written for a fixed number of docks.
        //
        // The column itself takes the whole region (`.flex(1.0)`), so it is the region's box that
        // gets divided rather than its content's height.
        _ => {
            let gap = host
                .layout(region)
                .gap
                .unwrap_or(heca_grid_ui::style::Space::Step(
                    heca_grid_ui::style::Spacing::Sm,
                ));
            // No gap on the column itself: the space between two docks is a widget of its own, so a
            // dock dropped there has somewhere to land (after the dock above). It is the same space
            // as before — the gap above the rule, the rule, the gap below.
            // Docks in the host's order, with the space between each pair before the one below.
            let mut bodies = bodies.into_iter();
            let first = bodies.next();
            let rest: Vec<WidgetModel> = ids
                .windows(2)
                .zip(bodies)
                .flat_map(|(pair, body)| [dock_gap(&pair[0], &pair[1], gap, drag, ctx), body])
                .collect();
            Some(heca_grid_ui::WidgetModel::new(
                Flex::column().flex(1.0).child(first).child(rest),
            ))
        }
    }
}

/// **A dock's header**, shown while its region holds more than one: the grip it is dragged by, its
/// name, and the fold at the end — then the body, hidden while folded. A `Row` (the row a dock's
/// cursor can stand on) holding the small widgets, at the region's normal size; the rows inside a
/// dock are smaller.
fn with_header(body: WidgetModel, id: &str, title: &str, folded: bool, parts: f32) -> WidgetModel {
    let mut body = body;
    if folded {
        body.base_mut().style.layout.hidden = true;
    }
    let fold = Intent::new("chrome.container.toggle_collapsed")
        .arg("container_id", PropValue::Text(id.to_string()));
    let caret = match folded {
        true => heca_grid_ui::Glyph::CaretRight,
        false => heca_grid_ui::Glyph::CaretDown,
    };
    // A `Row` so the dock's cursor can stand on it (it lights by its key, like any row), holding
    // the parts: the grip, the name, the caret.
    let header = heca_grid_ui::widgets::Row::new()
        // The theme's surface behind it — a shade apart from the dock's rows (their titles have
        // none, its pane cards the `Tinted` veil), so the dock reads as the thing that holds them.
        .variant(heca_grid_ui::widgets::RowVariant::Filled)
        .key(crate::chrome::dock_header_key(id))
        .on_press(fold)
        // The parts straight in the row, so its own air is the only air: grip, name, caret.
        .align("center")
        .gap(heca_grid_ui::style::Spacing::Sm)
        .child(Label::new(DockFrame::GRIP).drag_handle())
        .child(Label::new(title).grow(1.0))
        .child(Icon::new(caret));
    let column = Flex::column().child(header).child(body);
    // Pass the share down through the column, so the body fills what the dock was given.
    let mut column = WidgetModel::new(column);
    if parts > 0.0 {
        pass_box_down(column.as_mut());
        if let Some(body) = column.base_mut().children.last_mut() {
            pass_box_down(body.as_mut());
        }
    }
    column
}

/// **The space between two docks**: the rule that tells them apart, with the region's gap above and
/// below it — a place a dock dropped there lands, after the dock above (`after`) — and **the edge
/// that resizes them**: dragged, it sends `chrome.container.resize` for the dock above, the same
/// action a key and a script send.
///
/// A rule rather than a gap alone, so two docks read as two things rather than one long list. It is
/// a sibling, not part of a dock: it asks for no part, so it keeps its natural size and the docks
/// divide what is left exactly as they asked. Its colour comes from the theme's border token, so it
/// follows a reload.
fn dock_gap(
    after: &str,
    before: &str,
    gap: heca_grid_ui::style::Space,
    drag: &mut DragItemRegistry,
    ctx: &crate::providers::ChromeCtx<'_>,
) -> WidgetModel {
    let key = format!("dock-gap:{after}");
    drag.register(
        key.clone(),
        ChromeDragItem::DockGap {
            after: after.to_string(),
        },
    );
    WidgetModel::new(
        Flex::column()
            .key(key)
            .padding_y(gap)
            .accepts([crate::chrome::DOCK_DRAG_KIND])
            // Either neighbour dropped here stays where it is, so neither is offered the place.
            .accepts_when({
                let (above, below) = (after.to_string(), before.to_string());
                move |source| source != above && source != below
            })
            .child(
                heca_grid_ui::widgets::Splitter::horizontal()
                    .line(true)
                    .on_resize({
                        let (emit, dock) = (ctx.emit_intent().cloned(), after.to_string());
                        move |by| {
                            if let Some(emit) = &emit {
                                emit.fire_intent(
                                    Intent::new("chrome.container.resize")
                                        .arg("container_id", PropValue::Text(dock.clone()))
                                        .arg("by", PropValue::Float(f64::from(by))),
                                );
                            }
                        }
                    }),
            ),
    )
}

/// Build a sidebar **SHELL** — a full-height, bracket-framed, frosted panel filling a
/// sidebar column. This is **one component, two instances**: the left and right
/// sidebars are the same shell differing only by width/position and the `content`
/// mounted inside. Per the chrome plan (`pluggable-chrome-plugin-plan.md` §2.1 / §2.8,
/// `docs/chrome-and-ui.md` § 2.5) the sidebar is a *shell* that hosts a Provider's
/// content — and that is now literally true: `content` is whatever
/// [`ChromeHost::contributions`] seats in the region (see [`build_region_content`]), so
/// the shell knows nothing about workspaces. A region with no provider mounted passes
/// `None` and renders as an empty frame.
///
/// The body is a [`heca_grid_ui::WidgetModel`](heca_grid_ui::Component) — a provider's render
/// seam returns a built subtree, not a concrete widget type — which is why it is mounted
/// as a boxed child rather than through `Parent::child`.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_sidebar_shell(
    region: RegionId,
    region_w: f32,
    sidebar_h: f32,
    look: &SidebarLook,
    content: Option<WidgetModel>,
) -> Flex {
    let inner_w = (region_w - look.gap * 2.0).max(0.0);
    let inner_h = (sidebar_h - look.gap * 2.0).max(0.0);
    // The collapse toggle lives in the always-visible top bar (sidebar-fu-14), so the
    // shell has no header row — the mounted content (if any) fills the body.
    let body = Pane::new()
        .border_style(look.border_style.into())
        .border_width(look.border_width)
        .radius(look.border_radius)
        .width(inner_w)
        .height(inner_h)
        .padding(Spacing::Md)
        .gap(Spacing::Sm)
        .background(look.background)
        // The sidebar's own colour, when it has one; otherwise the frame's default.
        .border_color(look.border_color);
    // **An empty sidebar is still somewhere to put a dock.** Its whole body takes one, named by the
    // region — the name `regions("sidebar.left")` takes — so the drop moves the dock into it
    // (`chrome::dock_drop_action`).
    let place = content.is_none().then(|| {
        Flex::column()
            .grow(1.0)
            .key(region.as_str())
            .accepts([crate::chrome::DOCK_DRAG_KIND])
    });
    // The content is mounted directly: **the shell does not scroll**.
    //
    // Scrolling is per container. Each one nests its own scroll area and scrolls its own content,
    // which is what makes two of them in a sidebar independent. A scroll viewport around the whole
    // stack would defeat that twice over: it takes the wheel for the sidebar instead of the
    // container under the cursor, and — because a viewport measures its content at its natural
    // height, which is the whole point of one — it leaves the containers content-sized, so a
    // fractional share has no height to divide and they bunch at the top.
    //
    // The shell's job is to give containers bounds. It hands them the region's height, they take
    // their shares of it, and each scrolls inside what it got.
    let body = body.child(content).child(place);
    Flex::column().width(region_w).height(sidebar_h).child(
        Surface::column()
            .width(region_w)
            .height(sidebar_h)
            .background(look.background)
            .padding(look.gap)
            .child(body),
    )
}

/// The chrome frame's geometry, colors, and status text — grouped so the assembly
/// helpers stay under clippy's argument-count lint. Borrowed `status` keeps the
/// caller's `String` in place.
#[derive(Clone, Copy)]
pub(super) struct ChromeFrame<'a> {
    pub(super) w: f32,
    pub(super) h: f32,
    pub(super) tab_bar_height: f32,
    pub(super) status_bar_height: f32,
    pub(super) status: &'a str,
    /// This window's own title (session folder + ordinal), shown in the top bar.
    pub(super) title: &'a str,
    /// The title of the window this one follows, when it is following one — shown after its own
    /// title as `title → leader`. `None` when independent.
    pub(super) follow_target: Option<&'a str>,
    pub(super) side_bg: Color,
}

/// A sidebar collapse toggle for the **top bar** (sidebar-fu-14): a small arrow
/// `IconButton` that emits `ActivateAction(action)` (show↔hide for its region),
/// carrying a tooltip with its keybind (resolved centrally by `action_name` — like every other
/// chrome button; the tip is a property of the widget, so this still returns the widget). Lives in the always-visible top bar so it
/// works in both expanded and collapsed states.
#[allow(clippy::too_many_arguments)]
pub(super) fn sidebar_toggle_button(
    glyph: Glyph,
    action_name: &str,
    shortcuts: &ActionShortcuts,
    catalog: &crate::actions::ActionCatalog,
) -> IconButton {
    // Label from the action descriptor (catalog-owned), never re-spelled here.
    let label = catalog.label(action_name).unwrap_or(action_name);
    // Just pick the size variant — the widget derives icon px + padding from the
    // theme font internally (`Icon` with no explicit px uses the variant-scaled font,
    // `IconButton` scales its padding). No caller-side size math.
    // **Its identity is the action it runs, not the arrow it shows.** Both toggles flip their
    // glyph with the sidebar's state — left is `ArrowLineLeft` expanded and `ArrowLineRight`
    // collapsed, right is the mirror — so a derived identity (the glyph's name) changes under the
    // user every time they use the button, and the two buttons can even derive the *same* name at
    // once (left expanded and right collapsed are both `arrow_line_left`), at which point document
    // order decides which one wears the index. Either way the `prefix+/` letters moved on every
    // pick. The action name is stable through both states.
    // The quiet tone of a structural control, resolved against the theme when drawn.
    let button = IconButton::new(heca_grid_ui::builders::ToneExt::content_tone(
        Icon::new(glyph),
        heca_grid_ui::Tone::Muted,
    ))
    .key(action_name)
    .size(WidgetSize::Small)
    // One gesture, named: a click and a `prefix+/` pick both run the action, and the chrome
    // root's listener sends it as a gesture in the chrome — the button carries no emitter.
    .on_press(crate::chrome::Intent::new(action_name));
    action_tooltip(button, action_name, label, shortcuts)
}

/// Assemble the chrome root widget tree (no layout/paint): a transparent tab band
/// (carrying the left/right sidebar collapse toggles at its outer corners), a middle
/// row hosting the (optional) full-height sidebar shell + a transparent content spacer,
/// and the opaque status bar at the bottom. Returns the concrete [`Flex`] so it can be
/// **retained** across frames (see [`RetainedChrome`]).
fn chrome_root(
    frame: &ChromeFrame,
    left_sidebar: Option<Flex>,
    right_sidebar: Option<Flex>,
    left_toggle: Option<IconButton>,
    right_toggle: Option<IconButton>,
    signals: &mut ChromeSignals,
) -> Flex {
    let ChromeFrame {
        w,
        h,
        tab_bar_height,
        status_bar_height,
        status,
        title,
        follow_target,
        side_bg,
    } = *frame;
    let middle_h = (h - tab_bar_height - status_bar_height).max(0.0);

    // Middle row: the full-height sidebar shell (when expanded) + a transparent
    // spacer over the content area (panes are drawn by the hand-drawn path under
    // this scene). The shell sizes its own width/height.
    let mut middle = Flex::row().width(w).height(middle_h);
    if let Some(shell) = left_sidebar {
        middle = middle.child(shell);
    }
    middle = middle.child(Flex::row().grow(1.0));
    if let Some(shell) = right_sidebar {
        middle = middle.child(shell);
    }

    let mut root = Flex::column().width(w).height(h);
    // Transparent tab band — the hand-drawn tab bar paints underneath. Omitted
    // entirely when the top bar is hidden (`show_top_bar = false`).
    if tab_bar_height > 0.0 {
        // Left toggle at the far-left corner, right toggle at the far-right, spacer
        // between (over the hand-drawn tab bar). sidebar-fu-14.
        // Edge inset from a theme spacing token (resolved from the font at layout — no
        // hand-computed px). Vertical breathing room comes from centering a `Small` toggle.
        let mut band = Flex::row()
            .width(w)
            .height(tab_bar_height)
            .align("center")
            .padding_x(Spacing::Sm);
        // Left cluster: the sidebar toggle and, beside it, this window's title (the session folder,
        // numbered from the second window on — formatted client-side, decision e36aba0c). The
        // top/bottom bars are slated for a rebuild, so this is a deliberate slot, not the final bar.
        let mut left_cluster = Flex::row().align("center").gap(Spacing::Sm);
        if let Some(t) = left_toggle {
            left_cluster = left_cluster.child(t);
        }
        // One segmented pill — built like the pane header's info bar (`build_pane_info_bar`) — for
        // this window's identity and, while following, the window it mirrors: `[ ⬚ ~ #2 │ → ~ ]`.
        let mut marker = heca_grid_ui::widgets::Tag::from_label(Label::new(title))
            .leading(Icon::new(Glyph::Cards));
        if let Some(leader) = follow_target {
            marker = marker.segment_label_with(Label::new(leader), Icon::new(Glyph::ArrowRight));
        }
        left_cluster = left_cluster.child(marker);
        band = band.child(left_cluster);
        band = band.child(Flex::row().grow(1.0));
        if let Some(t) = right_toggle {
            band = band.child(t);
        }
        root = root.child(band);
    }
    root = root.child(middle);
    // Status (bottom) bar. Built only when shown — a zero-height `Surface` would
    // still paint its overflowing `Label`, so when `show_bottom_bar = false` we drop
    // the whole bar (and leave `signals.status` unset, which the per-frame updater
    // already treats as "nothing to update").
    if status_bar_height > 0.0 {
        // The status label's text is bound so mode/focus changes update it in place.
        // No colour of its own: the bar's text takes the theme's text colour where it is drawn.
        let status_label = Label::new(status);
        let status_signal = status_label.text_signal();
        signals.status = Some(status_signal);
        root = root.child(
            Surface::row()
                .width(w)
                .height(status_bar_height)
                .background(side_bg)
                .align("center")
                .padding_x(Spacing::Sm)
                .child(status_label),
        );
    }
    root
}

/// Layout + paint a (retained) chrome root tree into a [`Scene`] at the window size.
/// Re-run every frame; cheap and creates no signals (those live in the retained tree).
pub(crate) fn paint_chrome_root(root: &mut Flex, w: f32, h: f32, theme: &GuiTheme) -> Scene {
    let mut scene = Scene::new();
    {
        let _s = crate::slow_steps::step("frame.chrome_layout");
        LayoutEngine::new()
            .theme(theme)
            .compute(root, Size::new(w as f64, h as f64));
    }
    {
        let _s = crate::slow_steps::step("frame.chrome_paint");
        let mut cx = PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
        heca_grid_ui::paint_child(root, &mut cx);
    }
    scene
}

/// Test helper: build + layout + paint in one shot. Runtime uses the retained tree
/// ([`build_chrome_root`] + [`paint_chrome_root`]) instead.
#[cfg(test)]
pub(super) fn chrome_scene(
    frame: &ChromeFrame,
    theme: &GuiTheme,
    left_sidebar: Option<Flex>,
    right_sidebar: Option<Flex>,
) -> Scene {
    let mut signals = ChromeSignals::default();
    let mut root = chrome_root(frame, left_sidebar, right_sidebar, None, None, &mut signals);
    paint_chrome_root(&mut root, frame.w, frame.h, theme)
}

/// Build the chrome root tree from app state (the expensive part — creates the
/// widget tree and its signals). Call only when [`chrome_signature`] changes.
pub(crate) fn build_chrome_root(
    state: &crate::app_state::AppState,
    chrome: ChromeConfig,
) -> (
    Flex,
    ChromeSignals,
    DragItemRegistry,
    crate::app::interaction::InteractionSource,
) {
    let phys = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let w = phys.width as f32 / scale;
    let h = phys.height as f32 / scale;
    let (side_bg, _sidebar_bg) = chrome_colors(state);
    let theme = chrome_gui_theme(state);
    let status = chrome_status(state);
    let mut signals = ChromeSignals::default();
    let mut drag_items = DragItemRegistry::default();
    let emit_intent = ChromeIntentEmitter::new(
        &state.event_proxy,
        crate::app::interaction::InteractionSource::MouseLeftSidebar,
    );

    // Expanded ⇄ Hidden: width is 0 when the region is Hidden (no icon rail — see
    // `docs/chrome-and-ui.md` § 2.5), so a positive width means Expanded.
    // Left and right are two instances of the SAME `build_sidebar_shell` (one
    // component), differing only by width and mounted content: the left hosts the
    // `WorkspacesContainer`, the right is an empty placeholder until it gains a Provider.
    let sidebar_h = (h - chrome.tab_bar_height - chrome.status_bar_height).max(0.0);

    // The region body is whatever the `ChromeHost` has seated in that region — the app
    // no longer knows that the left sidebar happens to hold the workspace tree. Moving
    // the `workspaces` container to the right region (`ChromeHost::move_container`) moves
    // its UI with it, with no change here.
    //
    // The context carries only what every component needs — the frame's theme and the intent sink.
    // A component's own model is its own to read, so the host no longer borrows
    // one component's tree here on everybody's behalf.
    let ctx = crate::providers::ChromeCtx::for_build(
        &state.chrome_state,
        &theme,
        &emit_intent,
        &state.action_catalog,
    )
    .with_move_key(state.settings.interactive_move_modifier);

    let left_w = chrome.left_sidebar_width;
    let left_sidebar = (left_w > 0.0).then(|| {
        let content = build_region_content(
            &state.chrome_host,
            RegionId::LeftSidebar,
            &ctx,
            &mut signals,
            &mut drag_items,
        );
        build_sidebar_shell(
            RegionId::LeftSidebar,
            left_w,
            sidebar_h,
            &SidebarLook::left(state),
            content,
        )
    });
    let right_w = chrome.right_sidebar_width;
    let right_sidebar = (right_w > 0.0).then(|| {
        let content = build_region_content(
            &state.chrome_host,
            RegionId::RightSidebar,
            &ctx,
            &mut signals,
            &mut drag_items,
        );
        build_sidebar_shell(
            RegionId::RightSidebar,
            right_w,
            sidebar_h,
            &SidebarLook::right(state),
            content,
        )
    });

    // Top-bar sidebar toggles: always on screen — a hidden sidebar is brought back by the same
    // button — and the arrow flips with the one visible/hidden state: shown → point at the edge
    // (hide); hidden → point away from the edge (show).
    let left_toggle = Some({
        let glyph = if state.chrome_state.is_visible(RegionId::LeftSidebar) {
            Glyph::ArrowLineLeft
        } else {
            Glyph::ArrowLineRight
        };
        sidebar_toggle_button(
            glyph,
            "sidebar_left",
            &state.action_shortcuts,
            &state.action_catalog,
        )
    });
    let right_toggle = Some({
        let glyph = if state.chrome_state.is_visible(RegionId::RightSidebar) {
            Glyph::ArrowLineRight
        } else {
            Glyph::ArrowLineLeft
        };
        sidebar_toggle_button(
            glyph,
            "sidebar_right",
            &state.action_shortcuts,
            &state.action_catalog,
        )
    });

    let root = chrome_root(
        &ChromeFrame {
            w,
            h,
            tab_bar_height: chrome.tab_bar_height,
            status_bar_height: chrome.status_bar_height,
            status: &status,
            title: state.client.title.as_str(),
            follow_target: state
                .client
                .following()
                .and_then(|id| state.windows.iter().find(|window| window.id == id))
                .map(|window| window.title.as_str()),
            side_bg,
        },
        left_sidebar,
        right_sidebar,
        left_toggle,
        right_toggle,
        &mut signals,
    );
    // The identity rule's warning half: a collection of ours whose items were
    // never keyed loses its cursor position and its hint letters on the next rebuild, and nothing
    // fails when it does. Said once per distinct finding, in debug builds only.
    crate::chrome::input::identity::report_ambiguous_widgets("chrome", &root);
    // **What a widget in the chrome names goes out as a chrome gesture** — through this tree's own
    // emitter, the source `pick_source` reads for the chrome. A dock seated inside listens for
    // itself (nearer wins) and adds its seat.
    let root = {
        let emit = emit_intent.clone();
        root.on_dispatch(move |intent| emit.fire_intent(intent))
    };
    let source = emit_intent.source();
    (root, signals, drag_items, source)
}

#[cfg(test)]
mod tests;
