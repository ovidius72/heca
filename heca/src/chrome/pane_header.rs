//! **The in-pane header** — its info bar, its action buttons, and the retained trees behind them
//! (F003/P082/T427 split).
//!
//! It owns what a pane says about itself: the composed name and program icon, the git and cwd
//! segments, the action buttons and their shortcuts, and the per-pane retained widgets the host
//! keeps in step with the session. It owns **nothing** about the chrome around it — the regions,
//! the layer stack and the letters all live beside it, not here.
//!
//! Split out of a 5236-line `chrome/mod.rs` that held every kind of logic at once.

use super::*;
use heca_grid_ui::builders::StyleExt as _;
use heca_grid_ui::widgets::{Button, ButtonGroup, ButtonVariant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PaneInfoView {
    pub(crate) icon: Glyph,
    /// Custom name if set, else the program name — the sidebar card's label.
    pub(crate) title: String,
    /// The program/application name (always the process, never the rename) — the info-bar
    /// `AppName` segment shows this, so renaming a pane doesn't hide what's running in it.
    pub(crate) app_name: String,
    /// The process/program name shown as a small dimmed label *next to* a custom name (e.g.
    /// `(nvim)`). `Some` only when the pane has a custom name and `pane_renamed_add_process_name`
    /// is on — a pane that merely tracks its process has the process name *as* its title already.
    pub(crate) process_hint: Option<String>,
    pub(crate) status: ProcessStatus,
    pub(crate) git_branch: Option<String>,
    pub(crate) git_added: Option<String>,
    pub(crate) git_modified: Option<String>,
    pub(crate) git_deleted: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaneInfoSignals {
    pub(crate) icon: Signal<Glyph>,
    /// The pane's name. **One signal, because there is one name** — its colour follows the
    /// row's selected state through the inherited content colour, so no second copy exists to
    /// keep in step (F003/P082/T480).
    pub(crate) title: Signal<String>,
    /// The dimmed `(process)` suffix text beside a renamed pane's name (e.g. `(nvim)`),
    /// or empty when hidden. Signal-driven so a rename toggles it live without a tree
    /// rebuild (renames update signals, they don't rebuild the sidebar card).
    pub(crate) process_hint: Signal<String>,
    pub(crate) process_hint_visible: Signal<bool>,
    /// The pane's working-directory row text (home-relative path), and its visibility
    /// (`[settings] pane_show_cwd` and the pane has a cwd). Signal-driven so a `cd` in the
    /// pane updates the path live, mirroring the git-branch row.
    pub(crate) cwd: Signal<String>,
    pub(crate) cwd_visible: Signal<bool>,
    /// What the row's status pip shows. **One signal, not one dot per state**: the row keeps a
    /// single `StatusDot` and rewrites what it is, instead of building four and toggling four
    /// booleans to reveal one (F003/P096/T483).
    pub(crate) status: Signal<heca_grid_ui::DotStatus>,
    pub(crate) git_visible: Signal<bool>,
    pub(crate) git_branch: Signal<String>,
    pub(crate) git_branch_display: Signal<String>,
    pub(crate) git_added_visible: Signal<bool>,
    pub(crate) git_added: Signal<String>,
    pub(crate) git_modified_visible: Signal<bool>,
    pub(crate) git_modified: Signal<String>,
    pub(crate) git_deleted_visible: Signal<bool>,
    pub(crate) git_deleted: Signal<String>,
}

fn program_glyph(icon: ProgramIcon) -> Glyph {
    match icon {
        ProgramIcon::Terminal => Glyph::Terminal,
        ProgramIcon::FileCode => Glyph::FileCode,
        ProgramIcon::Folder => Glyph::Folder,
        ProgramIcon::FolderOpen => Glyph::FolderOpen,
        ProgramIcon::GitBranch => Glyph::GitBranch,
        ProgramIcon::Gear => Glyph::Gear,
        ProgramIcon::Search => Glyph::Search,
    }
}

pub(crate) fn pane_info_view(
    programs: &ProgramsConfig,
    fallback_name: &str,
    custom_name: Option<&str>,
    runtime: Option<&PaneRuntime>,
    add_process_name: bool,
) -> PaneInfoView {
    let raw = runtime
        .and_then(|pane| pane.program.as_deref())
        .filter(|raw| !raw.is_empty())
        .unwrap_or(fallback_name);
    let program = programs.resolve(raw);
    let git = runtime.and_then(|pane| pane.git.as_ref());
    let program_name = program.name.to_string();
    // A user-set custom name wins over the process-derived program name; the icon still tracks
    // the running program. When the pane has a custom name (and the setting is on), the program
    // name is surfaced separately as `process_hint` (a small dimmed label next to the name).
    let has_custom = custom_name.is_some_and(|name| !name.is_empty());
    let title = if has_custom {
        custom_name.unwrap_or_default().to_string()
    } else {
        program_name.clone()
    };
    let process_hint = (has_custom && add_process_name).then(|| program_name.clone());
    PaneInfoView {
        icon: program_glyph(program.icon),
        title,
        app_name: program_name,
        process_hint,
        status: runtime
            .map(|pane| pane.status.clone())
            .unwrap_or(ProcessStatus::Idle),
        git_branch: git.map(|info| {
            info.branch
                .clone()
                .unwrap_or_else(|| "detached".to_string())
        }),
        git_added: git.and_then(|info| (info.added > 0).then(|| format!("+{}", info.added))),
        git_modified: git
            .and_then(|info| (info.modified > 0).then(|| format!("~{}", info.modified))),
        git_deleted: git.and_then(|info| (info.deleted > 0).then(|| format!("-{}", info.deleted))),
    }
}

/// Left-truncate `text` to `max_chars`, keeping the **tail** with a leading
/// ellipsis (`…/HypeSupport`) — paths read most usefully from the end.
pub(crate) fn truncate_path_left(text: &str, max_chars: usize) -> String {
    let len = text.chars().count();
    if len <= max_chars {
        return text.to_string();
    }
    match max_chars {
        0 => String::new(),
        1 => "…".to_string(),
        n => {
            let tail: String = text.chars().skip(len - (n - 1)).collect();
            format!("…{tail}")
        }
    }
}

/// A path shown home-relative (`/Users/x/proj` → `~/proj`).
pub(crate) fn home_relative_path(path: &std::path::Path) -> String {
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::Path::new(&home);
        if let Ok(rest) = path.strip_prefix(home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

/// Build the pane info bar's segmented [`Tag`] from the configured `segments` and
/// the pane's runtime, reusing the same catalog projection as the sidebar card.
/// **What each segment has to say right now** — the one derivation, read three times.
///
/// The builder turns these into a tag, the header's identity reads only *which kinds* came back
/// (that is its shape), and the per-frame update writes the texts into the tree that is already on
/// screen. Three readers, one answer, so a segment cannot be built from one thing and rewritten
/// from another.
pub(crate) fn segment_items(
    programs: &ProgramsConfig,
    fallback_name: &str,
    custom_name: Option<&str>,
    runtime: Option<&PaneRuntime>,
    segments: &[heca_config::appearance::PaneSegment],
    max_width: f32,
    font: f32,
) -> Vec<(heca_config::appearance::PaneSegment, Glyph, String)> {
    use heca_config::appearance::PaneSegment;

    // The info bar never renders the `(process)` suffix — that's a sidebar-card affordance
    // (see `pane_card`) — so it always projects the view without the hint.
    let view = pane_info_view(programs, fallback_name, custom_name, runtime, false);
    let git = runtime.and_then(|pane| pane.git.as_ref());

    // Collect the produced (icon, text) segments, noting the location (the long,
    // truncatable one), so we can fit the bar to `max_width` before building it.
    let mut items: Vec<(PaneSegment, Glyph, String)> = Vec::new();
    let mut location_idx: Option<usize> = None;
    for seg in segments {
        let item = match seg {
            PaneSegment::Location => match runtime.and_then(|pane| pane.cwd.as_ref()) {
                Some(cwd) => {
                    location_idx = Some(items.len());
                    (Glyph::Folder, home_relative_path(cwd))
                }
                None => continue,
            },
            PaneSegment::AppName => (view.icon, view.app_name.clone()),
            PaneSegment::PaneName => {
                if view.title.is_empty() {
                    continue;
                }
                (view.icon, view.title.clone())
            }
            PaneSegment::GitBranch => match git.and_then(|info| info.branch.clone()) {
                Some(branch) => (Glyph::GitBranch, branch),
                None => continue,
            },
            PaneSegment::GitStatus => {
                let Some(info) = git else { continue };
                let mut parts = Vec::new();
                if info.added > 0 {
                    parts.push(format!("+{}", info.added));
                }
                if info.modified > 0 {
                    parts.push(format!("~{}", info.modified));
                }
                if info.deleted > 0 {
                    parts.push(format!("-{}", info.deleted));
                }
                if parts.is_empty() {
                    continue;
                }
                (Glyph::GitCommit, parts.join(" "))
            }
        };
        items.push((*seg, item.0, item.1));
    }

    // Fit to width: if the bar would overflow the pane, shrink the location
    // segment (left-ellipsised) by the overflow. The per-pane render clip is the
    // hard backstop; this keeps it readable instead of a hard cut.
    let char_w = (font * 0.6).max(1.0);
    let per_segment_overhead = font * 2.5; // icon + gaps + segment padding + divider
    let text_chars: usize = items.iter().map(|(_, _, text)| text.chars().count()).sum();
    let estimated = text_chars as f32 * char_w + items.len() as f32 * per_segment_overhead;
    if estimated > max_width
        && let Some(idx) = location_idx
    {
        let overflow_chars = ((estimated - max_width) / char_w).ceil() as usize;
        let loc_chars = items[idx].2.chars().count();
        let keep = loc_chars.saturating_sub(overflow_chars).max(1);
        items[idx].2 = truncate_path_left(&items[idx].2, keep);
    }

    items
}

/// Segments with no data (e.g. git outside a repo) are skipped; returns `None`
/// when nothing is produced. Used by the terminal pane shell (`app::terminal_render`).
#[expect(
    clippy::too_many_arguments,
    reason = "pane-info projection threads program/name/runtime + layout context explicitly; grouping into a struct is a later chrome refactor"
)]
pub(crate) fn build_pane_info_bar(
    programs: &ProgramsConfig,
    fallback_name: &str,
    custom_name: Option<&str>,
    runtime: Option<&PaneRuntime>,
    segments: &[heca_config::appearance::PaneSegment],
    theme: &GuiTheme,
    max_width: f32,
    font: f32,
) -> Option<Tag> {
    let items = segment_items(programs, fallback_name, custom_name, runtime, segments, max_width, font);
    if items.is_empty() {
        return None;
    }

    let mut tag: Option<Tag> = None;
    for (seg, glyph, text) in items {
        // No explicit size → the icon inherits the bar's base font, so glyph and
        // label stay balanced when the bar font changes.
        let leading = Icon::new(glyph).color(theme.colors.foreground);
        // **Each segment is named by what it is**, so its words can be rewritten without the bar
        // being rebuilt (F003/P097/T500). The first segment is the tag's own label; the rest are
        // child labels. Both answer `set_text`.
        let key = segment_text_key(seg);
        tag = Some(match tag.take() {
            None => {
                use heca_grid_ui::builders::ComponentExt as _;
                Tag::new(text).leading(leading).key(key)
            }
            Some(existing) => existing.segment_text_keyed(text, Some(Box::new(leading)), Some(&key)),
        });
    }
    tag
}

/// **A freshly built header**: its tree, the identity that says when it must be rebuilt, and the
/// words to write into it this frame — which are deliberately not part of that identity.
pub(crate) type BuiltPaneHeader = (
    Surface,
    String,
    Vec<(heca_config::appearance::PaneSegment, String)>,
);

/// One per pane that has a header.
pub(crate) type BuiltPaneHeaders = std::collections::HashMap<PaneId, BuiltPaneHeader>;

/// **What a segment's words are called**, so they can be rewritten in place.
///
/// One derivation, read by the builder and by the update pass, so the two cannot address different
/// nodes. Keyed by the segment *kind* rather than its position: which segments have data changes
/// (a pane outside a repository has no branch), and a positional name would then follow whichever
/// segment happened to take that slot.
fn segment_text_key(seg: heca_config::appearance::PaneSegment) -> String {
    format!("hdr.seg:{seg:?}")
}

/// **Write the current words into a header that is already on screen** (F003/P097/T500).
///
/// The words a header shows — the foreground program, the working directory, the branch — are a
/// *per-frame input*, not part of what the header is. This is the same rule the pane's own rect
/// already follows: written onto the retained tree rather than built into it, so a change moves
/// nothing else.
///
/// It matters because a rebuild is visible. A freshly built widget has no layout node until the
/// walk reaches it, and nothing is painted before it has a box — so the frame after a rebuild
/// draws nothing where the bar was. While these words were part of the header's identity, running
/// one command rebuilt it twice (the command starting, and finishing), and the buttons blinked out
/// and back both times (Antonio, driving, 2026-09-05).
///
/// A segment that has *appeared or gone* is a different matter and does rebuild: that is a change
/// of shape, and it stays in the key.
pub(crate) fn refresh_pane_header_text(root: &dyn heca_grid_ui::Component, texts: &[(heca_config::appearance::PaneSegment, String)]) {
    for (seg, text) in texts {
        heca_grid_ui::set_text_by_key(root, &segment_text_key(*seg), text);
    }
}

// ── In-pane info-bar header: segments (left) + interactive action buttons (right) ──


/// Font multiplier for a sidebar card's **secondary metadata** — the dimmed `(process)`
/// suffix and the cwd row — smaller than the name so it reads as supporting detail.
pub(crate) const CARD_META_FONT_SCALE: f32 = 0.8;

/// Per-pane context the header buttons need to build their (parameterized) actions
/// and emit them through the app event loop.
pub(crate) struct PaneHeaderCtx {
    pub(crate) pane_id: PaneId,
    pub(crate) ws_idx: usize,
    pub(crate) col_idx: usize,
    pub(crate) event_proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
}

/// Retained per-pane terminal viewport widgets (scrollbar + scrolled-up badge).
/// Built once per pane, updated/repositioned every frame by
/// [`sync_pane_viewport_widgets`], painted read-only in `terminal_render`, and
/// dispatched pointer events in `events.rs`.
pub(crate) struct RetainedPaneViewportWidgets {
    pub(crate) scrollbar: ScrollBar,
    pub(crate) badge: BadgeButton,
}

/// Resolved display shortcuts for **every bound action**, keyed by config name
/// (`"close"`, `"sidebar_left"`, …). Built once at config load/reload from the
/// default+user keybindings and stored on `AppState`; any button looks its own
/// shortcut up by the name of the action it triggers, so the choice is never
/// hand-made by a caller or plugin. The leader renders through the central
/// [`PREFIX_SYMBOL`](crate::shortcut) seam, uniform across the app.
/// **The bindings, not a sentence** (F003/P085/T358). One entry per binding, in the index's order —
/// an action bound in two layers has two — and each stored as the **raw** config key
/// (`"prefix+Shift+e"`), spelled for display on read.
///
/// It used to store one pre-joined display string, which decided for every surface at once. That is
/// what left the command palette unable to draw keycaps: a surface that wants a line joins, a
/// surface that wants a chip per key cannot un-join — and the two want different spellings anyway
/// (a keycap draws a Nerd Font `⇧`, a tooltip has to write `Shift`).
#[derive(Clone, Default, PartialEq)]
pub(crate) struct ActionShortcuts {
    by_action: std::collections::HashMap<String, Vec<String>>,
    /// The same bindings kept **per layer** — `(layer label, action) → keys`.
    ///
    /// [`by_action`](Self#structfield.by_action) flattens every layer together, which is right for
    /// a tooltip (*"what key runs this?"*) and wrong for a surface asking *"what key runs this **in
    /// me**?"*. The exposé's cards need the second: their `x` is theirs, and a `close_pane_by_id`
    /// bound somewhere else is not the card's gesture (F003/P082/T416).
    by_layer: std::collections::HashMap<(String, String), Vec<String>>,
    /// How [`get`](ActionShortcuts::get) spells a binding for a **text** surface.
    style: crate::shortcut::KeyStyle,
}

impl ActionShortcuts {
    /// Resolve a display shortcut for every action the **built keymaps** bind (F003/P086/T366).
    ///
    /// Built from [`Keymaps::by_action`](crate::keymap::Keymaps::by_action) rather than from the
    /// config file, because the file is only part of the answer: reading `[keys]` alone missed every
    /// `[[keys.mode]]` binding, would miss every `[[keys.component]]` one, and could never see a key
    /// a plugin registered at runtime. A tooltip that says nothing for a key the user can actually
    /// press is the bug this closes.
    ///
    /// An action bound in several layers keeps all of them — a component's `j` and a mode's `j` are
    /// both real, and picking the first would be a guess.
    ///
    /// `style` is how [`get`](Self::get) spells a binding for a text surface — the optional
    /// spelling switch, chosen once here rather than at each of the call sites that render a tip.
    /// Keycaps are unaffected: they read [`chords`](Self::chords) and draw glyphs.
    pub(crate) fn from_index(
        index: &crate::keymap::BindingIndex,
        style: crate::shortcut::KeyStyle,
    ) -> Self {
        let by_action = index
            .iter()
            .filter_map(|(action, bound)| {
                // **One entry per distinct key.** A config can bind the same physical key twice
                // under two spellings — the shipped default did: `cursor_up = "k,Up,ArrowUp"`, and
                // `Up` and `ArrowUp` both parse to `GridKey::ArrowUp` — and the index keeps both,
                // correctly, because it records what was written. A surface showing them draws the
                // same keycap twice. Deduped here rather than in the config, because a user's own
                // file can do it too and their config is not ours to police.
                let mut keys: Vec<String> = Vec::new();
                let mut seen: Vec<Vec<heca_grid_ui::widgets::KeyCap>> = Vec::new();
                for b in bound {
                    // Compared as **caps**, not as text: the caps come from the one key table, which
                    // maps `up` and `arrowup` (and `enter`/`return`) to a single glyph — so two
                    // spellings of one key collapse in every display style, while two genuinely
                    // different keys stay two.
                    let caps = crate::shortcut::chord_caps(&b.key);
                    if !seen.contains(&caps) {
                        seen.push(caps);
                        keys.push(b.key.clone());
                    }
                }
                (!keys.is_empty()).then(|| (action.clone(), keys))
            })
            .collect();
        // The same index again, kept by the layer each binding was written in, so a surface can ask
        // for its own keys without a second reader of the config file.
        let mut by_layer: std::collections::HashMap<(String, String), Vec<String>> =
            std::collections::HashMap::new();
        for (action, bound) in index {
            for b in bound {
                by_layer
                    .entry((b.layer.clone(), action.clone()))
                    .or_default()
                    .push(b.key.clone());
            }
        }
        Self { by_action, by_layer, style }
    }

    /// **The keys `action_name` answers to in one surface's own layer** — empty when that surface
    /// binds it to nothing.
    ///
    /// `surface` is the surface's name (`heca.expose`, `workspaces`) and `action_name` is the
    /// **short name as written in its entry** — `delete_pane`, not `heca.expose.delete_pane`. Both
    /// halves of the stored key are rebuilt from the functions that wrote it: the layer label the
    /// way [`build_component_keymaps`](crate::app::registry::build_component_keymaps) spells it,
    /// and the action id through
    /// [`surface_action_id`](crate::app::registry::surface_action_id) — so neither can drift into a
    /// second spelling of one thing.
    ///
    /// This exists so a widget's own key handler can take its letters from config instead of
    /// holding them as literals — the exposé's `x` / `r` / `d` were hardcoded in `chrome/expose.rs`
    /// while the workspaces dock's identical `x` came from `keybindings.default.toml`.
    ///
    /// Args are empty here because this answers for the flat `action = "key"` form. The
    /// arg-carrying `[[keys.surface.bind]]` form names its action in full, which
    /// `surface_action_id` leaves alone either way.
    pub(crate) fn in_surface(&self, surface: &str, action_name: &str) -> &[String] {
        let label = format!("[[keys.surface]] {surface}");
        let id = crate::app::registry::surface_action_id(
            surface,
            action_name,
            &std::collections::HashMap::new(),
        );
        self.by_layer.get(&(label, id)).map_or(&[], Vec::as_slice)
    }

    /// Every binding of `action_name` as its **raw config key**, in the index's order — empty when
    /// unbound. What a surface drawing keycaps reads, through
    /// [`chord_caps`](crate::shortcut::chord_caps).
    pub(crate) fn chords(&self, action_name: &str) -> &[String] {
        self.by_action.get(action_name).map_or(&[], Vec::as_slice)
    }

    /// The display shortcut for `action_name` as **one line** (several joined with ` / `), or `None`
    /// when unbound — what a tooltip, which has a single line, shows.
    pub(crate) fn get(&self, action_name: &str) -> Option<String> {
        let keys = self.by_action.get(action_name)?;
        let line = keys
            .iter()
            .map(|k| {
                crate::shortcut::format_shortcut_styled(k, k.starts_with("prefix+"), self.style)
            })
            .collect::<Vec<_>>()
            .join(" / ");
        (!line.is_empty()).then_some(line)
    }
}

/// The config action name a [`PaneAction`] button triggers — the canonical
/// identity used to resolve its shortcut (the emitted `WmAction` may be a
/// button-only variant like `ClosePaneById`, so the name is the stable key).
pub(crate) fn pane_action_name(action: heca_config::appearance::PaneAction) -> &'static str {
    use heca_config::appearance::PaneAction;
    match action {
        PaneAction::Split => "split_vertical",
        PaneAction::MoveLeft => "move_pane_left",
        PaneAction::MoveRight => "move_pane_right",
        PaneAction::Close => "close",
        PaneAction::Zoom => "zoom_column",
        PaneAction::Float => "float",
    }
}

/// Wrap a clickable `child` in a tooltip = `label` + the action's current shortcut,
/// resolved centrally from `shortcuts` by `action_name`. The one place any button's
/// tooltip is composed: callers name the action, never the shortcut, so a rebind
/// updates every tip and no surface can drift on the leader symbol or format.
pub(crate) fn action_tooltip<C: Component + heca_grid_ui::ComponentExt + 'static>(
    child: C,
    action_name: &str,
    label: &str,
    shortcuts: &ActionShortcuts,
) -> C {
    let tip = match shortcuts.get(action_name) {
        Some(sc) if !sc.is_empty() => format!("{label}  {sc}"),
        _ => label.to_string(),
    };
    // **A button's pick is named here too, from the same action name the tooltip resolves.**
    //
    // A chrome button declares its pick as a closure — the click and the pick are the same gesture
    // for a button, so it hands over the one it already built. A closure is opaque, and a host
    // cannot ask its policy about an opaque thing: the `prefix+/` picker therefore lettered the
    // sidebar toggles while a pane was floating, even though `sidebar_left` is `TiledOnly` and the
    // click was already being refused (Antonio, driving 2026-08-21). The letter did nothing.
    //
    // Naming it here rather than at each button is the whole point of this function: it is the one
    // place a chrome button's action name is known, which is why the tooltip and its live keybinding
    // are resolved here and not spelled at call sites (AGENTS § "Chrome buttons → action, tooltip,
    // KeyHint"). Every button — this pane header's, the sidebar toggles, a modal's — is covered
    // without any of them saying anything new.
    let mut child = child;
    if let Some(hint) = child.base_mut().hint.as_mut()
        && hint.intent.is_none()
    {
        hint.intent = Some(heca_view::Intent::new(action_name));
    }
    // **The tip is a property of the button, not a box around it** — so the button that comes back
    // is still a button, and can go into a `ButtonGroup`, which takes `Button` children. Wrapping
    // it returned a `Tooltip`, which the group would refuse; that is what forced the tooltip onto
    // every widget in the first place.
    child.tooltip(tip).tooltip_side(TooltipSide::Bottom)
}

/// Map a configured [`PaneAction`] to its `(icon, WM action, label, needs_focus)`.
///
/// `needs_focus` is `true` for **active-targeted** unit actions (`ZoomColumn`/
/// `Float`) — the button must focus its owning pane before dispatching so the
/// action lands on the clicked pane, not whatever happened to be active. The other
/// buttons carry the pane/column in the action itself, so they don't steal focus.
pub(crate) fn pane_action_spec(
    catalog: &crate::actions::ActionCatalog,
    action: heca_config::appearance::PaneAction,
    pane_id: PaneId,
    ws_idx: usize,
    col_idx: usize,
) -> (Glyph, crate::input::WmAction, &'static str, bool) {
    use crate::input::WmAction;
    use heca_config::appearance::PaneAction;
    // Icons come from the action catalog (the single source); the literal is a
    // defensive fallback only, so the bar and the context menu can never drift.
    let icon = |name: &str, fallback: Glyph| catalog.icon(name).unwrap_or(fallback);
    match action {
        PaneAction::Split => (
            // Icon = the add-pane identity (FolderSimplePlus); the shortcut/tooltip name
            // stays `split_vertical` (see `pane_action_name`) so the button still hints `v`.
            icon("add_pane_to_column", Glyph::FolderSimplePlus),
            WmAction::AddPaneToColumn { ws_idx, col_idx },
            "New pane",
            false,
        ),
        PaneAction::MoveLeft => (
            icon("move_pane_left", Glyph::ArrowLineLeft),
            WmAction::MovePaneLeft {
                pane_id: Some(pane_id),
            },
            "Move left",
            false,
        ),
        PaneAction::MoveRight => (
            icon("move_pane_right", Glyph::ArrowLineRight),
            WmAction::MovePaneRight {
                pane_id: Some(pane_id),
            },
            "Move right",
            false,
        ),
        PaneAction::Close => (
            icon("close", Glyph::FolderSimpleMinus),
            WmAction::ClosePaneById { pane_id },
            "Close",
            false,
        ),
        PaneAction::Zoom => (
            icon("zoom_column", Glyph::FrameCorners),
            WmAction::ZoomColumn,
            "Zoom",
            true,
        ),
        PaneAction::Float => (icon("float", Glyph::Cards), WmAction::Float, "Float", true),
    }
}

/// What a pane header *renders* — the projection inputs shared by the rebuild key
/// and the tree builder (groups args so neither fn explodes).
pub(crate) struct PaneHeaderContent<'a> {
    pub(crate) programs: &'a ProgramsConfig,
    pub(crate) fallback_name: &'a str,
    /// User-set override name (wins over the process-derived title), if any.
    pub(crate) custom_name: Option<&'a str>,
    pub(crate) runtime: Option<&'a PaneRuntime>,
    pub(crate) segments: &'a [heca_config::appearance::PaneSegment],
    pub(crate) actions: &'a [heca_config::appearance::PaneAction],
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

/// Whether a pane-action button stays visible when its pane is **floating**, driven
/// by the shared [`action_policy`](crate::app::interaction) classification (split /
/// zoom / move are tiled-only ⇒ hidden; float / close are focused-pane-local ⇒ kept).
pub(crate) fn pane_action_visible_when_floating(
    catalog: &crate::actions::ActionCatalog,
    action: heca_config::appearance::PaneAction,
) -> bool {
    // Policy ignores the concrete ids (and the icon), so dummy ids are fine here.
    let (_, wm, _, _) = pane_action_spec(catalog, action, PaneId(0), 0, 0);
    crate::app::interaction::action_allowed_when_floating(&wm)
}

/// One resolved pane-header action button — the generic unit the header renders. The
/// header is a **dynamic vector** of these, so nothing about the button set is baked
/// into the render loop: today they come from `config.toml`'s `[pane] title_actions`
/// (map via [`pane_action_spec`]), and this is also the seam where a **plugin** will
/// append its own buttons once the plugin action surface exists (a plugin declares the
/// same fields: an icon, an action name, and the `WmAction` to emit). The loop in
/// [`build_pane_header`] never matches on the concrete [`PaneAction`] enum — it only
/// reads these fields — so a new source of buttons needs no loop changes.
pub(crate) struct PaneHeaderButton {
    glyph: Glyph,
    /// Canonical action name — the stable key for the tooltip shortcut lookup (the
    /// emitted `WmAction` may be a button-only variant, so the name is the identity).
    action_name: &'static str,
    /// The action the button emits (click) and the KeyHint fires (picker).
    wm_action: crate::input::WmAction,
    label: &'static str,
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
pub(crate) fn pane_header_buttons(content: &PaneHeaderContent, ctx: &PaneHeaderCtx) -> Vec<PaneHeaderButton> {
    use heca_config::appearance::PaneAction;
    let mut out: Vec<PaneHeaderButton> = content
        .actions
        .iter()
        .copied()
        .filter(|&a| !content.floating || pane_action_visible_when_floating(content.catalog, a))
        .map(|action| {
            let (glyph, wm_action, label, needs_focus) =
                pane_action_spec(content.catalog, action, ctx.pane_id, ctx.ws_idx, ctx.col_idx);
            let is_active = match action {
                PaneAction::Zoom => content.zoomed,
                PaneAction::Float => content.floating,
                _ => false,
            };
            PaneHeaderButton {
                glyph,
                action_name: pane_action_name(action),
                wm_action,
                label,
                // Don't focus-first when floating: the floating pane is already active,
                // and a `FocusPane` from MouseContent is blocked in the floating domain.
                needs_focus: needs_focus && !content.floating,
                is_active,
                destructive: content.catalog.destructive(pane_action_name(action)),
            }
        })
        .collect();
    // ── Plugin seam ──────────────────────────────────────────────────────────────
    // Plugin-contributed pane-header buttons will be appended to `out` here once the
    // plugin action surface lands (each plugin supplies a `PaneHeaderButton`). Keeping
    // the render loop descriptor-driven means that wiring needs no changes below.
    let _ = &mut out;
    out
}

/// A content key identifying everything the header *renders* — used to decide when
/// the retained tree must be rebuilt (vs. just re-laid-out). Cheap per-frame string
/// build (≤20 panes); avoids deriving `Hash` on the projection enums.
pub(crate) fn pane_header_key(content: &PaneHeaderContent, font: f32, avail_w: f32) -> String {
    let view = pane_info_view(
        content.programs,
        content.fallback_name,
        content.custom_name,
        content.runtime,
        // The info bar never shows the process suffix, so it plays no part in the key.
        false,
    );
    let cwd = content
        .runtime
        .and_then(|r| r.cwd.as_ref())
        .map(|c| c.display().to_string());
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
        .map(|&a| content.shortcuts.get(pane_action_name(a)).unwrap_or_default())
        .collect();
    // **The words are NOT part of this key** (F003/P097/T500). What a header *shows* — the
    // foreground program, the branch, the working directory — changes constantly and changes
    // nothing about the header's shape, so it is written into the retained tree each frame
    // (`refresh_pane_header_text`) instead. Keying on it rebuilt the whole bar twice per command,
    // and a rebuilt widget paints nothing until the layout walk reaches it, so the buttons blinked
    // out and back both times.
    //
    // ⚠️ **`view.status` never belonged here at all** — it is carried on the view and rendered
    // nowhere in this bar, so Idle → Running → Success churned the identity while changing nothing
    // a user could see.
    //
    // What stays is what changes the SHAPE: which segments have data at all (a pane outside a
    // repository has no branch segment), and the icon, since a glyph is not text.
    // Which segments have data at all — the shape. `max_width` is passed as unbounded because
    // truncation changes a segment's *words*, never whether it exists, and the words are not here.
    let present: Vec<_> = segment_items(
        content.programs,
        content.fallback_name,
        content.custom_name,
        content.runtime,
        content.segments,
        f32::INFINITY,
        font,
    )
    .into_iter()
    .map(|(seg, glyph, _)| (seg, glyph))
    .collect();
    let _ = (&view.title, &cwd);
    format!(
        "{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{}|{}|{:?}",
        view.icon,
        present,
        content.segments,
        content.actions,
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
            let wm_action = spec.wm_action.clone();
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
                let _ = proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                    source: InteractionSource::MouseContent,
                    intent: InteractionIntent::ActivateAction(wm_action.clone()),
                });
            };
            // **Its words are carried even while only its icon shows** — they are what it says on
            // hover, and what its row reads in the menu once the pane is too narrow to hold it.
            // Every `Button` constructor takes them, which is what makes a collapsed group
            // readable with nothing extra written here.
            // `Primary` is the group's cue that this button named nothing of its own, so the
            // group's variant applies; `Destructive` is a button naming one, and it keeps it.
            let button = Button::new(spec.label)
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
            // The action name is exactly what a key should be — from the data, never a counter —
            // and it is unique WITHOUT the pane id in it, because each pane's header is its own
            // hint surface: `target_identity` prefixes it, giving `pane-header:7/zoom` and
            // `pane-header:9/zoom`.
            //
            // **Nothing is declared about picking.** These buttons used to repeat their own click
            // as a hint, purely to win back a letter the picker was withholding from anything
            // inside a pane. The picker counts things now, so a button gets its letter for being a
            // button (Antonio, 2026-09-04: *"Users/Developers MUST not think where a widget is"*).
            let button = button.key(spec.action_name);
            // Tooltip = label + the action's current keybind(s), resolved centrally
            // by name (never hand-picked here); the leader renders via PREFIX_SYMBOL. It is a
            // property now, so what comes back is still a `Button` and the group will take it.
            group = group.child(action_tooltip(
                button,
                spec.action_name,
                spec.label,
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
        content.programs,
        content.fallback_name,
        content.custom_name,
        content.runtime,
        content.segments,
        theme,
        bar_max,
        font,
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
        .width(Length::Pct(1.0))
        // **Air between the title and the actions**, as a token — it resolves against the inherited
        // font, so it holds at every font size and UI zoom instead of being tuned for one.
        .gap_spacing(heca_grid_ui::Spacing::Sm)
        // **The bar's own breathing room, which is what makes the strip the height it is.** The
        // host reserves `title_bar_reserve` at the pane top — the bar's content height plus this
        // margin twice over — so carrying the margin here is what makes the bar exactly fill the
        // strip that was reserved for it, instead of sitting at the top of it with dead space
        // below (Antonio, driving, 2026-09-02).
        .align(Align::Center);
    let row = match (bar, buttons) {
        (Some(bar), Some(buttons)) => row
            .justify(Justify::SpaceBetween)
            .child(bar)
            .child(buttons),
        (Some(bar), None) => row.justify(Justify::Start).child(bar),
        (None, Some(buttons)) => row.justify(Justify::End).child(buttons),
        (None, None) => return None,
    };
    // **The band is the bar's own surface**, so the strip is exactly as tall as what is in it —
    // rather than a rect the host drew at a height it had worked out separately, which anything
    // else placed in a pane's header slot would have had to match. `Flex` carries no background on
    // purpose: it lays out, a `Surface` decorates.
    Some(
        Surface::new()
            .background(band)
            .width(Length::Pct(1.0))
            // **The strip's own inset, on both axes.** It replaces a hand-subtracted 6px margin
            // that the host used to take off the pane width before handing the bar a budget — the
            // container holds its own padding, and nothing outside it has to know the number.
            .pad_x(heca_grid_ui::Spacing::Xs)
            // **A theme token, not a pixel count.** `Spacing` resolves against the inherited font
            // at layout, so the bar's breathing room scales with the font, the size variant and UI
            // zoom. A raw px value is tuned for one font size and wrong at every other
            // (`docs/widgets.md` § Flex).
            .pad_y(heca_grid_ui::Spacing::Xs)
            .child(row),
    )
}

const VIEWPORT_BADGE_MARGIN: f32 = 0.0;

fn format_lines_above(lines: usize) -> String {
    if lines == 1 {
        "1 line above".to_string()
    } else {
        format!("{lines} lines above")
    }
}

fn build_pane_viewport_widgets(
    pane_id: PaneId,
    event_proxy: &winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
) -> RetainedPaneViewportWidgets {
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    let badge_proxy = event_proxy.clone();
    let mut badge = BadgeButton::accent("0 lines above").on_click(move || {
        let _ = badge_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
            source: InteractionSource::MouseContent,
            intent: InteractionIntent::FocusPane { pane_id },
        });
        let _ = badge_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
            source: InteractionSource::MouseContent,
            intent: InteractionIntent::ActivateAction(crate::input::WmAction::ScrollToBottom),
        });
    });
    badge.base_mut().visible.set(false);

    let mut scrollbar = ScrollBar::new();
    let content_signal = scrollbar.content_extent_signal();
    let viewport_signal = scrollbar.viewport_extent_signal();
    let bar_proxy = event_proxy.clone();
    scrollbar = scrollbar.on_change(move |action| {
        if let heca_grid_ui::SignalData::Float(offset_top) = action.data {
            let max = (content_signal.get_untracked() - viewport_signal.get_untracked()).max(0.0);
            let rows = ((max as f64) - offset_top)
                .round()
                .clamp(0.0, max as f64) as usize;
            let _ = bar_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                source: InteractionSource::MouseContent,
                intent: InteractionIntent::FocusPane { pane_id },
            });
            let _ = bar_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                source: InteractionSource::MouseContent,
                intent: InteractionIntent::ActivateAction(crate::input::WmAction::ScrollToOffset {
                    rows,
                }),
            });
        }
    });
    scrollbar.base_mut().visible.set(false);

    RetainedPaneViewportWidgets { scrollbar, badge }
}

/// Build/update/position the retained per-pane terminal viewport widgets for every
/// visible terminal pane. Unlike pane headers, the tree shape is static, so the
/// widgets are built once per pane and then driven by signals / relaid out.
pub(crate) fn sync_pane_viewport_widgets(
    state: &mut crate::app_state::AppState,
    panes: &[&crate::app::terminal_render::PaneRenderState],
) {
    let font = chrome_gui_theme(state).font_size;
    let show_mode = state.appearance.terminal.show_scrollbar;
    // Measured before the loop takes a mutable borrow of the widgets it positions.
    let header_heights: std::collections::HashMap<PaneId, f32> = panes
        .iter()
        .map(|p| (p.pane_id, crate::chrome::pane_header_height(state, p.pane_id)))
        .collect();
    let mut seen: std::collections::HashSet<PaneId> = std::collections::HashSet::new();
    for pane in panes {
        seen.insert(pane.pane_id);
        let Some(mount) = pane.mount.as_ref() else {
            state.pane_viewport_widgets.remove(&pane.pane_id);
            continue;
        };
        let widgets = state
            .pane_viewport_widgets
            .entry(pane.pane_id)
            .or_insert_with(|| build_pane_viewport_widgets(pane.pane_id, &state.event_proxy));

        let scrollable = mount.snapshot.scrollback_rows > mount.snapshot.rows;
        let max_offset = mount
            .snapshot
            .scrollback_rows
            .saturating_sub(mount.snapshot.rows) as f32;
        widgets
            .scrollbar
            .content_extent_signal()
            .set(mount.snapshot.scrollback_rows as f32);
        widgets
            .scrollbar
            .viewport_extent_signal()
            .set(mount.snapshot.rows as f32);
        widgets
            .scrollbar
            .offset_signal()
            .set((max_offset - mount.snapshot.viewport_offset as f32).max(0.0));
        let scrollbar_visible = match show_mode {
            heca_config::appearance::ScrollbarVisibility::Always => scrollable,
            heca_config::appearance::ScrollbarVisibility::WhenNeeded => {
                scrollable && !mount.snapshot.at_bottom
            }
            heca_config::appearance::ScrollbarVisibility::Never => false,
        };
        widgets.scrollbar.base_mut().visible.set(scrollbar_visible);

        let badge_visible = state.appearance.terminal.show_scrolled_up_badge
            && !mount.snapshot.at_bottom
            && mount.snapshot.viewport_offset > 0;
        widgets.badge.base_mut().visible.set(badge_visible);
        widgets
            .badge
            .label_signal()
            .set(format_lines_above(mount.snapshot.viewport_offset));

        let Some(content_rect) = pane.content_rect else {
            continue;
        };
        if scrollbar_visible {
            widgets.scrollbar.base_mut().style.layout.width =
                heca_grid_ui::style::Length::Px(8.0);
            widgets.scrollbar.base_mut().style.layout.height =
                heca_grid_ui::style::Length::Px(content_rect.size.h as f32);
            LayoutEngine::new().base_font(font).compute(
                &mut widgets.scrollbar,
                Size::new(8.0, content_rect.size.h),
            );
            let bar_bounds = widgets.scrollbar.base().bounds;
            // X hugs the pane's outer right edge; Y/H follow the terminal content
            // rect so the thumb stays below the header and above the bottom inset.
            let bar_x = pane.x + pane.w - bar_bounds.size.w as f32;
            translate_tree(&mut widgets.scrollbar, bar_x as f64, content_rect.loc.y);
        }
        if badge_visible {
            LayoutEngine::new().base_font(font).compute(
                &mut widgets.badge,
                Size::new(pane.w as f64, pane.h as f64),
            );
            let badge_bounds = widgets.badge.base().bounds;
            // Align to the pane's outer right edge (flush, like the scrollbar).
            let badge_x = pane.x + pane.w - badge_bounds.size.w as f32;
            // Sit *below* the pane's header so it doesn't cover the action buttons — measured
            // from the pane's own laid-out tree, so it clears whatever is actually in the header
            // slot rather than a height guessed from the font. Zero when there is no header.
            let header_h = header_heights.get(&pane.pane_id).copied().unwrap_or(0.0);
            let badge_y = pane.y + header_h + VIEWPORT_BADGE_MARGIN;
            translate_tree(&mut widgets.badge, badge_x as f64, badge_y as f64);
        }
    }
    state.pane_viewport_widgets.retain(|id, _| seen.contains(id));
}

/// Build/position the retained per-pane info-bar headers for every visible pane.
/// Runs at the **top** of `render_frame` (before the `scene_view` borrow of
/// `state.compositor`) so it can mutate `state.pane_headers`; render then paints
/// them read-only and `mouse.rs` dispatches pointer events into them. Rebuilds a
/// pane's tree only when its content key changes; re-lays-out + repositions every
/// frame; prunes panes that disappeared.
pub(crate) fn build_pane_headers(state: &crate::app_state::AppState) -> BuiltPaneHeaders {
    let mut built = std::collections::HashMap::new();
    let segments = state.appearance.pane.title_segments.clone();
    let actions = state.appearance.pane.title_actions.clone();
    if segments.is_empty() && actions.is_empty() {
        // Info bar disabled: no pane gets one.
        return built;
    }
    let theme = chrome_gui_theme(state);
    // The strip behind the bar, from the same token the host used to paint it with.
    let band = crate::chrome::theme::top_bottom_pane_background_color(&state.theme);
    let font = theme.font_size;

    // Phase 1: gather per-pane inputs with only immutable borrows of `state`.
    struct Input {
        pane_id: PaneId,
        ws_idx: usize,
        col_idx: usize,
        name: String,
        custom_name: Option<String>,
        runtime: Option<PaneRuntime>,
        zoomed: bool,
        floating: bool,
        avail_w: f32,
    }
    let frames = crate::app::terminal_host::pane_outer_frames(state);
    let active_ws = state.session.active_workspace_idx;
    let mut inputs = Vec::with_capacity(frames.len());
    for (pane_id, _x, _y, w, _h) in frames {
        let (ws_idx, col_idx) = crate::find_pane_location(&state.session, pane_id)
            .map(|(ws, col, _)| (ws, col))
            .unwrap_or((active_ws, 0));
        let (name, custom_name, runtime) = state
            .session
            .active_workspace()
            .and_then(|ws| ws.find_pane(pane_id))
            .map(|p| {
                (
                    p.title.clone(),
                    p.custom_name.clone(),
                    Some(p.runtime.clone()),
                )
            })
            .unwrap_or_else(|| (String::new(), None, None));
        // Floating panes aren't in any column (`find_pane_location` returns None);
        // detect them directly so the bar hides tiled-only buttons + flags float active.
        let floating = state
            .session
            .active_workspace()
            .map(|ws| ws.floating_panes.iter().any(|f| f.pane.id == pane_id))
            .unwrap_or(false);
        let zoomed = !floating
            && state
                .session
                .active_workspace()
                .and_then(|ws| ws.scrolling.columns.get(col_idx))
                .map(|c| c.is_zoomed() || c.is_full_width)
                .unwrap_or(false);
        inputs.push(Input {
            pane_id,
            ws_idx,
            col_idx,
            name,
            custom_name,
            runtime,
            zoomed,
            floating,
            // The pane's own width. The strip insets its content with its own padding, so nothing
            // out here subtracts a margin from it any more.
            avail_w: w.max(0.0),
        });
    }

    // Phase 2: build one bar per pane. No key comparison and no pruning here — `sync_panes` folds
    // this key into the pane's own, so a pane and the bar inside it rebuild together or not at all,
    // and a bar disappears with the pane that held it.
    for input in &inputs {
        let content = PaneHeaderContent {
            programs: &state.programs,
            fallback_name: &input.name,
            custom_name: input.custom_name.as_deref(),
            runtime: input.runtime.as_ref(),
            segments: &segments,
            actions: &actions,
            ws_idx: input.ws_idx,
            col_idx: input.col_idx,
            zoomed: input.zoomed,
            floating: input.floating,
            shortcuts: &state.action_shortcuts,
            catalog: &state.action_catalog,
        };
        let key = pane_header_key(&content, font, input.avail_w);
        let ctx = PaneHeaderCtx {
            pane_id: input.pane_id,
            ws_idx: input.ws_idx,
            col_idx: input.col_idx,
            event_proxy: state.event_proxy.clone(),
        };
        if let Some(root) = build_pane_header(&content, &theme, band, font, input.avail_w, ctx) {
            // The identity rule's warning half (F003/P082/T444). This tree is where it was actually
            // broken: zooming changed what the bar rendered, and the buttons' hint letters moved
            // under Antonio while he was driving (F011/P094/T451). They carry `.key(action_name)`
            // now, and this is what says so if that ever goes.
            super::identity::report_ambiguous_widgets("pane-header", &root);
            // **The words travel beside the tree, not inside its identity.** Whoever holds the
            // retained header writes these in each frame, so a command changing the program it
            // shows moves the words and nothing else (F003/P097/T500).
            let texts: Vec<_> = segment_items(
                content.programs,
                content.fallback_name,
                content.custom_name,
                content.runtime,
                content.segments,
                input.avail_w,
                font,
            )
            .into_iter()
            .map(|(seg, _, text)| (seg, text))
            .collect();
            built.insert(input.pane_id, (root, key, texts));
        }
    }
    built
}


#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::builders::ComponentExt as _;
    use heca_grid_ui::widgets::Label;

    /// **Which actions are destructive is the action's own declaration, not a surface's.**
    ///
    /// It was written into this file as `matches!(action, PaneAction::Close)` — a styling rule keyed
    /// to a name, in a file that should know nothing about which acts cannot be undone (Antonio,
    /// 2026-09-03). Every surface that renders an action reads the same answer now, exactly as they
    /// already do for its icon and its label, so a header button, a menu entry and the palette
    /// cannot disagree about what is dangerous.
    #[test]
    fn a_headers_danger_hue_comes_from_the_action_not_from_its_name() {
        let catalog = crate::actions::ActionCatalog::with_builtins();
        assert!(
            catalog.destructive("close"),
            "closing a pane is declared destructive by the action"
        );
        for safe in ["zoom_column", "float", "add_pane_to_column"] {
            assert!(
                !catalog.destructive(safe),
                "{safe} is not destructive, so nothing should draw it as though it were"
            );
        }
    }

    /// **A chrome button's pick says what it is, so the picker can refuse it** (F003/P082/T432).
    ///
    /// A button hands over the same closure for its click and its pick — they are one gesture for a
    /// button — and a closure is opaque, so the policy had nothing to ask about. `prefix+/`
    /// therefore lettered the sidebar toggles while a pane was floating, even though `sidebar_left`
    /// is `TiledOnly` and the click was already refused: a letter that did nothing.
    ///
    /// It is named here because this is the one place a chrome button's action name is known — the
    /// same place the tooltip and its live keybinding come from. If that ever goes, every chrome
    /// button silently escapes the filter again and nothing else would notice.
    #[test]
    fn a_buttons_pick_is_named_from_its_action() {
        let shortcuts = ActionShortcuts::default();
        let button = Label::new("×").on_hint(|| {});
        let tip = action_tooltip(button, "close", "Close", &shortcuts);

        // Read straight off the widget: the tip is a **property** now, so what comes back is the
        // button itself rather than a wrapper around it — which is exactly what lets a pane header
        // button go into a `ButtonGroup`.
        let named = tip
            .base()
            .hint
            .as_ref()
            .expect("the button declares a pick")
            .intent
            .as_ref()
            .expect("…and it is named");
        assert_eq!(named.action, "close");
    }

    /// A pick that already said what it is keeps it — the caller knows more than the action name
    /// alone (a pane button carries the pane it acts on).
    #[test]
    fn a_pick_that_already_named_itself_is_left_alone() {
        let shortcuts = ActionShortcuts::default();
        let button = Label::new("×").on_hint(heca_grid_ui::Hint::of(
            heca_view::Intent::new("close_pane_by_id").arg("pane_id", heca_view::PropValue::Int(7)),
            || {},
        ));
        let tip = action_tooltip(button, "close", "Close", &shortcuts);

        let named = tip
            .base()
            .hint
            .as_ref()
            .unwrap()
            .intent
            .as_ref()
            .unwrap();
        assert_eq!(named.action, "close_pane_by_id", "the caller's own naming wins");
    }
}
