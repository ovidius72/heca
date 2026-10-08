//! The keys each action is bound to, and the tooltip that shows them.

use super::*;

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
    // click was already being refused. The letter did nothing.
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
