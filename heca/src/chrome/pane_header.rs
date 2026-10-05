//! **The in-pane header** — its info bar, its action buttons, and the retained trees behind them
//! (F003/P082/T427 split).
//!
//! It owns what a pane says about itself: the composed name and program icon, the chips (cwd, git,
//! and any a program built on heca adds), the action buttons and their shortcuts, and the per-pane retained widgets the host
//! keeps in step with the session. It owns **nothing** about the chrome around it — the regions,
//! the layer stack and the letters all live beside it, not here.
//!
//! Split out of a 5236-line `chrome/mod.rs` that held every kind of logic at once.

use super::*;
use heca_grid_ui::builders::StyleExt as _;
use heca_grid_ui::widgets::{Button, ButtonGroup, ButtonVariant};

mod env;
pub(crate) use env::{HeaderEnv, PaneHeader, give_header_facts};

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
        git_branch: git.map(git_branch),
        git_added: git.and_then(git_added),
        git_modified: git.and_then(git_modified),
        git_deleted: git.and_then(git_deleted),
    }
}

/// **How git state is worded** — one place, read by the header projection and by the sidebar row's
/// git line, so the two cannot describe one repository two ways. A repository with no branch is
/// `detached`; a count of nothing is no text at all.
pub(crate) fn git_branch(info: &heca_core::runtime::GitInfo) -> String {
    info.branch
        .clone()
        .unwrap_or_else(|| "detached".to_string())
}

/// `+N` for N added files, or `None` when there are none. See [`git_branch`].
pub(crate) fn git_added(info: &heca_core::runtime::GitInfo) -> Option<String> {
    (info.added > 0).then(|| format!("+{}", info.added))
}

/// `~N` for N modified files, or `None`. See [`git_branch`].
pub(crate) fn git_modified(info: &heca_core::runtime::GitInfo) -> Option<String> {
    (info.modified > 0).then(|| format!("~{}", info.modified))
}

/// `-N` for N deleted files, or `None`. See [`git_branch`].
pub(crate) fn git_deleted(info: &heca_core::runtime::GitInfo) -> Option<String> {
    (info.deleted > 0).then(|| format!("-{}", info.deleted))
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

/// **What each chip has to say right now** — the one derivation, read three times.
///
/// The builder turns these into a tag, the header's identity reads only *which chips came back*
/// (that is its shape), and the per-frame update writes the texts into the tree that is already on
/// screen. Three readers, one answer, so a chip cannot be built from one thing and rewritten from
/// another. A chip with nothing to say for this pane is skipped (no empty pill).
///
/// Returns `(chip name, icon, text)`.
pub(crate) fn segment_items(
    chips: &super::pane_items::PaneChips,
    defs: &[&super::pane_items::PaneChipDef],
    facts: &super::pane_items::PaneFacts,
    max_width: f32,
    font: f32,
) -> Vec<(String, Glyph, String)> {
    use super::pane_items::Fit;

    // Collect the produced (icon, text) chips, noting the one that can shorten itself (a path), so
    // we can fit the bar to `max_width` before building it.
    let mut items: Vec<(String, Glyph, String)> = Vec::new();
    let mut shrinkable: Option<usize> = None;
    for def in defs {
        let Some(chip) = chips.chip(def, facts) else {
            continue;
        };
        if def.fit == Fit::PathLeft {
            shrinkable = Some(items.len());
        }
        items.push((def.name.clone(), chip.icon, chip.text));
    }

    // Fit to width: if the bar would overflow the pane, shorten the path chip (left-ellipsised) by
    // the overflow. The per-pane render clip is the hard backstop; this keeps it readable instead of
    // a hard cut.
    let char_w = (font * 0.6).max(1.0);
    let per_segment_overhead = font * 2.5; // icon + gaps + segment padding + divider
    let text_chars: usize = items.iter().map(|(_, _, text)| text.chars().count()).sum();
    let estimated = text_chars as f32 * char_w + items.len() as f32 * per_segment_overhead;
    if estimated > max_width
        && let Some(idx) = shrinkable
    {
        let overflow_chars = ((estimated - max_width) / char_w).ceil() as usize;
        let loc_chars = items[idx].2.chars().count();
        let keep = loc_chars.saturating_sub(overflow_chars).max(1);
        items[idx].2 = truncate_path_left(&items[idx].2, keep);
    }

    items
}

/// Build the pane info bar's segmented [`Tag`] from what the chips said (`segment_items`). `None`
/// when nothing was produced. Used by the terminal pane shell.
pub(crate) fn build_pane_info_bar(
    items: Vec<(String, Glyph, String)>,
    theme: &GuiTheme,
) -> Option<Tag> {
    if items.is_empty() {
        return None;
    }

    let mut tag: Option<Tag> = None;
    for (name, glyph, text) in items {
        // No explicit size → the icon inherits the bar's base font, so glyph and
        // label stay balanced when the bar font changes.
        let leading = Icon::new(glyph).color(theme.colors.foreground);
        // **Each chip is named by what it is**, so its words can be rewritten without the bar
        // being rebuilt (F003/P097/T500). The first chip is the tag's own label; the rest are
        // child labels. Both answer `set_text`.
        let key = segment_text_key(&name);
        tag = Some(match tag.take() {
            None => {
                use heca_grid_ui::builders::ComponentExt as _;
                Tag::new(text).leading(leading).key(key)
            }
            Some(existing) => {
                existing.segment_text_keyed(text, Some(Box::new(leading)), Some(&key))
            }
        });
    }
    tag
}

/// **What a chip's words are called**, so they can be rewritten in place.
///
/// One derivation, read by the builder and by the update pass, so the two cannot address different
/// nodes. Keyed by the chip's *name* rather than its position: which chips have data changes (a pane
/// outside a repository has no branch), and a positional name would then follow whichever chip
/// happened to take that slot.
fn segment_text_key(name: &str) -> String {
    format!("hdr.seg:{name}")
}

// ── In-pane info-bar header: segments (left) + interactive action buttons (right) ──

/// Per-pane context the header buttons need to build their (parameterized) actions
/// and emit them through the app event loop.
pub(crate) struct PaneHeaderCtx {
    pub(crate) pane_id: PaneId,
    pub(crate) ws_idx: usize,
    pub(crate) col_idx: usize,
    pub(crate) event_proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
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
        Self {
            by_action,
            by_layer,
            style,
        }
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
            .map(|k| crate::shortcut::format_shortcut_styled(k, self.style))
            .collect::<Vec<_>>()
            .join(" / ");
        (!line.is_empty()).then_some(line)
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

/// What one pane's header needs to know, as plain data gathered from the session.
pub(crate) struct PaneHeaderInput {
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

#[cfg(test)]
impl PaneHeaderInput {
    /// Facts for pane `pane_id` and nothing else — what a test hands a pane.
    pub(crate) fn for_test(pane_id: PaneId) -> Self {
        Self {
            pane_id,
            ws_idx: 0,
            col_idx: 0,
            name: String::new(),
            custom_name: None,
            runtime: None,
            zoomed: false,
            floating: false,
            avail_w: 100.0,
        }
    }

    /// Which pane these facts are about.
    pub(crate) fn pane(&self) -> PaneId {
        self.pane_id
    }
}

/// **Read what every pane's header depends on**, once a frame. `None` when the info bar is
/// disabled: no pane gets one.
///
/// This builds no widget. Each pane's header (a [`Keyed`](heca_grid_ui::widgets::Keyed) the pane
/// holds) is told the result by [`show_pane_header`], and builds only if what it shows changed.
pub(crate) fn pane_header_inputs(
    state: &mut crate::app_state::AppState,
) -> Option<std::collections::HashMap<PaneId, PaneHeaderInput>> {
    let segments = state
        .pane_chips
        .shown(&state.appearance.pane.title_segments);
    let actions = state
        .pane_buttons
        .shown(&state.appearance.pane.title_actions);
    if segments.is_empty() && actions.is_empty() {
        return None;
    }
    let frames = crate::app::terminal_host::pane_outer_frames(state);
    let active_ws = state.session.active_workspace_idx;
    let mut inputs = std::collections::HashMap::with_capacity(frames.len());
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
        // Floating panes aren't in any column (`find_pane_location` returns None); detect them
        // directly so the bar hides tiled-only buttons + flags float active.
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
        inputs.insert(
            pane_id,
            PaneHeaderInput {
                pane_id,
                ws_idx,
                col_idx,
                name,
                custom_name,
                runtime,
                zoomed,
                floating,
                // The pane's own width. The strip insets its content with its own padding, so
                // nothing out here subtracts a margin from it.
                avail_w: w.max(0.0),
            },
        );
    }
    // An added chip's kept answers are for panes that are still there.
    state
        .pane_chips
        .retain_panes(&inputs.keys().copied().collect());
    Some(inputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::builders::ComponentExt as _;
    use heca_grid_ui::widgets::Label;

    fn facts_for(cwd: &str) -> super::super::pane_items::PaneFacts {
        let rt = PaneRuntime {
            program: Some("nvim".into()),
            cwd: Some(std::path::PathBuf::from(cwd)),
            ..PaneRuntime::default()
        };
        super::super::pane_items::PaneFacts::of(
            PaneId(1),
            &ProgramsConfig::default(),
            "shell",
            None,
            Some(&rt),
        )
    }

    /// **When the bar is too narrow only the path gives way.** The chip that is a path carries that
    /// as its own property (`Fit::PathLeft`), so the header keeps no list of which chip is the long
    /// one — and the others keep their words.
    #[test]
    fn a_narrow_bar_shortens_the_path_chip_and_nothing_else() {
        let chips = super::super::pane_items::PaneChips::default();
        let defs: Vec<_> = ["location", "app_name"]
            .iter()
            .map(|n| chips.get(n).expect("shipped"))
            .collect();
        let facts = facts_for("/a/very/long/path/to/some/project/directory");

        let roomy = segment_items(&chips, &defs, &facts, 10_000.0, 13.0);
        let tight = segment_items(&chips, &defs, &facts, 120.0, 13.0);

        assert_eq!(roomy[0].2, "/a/very/long/path/to/some/project/directory");
        assert!(
            tight[0].2.chars().count() < roomy[0].2.chars().count(),
            "the path shortened"
        );
        assert_eq!(tight[1].2, roomy[1].2, "the program name is untouched");
        assert_eq!(tight.len(), 2);
    }

    /// A pane_name chip builds a bar for a renamed pane and for one that is not renamed.
    #[test]
    fn a_bar_is_built_from_what_the_chips_say() {
        let chips = super::super::pane_items::PaneChips::default();
        let defs = [chips.get("pane_name").expect("shipped")];
        let facts = facts_for("/tmp");
        let items = segment_items(&chips, &defs, &facts, 400.0, 13.0);
        assert!(build_pane_info_bar(items, &GuiTheme::default()).is_some());
        assert!(
            build_pane_info_bar(Vec::new(), &GuiTheme::default()).is_none(),
            "no chips, no bar"
        );
    }

    /// **Which actions are destructive is the action's own declaration, not a surface's.**
    ///
    /// It was written into this file as a `matches!` on the close button — a styling rule keyed to a
    /// name, in a file that should know nothing about which acts cannot be undone (Antonio,
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

        let named = tip.base().hint.as_ref().unwrap().intent.as_ref().unwrap();
        assert_eq!(
            named.action, "close_pane_by_id",
            "the caller's own naming wins"
        );
    }
}
