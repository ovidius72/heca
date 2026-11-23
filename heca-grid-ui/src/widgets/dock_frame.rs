//! [`DockFrame`] — a titled, collapsible frame for hosting a Dock's content.
//!
//! It is the bracket-framed shell an app-side Dock (e.g. `WorkspacesDock`) drops
//! its content into: a **title bar** (drag-handle grip + collapse chevron +
//! title, plus a header-actions slot the Dock fills with its own affordances,
//! e.g. a search field) over a **body** that holds the content. The whole frame
//! reuses [`Pane`](super::Pane)'s prominent corner brackets via the shared
//! [`PaintCx::bracket_frame`].
//!
//! Presentation-only and domain-neutral, like [`ItemGroup`](super::ItemGroup):
//! the header toggles an `expanded` [`Signal`] and (optionally) reports it via
//! [`on_toggle`](DockFrame::on_toggle); collapsing hides the body via
//! `style.hidden` (`display: none`), so it folds out of layout entirely.
//!
//! **The grip is the frame's handle.** A frame made draggable (`.draggable_as(..)`) is picked up by
//! its grip and nowhere else ([`DragExt::drag_by_handle`](crate::builders::DragExt::drag_by_handle)),
//! so its title row still folds on a click and its body keeps its own presses. A frame that is not
//! draggable leaves the grip doing nothing: it never reaches past the frame to pick up what holds
//! it. heca's workspace rows and heca-pro's rooms are frames dragged this way; heca's docks are
//! moved by the host, which makes the seat around every dock draggable with the window's move key.
//!
//! **Rail mode (built).** A Dock hosted in a
//! [`ChromeRegion`](super::ChromeRegion) can collapse to an **icon rail**. Bind
//! the region's [`RegionMode`] signal with
//! [`rail`](DockFrame::rail): while the region is in
//! [`RegionMode::CollapsedRail`] the frame
//! folds its header + body away and shows a single centered [`Icon`] instead.
//! Reading the mode (never writing it) keeps the chrome plan's *read-via-signals,
//! write-via-actions* contract — the host's toggle action expands the rail back.

use crate::action::{Action, SignalData};
use crate::builders::{DragExt, EventExt, LayoutExt, Parent, PlaceExt, StyleExt};
use crate::component::{Base, Component, Event, Handled, PaintCx, paint_child};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, Glow};
use crate::style::{Spacing, Track};
use crate::widgets::{Flex, Glyph, Grid, Icon, Item, Label, RegionMode};

/// Chevron glyphs for expanded / collapsed states.
const CHEVRON_OPEN: &str = "▾";
const CHEVRON_CLOSED: &str = "▸";
/// The grip glyph a [`Dock`](DockFrameVariant::Dock)'s title row shows by default — **the handle
/// a row of a reorderable list is dragged by**, the same on every list so one gesture reads one
/// way. Another one is set with [`DockFrame::grip_glyph`].
const GRIP: &str = "⠿";
/// Gap between the grip and the chevron in the header's leading slot.
const LEADING_GAP: Spacing = Spacing::Sm;
/// The frame's rest halo, as a share of the theme's glow radius (`active_glow_radius`).
const GLOW_SCALE: f32 = 1.2;
/// Inset of the header + body from the bracket frame, so content (title, the
/// header-actions slot, body rows) never collides with the corner brackets.
const CONTENT_PAD: Spacing = Spacing::Sm;
/// Tighter inset used in [`RegionMode::CollapsedRail`] so the centered icon fits
/// a thin rail without colliding with the brackets.
const RAIL_PAD: Spacing = Spacing::Xs;
/// Inset used when [`frameless`](DockFrame::frameless): no brackets to clear, so the
/// content can sit tight against the (host-provided) outer frame.
const FRAMELESS_PAD: Spacing = Spacing::Hairline;
/// Gap between the title bar and the body.
const HEADER_BODY_GAP: Spacing = Spacing::Sm;
/// Gap between body rows.
const BODY_GAP: Spacing = Spacing::Xs;
/// Size (logical px) of the centered glyph shown in [`RegionMode::CollapsedRail`].
const RAIL_ICON_SIZE: f32 = 22.0;

/// **The frame's parts, named.** Each says which one it is
/// ([`PlaceExt::area`]) and the frame asks by name within its own template — never
/// by its position in `children`, or by searching a nested frame's parts.
///
/// An index is right only for the arrangement it was written for: insert a part and every index
/// after it is wrong, while nothing fails, because the indices are still valid indices. These used
/// to be `HEADER = 0`, `BODY = 1`, `RAIL = 2`, `TOGGLE = 0`, `CONTROLS = 1`, documented as an
/// invariant and asserted at construction — which is the shape a template removes the need for.
pub const HEADER_AREA: &str = "header";
/// The dock's content.
pub const BODY_AREA: &str = "body";
/// The single glyph shown instead of everything else in [`RegionMode::CollapsedRail`].
pub const RAIL_AREA: &str = "rail";
/// The title row's fold toggle.
pub const TOGGLE_AREA: &str = "toggle";
/// The title row's slot for a caller's own affordances.
pub const HEADER_ACTIONS_AREA: &str = "header_actions";
/// The place before the title — an avatar, an icon ([`leading`](DockFrame::leading)).
pub const LEADING_AREA: &str = "leading";
/// The fold chevron, wherever the variant puts it.
const CHEVRON_AREA: &str = "dock.chevron";
/// A header written by the caller ([`header`](DockFrame::header)), inside the fold toggle.
const CUSTOM_AREA: &str = "dock.header";

/// **How a [`DockFrame`]'s title row is laid out.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum DockFrameVariant {
    /// The dock's own frame: a grip and the fold chevron before the title, header actions at the right,
    /// the body under the whole row. The default.
    #[default]
    Dock,
    /// **A section of a list** — a modern sidebar's group: an optional
    /// [`leading`](DockFrame::leading) part (an avatar, an icon) right before the title, the fold
    /// chevron at the end of the row, no grip. Only the title row differs: the body is laid out
    /// as in a `Dock` — whether its rows are indented is the caller's decision, written in the
    /// content it gives.
    Section,
}
/// A titled, collapsible, bracket-framed container for a Dock.
pub struct DockFrame {
    base: Base,
    expanded: Signal<bool>,
    /// `expanded` as it was when the current event entered this widget, so
    /// [`after_subtree`](crate::component::Component::after_subtree) can tell whether the subtree
    /// flipped it.
    was_expanded: bool,
    /// Active (current) state: when `true` the frame paints a faint **wash** of the theme's
    /// workspace colour at `active_wash_alpha` over itself — e.g.
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
    /// The named rows of [`ROWS`], as a stylesheet writes `grid-template-areas`: the title
    /// then the body. Held rather than made per call because
    /// [`grid_template`](Component::grid_template) hands out a borrow of it.
    areas: Vec<String>,
    /// The part before the title ([`leading`](DockFrame::leading)), waiting for the title row to
    /// be made with it. Once made, it lives in the row and is taken back from there by name.
    leading: Option<crate::WidgetModel>,
    /// How the title row is laid out.
    variant: DockFrameVariant,
    /// The title the default toggle shows.
    title: String,
    /// Whether the grip shows (a [`Dock`](DockFrameVariant::Dock)'s only).
    grip: bool,
    /// The glyph the grip shows ([`grip_glyph`](Self::grip_glyph)); [`GRIP`] unless set.
    grip_glyph: String,
    /// Whether the header folds the body; `false` hides the chevron and keeps the body open.
    collapsible: Signal<bool>,
    /// The fold letter's tone ([`fold_hint_tone`](DockFrame::fold_hint_tone)), kept so a toggle
    /// made again keeps it.
    fold_tone: Option<crate::Tone>,
}

#[heca_grid_ui_macros::props]
impl DockFrame {
    /// A new expanded frame titled `title`. Add body content with `.child(...)`
    /// and header controls with `.header_actions(...)`.
    pub fn new(title: impl Into<String>) -> Self {
        let expanded = signal(true);
        let chevron = signal(CHEVRON_OPEN.to_string());
        // The toggle takes remaining width; header actions keep their natural width and own input. The
        // toggle itself is made by `build_toggle`, from the variant and what the caller gave.
        let header = Grid::new()
            .template_column("1fr auto")
            .template_area(&format!("{TOGGLE_AREA} {HEADER_ACTIONS_AREA}"))
            .align("center")
            .child(Flex::empty().area(TOGGLE_AREA))
            .child(Flex::empty().area(HEADER_ACTIONS_AREA))
            .area(HEADER_AREA);

        // Body holds the dock content; folds out of layout when collapsed.
        let body = Flex::column().gap(BODY_GAP).area(BODY_AREA);

        let mut base = Base::new();
        // Inset content from the brackets and space the title bar off the body.
        base.style.layout.padding = (CONTENT_PAD).into();
        base.style.layout.gap = (HEADER_BODY_GAP).into();
        base.children.push(crate::WidgetModel::new(header));
        base.children.push(crate::WidgetModel::new(body));
        let mut frame = Self {
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
            areas: vec![HEADER_AREA.to_string(), BODY_AREA.to_string()],
            leading: None,
            variant: DockFrameVariant::Dock,
            title: title.into(),
            grip: true,
            grip_glyph: GRIP.to_owned(),
            collapsible: signal(true),
            fold_tone: None,
        };
        frame.build_toggle();
        frame
    }

    /// **The fold toggle — the title row's clickable part**, made from the variant and what the
    /// caller gave: the title, or a header of its own ([`header`](Self::header)). Made again when
    /// either changes, keeping a caller's header.
    ///
    /// **Its letter sits on the chevron it folds.** Folding is a real act, so it earns one of the
    /// 52 — top-left over a [`Dock`](DockFrameVariant::Dock)'s chevron, top-right over a
    /// [`Section`](DockFrameVariant::Section)'s; that is geometry only this widget knows. **What
    /// the letter MEANS is not this widget's to say** — a caller says it with `fold_hint_tone`.
    fn build_toggle(&mut self) {
        let (expanded, collapsible) = (self.expanded, self.collapsible);
        let fold = move || {
            if collapsible.get_untracked() {
                expanded.set(!expanded.get_untracked());
            }
        };
        let chevron_text = self.chevron;
        let chevron = move || {
            Label::new("")
                .with_text_signal(chevron_text)
                .area(CHEVRON_AREA)
        };
        let mut take = |name: &str| {
            self.part_mut(TOGGLE_AREA)
                .and_then(|toggle| crate::component::area_slot(toggle.base_mut(), name))
                .map(|slot| std::mem::replace(slot, crate::WidgetModel::new(Flex::empty())))
        };
        let custom = take(CUSTOM_AREA);
        let kept_leading = take(LEADING_AREA);
        let leading_part = self.leading.take().or(kept_leading);
        // Picked up only by the grip while the title row shows one.
        self.base.drag_by_handle =
            self.grip && custom.is_none() && self.variant == DockFrameVariant::Dock;
        let (toggle, corner): (crate::WidgetModel, _) = match (custom, self.variant) {
            (Some(content), _) => (
                crate::WidgetModel::new(
                    crate::widgets::Row::new()
                        .on_click(move |_| fold())
                        .gap(LEADING_GAP)
                        .padding_y(crate::widgets::item::ROW_PAD_Y)
                        .child(leading_part)
                        .child(content)
                        .child(chevron()),
                ),
                crate::widgets::HintPlacement::TopRight,
            ),
            (None, DockFrameVariant::Dock) => {
                let mut leading = Flex::row().align("center").gap(LEADING_GAP);
                if self.grip {
                    // **The grip is the frame's handle**: a frame that can be dragged is picked up
                    // by it, and nowhere else, so its content keeps its own clicks.
                    leading = leading.child(Label::new(self.grip_glyph.clone()).drag_handle());
                }
                (
                    crate::WidgetModel::new(
                        Item::new(self.title.clone())
                            .leading(leading.child(chevron()).child(leading_part))
                            .on_click(move |_| fold()),
                    ),
                    crate::widgets::HintPlacement::TopLeft,
                )
            }
            (None, DockFrameVariant::Section) => {
                let item = Item::new(self.title.clone())
                    .trailing(chevron())
                    .on_click(move |_| fold());
                let item = match leading_part {
                    Some(part) => item.leading(part),
                    None => item,
                };
                (
                    crate::WidgetModel::new(item),
                    crate::widgets::HintPlacement::TopRight,
                )
            }
        };
        let mut toggle = toggle;
        toggle.base_mut().grid_area = Some(TOGGLE_AREA.to_string());
        toggle.base_mut().hint_style.placement = corner;
        toggle.base_mut().hint_style.tone = self.fold_tone;
        if let Some(slot) = self.part_mut(TOGGLE_AREA) {
            *slot = toggle;
        }
        self.sync();
    }

    /// **How the title row is laid out** ([`DockFrameVariant`]): `Dock` (the default) or `Section`
    /// for a group in a list — leading part right before the title, chevron at the end. The body
    /// is the same in both.
    ///
    /// ```ignore
    /// DockFrame::new("Development")
    ///     .variant(DockFrameVariant::Section)
    ///     .leading(avatar)
    ///     .child(members)
    /// ```
    #[heca_grid_ui_macros::prop]
    pub fn variant(mut self, variant: DockFrameVariant) -> Self {
        self.variant = variant;
        self.build_toggle();
        self
    }

    /// **The part before the title** — an avatar, an icon, an image — inside the title row,
    /// centred on it like the row's other parts: right before the title in a
    /// [`Section`](DockFrameVariant::Section), after the chevron in a
    /// [`Dock`](DockFrameVariant::Dock), before a [`header`](Self::header) of the caller's own.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn leading(mut self, c: impl crate::builders::IntoComponent) -> Self {
        self.leading = Some(crate::WidgetModel::new(
            Flex::row()
                .align("center")
                .area(LEADING_AREA)
                .child(c.into_component()),
        ));
        self.build_toggle();
        self
    }

    /// **A title row written by the caller** — any widget, composed as it likes. The frame still
    /// owns folding: a click on the row folds the body, and the frame's chevron stays at its end
    /// (unless [`collapsible(false)`](Self::collapsible)). Controls inside it keep their own input.
    ///
    /// ```ignore
    /// DockFrame::new("Development").header(Flex::row().gap(Spacing::Xs).child([avatar, title, count]))
    /// ```
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn header(mut self, c: impl crate::builders::IntoComponent) -> Self {
        let content = c.into_component();
        // Hand the content to `build_toggle` through the slot it reads a caller's header from.
        // The holder keeps its area through every rebuild, so the content is wrapped once.
        // It starts where a row's content starts, as a Dock's own title row (an `Item`) does.
        let holder = Flex::row()
            .child(
                Flex::row()
                    .grow(1.0)
                    .padding_left(crate::widgets::item::ROW_INSET)
                    .area(CUSTOM_AREA)
                    .child(content),
            )
            .area(TOGGLE_AREA);
        if let Some(slot) = self.part_mut(TOGGLE_AREA) {
            *slot = crate::WidgetModel::new(holder);
        }
        self.build_toggle();
        self
    }

    /// **Whether the header folds the body** (default `true`). `false`: no chevron, the body stays
    /// open and a click on the header folds nothing.
    #[heca_grid_ui_macros::prop]
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible.set(collapsible);
        if !collapsible {
            self.expanded.set(true);
        }
        self.sync();
        self
    }

    /// **Whether a [`Dock`](DockFrameVariant::Dock)'s title row shows the grip** (default `true`).
    /// A [`Section`](DockFrameVariant::Section) has none.
    #[heca_grid_ui_macros::prop]
    pub fn grip(mut self, grip: bool) -> Self {
        self.grip = grip;
        self.build_toggle();
        self
    }

    /// **The glyph the grip shows** (default `⠿`, the reorder handle every reorderable list shares —
    /// workspaces, rooms). Set one only for a handle that genuinely means something else:
    ///
    /// ```ignore
    /// DockFrame::new("Lobby").grip_glyph("≡")
    /// ```
    ///
    /// Takes an `Option` too: `None` keeps the glyph it has, so a frame that is only sometimes a
    /// reorderable row needs no branch.
    #[heca_grid_ui_macros::prop]
    pub fn grip_glyph(mut self, glyph: impl crate::builders::IntoName) -> Self {
        if let Some(glyph) = glyph.into_name() {
            self.grip_glyph = glyph;
            self.build_toggle();
        }
        self
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

    /// Fill the header-actions slot at the right of the title row — the Dock's own affordances
    /// (e.g. a search field, a count). Controls retain their own input and focus; clicking one does
    /// not toggle the dock.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn header_actions(mut self, c: impl crate::builders::IntoComponent) -> Self {
        // They end where a row's content ends, so a count in the title row lines up with the
        // badges at the end of the rows under it — whoever gives them adds no spacer.
        let fresh = crate::WidgetModel::new(
            Flex::row()
                .align("center")
                .justify("end")
                .padding_right(crate::widgets::item::ROW_INSET)
                .area(HEADER_ACTIONS_AREA)
                .child(c.into_component()),
        );
        if let Some(slot) = self.part_mut(HEADER_ACTIONS_AREA) {
            *slot = fresh;
        }
        self
    }

    /// Append body content (folds away when collapsed). This is also the seam a dock
    /// drag uses to make the frame draggable via the shipped `drag/` framework.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn child(mut self, c: impl crate::builders::IntoChildren) -> Self {
        if let Some(body) = self.part_mut(BODY_AREA) {
            body.base_mut().children.extend(c.into_children());
        }
        self
    }

    /// **What the fold control's letter means**, for the theme to colour.
    ///
    /// Folding is a real act, so the toggle earns a letter — but which *class* of target it reads
    /// as belongs to whoever is assembling the surface, not here: `accent`, `warning` and the rest
    /// are a vocabulary an app assigns across its own kinds, and a domain-neutral widget claiming
    /// one of them would be the library deciding app styling.
    ///
    /// Unset, the letter takes the picker's own colour, exactly as every letter did before tones
    /// existed. heca passes `Muted`: a fold is a structural control, not somewhere to navigate to.
    ///
    /// ```ignore
    /// DockFrame::new("Docker").fold_hint_tone(Tone::Muted)
    /// ```
    #[heca_grid_ui_macros::prop]
    pub fn fold_hint_tone(mut self, tone: crate::Tone) -> Self {
        self.fold_tone = Some(tone);
        self.part_mut(TOGGLE_AREA)
            .expect("the title row always holds its toggle")
            .base_mut()
            .hint_style
            .tone = Some(tone);
        self
    }

    /// The expanded-state signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.expanded
    }

    /// Mark the frame active (the current one). A state background is painted only when the
    /// theme enables `active_wash_alpha`; it is off by default. Defaults to inactive.
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
            .justify("center")
            .child(Icon::new(glyph).size(RAIL_ICON_SIZE));
        // The rail replaces everything, so it covers both tracks. It is not in the template's
        // areas — it is not a third part beside the other two, it is what is there instead of
        // them — so it says where it sits itself.
        let icon = icon
            .area(RAIL_AREA)
            .row(1)
            .row_span(crate::style::Span::All);
        match self.part_mut(RAIL_AREA) {
            Some(slot) => *slot = crate::WidgetModel::new(icon),
            None => self.base.children.push(crate::WidgetModel::new(icon)),
        }
        self.sync();
        self
    }

    /// Apply the current state to child visibility + the chevron glyph. In rail
    /// mode the header + body fold away and only the rail icon shows; otherwise
    /// the body follows `expanded` and the rail icon (if any) stays hidden.
    fn sync(&mut self) {
        let collapsible = self.collapsible.get_untracked();
        let open = self.expanded.get_untracked() || !collapsible;
        self.chevron.set(chevron_for(open).to_string());
        if let Some(header) = self.base.child_in_area(HEADER_AREA)
            && let Some(chevron) = crate::component::area_slot(header.base_mut(), CHEVRON_AREA)
        {
            chevron.base_mut().set_hidden(!collapsible);
        }
        let rail = self
            .rail_mode
            .is_some_and(|m| m.get_untracked() == RegionMode::CollapsedRail);
        let hide = |base: &mut Base, name: &str, hidden: bool| {
            if let Some(part) = base.child_in_area(name) {
                part.base_mut().set_hidden(hidden);
            }
        };
        hide(&mut self.base, HEADER_AREA, rail);
        hide(&mut self.base, BODY_AREA, rail || !open);
        hide(&mut self.base, RAIL_AREA, !rail);
        // Tighten the frame inset in the rail so the icon fits the thin column;
        // frameless docks tighten too since there are no brackets to clear.
        self.base.style.layout.padding = if rail {
            RAIL_PAD
        } else if self.frameless {
            FRAMELESS_PAD
        } else {
            CONTENT_PAD
        }
        .into();
    }

    /// Only this frame's template owns its parts; a component in the header may own the same names.
    fn part_mut(&mut self, name: &str) -> Option<&mut crate::WidgetModel> {
        if matches!(name, TOGGLE_AREA | HEADER_ACTIONS_AREA) {
            self.base
                .child_in_area(HEADER_AREA)?
                .base_mut()
                .child_in_area(name)
        } else {
            self.base.child_in_area(name)
        }
    }
}

fn chevron_for(open: bool) -> &'static str {
    if open { CHEVRON_OPEN } else { CHEVRON_CLOSED }
}

/// **The frame's shape, as a stylesheet would write it: a title at its own height over a body
/// that takes the rest.**
///
/// It used to be a flex column, which meant the body sized to its rows — so a dock given half a
/// sidebar drew a 914px body inside a 450px frame, its rows ran off the bottom, and its own scroll
/// area, handed more height than it held, had nothing left to scroll. `1fr` is the whole of that:
/// what the title leaves, and no more, whatever is inside it.
const ROWS: [Track; 2] = [Track::Auto, Track::Fr(1.0)];

impl Component for DockFrame {
    /// **The cursor is on this frame**: lights the outline its nav signal is bound to, the way a
    /// [`Row`](crate::widgets::Row) does, so a dock's cursor reaches it by key with nothing wired.
    fn set_selected(&self, on: bool) -> bool {
        use crate::reactive::{SignalGet as _, SignalUpdate as _};
        if self.nav.get_untracked() != on {
            self.nav.set(on);
        }
        true
    }

    /// The navigation cursor is "the current one" for this list, so an enclosing scroll region
    /// keeps it in view — the keyboard half of scrolling, without the host wiring it per list.
    fn wants_visible(&self) -> bool {
        self.nav.get_untracked() || self.base.focused_by_keyboard()
    }

    /// The frame arranges its parts; `ROWS` says how.
    fn taffy_style(&self) -> taffy::Style {
        self.base
            .style
            .layout
            .to_taffy_grid(self.base.font, &[], &ROWS)
    }

    /// **The header and the body are placed by NAME, against this.** The column track is left
    /// unsaid on purpose: an implicit track fills, so the frame's parts are as wide as the frame.
    fn grid_template(&self) -> Option<crate::style::GridTemplate<'_>> {
        Some(crate::style::GridTemplate {
            columns: &[],
            rows: &ROWS,
            areas: &self.areas,
        })
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
        // shared neon identity at rest, scaled by `glow_size`.
        if let Some(f) = fill {
            let glow = self
                .base
                .style
                .visual
                .glow
                .or_else(|| cx.rest_glow(cx.theme().colors.active_glow_radius * GLOW_SCALE));
            cx.rect(b, f, None, radius, glow);
        }

        // Container-wide state backgrounds are optional and use the documented theme opacity.
        // Apply it only to these draws: text, badges and the navigation outline stay crisp.
        cx.with_opacity(cx.theme().colors.active_wash_alpha, |cx| {
            if self.previous.get_untracked() && !self.active.get_untracked() {
                cx.rect(
                    b,
                    cx.theme().colors.effective_workspace_previous_background(),
                    None,
                    radius,
                    None,
                );
            }
            if self.active.get_untracked() {
                cx.rect(
                    b,
                    cx.theme().colors.effective_workspace_active_background(),
                    None,
                    radius,
                    None,
                );
            }
        });

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
                (
                    cx.accent(),
                    t.colors.glow,
                    t.focus_border_width,
                    t.colors.interaction.nav_wash,
                    t.colors.interaction.nav_outline,
                )
            };
            cx.rect(
                b,
                cursor_c.with_alpha(nav_wash),
                Some(Border {
                    color: cursor_c.with_alpha(nav_outline),
                    width: (border_w * 2.0).max(2.5),
                }),
                radius,
                Some(Glow {
                    color: glow,
                    radius: 8.0,
                    intensity: 0.25,
                }),
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

impl Parent for DockFrame {
    fn child(self, children: impl crate::builders::IntoChildren) -> Self {
        DockFrame::child(self, children)
    }
}
