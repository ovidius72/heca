//! [`DockFrame`] — a titled, collapsible frame for hosting a Dock's content.
//!
//! It is the bracket-framed shell an app-side Dock (e.g. `WorkspacesDock`) drops
//! its content into: a **title bar** (drag-handle grip + collapse chevron +
//! title, plus a header-controls slot the Dock fills with its own affordances,
//! e.g. a search field) over a **body** that holds the content. The whole frame
//! reuses [`Pane`](super::Pane)'s prominent corner brackets via the shared
//! [`PaintCx::bracket_frame`].
//!
//! Presentation-only and domain-neutral, like [`ItemGroup`](super::ItemGroup):
//! the header toggles an `expanded` [`Signal`] and (optionally) reports it via
//! [`on_toggle`](DockFrame::on_toggle); collapsing hides the body via
//! `style.hidden` (`display: none`), so it folds out of layout entirely. The
//! drag-handle grip is **drawn but wired to nothing** (see [`child`](DockFrame::child) seam).
//!
//! **Two different drags will use it, and they are not the same feature** — worth knowing before
//! reading it as one (user, 2026-07-30):
//!
//! - the frame as a **container**: drag the whole dock into another chrome region. The host's
//!   actions for this already exist (`chrome.container.move_to_region` and friends); only the mouse
//!   surface is missing — **P079(F004)**, task `J5592` "DockFrame gets the drag handle it was
//!   specified with".
//! - the frame as a **row inside** a container: reorder it within the list. heca mounts one frameless
//!   `DockFrame` per workspace row, so this is the drag its grip sits next to — **P030(F006)**
//!   (app-05, workspace drag-to-reorder), which still needs its own action and drop logic.
//!
//! Until one of them wires a drag, the grip promises something nothing does. It is left in place
//! deliberately, as a placeholder for those phases.
//!
//! **Rail mode (built).** A Dock hosted in a
//! [`ChromeRegion`](super::ChromeRegion) can collapse to an **icon rail**. Bind
//! the region's [`RegionMode`](super::RegionMode) signal with
//! [`rail`](DockFrame::rail): while the region is in
//! [`RegionMode::CollapsedRail`](super::RegionMode::CollapsedRail) the frame
//! folds its header + body away and shows a single centered [`Icon`] instead.
//! Reading the mode (never writing it) keeps the chrome plan's *read-via-signals,
//! write-via-actions* contract — the host's toggle action expands the rail back.

use crate::action::{Action, SignalData};
use crate::builders::{ComponentExt, LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component, Event, Handled, PaintCx, paint_child};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::style::{Align, Direction, Justify};
use crate::scene::{Border, Glow};
use crate::widgets::{Flex, Glyph, Icon, Item, Label, RegionMode};

/// Chevron glyphs for expanded / collapsed states.
const CHEVRON_OPEN: &str = "▾";
const CHEVRON_CLOSED: &str = "▸";
/// Drag-handle grip glyph shown at the start of the title bar. Unwired — see the module docs for
/// which two phases claim it.
const GRIP: &str = "⠿";
/// Gap between the grip and the chevron in the header's leading slot.
const LEADING_GAP: f32 = 8.0;
/// Rest-glow spread radius (px) — the frame's share of the theme rest halo.
const GLOW_RADIUS: f32 = 12.0;
/// Inset of the header + body from the bracket frame, so content (title, the
/// header-controls slot, body rows) never collides with the corner brackets.
const CONTENT_PAD: f32 = 10.0;
/// Tighter inset used in [`RegionMode::CollapsedRail`] so the centered icon fits
/// a thin rail without colliding with the brackets.
const RAIL_PAD: f32 = 6.0;
/// Inset used when [`frameless`](DockFrame::frameless): no brackets to clear, so the
/// content can sit tight against the (host-provided) outer frame.
const FRAMELESS_PAD: f32 = 2.0;
/// Gap between the title bar and the body.
const HEADER_BODY_GAP: f32 = 8.0;
/// Gap between body rows.
const BODY_GAP: f32 = 4.0;
/// Size (logical px) of the centered glyph shown in [`RegionMode::CollapsedRail`].
const RAIL_ICON_SIZE: f32 = 22.0;

/// Index of the header (a [`Flex`] row) / body within `base.children`. The rail
/// icon, when configured via [`DockFrame::rail`], is appended at [`RAIL`].
const HEADER: usize = 0;
const BODY: usize = 1;
const RAIL: usize = 2;
/// Index of the controls slot within the header row (after the toggle [`Item`]).
const CONTROLS: usize = 1;
/// A titled, collapsible, bracket-framed container for a Dock.
pub struct DockFrame {
    base: Base,
    expanded: Signal<bool>,
    /// `expanded` as it was when the current event entered this widget, so
    /// [`after_subtree`](crate::component::Component::after_subtree) can tell whether the subtree
    /// flipped it.
    was_expanded: bool,
    /// Active (current) state: when `true` the frame paints a faint **wash** of the theme's
    /// selected colour (`effective_selected_background` at `active_wash_alpha`) over itself — e.g.
    /// the active workspace in the sidebar. A wash, not the panel a selected *row* gets: a
    /// container hint must not cancel out the selection inside it. Signal-backed so a host can flip it in place via
    /// [`active_state`](DockFrame::active_state) without rebuilding the tree.
    active: Signal<bool>,
    /// Sidebar-nav cursor state: paints a hollow **accent ring**, which reads over the selected
    /// panel rather than replacing it — an edge and a fill, never two fills. Signal-backed so the host flips it in place.
    nav: Signal<bool>,
    /// **"You were just here"** — the workspace `prefix+Shift+l` would return to. The faintest
    /// of the four weights, painted under the active wash. See [`previous`](DockFrame::previous).
    previous: Signal<bool>,
    /// Text signal of the header's chevron glyph (flipped on toggle).
    chevron: Signal<String>,
    on_toggle: Option<Box<dyn Fn(Action)>>,
    /// Hosting region's display mode; when present and `CollapsedRail`, the frame
    /// renders icon-only. Read-only — the host writes it via its toggle action.
    rail_mode: Option<Signal<RegionMode>>,
    /// When true, the corner-bracket frame is not painted and the content inset is
    /// tightened — for docks hosted inside an already-framed container (e.g. a
    /// sidebar shell) where per-dock brackets would be a redundant double border.
    frameless: bool,
}

#[heca_grid_ui_macros::props]
impl DockFrame {
    /// A new expanded frame titled `title`. Add body content with `.child(...)`
    /// and header controls with `.header(...)`.
    pub fn new(title: impl Into<String>) -> Self {
        let expanded = signal(true);
        let chevron_label = Label::new(CHEVRON_OPEN);
        let chevron = chevron_label.text_signal();

        // Leading slot: drag grip + chevron. Body visibility + chevron are synced
        // in `remeasure`/`event` (which have `&mut self`).
        let leading = Flex::row()
            .align(Align::Center)
            .gap(LEADING_GAP)
            .child(Label::new(GRIP))
            .child(chevron_label);
        // The toggle Item carries the title and flips `expanded` on activate; it
        // grows so the controls slot sits at the right edge.
        // **Its letter sits on the chevron it folds, in the muted tone.**
        //
        // Folding a dock is a real act, so it earns one of the 52 — but it is a structural control,
        // not somewhere to navigate to, and it was wearing the default cap in the default place:
        // centred on the title, on top of whatever the dock's own author declared there, in the
        // tone that means "a pane" (Antonio, driving, 2026-09-14 — two letters stacked on one
        // workspace header). Top-left puts it over the chevron, which is the thing it operates.
        let toggle = Item::new(title)
            .leading(leading)
            .on_activate(move || expanded.set(!expanded.get_untracked()))
            .hint_placement(crate::widgets::HintPlacement::TopLeft)
            .hint_tone(crate::widgets::HintTone::Muted)
            .grow(1.0);
        // Header row: [toggle (grows), controls slot]. Children — not the toggle
        // Item — so an interactive control (e.g. a search Input) still receives
        // events: `event` routes to the header's children, controls-first.
        let header = Flex::row()
            .align(Align::Center)
            .child(toggle)
            .child(Flex::empty());

        // Body holds the dock content; folds out of layout when collapsed.
        let body = Flex::column().gap(BODY_GAP);

        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        // Inset content from the brackets and space the title bar off the body.
        base.style.layout.padding = CONTENT_PAD;
        base.style.layout.gap = HEADER_BODY_GAP;
        base.children.push(Box::new(header));
        base.children.push(Box::new(body));
        // Invariant relied on by `header`/`child`/`sync` index access below.
        debug_assert_eq!(base.children.len(), 2, "DockFrame children: [HEADER, BODY]");
        debug_assert_eq!(
            base.children[HEADER].base().children.len(),
            2,
            "DockFrame header children: [toggle, CONTROLS]"
        );
        Self {
            base,
            expanded,
            was_expanded: expanded.get_untracked(),
            active: signal(false),
            nav: signal(false),
            previous: signal(false),
            chevron,
            on_toggle: None,
            rail_mode: None,
            frameless: false,
        }
    }

    /// Drop the corner-bracket frame (and tighten the content inset). Use when the
    /// dock is hosted inside an already-framed container — e.g. a sidebar shell —
    /// so it reads as a flat section rather than a redundant nested border.
    #[heca_grid_ui_macros::prop]
    pub fn frameless(mut self, frameless: bool) -> Self {
        self.frameless = frameless;
        self.sync();
        self
    }

    /// Set the initial expanded state.
    #[heca_grid_ui_macros::prop]
    pub fn expanded(self, open: bool) -> Self {
        self.expanded.set(open);
        self.chevron.set(chevron_for(open).to_string());
        self
    }

    /// Report toggles. Receives `Action::value("dock-toggle", Bool(expanded))`.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_toggle(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }

    /// Fill the header-controls slot — the Dock's own affordances (e.g. a search
    /// field). Interactive controls work: events reach the slot before the toggle.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn header(mut self, c: impl crate::builders::IntoComponent) -> Self {
        self.base.children[HEADER].base_mut().children[CONTROLS] = c.into_component();
        self
    }

    /// Append body content (folds away when collapsed). This is also the seam P079(F004)
    /// uses to make the frame draggable via the shipped `drag/` framework.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn child(mut self, c: impl crate::builders::IntoComponent) -> Self {
        self.base.children[BODY]
            .base_mut()
            .children
            .push(c.into_component());
        self
    }

    /// The expanded-state signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.expanded
    }

    /// Mark the frame **active** (the current one). An active frame paints a faint wash of the
    /// theme's selected colour over itself. Defaults to inactive.
    #[heca_grid_ui_macros::prop]
    pub fn active(self, active: bool) -> Self {
        self.active.set(active);
        self
    }

    /// Mark this frame as the workspace a **back-and-forth** binding would return to
    /// (`prefix+Shift+l`). The faintest wash of the selection colour, under the active one, so a
    /// frame that is both reads as active — you are there now, which outranks where you were.
    #[heca_grid_ui_macros::prop]
    pub fn previous(self, on: bool) -> Self {
        self.previous.set(on);
        self
    }

    /// The previous-state signal — bind it so the host flips the mark in place.
    pub fn previous_state(&self) -> Signal<bool> {
        self.previous
    }

    /// The active-state signal — bind it so the host can flip the wash in place
    /// (via the chrome's per-frame signal sync) without rebuilding the tree.
    pub fn active_state(&self) -> Signal<bool> {
        self.active
    }

    /// Mark the frame as the sidebar-nav **cursor** — a hollow accent border,
    /// shown distinctly from the active wash. Defaults to off.
    #[heca_grid_ui_macros::prop]
    pub fn nav_selected(self, on: bool) -> Self {
        self.nav.set(on);
        self
    }

    /// The nav-cursor signal — bind it so the host can flip the outline in place.
    pub fn nav_state(&self) -> Signal<bool> {
        self.nav
    }

    /// Make the frame **rail-aware**: it observes the hosting region's
    /// [`RegionMode`] signal and, while that signal is
    /// [`RegionMode::CollapsedRail`], folds its header + body away and shows
    /// `glyph` (a centered [`Icon`]) instead. Obtain the signal from the region
    /// before it is moved into `.dock(...)`:
    ///
    /// ```ignore
    /// let sidebar = ChromeRegion::vertical();
    /// let mode = sidebar.mode_signal();
    /// let files = DockFrame::new("FILES").rail(mode, Glyph::FolderOpen);
    /// let sidebar = sidebar.dock(files);
    /// ```
    #[heca_grid_ui_macros::host_only("bound to a live host signal, which static data cannot drive")]
    pub fn rail(mut self, mode: Signal<RegionMode>, glyph: Glyph) -> Self {
        self.rail_mode = Some(mode);
        // Stretch the wrapper across the rail's width and center the glyph in it.
        let icon = Flex::row()
            .justify(Justify::Center)
            .child(Icon::new(glyph).size(RAIL_ICON_SIZE));
        if self.base.children.len() > RAIL {
            self.base.children[RAIL] = Box::new(icon);
        } else {
            self.base.children.push(Box::new(icon));
        }
        self.sync();
        self
    }

    /// Apply the current state to child visibility + the chevron glyph. In rail
    /// mode the header + body fold away and only the rail icon shows; otherwise
    /// the body follows `expanded` and the rail icon (if any) stays hidden.
    fn sync(&mut self) {
        let open = self.expanded.get_untracked();
        self.chevron.set(chevron_for(open).to_string());
        let rail = self
            .rail_mode
            .is_some_and(|m| m.get_untracked() == RegionMode::CollapsedRail);
        self.base.children[HEADER].base_mut().set_hidden(rail);
        self.base.children[BODY].base_mut().set_hidden(rail || !open);
        if self.base.children.len() > RAIL {
            self.base.children[RAIL].base_mut().set_hidden(!rail);
        }
        // Tighten the frame inset in the rail so the icon fits the thin column;
        // frameless docks tighten too since there are no brackets to clear.
        self.base.style.layout.padding = if rail {
            RAIL_PAD
        } else if self.frameless {
            FRAMELESS_PAD
        } else {
            CONTENT_PAD
        };
    }
}

fn chevron_for(open: bool) -> &'static str {
    if open { CHEVRON_OPEN } else { CHEVRON_CLOSED }
}

impl Component for DockFrame {
    /// The navigation cursor is "the current one" for this list, so an enclosing scroll region
    /// keeps it in view — the keyboard half of scrolling, without the host wiring it per list.
    fn wants_visible(&self) -> bool {
        self.nav.get_untracked() || self.base.focused_by_keyboard()
    }

    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Keep body visibility + chevron in sync with `expanded` every layout pass
    /// (covers the initial `.expanded(false)` state and external signal changes).
    fn remeasure(&mut self) {
        self.sync();
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let radius = cx.theme().colors.border_radius;
        let b = self.base.bounds;
        let fill = self.base.style.visual.fill;

        // Background fill — rounded by theme radius. An explicit `.glow(..)`
        // (StyleExt) wins; otherwise the theme rest glow gives the frame the
        // shared neon identity at rest, scaled by `glow_size` (T011).
        if let Some(f) = fill {
            let glow = self.base.style.visual.glow.or_else(|| cx.rest_glow(GLOW_RADIUS));
            cx.rect(b, f, None, radius, glow);
        }

        // Active-region wash — a faint accent overlay over the whole frame when
        // this is the active one (e.g. the active workspace). Theme-driven alpha
        // and signal-backed, so the host flips it in place (no tree rebuild).
        // **"You were just here"** — under the active wash, so a frame that is both simply reads
        // as active. Where you are outranks where you were.
        if self.previous.get_untracked() && !self.active.get_untracked() {
            // ⚠️ **A frame's wash is CONTAINER-scale, never the row's `previous_wash_alpha`.**
            //
            // That token is tuned against a single row; over a whole frame the same value lifts
            // every row inside it, and the pane mark within — which is the same colour — then has
            // almost nothing left to say. The last-visited pane became indistinguishable from its
            // siblings in exactly the workspace it was meant to be found in (Antonio, driving, both
            // dark themes, 2026-08-13). So this is derived from `active_wash_alpha`, the theme's own
            // answer for how faint a container hint is, and sits below it.
            cx.rect(b, cx.theme().colors.effective_workspace_previous_background(), None, radius, None);
        }
        if self.active.get_untracked() {
            // **A faint WASH of the selected colour — never the panel itself.**
            //
            // One colour family with `Row`'s selection, two different levels of hierarchy: this
            // says *the current workspace*, a container hint, while a row's opaque panel says
            // *this item is selected*. Painting both at full strength made them cancel exactly —
            // the selected row inside the active frame became the same colour as the frame, and
            // only the column's marker bar still said which row it was (Antonio, driving, with
            // three themes, 2026-08-13). `active_wash_alpha` is the theme's own answer to how
            // faint a container hint should be.
            cx.rect(b, cx.theme().colors.effective_workspace_active_background(), None, radius, None);
        }

        // Nav-cursor outline — a thick border + faint fill marking the cursor on this frame.
        // Drawn ALWAYS when nav, even on the active frame, so it stays visible where it coincides
        // with the active wash.
        //
        // ⚠️ **The cursor is an EDGE; what is selected is a FILL** — the same rule, and the same
        // reason, as `Row`'s nav outline. The two are drawn on the same box, so separating them by
        // hue cannot work (and was tried twice): separating them by *kind* can, because a ring lets
        // whatever is under it show through. So this draws the glowing accent border and only the
        // faintest wash — the selected panel underneath stays visible inside it.
        if self.nav.get_untracked() {
            let (cursor_c, glow, border_w, nav_wash, nav_outline) = {
                let t = cx.theme();
                (t.colors.accent, t.colors.glow, t.focus_border_width, t.colors.interaction.nav_wash, t.colors.interaction.nav_outline)
            };
            cx.rect(
                b,
                cursor_c.with_alpha(nav_wash),
                Some(Border {
                    color: cursor_c.with_alpha(nav_outline),
                    width: (border_w * 2.0).max(2.5),
                }),
                radius,
                Some(Glow { color: glow, radius: 8.0, intensity: 0.25 }),
            );
        }

        // Prominent flat corner-bracket frame (shared with Pane) — unless frameless
        // (hosted inside an already-framed container, e.g. a sidebar shell).
        if !self.frameless {
            cx.bracket_frame(b);
        }

        // Header + body draw themselves; a collapsed (hidden) body is skipped so
        // its rows don't stamp at the layout-collapsed top-left.
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
    }

    /// This widget **watches what its own subtree did**: the header row flips `expanded`, and the
    /// group reports that as a toggle. That has to happen even when the header consumed the click
    /// — which is the normal case, not the exception.
    ///
    /// It used to own the whole child walk to get that, which made every event kind depend on this
    /// one container forwarding it correctly forever. [`after_subtree`](Component::after_subtree)
    /// is the same observation with none of that: the framework still does the walk.
    /// Capture is where the "before" is taken: it runs on the way down, before anything in the
    /// subtree can have flipped anything. Nothing is consumed here.
    fn on_event_capture(&mut self, _ev: &Event) -> Handled {
        self.was_expanded = self.expanded.get_untracked();
        Handled::No
    }

    fn after_subtree(&mut self, _ev: &Event, _handled: Handled) {
        let now = self.expanded.get_untracked();
        if now != self.was_expanded {
            self.was_expanded = now;
            self.sync();
            if let Some(f) = &self.on_toggle {
                f(Action::value("dock-toggle", SignalData::Bool(now)));
            }
        }
    }
}

impl LayoutExt for DockFrame {}
impl StyleExt for DockFrame {}
