//! Assemble the catalog and its shared controls. Each demo lives beside this module.
use super::*;
mod connections_and_text;
mod images;
mod select;
mod button;
mod composed_buttons;
mod choices;
mod toggle;
mod checkbox;
mod label;
mod tabs;
mod grid;
mod grid_areas;
mod panel;
mod separator;
mod statusdot_badge;
mod tag;
mod toast;
mod effect_controls;
mod border_controls;
mod font_controls;
mod size_controls;
mod items;
mod icon;
mod keyboard_icons;
mod icon_glow;
mod button_overflow;
mod icon_buttons;
mod tooltips;
mod tooltip_options;
mod dialog;
mod pane_frames;
mod pane_headers;
mod scrollregion;
mod scroll_axes;
mod scrollbars;
mod chrome;

struct DemoCx<'a> {
    theme: &'a Theme,
    ctl: ThemeCtl,
    sidebar_mode: Signal<RegionMode>,
    rail_hints: &'a [Signal<Option<String>>],
    rail_states: Rc<RefCell<Vec<Signal<bool>>>>,
    attention_req: Signal<bool>,
    dialog_open: Signal<bool>,
    open_it: Signal<bool>,
    radius_opts: [f32; 4],
    border_opts: [f32; 4],
    font_opts: [f32; 6],
    glow_idx: usize,
    radius_idx: usize,
    border_idx: usize,
    font_idx: usize,
    intensity_idx: usize,
    overlay_frame_idx: usize,
    size_idx: usize,
    zoom_idx: usize,
    workspaces: Vec<String>,
}

pub(super) fn build_ui(theme: &Theme, ctl: ThemeCtl) -> BuiltUi {
    // Host-owned sidebar display mode (G5): the keymap toggles it (`[`), the
    // region sizes to it, and its rail-aware Docks fold to icons when collapsed.
    let sidebar_mode = signal(RegionMode::Expanded);
    // Workspaces rail (enumerate flavor): one icon cell per pane, with a generic
    // KeyHint keycap that lights up on a move/swap/select pick (`p` in the demo).
    let rail_letters = vec!['a', 'b', 'c', 'd', 'e'];
    let rail_hints: Vec<Signal<Option<String>>> =
        rail_letters.iter().map(|_| signal(None)).collect();
    let rail_states: Rc<RefCell<Vec<Signal<bool>>>> = Rc::new(RefCell::new(Vec::new()));
    // Host-owned "needs attention" request for a pane (`n` fires it): the row
    // flashes and the host plays its own beep — grid-ui stays audio-free.
    let attention_req = signal(false);
    // Command palette (Ctrl+K): a fuzzy launcher. Type to filter (smart-case),
    // ↑/↓ or Ctrl+J/K to move, Enter to run, Esc to close. Commands just print.
    let palette = CommandPalette::new()
        .placeholder("Type a command…   (↑/↓ · Ctrl+J/K · Enter)")
        .command(
            Command::new("Split pane right", || println!("[showcase] split right"))
                .description("New column to the right of the active pane.")
                .icon(Glyph::Sidebar)
                .keys([
                    KeyCap::Text(PREFIX_SYMBOL.into()),
                    KeyCap::Nf(NfGlyph::Enter),
                ]),
        )
        .command(
            Command::new("Close pane", || println!("[showcase] close pane"))
                .description("Close the focused pane.")
                .icon(Glyph::Close)
                .keys([KeyCap::Text(PREFIX_SYMBOL.into()), KeyCap::Text("x".into())]),
        )
        .command(
            Command::new("Toggle sidebar", || println!("[showcase] toggle sidebar"))
                .description("Show or hide the sidebar region.")
                .icon(Glyph::Sidebar)
                .keys([
                    KeyCap::Text(PREFIX_SYMBOL.into()),
                    KeyCap::Nf(NfGlyph::Shift),
                    KeyCap::Text("b".into()),
                ])
                .keys([KeyCap::Text(PREFIX_SYMBOL.into()), KeyCap::Text("b".into())]),
        )
        .command(
            Command::new("New terminal", || println!("[showcase] new terminal"))
                .description("Spawn a shell in a new pane.")
                .icon(Glyph::Terminal),
        )
        .command(
            Command::new("Search files", || println!("[showcase] search files"))
                .description("Fuzzy-find a file in the project.")
                .icon(Glyph::Search)
                .keys([KeyCap::Nf(NfGlyph::Control), KeyCap::Text("p".into())]),
        )
        .command(
            Command::new("Git: commit", || println!("[showcase] git commit"))
                .description("Commit the staged changes.")
                .icon(Glyph::GitCommit),
        )
        .command(
            Command::new("Settings", || println!("[showcase] settings"))
                .description("Open the configuration file.")
                .icon(Glyph::Gear)
                .keys([KeyCap::Nf(NfGlyph::Command), KeyCap::Text(",".into())]),
        );
    let palette_open = palette.open_signal();

    // Right-click context menu: the pointer counterpart to the keyboard picks.
    // Right-click anywhere to open it at the cursor. Entries carry an icon and a
    // quick-pick keycap (press the letter to run); ↑/↓ + Enter and click also work.
    // The keycap is the shared `paint_keycap` primitive in its `Bordered` variant —
    // the same chip the KeyHint overlays draw `Filled` — so the menu never hand-draws it.
    // Both row forms are here on purpose: the first four are the **sugar** form (a label and an
    // icon), and "Close" is **composed** out of widgets — the two are laid out by the same engine
    // and read as the same list, which is the thing to check on screen.
    let menu = ContextMenu::new("showcase.pane")
        .child(
            Menu::new("Pane", "What you can do with this pane")
                .child(
                    MenuItem::new()
                        .label("Rename")
                        .icon(Glyph::NotePencil)
                        .on_click(|| println!("[showcase] rename"))
                        .key('r')
                        .shortcut(display_shortcut("prefix+$")),
                )
                .child(
                    MenuItem::new()
                        .label("Move to workspace")
                        .icon(Glyph::ArrowRight)
                        .on_click(|| println!("[showcase] → workspace"))
                        .key('w'),
                )
                .child(
                    MenuItem::new()
                        .label("Move to column")
                        .icon(Glyph::SquareSplitVertical)
                        .on_click(|| println!("[showcase] → column"))
                        .key('c'),
                )
                .child(
                    MenuItem::new()
                        .label("Duplicate")
                        .icon(Glyph::Cards)
                        .on_click(|| println!("[showcase] duplicate"))
                        .key('d')
                        .enabled(false),
                )
                .child(
                    MenuItem::new()
                        .child(|| {
                            Flex::row()
                                .gap(10.0)
                                .align("center")
                                .child(Icon::new(Glyph::FolderSimpleMinus))
                                .child(Label::new("Close"))
                                .child(Badge::new("⌫"))
                        })
                        .on_click(|| println!("[showcase] close"))
                        .key('x')
                        .danger(true)
                        .shortcut(display_shortcut("prefix+x")),
                ),
        )
        // Fired only on Esc / outside-click (a dismissal, not a selection) — the host wires this
        // to its overlay-close path (in `heca`, emit `CloseOverlay`).
        .on_dismiss(|| println!("[showcase] menu dismissed"));
    let menu_open = menu.open_signal();
    let menu_anchor = menu.anchor_signal();
    // Initial positions for the control selects, read from the current control
    // values — so the selects stay in sync if the tree is rebuilt (on font change).
    let radius_opts = [0.0f32, 4.0, 8.0, 16.0];
    let border_opts = [0.0f32, 1.0, 2.0, 3.0];
    let font_opts = [8.0f32, 10.0, 12.0, 15.0, 20.0, 28.0];
    let near = |arr: &[f32], v: f32, dflt: usize| {
        arr.iter()
            .position(|x| (x - v).abs() < 0.01)
            .unwrap_or(dflt)
    };
    let glow_idx = GlowLevel::ALL
        .iter()
        .position(|g| *g == ctl.glow.get_untracked())
        .unwrap_or(2);
    let radius_idx = near(&radius_opts, ctl.radius.get_untracked(), 2);
    let border_idx = near(&border_opts, ctl.border.get_untracked(), 1);
    let font_idx = near(&font_opts, ctl.font.get_untracked(), 3);
    let intensity_idx = INTENSITY_OPTS
        .iter()
        .position(|x| *x == ctl.intensity.get_untracked())
        .unwrap_or(2);
    let overlay_frame_idx = OVERLAY_FRAME_OPTS
        .iter()
        .position(|x| *x == ctl.overlay_frame.get_untracked())
        .unwrap_or(0);
    let size_idx = SIZE_OPTS
        .iter()
        .position(|x| *x == ctl.size.get_untracked())
        .unwrap_or(0);
    let zoom_idx = (ctl.zoom.get_untracked().round() as usize).min(ZOOM_MAX as usize);

    let card = |title: &str, value: &str| {
        Card::new(title)
            .width(220.0)
            .height(140.0)
            .background(theme.colors.surface)
            .border(theme.colors.border, theme.colors.border_width)
            .glow(theme.colors.glow)
            .child(
                Label::new(value)
                    .color(theme.colors.foreground)
                    .font_scale(2.0),
            )
    };

    // A toggle sitting to the left of its (state-colored) label.

    let report = |a: Action| println!("[showcase] {} -> {:?}", a.name, a.data);
    // A small accent title naming the widget(s) a demo row shows, so each widget in the
    // showcase is quickly recognizable at a glance.
    let caption = |name: &str| {
        Label::new(name)
            .color(theme.colors.accent)
            .font_scale(0.72)
            .bold(true)
    };

    // 20-entry list so the dropdown caps its height and shows a scrollbar.
    let workspaces: Vec<String> = (1..=20).map(|n| format!("WORKSPACE {n:02}")).collect();

    // ToastStack (G-overlay): the app owns the render list (`toasts`) and the
    // lifecycle; the stack just corner-anchors + animates them and reports
    // intents. `t` pushes one (see the keymap); clicking × removes it here.
    // Start empty — press `t` to push toasts (keeps them out of the dropdown's
    // corner by default; the overlapping-overlays text-bleed is a separate
    // renderer limitation to fix later).
    let toasts = signal(Vec::<ToastSpec>::new());
    let toast_stack = ToastStack::new(toasts)
        .position(ToastPosition::TopRight)
        .on_dismiss(move |id| toasts.update(|v| v.retain(|s| s.id != id)))
        .on_action(|id, key| println!("[showcase] toast {id} action {key}"));

    // An **animated surface**: the same `Overlay` every layer is, declaring how
    // it arrives and leaves in one builder — which is the whole capability. Opening zooms it in
    // and dissolving rides the shrink on the way out, the exposé's own gesture; the surface stays
    // on screen, inert, until that gesture has played out. Click the scrim to dismiss it.
    //
    // Drive it to see the three things tests cannot check: that the movement reads as one gesture,
    // that nothing flashes at the end of the exit, and that the surface is gone the frame it
    // finishes rather than lingering faintly.
    let animated = Overlay::new()
        .animation(Animation::ZoomFade)
        .panel_size(Length::Px(420.0), Length::Px(220.0));
    let animated_open = animated.open_signal();
    let (open_it, dismiss_it) = (animated_open, animated_open);
    let animated = animated
        .panel(
            Flex::column()
                .gap(10.0)
                .padding(20.0)
                .child(Label::new("ANIMATED SURFACE"))
                .child(Label::new(
                    "Zoom in; on the way out the dissolve rides the shrink.",
                ))
                .child(Button::secondary("CLOSE").on_click(move || dismiss_it.set(false))),
        )
        .on_outside_click(move || dismiss_it.set(false));

    // Dialog: the widget is hosted in the OVERLAY layer above the
    // page scroll — never in-flow inside scrolled content, where taffy would center
    // it in its slot and put it off-screen once the page scrolls. The page keeps
    // only the trigger button; both sides share `dialog_open`.
    let dialog = Dialog::new("Delete pane?");
    let dialog_open = dialog.open_signal();
    let (cancel_open, delete_open, dismiss_open) = (dialog_open, dialog_open, dialog_open);
    let dialog = dialog
        // A RICH body — not just a message, but a Column holding a warning Label + real
        // form fields (Input + Select + Checkbox), proving the body is arbitrary composed
        // content. In `heca` any named value node in a `ViewNode` body
        // (Input/Toggle/Checkbox/Select) marshals its value into `ModalResult::Action.data`
        // (see docs/widgets.md §Dialog). The Select is an OVERLAY-IN-OVERLAY:
        // its open list must composite ABOVE the dialog's action buttons (a deeper scene
        // segment) and capture hover/wheel/keys over them — not show through or fall through.
        // SIZED panel + SCROLLABLE body: `panel_size` bounds the panel, which is what
        // lets the `ScrollRegion` around the body actually overflow and scroll. The
        // title and the action row stay put — only the body scrolls, because only the
        // body is inside the region. The nested Select above still has to composite
        // ABOVE the buttons even while the body is scrolled.
        // SIZED panel: `panel_size` bounds the panel so the scrollable list below has
        // something to overflow. A `Percent` tracks the window; `Px` would pin it.
        // A bounded panel (fixed px — a dialog is not viewport-proportional) so the
        // scrollable body has something to overflow. Note what the caller does NOT
        // have to say: the body fills the panel width and takes the leftover height
        // on its own, so the title and the action row stay put while it scrolls.
        .panel_size(Length::Px(560.0), Length::Px(420.0))
        .body(
            ScrollRegion::new()
                .child(Label::new("This action cannot be undone."))
                .child(caption("Confirm name"))
                .child(Input::new().value("pane-1").on_change(report))
                .child(caption("Archive target"))
                .child(
                    Select::new(["SCRATCHPAD", "WORKSPACE 01", "TRASH"])
                        .selected(2)
                        .on_change(report),
                )
                .child(
                    Checkbox::new()
                        .label("Also close its column")
                        .on_change(report),
                )
                .child(caption("Affected panes"))
                .child(Label::new("· pane-1  (zsh)"))
                .child(Label::new("· pane-2  (nvim)"))
                .child(Label::new("· pane-3  (cargo watch)"))
                .child(Label::new("· pane-4  (lazygit)"))
                .child(Label::new("· pane-5  (htop)"))
                .child(Label::new("· pane-6  (docker logs)"))
                .child(Label::new("· pane-7  (tail -f)"))
                .child(Label::new("· pane-8  (psql)"))
                .child(Label::new("· pane-9  (redis-cli)"))
                .child(Label::new("· pane-10 (k9s)")),
        )
        .action(Button::secondary("Cancel").on_click(move || {
            println!("[showcase] dialog cancelled");
            cancel_open.set(false);
        }))
        .action(Button::destructive("Delete").on_click(move || {
            println!("[showcase] dialog: pane deleted");
            delete_open.set(false);
        }))
        .on_dismiss(move || dismiss_open.set(false));

    let cx = DemoCx {
        theme,
        ctl,
        sidebar_mode,
        rail_hints: &rail_hints,
        rail_states: rail_states.clone(),
        attention_req,
        dialog_open,
        open_it,
        radius_opts,
        border_opts,
        font_opts,
        glow_idx,
        radius_idx,
        border_idx,
        font_idx,
        intensity_idx,
        overlay_frame_idx,
        size_idx,
        zoom_idx,
        workspaces: workspaces.clone(),
    };
    let page = Flex::column()
        .min_width("100%")
        .padding(40.0)
        .gap(28.0)
        .align("center")
        .child(caption("Card"))
        .child(
            Flex::row()
                .gap(28.0)
                .child(card("UPLINK", "ONLINE"))
                .child(card("POWER", "98%"))
                .child(card("GRID NODES", "1024")),
        )
        // Dropdowns near the top: open downward and must overlap the rows below.
        .child(caption("Select"))
        .child(select::build(&cx))
        // One button per GridCN variant.
        .child(caption("Button"))
        .child(button::build(&cx))
        // Composed content: a button's content is CHILD COMPONENTS, not text it draws. Hover /
        // disable these — the icon and labels inherit the button's state color (they carry no
        // color of their own), and the size variant cascades into them.
        .child(caption("Button — composed content (icon, tree, size cascade)"))
        .child(composed_buttons::build(&cx))
        // `Choice` — the option primitive behind Select/Tabs: a VALUE plus composed content.
        // The selected one tints its whole content (icon + label) via the inherited state color.
        .child(caption("Choice — a value + composed content (the option primitive)"))
        .child(choices::build(&cx))
        // Change widgets: Toggles across their states (on / off / disabled).
        .child(caption("Toggle"))
        .child(toggle::build(&cx))
        // Checkboxes: integrated labels (clickable), with one label on the left.
        .child(caption("Checkbox"))
        .child(checkbox::build(&cx))
        // Label: the four text attributes. `bold`/`italic` are FONT attributes (the shaper picks the
        // glyphs — italic is a synthesized oblique, since the embedded family has no italic face);
        // `underline`/`strikethrough` are DECORATIONS the label draws itself, in its own resolved
        // color. They compose freely.
        .child(caption("Label"))
        .child(label::build(&cx))
        // Text inputs: empty-with-placeholder, pre-filled, and disabled.
        .child(caption("Input"))
        .child(
            Flex::row()
                .gap(20.0)
                .align("center")
                .child(Input::new().placeholder("CALLSIGN").on_change(report))
                .child(Input::new().value("GRID-7").on_change(report))
                .child(Input::new().value("LOCKED").disabled(true)),
        )
        // Tabs: segmented selector with a sliding underline.
        .child(caption("Tabs"))
        .child(Tabs::new(["OVERVIEW", "SIGNALS", "LOGS"]).on_change(report))
        // Composed tabs: a tab is a value plus any content — the underline spans whatever it is.
        .child(tabs::build(&cx))
        // Grid: a layout-only CSS-Grid container — tracks + named areas, any child placed into one.
        // The classic rich row: [icon | title | status] over [· | subtext | ·].
        .child(caption("Grid"))
        .child(grid::build(&cx))
        .child(grid_areas::build(&cx))
        // Panel: a titled section container. Quiet by default (no frame of its own), which is what
        // distinguishes it from a Card — a Card stands apart, a Panel is a slice of a region. The
        // second one is untitled, to show the header taking no room rather than leaving a blank
        // line. Both are what `WidgetKind::Panel` realizes to now that Panel is a real widget.
        .child(caption("Panel"))
        .child(panel::build(&cx))
        .child(caption("Separator"))
        .child(Separator::horizontal().length(440.0))
        // Both orientations, so the vertical rule is visible too — it is the one a described
        // separator reaches through `orientation`, and it had no showcase entry before.
        // This row CENTRES its children, like most rows here: the full-height rule spans anyway,
        // because the separator asks to be stretched rather than waiting to be.
        .child(separator::build(&cx))
        // Display widgets: status dot + badges across variants.
        .child(caption("StatusDot · Badge"))
        .child(statusdot_badge::build(&cx))
        // Tags (G8): metadata chips that can carry a leading icon and multiple
        // segments (a status-bar pill). Quieter than a Badge; hue configurable.
        .child(caption("Tag"))
        .child(tag::build(&cx))
        // Spinner + Alert.
        .child(caption("Spinner · Alert"))
        .child(
            Flex::row()
                .gap(20.0)
                .align("center")
                .child(Spinner::new())
                .child(Alert::warning("LINK UNSTABLE").body("retrying handshake...")),
        )
        // Toasts: bracket-framed notification cards. Severity-toned, with a body
        // line, an optional inline action, and a × dismiss. Presentation only —
        // here the example plays the "host" (its callbacks just print); a real app
        // owns the queue + lifecycle. Stacked like a notification list (also the
        // shape they take inline in a sidebar).
        .child(caption("Toast"))
        .child(toast::build(&cx))
        // Value displays: progress bar + energy gauge.
        .child(caption("ProgressBar · Gauge"))
        .child(
            Flex::row()
                .gap(24.0)
                .align("center")
                .child(Label::new("POWER").color(theme.colors.muted).font_scale(0.85))
                .child(ProgressBar::new().value(0.72))
                .child(Gauge::new().value(0.85)),
        )
        // Dropdown (overlay layer): opens over the content below it.
        .child(caption("Select — live theme / appearance controls"))
        .child(effect_controls::build(&cx))
        // Live theme controls — glow size, corner radius, border width. Each
        // Select writes a signal that the next frame folds into `self.theme`.
        .child(border_controls::build(&cx))
        // Font-size control + widgets that re-measure from it: the Input, Select
        // and Label below are built at `theme.font_size`, so changing it rebuilds
        // them at a different size — you can see their boxes grow/shrink with text.
        .child(font_controls::build(&cx))
        // Widget size variant — applied to the WHOLE tree so the showcase reflects
        // Small / Normal / Large globally (font + padding scale together) — and a
        // global ZOOM (0–5, centered on 2) that scales the entire UI uniformly.
        .child(size_controls::build(&cx))
        // Item rows: a single-select menu panel. Clicking a row highlights it and
        // clears the others. Immediate-mode (no reactive effects): each row's
        // `active` signal is set directly on click via a shared list.
        .child(caption("Item · Pane"))
        .child(items::build(&cx))
        // Duotone icons (G2): the FULL icon set, iterated from `Glyph::ALL` (the single
        // source) so every glyph — including newly added ones — appears here with no
        // hand-maintained list. Each is two stacked layers: a dimmed secondary wash + a
        // full primary on top, same hue (secondary = primary at the theme's
        // icon_secondary_alpha). Wraps across rows so the whole set stays visible.
        .child(caption("Icon"))
        .child(icon::build(&cx))
        // Nerd Font keyboard glyphs: the SECOND icon set, iterated
        // from `NfGlyph::ALL`. Phosphor above has no keyboard glyphs at all and the UI face has no
        // ⌃ ⌥ ⌘ ⎋, so a keycap draws from here.
        //
        // **This gallery is the confirmation surface.** A test proves each codepoint is IN the
        // embedded font (`heca-renderer/tests/font_coverage.rs`); nothing can prove a codepoint
        // draws the key it is named after, because the font names its glyphs `uniF0636`. The five
        // apple_keyboard ones are read off the Material block's alphabetical order — look at them
        // here, and if one is off by one it is a one-character fix in `NfGlyph::codepoint`.
        .child(caption("NfIcon — Nerd Font keyboard set (confirm each name matches its picture)"))
        .child(keyboard_icons::build(&cx))
        // Glyph halo: a bare icon can carry the same resting glow every bordered
        // SURFACE has. Flat vs `.glow(true)`, side by side, so the difference is
        // visible without hunting. It goes through the one `scaled_glow` chokepoint,
        // so the GLOW SIZE select above scales both — and `none` removes the halo
        // here exactly as it removes every other glow in the UI.
        .child(caption("Icon — glyph halo (glow_size scales it; none removes it)"))
        .child(icon_glow::build(&cx))
        // ButtonGroup: a row of actions that fits the space it is given. Drag the window narrow
        // and watch it give things up in order — first the words (the buttons become their icons,
        // and the words move to the hover bubble), then the buttons that still do not fit, into the
        // menu behind the trailing ⋮. Nothing is ever squashed, which is what a plain Flex of
        // buttons does instead.
        .child(caption("ButtonGroup · fits the space it is given"))
        .child(button_overflow::build(&cx))
        // A group pinned to icons, for a strip that never wants words. The labels are still
        // carried — they are what the hover bubble and the collapsed menu say.
        .child(caption("ButtonGroup · Display::IconOnly"))
        .child(icon_buttons::build(&cx))
        // IconButton + Tooltip: a toolbar of compact, clickable icon affordances —
        // ghost at rest, tinted hover frame + press flash + focus ring — each
        // wrapped in a hover-revealed Tooltip label. The danger one uses `.accent()`.
        .child(caption("IconButton · Tooltip"))
        .child(tooltips::build(&cx))
        // The four-sided placement authority (`place_beside`), one button per side.
        // Hover each: the bubble centers on that side of its target and FLIPS to the
        // opposite side when the viewport edge leaves no room — scroll this row to the
        // top/bottom of the window to see Top/Bottom flip. The bubbles paint through
        // the shared `paint_panel_chrome`, so the OVERLAY FRAME select at the top of
        // the page restyles them exactly as it restyles the dialog and the dropdown.
        .child(tooltip_options::build(&cx))
        // Dialog: a centered confirm panel over a scrim that holds REAL child components
        // (the body + real action `Button`s), so its buttons are hint targets +
        // focus-traversed as components (a hand-drawn panel could do neither). The
        // destructive button opens it. Tab / ←→ move focus, Enter/Space activate. The
        // buttons close it via the open signal; Esc / scrim fire `on_dismiss` (here it
        // closes the demo; in `heca` the host points it at a `CloseOverlay` action).
        // Renders nothing until opened. The Dialog WIDGET lives in the overlay layer
        // (see `dialog` above the page) — only its trigger sits in the scrolled page.
        .child(caption("Dialog"))
        .child(dialog::build(&cx))
        // Pane frame variants: three Panes side-by-side showing None, Bordered,
        // and Bracketed modes. Each has a background + border so the decoration
        // is visible. Click and keyboard interactivity via the `.on_activate`
        // on each child Item.
        .child(caption("Pane — frame variants"))
        .child(pane_frames::build(&cx))
        // In-pane info bar: a `Tag` chip composed *inside* the pane top (the app's
        // pane-info header). The `Pane` carries no built-in title — the bar is a
        // separate widget, so frame decoration and header stay independent. Left =
        // metadata segments (location · app · branch · diff-stat); the app adds the
        // action-button cluster on the right.
        .child(caption("Pane — in-pane info bar"))
        .child(pane_headers::build(&cx))
        // ScrollRegion (gridui-01): an embeddable vertical scroll viewport —
        // 25 rows in a 180px window → auto-scrollbar + wheel/drag. Pointer
        // coords are translated into content space so the rows stay clickable at
        // their visual position while scrolled. (The chrome sidebar itself is
        // not yet wired to it — that needs scroll-aware DnD hit-testing, a
        // follow-up.)
        .child(caption("ScrollRegion"))
        .child(scrollregion::build(&cx))
        // ScrollRegion two-axis + scrollable surface:
        // .both() enables horizontal + vertical scrolling; .background()/.border()
        // (StyleExt) make it a framed scrollable *surface*. Each row is explicitly
        // wider than the 240px viewport (→ horizontal overflow + bottom scrollbar),
        // and there are enough rows to overflow the 140px height (→ vertical
        // scrollbar). Plain wheel scrolls vertically, Shift+wheel horizontally.
        .child(caption("ScrollRegion — two-axis + scrollable surface"))
        .child(scroll_axes::build(&cx))
        // ScrollBar + BadgeButton: the standalone scrollback affordances used by
        // terminal panes — a draggable vertical thumb plus a clickable
        // "N lines above" chip. Generic widgets; the app wires them to terminal
        // viewport state.
        .child(caption("ScrollBar · BadgeButton"))
        .child(scrollbars::build(&cx))
        // Chrome vocabulary (G1 Grid · G3 ItemGroup · G4 DockFrame · G5
        // ChromeRegion): a sidebar region hosting two DockFrames of grouped rows,
        // beside a PANES dock of composed, state-colored cards. Headers and the
        // file rows respond to click and keyboard (Tab to focus, Enter to activate).
        .child(caption(
            "Chrome — ChromeRegion · DockFrame · ItemGroup · MarkerGroup · KeyHint · RailCell · Row",
        ))
        .child(chrome::build(&cx))
        .child(caption(
            "Described tree — left is data (heca-view), right is the same widgets by hand",
        ))
        .child(described_vs_native(theme))
        // The command palette overlays everything when open (Ctrl+K) — the widget
        // itself lives in the overlay layer; this caption just documents the key.
        .child(caption("CommandPalette — press Ctrl+K"));

    // The text demo uses the window width. Oversized catalog recipes have their own
    // horizontal viewport, so they cannot enlarge a wrapping editor or message list.
    let ui = ScrollRegion::new()
        .child(images::build(theme).padding("lg"))
        .child(connections_and_text::build(theme).padding("lg"))
        .child(Flex::column().width("100%")
            .child(ScrollRegion::new().horizontal().align("start").width("100%").child(page)));

    // The overlay layer: viewport-anchored widgets composited ABOVE the scrolled
    // page — a Dialog taffy-centers on the real window (BUG B), the palette/menu
    // anchor to the viewport, toasts corner-anchor. Input is offered to this tree
    // first; everything here paints after (= over) the page.
    let overlays = Flex::column()
        .child(dialog)
        .child(animated)
        // The right-click context menu overlays at the cursor when open.
        .child(menu)
        // The toast stack overlays a corner (presentation only; app owns the list).
        .child(toast_stack)
        .child(palette);
    BuiltUi {
        ui,
        overlays,
        sidebar_mode,
        rail_hints,
        rail_states: rail_states.borrow().clone(),
        rail_letters,
        attention_req,
        palette_open,
        menu_open,
        menu_anchor,
        toasts,
    }
}
