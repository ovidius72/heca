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

use super::*;

/// Make a node take the box its parent gives it instead of the size of its own content.
///
/// A wrapper that hugs its child measures the child's *content*, and a flex item's automatic minimum
/// size then stops it shrinking back — so a dock 1214px tall keeps all 1214px inside the 296px slot
/// its share won and overflows the frame. The fix is CSS's `flex: 1 1 0`: a zero base size plus
/// permission to shrink, so the parent's box is what there is to divide.
///
/// **Do not copy this trio into new code.** It is the same debt [`with_share`] carries and for the
/// same reason — `Layout` has no `flex_basis`, so a zero base size has to be written as a height,
/// which is a fixed measure standing in for a proportion. P052(F004)/T350 replaces both with one
/// `share(n)` setter in the library; this exists so a *transparent* wrapper stays transparent until
/// then, rather than each caller rediscovering the combination.
pub(super) fn pass_box_down(node: &mut dyn Component) {
    let layout = &mut node.base_mut().style.layout;
    layout.flex_grow = 1.0;
    layout.height = heca_grid_ui::Length::Px(0.0);
    layout.min_height = Some(heca_grid_ui::Length::Px(0.0));
    layout.flex_shrink = Some(1.0);
}

/// Give a container body its declared share of the region's **main axis**, as a flex grow factor
/// (F003/P011/T021) — height in a sidebar, width in a bar, one number either way.
///
/// Applied to every container however many are seated, so the rule needs no special case: alone it
/// takes the whole region, two equal shares take half each, `2.0` beside `1.0` takes two thirds,
/// and `0.0` is content-sized.
///
/// Set by the region rather than by the container, because a share only means anything relative to
/// its siblings — which a container cannot see and should not have to.
pub(super) fn with_share(mut body: WidgetModel, grow: f32) -> WidgetModel {
    let layout = &mut body.base_mut().style.layout;
    layout.flex_grow = grow;
    if grow > 0.0 {
        // A share has to be **of the region**, not of what is left over after the content.
        //
        // `flex_grow` alone distributes only *positive* free space, and a container's content is
        // routinely taller than the sidebar — so two containers measured 1214px each inside a 600px
        // body, overflowed the frame, and got no share at all. In CSS this is `flex: 1 1 0`; there
        // is no `flex_basis` in this vocabulary, so the equivalent is a **zero base size** plus
        // permission to shrink. Then the free space is the whole region and the shares divide it:
        // 296px each, measured.
        //
        // Safe because a shared container is expected to scroll its own content — it nests its own
        // scroll area, so being handed less height than its content is the normal case, not a
        // squeeze. A container that asked for `0.0` is saying "size me to my content" and keeps its
        // natural height.
        layout.height = heca_grid_ui::Length::Px(0.0);
        layout.min_height = Some(heca_grid_ui::Length::Px(0.0));
        layout.flex_shrink = Some(1.0);
    }
    body
}

/// Wrap a container body in the two things the **host** owns about it: whether it holds chrome
/// keyboard focus, and its letter while a dock pick is open (F003/P011/T020).
///
/// Both are host state, not container state — a container cannot know that it is the focused one, or
/// which letter it was given among its siblings — so they are applied here rather than left to each
/// provider to remember. Both wrappers are transparent: they hug the body and route events, focus and
/// drag straight through, so the container behaves exactly as it does unwrapped.
///
/// The focus signal is the **same one** the container's own scroll area binds as its keyboard target
/// (`StateView::container_keyboard_target`), so the ring and the keys can never disagree about which
/// dock has focus.
fn focus_and_pick(
    mut body: WidgetModel,
    container: &str,
    share: f32,
    ctx: &crate::providers::ChromeCtx<'_>,
) -> WidgetModel {
    // The share lands on the outermost node, so every level below it has to pass the box down or the
    // dock keeps its content height inside the slot its share won — measured 1214px inside 296px, the
    // same failure F003/P011/T021 fixed one level up.
    //
    // Only when there *is* a share to pass: a container that asked for `0.0` is saying "size me to my
    // content", and a zero base size inside a content-sized parent would collapse it to nothing.
    if share > 0.0 {
        pass_box_down(body.as_mut());
    }
    let mut picked = KeyHint::new(body)
        // Top-centre over a tall dock. Outside a render pass there is no theme to tint it with, and
        // there is no keycap to draw either (no pick is open while a host reads metadata), so the
        // default accent stands.
        .placement(HintPlacement::TopCenter);
    if let Some(theme) = ctx.theme() {
        // The theme's `warning` tone, so a dock letter reads distinctly from a pane pick (accent)
        // and a column pick (success).
        picked = picked.color(theme.colors.warning);
    }
    if share > 0.0 {
        pass_box_down(&mut picked);
    }
    // Stamp the placement id on the outermost wrapper, so a press anywhere inside — including on a
    // widget that consumes it — resolves back to this container (`nav::scope_at`,
    // F003/P086/T365). It goes here because this is the one place the host already wraps every
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
    let focus_scope = match ctx.emit_intent() {
        Some(emit) => {
            let emit = emit.clone();
            let id = container.to_string();
            FocusScope::new(picked).on_click(move |_| {
                emit.fire(crate::app::interaction::InteractionIntent::ActivateAction(
                    crate::input::WmAction::FocusDock {
                        dock: Some(id.clone()),
                    },
                ));
            })
        }
        None => FocusScope::new(picked),
    };
    Box::new(
        focus_scope
            // **The dock's own letter belongs to the dock pick, not the ordinary one.** Clicking
            // this wrapper focuses the container, which is what makes it actionable and therefore
            // lettered — but that is a keyboard destination `prefix+Shift+e` already offers, so in
            // `prefix+/` it was a letter per placement pointing at something with its own key.
            .hint_scope([crate::chrome::DOCK_PICK_SCOPE])
            .focus(ctx.state().container_keyboard_target(container))
            // Still declared: `offer_hint_by_key` matches it so the DOCK PICK can letter this
            // container (`chrome/hint/letters.rs`). It is no longer read by any hit-test.
            .scope_key(container),
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
/// carry that (F003/P011/T021).
pub(super) fn build_region_content(
    host: &ChromeHost,
    region: RegionId,
    ctx: &crate::providers::ChromeCtx<'_>,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
) -> Option<WidgetModel> {
    let mut bodies = host
        .contributions(region)
        .iter()
        .filter_map(|mounted| match mounted.provider().build_contribution(ctx) {
            Contribution::Container(c) => {
                let mut bx = BuildCx::new(&c.id, signals, drag);
                let body = (c.build)(ctx, &mut bx);
                // The share goes on the OUTERMOST node, so it has to be applied after the wrappers:
                // a share set on the body would leave the wrapper content-sized and divide nothing
                // (F003/P011/T021's lesson, one level up).
                Some(with_share(focus_and_pick(body, &c.id, c.grow, ctx), c.grow))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    match bodies.len() {
        0 => None,
        // One container owns the region: its own share already makes it fill the region, so
        // there is nothing to wrap it in.
        1 => bodies.pop(),
        // Several containers share a region: stack them in the host's order (the order
        // `reorder`/`move_container` maintain), each keeping its own body and its own share. The
        // stack itself must be allowed to shrink to the region, or it takes its content's height
        // and overflows before the shares are ever divided.
        _ => {
            let mut stack = Flex::column().gap(8.0).grow(1.0);
            {
                let layout = &mut stack.base_mut().style.layout;
                layout.min_height = Some(heca_grid_ui::Length::Px(0.0));
                layout.flex_shrink = Some(1.0);
            }
            // A rule between containers, so two of them read as two things rather than one long
            // list. It takes no share: a `Separator` is a leaf with its own height, and `with_share`
            // only touches the containers, so the rule keeps its natural 1px and the shares divide
            // what is left. Its colour comes from the theme's border token, so it follows a reload.
            let count = bodies.len();
            Some(Box::new(bodies.into_iter().enumerate().fold(
                stack,
                |col, (i, body)| {
                    let col = if i > 0 && i < count {
                        col.child(Separator::horizontal())
                    } else {
                        col
                    };
                    col.child(body)
                },
            )))
        }
    }
}

/// Build a sidebar **SHELL** — a full-height, bracket-framed, frosted panel filling a
/// sidebar column. This is **one component, two instances**: the left and right
/// sidebars are the same shell differing only by width/position and the `content`
/// mounted inside. Per the chrome plan (`pluggable-chrome-plugin-plan.md` §2.1 / §2.8,
/// `docs/sidebar-provider-modes.md`) the sidebar is a *shell* that hosts a Provider's
/// content — and that is now literally true: `content` is whatever
/// [`ChromeHost::contributions`] seats in the region (see [`build_region_content`]), so
/// the shell knows nothing about workspaces. A region with no provider mounted passes
/// `None` and renders as an empty frame.
///
/// The body is a [`Box<dyn Component>`](heca_grid_ui::Component) — a provider's render
/// seam returns a built subtree, not a concrete widget type — which is why it is mounted
/// with [`Pane::child_boxed`] rather than `Parent::child`.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_sidebar_shell(
    region_w: f32,
    sidebar_h: f32,
    shell_bg: Color,
    sidebar_gap: f32,
    border_style: heca_config::appearance::BorderStyle,
    border_width: f32,
    border_radius: f32,
    content: Option<WidgetModel>,
) -> Flex {
    let inner_w = (region_w - sidebar_gap * 2.0).max(0.0);
    let inner_h = (sidebar_h - sidebar_gap * 2.0).max(0.0);
    // The collapse toggle lives in the always-visible top bar (sidebar-fu-14), so the
    // shell has no header row — the mounted content (if any) fills the body.
    let mut body = Pane::new()
        .border_style(border_style.into())
        .border_width(border_width)
        .radius(border_radius)
        .width(inner_w)
        .height(inner_h)
        .padding(10.0)
        .gap(8.0)
        .background(shell_bg);
    if let Some(content) = content {
        // Mounted directly: **the shell does not scroll** (F003/P011/T021).
        //
        // Scrolling is per container. Each one nests its own scroll area and scrolls its own
        // content, which is what makes two of them in a sidebar independent. A scroll viewport
        // around the whole stack would defeat that twice over: it takes the wheel for the sidebar
        // instead of the container under the cursor, and — because a viewport measures its content
        // at its natural height, which is the whole point of one — it leaves the containers
        // content-sized, so a fractional share has no height to divide and they bunch at the top.
        //
        // The shell's job is to give containers bounds. It hands them the region's height, they
        // take their shares of it, and each scrolls inside what it got.
        body = body.child(content);
    }
    Flex::column().width(region_w).height(sidebar_h).child(
        Surface::column()
            .width(region_w)
            .height(sidebar_h)
            .background(shell_bg)
            .padding(sidebar_gap)
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
    pub(super) side_bg: Color,
    pub(super) fg: Color,
}

/// A sidebar collapse toggle for the **top bar** (sidebar-fu-14): a small arrow
/// `IconButton` that emits `ActivateAction(action)` (expand↔rail for its region),
/// carrying a tooltip with its keybind (resolved centrally by `action_name` — like every other
/// chrome button; the tip is a property of the widget, so this still returns the widget). Lives in the always-visible top bar so it
/// works in both expanded and collapsed states.
#[allow(clippy::too_many_arguments)]
pub(super) fn sidebar_toggle_button(
    glyph: Glyph,
    action: crate::input::WmAction,
    action_name: &str,
    shortcuts: &ActionShortcuts,
    catalog: &crate::actions::ActionCatalog,
    emit: ChromeIntentEmitter,
    color: Color,
) -> IconButton {
    use crate::app::interaction::InteractionIntent;
    // Label from the action descriptor (catalog-owned), never re-spelled here.
    let label = catalog.label(action_name).unwrap_or(action_name);
    // One gesture: a click and a `prefix+/` pick both fire the button's action.
    let fire = {
        let emit = emit.clone();
        let action = action.clone();
        move || emit.fire(InteractionIntent::ActivateAction(action.clone()))
    };
    let hint = fire.clone();
    // Just pick the size variant — the widget derives icon px + padding from the
    // theme font internally (`Icon` with no explicit px uses the variant-scaled font,
    // `IconButton` scales its padding). No caller-side size math.
    // **Its identity is the action it runs, not the arrow it shows.** Both toggles flip their
    // glyph with the sidebar's state — left is `ArrowLineLeft` expanded and `ArrowLineRight`
    // collapsed, right is the mirror — so a derived identity (the glyph's name) changes under the
    // user every time they use the button, and the two buttons can even derive the *same* name at
    // once (left expanded and right collapsed are both `arrow_line_left`), at which point document
    // order decides which one wears the index. Either way the `prefix+/` letters moved on every
    // pick (Antonio, driving, 2026-08-19). The action name is stable through both states.
    let button = IconButton::new(Icon::new(glyph).color(color))
        .key(action_name)
        .size(WidgetSize::Small)
        .on_click(fire)
        // Declares its own pick — `on_hint` is on `ComponentExt`, so every widget has it. A
        // `KeyHint` wrapper here used to be a second pick target on top of the button's own
        // actionability (AGENTS § 5a).
        .on_hint(hint);
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
        side_bg,
        fg,
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
        if let Some(t) = left_toggle {
            band = band.child(t);
        }
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
        let status_label = Label::new(status).font_size(CHROME_TEXT_SIZE).color(fg);
        let status_signal = status_label.text_signal();
        let (status_watch, _status_repaint) = RepaintWatch::new(status_label);
        signals.status = Some(status_signal);
        root = root.child(
            Surface::row()
                .width(w)
                .height(status_bar_height)
                .background(side_bg)
                .radius(0.0)
                .align("center")
                .padding_xy(8.0, 0.0)
                .child(status_watch),
        );
    }
    root
}

/// Layout + paint a (retained) chrome root tree into a [`Scene`] at the window size.
/// Re-run every frame; cheap and creates no signals (those live in the retained tree).
pub(crate) fn paint_chrome_root(root: &mut Flex, w: f32, h: f32, theme: &GuiTheme) -> Scene {
    let mut scene = Scene::new();
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(root, Size::new(w as f64, h as f64));
    {
        let mut cx = PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
        heca_grid_ui::paint_child(root, &mut cx);
    }
    scene
}

/// Keycap glyph size (logical px) for follow-link hints — compact so a label sits
/// legibly over a single terminal cell.
const LINK_HINT_FONT: f32 = 13.0;

/// Peak alpha of the visual-bell flash overlay (faded out over the flash window).
const BELL_FLASH_MAX_ALPHA: u8 = 56;

/// Paint the **visual-bell** flash: a brief accent-tinted overlay over the content
/// area that fades out, while `state.bell_flash_until` is in the future. Drawn into
/// the chrome scene (on top). No-op when no flash is active. terminal-task-17.
pub(crate) fn paint_bell_flash(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    content_rect: Rectangle,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let Some(deadline) = state.bell_flash_until else {
        return;
    };
    let now = std::time::Instant::now();
    if now >= deadline {
        return;
    }
    let frac = deadline.saturating_duration_since(now).as_secs_f32()
        / crate::app::lifecycle::BELL_FLASH_DURATION.as_secs_f32();
    let alpha = (frac.clamp(0.0, 1.0) * BELL_FLASH_MAX_ALPHA as f32).round() as u8;
    if alpha == 0 {
        return;
    }
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    cx.rect(
        content_rect,
        theme.colors.accent.with_alpha(alpha),
        None,
        0.0,
        None,
    );
}

/// Paint follow-link keycaps over the focused terminal's hyperlinks while
/// [`InputMode::FollowLink`](crate::app_state::InputMode::FollowLink) is active.
/// Drawn into the chrome scene (painted last, on top of pane content) so the
/// letters sit above the terminal text, reusing the shared
/// [`paint_keycap`](heca_grid_ui::paint_keycap) visual. terminal-task-18.
pub(crate) fn paint_link_hints(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let crate::app_state::InputMode::FollowLink { candidates } = &state.input_mode else {
        return;
    };
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    for hint in candidates {
        let Some((x, y)) = crate::app::terminal_host::cell_screen_pos(
            state,
            hint.pane_id,
            hint.row,
            hint.start_col,
        ) else {
            continue;
        };
        let label = hint.label.to_string();
        let size = heca_grid_ui::keycap_size(LINK_HINT_FONT, &label);
        // Anchor the keycap's top-left at the link's first cell.
        let cap = Rectangle::new(Point::new(x as f64, y as f64), size);
        heca_grid_ui::paint_keycap(
            &mut cx,
            cap,
            &label,
            LINK_HINT_FONT,
            None,
            heca_grid_ui::KeycapVariant::Filled,
        );
    }
}

/// Peak alpha for a non-current search-match highlight; the current match is bolder.
const SEARCH_HL_ALPHA: u8 = 64;
const SEARCH_HL_CURRENT_ALPHA: u8 = 150;

/// Paint the scrollback-search overlay: a highlight rect over every visible match
/// (the focused one bolder) plus a `/query` bar anchored to the searched pane's
/// bottom-right. Drawn into the chrome scene (on top). No-op when no search is
/// active. terminal-task-19.
pub(crate) fn paint_search(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    if state.searches.is_empty() {
        return;
    }
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    // Every pane that has a search draws its own highlights and bar. They are
    // independent, so a search in one pane never disturbs another's.
    for (&pane_id, search) in &state.searches {
        paint_pane_search(
            state,
            &mut cx,
            pane_id,
            search,
            theme,
            Size::new(w as f64, h as f64),
        );
    }
}

/// Match highlights + query bar for one pane's search.
fn paint_pane_search(
    state: &crate::app_state::AppState,
    cx: &mut PaintCx,
    pane_id: PaneId,
    search: &crate::app_state::SearchState,
    theme: &GuiTheme,
    viewport: Size,
) {
    let Some(snapshot) = state
        .backends
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
    else {
        return;
    };
    let (cell_w, cell_h) = state
        .backends
        .get(pane_id)
        .map(|b| b.cell_size())
        .unwrap_or((8.0, 16.0));
    let top = snapshot.viewport_top_stable_row;
    let rows = snapshot.rows as isize;

    // Match highlights over the visible viewport.
    for (i, m) in search.matches.iter().enumerate() {
        let visible = m.stable_row - top;
        if visible < 0 || visible >= rows {
            continue;
        }
        let Some((x, y)) = crate::app::terminal_host::cell_screen_pos(
            state,
            pane_id,
            visible as usize,
            m.start_col,
        ) else {
            continue;
        };
        let width = m.end_col.saturating_sub(m.start_col) as f32 * cell_w;
        let rect = Rectangle::new(
            Point::new(x as f64, y as f64),
            Size::new(width as f64, cell_h as f64),
        );
        let alpha = if Some(i) == search.current {
            SEARCH_HL_CURRENT_ALPHA
        } else {
            SEARCH_HL_ALPHA
        };
        // A match highlight tracks terminal cells, not chrome, so it stays a painted
        // rect rather than a widget — but its corner still comes from the theme.
        cx.rect(
            rect,
            theme.colors.accent.with_alpha(alpha),
            None,
            theme.colors.control_radius(),
            None,
        );
    }

    paint_search_bar(state, cx, pane_id, search, theme, viewport);
}

/// Build the search bar's widget tree, positioned at `pane`'s bottom-right corner.
///
/// `field` is the size the query [`Input`] measured to — the tree reserves a slot of
/// exactly that size and the caller paints the retained field into it. The size is
/// measured by the layout engine, never derived from a character count.
///
/// Pure so it can be tested without a GPU or an `AppState`, which is how its
/// placement is covered.
pub(super) fn search_bar_tree(
    field: Size,
    count: Option<String>,
    pane: Rectangle,
    theme: &GuiTheme,
) -> Flex {
    // The query slot, then the match position as a separate chip so it reads as
    // distinct information rather than as part of what was typed.
    let mut row = Flex::row()
        .align("center")
        .gap(Spacing::Sm)
        .child(Flex::row().width(field.w as f32).height(field.h as f32));
    if let Some(count) = count {
        row = row.child(Tag::new(count).color(theme.colors.accent));
    }

    // A box the size of the pane, offset to the pane's origin, with the bar pushed
    // into its bottom-right corner. The engine does the positioning; nothing here
    // measures text or computes a coordinate.
    Flex::row()
        .justify("end")
        .align("end")
        .width(pane.size.w as f32)
        .height(pane.size.h as f32)
        .margin_left(pane.loc.x as f32)
        .margin_top(pane.loc.y as f32)
        .padding(Spacing::Sm.scale() * theme.font_size)
        .child(row)
}

/// The query field + match counter at the searched pane's bottom-right corner.
///
/// The query is a real [`Input`], so its caret, selection and the whole editing model
/// are the library's rather than reimplemented here. It is retained in [`SearchState`]
/// (a field must keep its caret across frames) and therefore cannot be moved into the
/// per-frame tree — so the tree reserves a slot and the field is painted into it, the
/// same arrangement [`CommandPalette`](heca_grid_ui::widgets::CommandPalette) uses for
/// its own query line.
fn paint_search_bar(
    state: &crate::app_state::AppState,
    cx: &mut PaintCx,
    pane_id: PaneId,
    search: &crate::app_state::SearchState,
    theme: &GuiTheme,
    viewport: Size,
) {
    let Some((_, px, py, pw, ph)) = crate::app::terminal_host::pane_outer_frames(state)
        .into_iter()
        .find(|(id, ..)| *id == pane_id)
    else {
        return;
    };

    let query = search.input.borrow().value_str();
    let count = (!query.is_empty()).then(|| {
        if search.matches.is_empty() {
            "no matches".to_string()
        } else {
            let pos = search.current.map(|i| i + 1).unwrap_or(0);
            format!("{}/{}", pos, search.matches.len())
        }
    });

    // Measure the field on its own first: the engine sizes it, so the bar reserves
    // exactly what it needs without anyone estimating a width from the query length.
    let field_size = {
        let mut field = search.input.borrow_mut();
        LayoutEngine::new()
            .base_font(theme.font_size)
            .compute(&mut *field, viewport);
        field.base().bounds.size
    };

    let pane = Rectangle::new(
        Point::new(px as f64, py as f64),
        Size::new(pw as f64, ph as f64),
    );
    let mut root = search_bar_tree(field_size, count, pane, theme);
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(&mut root, viewport);
    heca_grid_ui::paint_child(&root, cx);

    // Draw the retained field into the slot the tree reserved for it.
    let Some(slot) = search_field_slot(&root) else {
        return;
    };
    let mut field = search.input.borrow_mut();
    field.base_mut().bounds = slot;
    field.base_mut().font = theme.font_size;
    field.paint(cx);
}

/// Bounds of the slot [`search_bar_tree`] reserved for the query field:
/// pane box → row → first child.
pub(super) fn search_field_slot(root: &Flex) -> Option<Rectangle> {
    let row = root.base().children.first()?;
    Some(row.base().children.first()?.base().bounds)
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
    let (side_bg, _sidebar_bg, fg) = chrome_colors(state);
    let theme = chrome_gui_theme(state);
    let status = chrome_status(state);
    let mut signals = ChromeSignals::default();
    let mut drag_items = DragItemRegistry::default();
    let emit_intent = ChromeIntentEmitter::new(
        &state.event_proxy,
        crate::app::interaction::InteractionSource::MouseLeftSidebar,
    );

    // Expanded ⇄ Hidden: width is 0 when the region is Hidden (no icon rail — see
    // `docs/sidebar-provider-modes.md`), so a positive width means Expanded.
    // Left and right are two instances of the SAME `build_sidebar_shell` (one
    // component), differing only by width and mounted content: the left hosts the
    // `WorkspacesContainer`, the right is an empty placeholder until it gains a Provider.
    let sidebar_gap = state.appearance.effective_sidebar_gap(&state.theme);
    let border_style = state.appearance.effective_sidebar_border_style();
    let border_width = state
        .appearance
        .effective_sidebar_border_width(&state.theme);
    let border_radius = state
        .appearance
        .effective_sidebar_border_radius(&state.theme);
    let sidebar_h = (h - chrome.tab_bar_height - chrome.status_bar_height).max(0.0);

    // The region body is whatever the `ChromeHost` has seated in that region — the app
    // no longer knows that the left sidebar happens to hold the workspace tree. Moving
    // the `workspaces` container to the right region (`ChromeHost::move_container`) moves
    // its UI with it, with no change here.
    //
    // The context carries only what every component needs — the frame's theme and the intent sink
    // (F003/P086/T367). A component's own model is its own to read, so the host no longer borrows
    // one component's tree here on everybody's behalf.
    let ctx = crate::providers::ChromeCtx::for_build(
        crate::host::App::new(&state.chrome_state),
        &theme,
        &emit_intent,
        &state.action_catalog,
    );

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
            left_w,
            sidebar_h,
            left_sidebar_shell_background_color(state),
            sidebar_gap,
            border_style,
            border_width,
            border_radius,
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
            right_w,
            sidebar_h,
            right_sidebar_shell_background_color(state),
            sidebar_gap,
            border_style,
            border_width,
            border_radius,
            content,
        )
    });

    // Top-bar collapse toggles (sidebar-fu-14): shown for each mounted sidebar so the
    // expand/collapse control is always visible (works in both expanded + collapsed).
    // The arrow flips with the state: expanded → point at the edge (collapse); collapsed
    // → point away from the edge (expand).
    let left_toggle = state.show_left_sidebar.then(|| {
        let glyph = if state.chrome_state.left_visible() {
            Glyph::ArrowLineLeft
        } else {
            Glyph::ArrowLineRight
        };
        sidebar_toggle_button(
            glyph,
            crate::input::WmAction::SidebarLeft,
            "sidebar_left",
            &state.action_shortcuts,
            &state.action_catalog,
            emit_intent.clone(),
            theme.colors.muted,
        )
    });
    let right_toggle = state.show_right_sidebar.then(|| {
        let glyph = if state.chrome_state.right_visible() {
            Glyph::ArrowLineRight
        } else {
            Glyph::ArrowLineLeft
        };
        sidebar_toggle_button(
            glyph,
            crate::input::WmAction::SidebarRight,
            "sidebar_right",
            &state.action_shortcuts,
            &state.action_catalog,
            emit_intent.clone(),
            theme.colors.muted,
        )
    });

    let root = chrome_root(
        &ChromeFrame {
            w,
            h,
            tab_bar_height: chrome.tab_bar_height,
            status_bar_height: chrome.status_bar_height,
            status: &status,
            side_bg,
            fg,
        },
        left_sidebar,
        right_sidebar,
        left_toggle,
        right_toggle,
        &mut signals,
    );
    // The identity rule's warning half (F003/P082/T444): a collection of ours whose items were
    // never keyed loses its cursor position and its hint letters on the next rebuild, and nothing
    // fails when it does. Said once per distinct finding, in debug builds only.
    identity::report_ambiguous_widgets("chrome", &root);
    let source = emit_intent.source();
    (root, signals, drag_items, source)
}
