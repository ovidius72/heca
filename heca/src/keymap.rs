//! Keymap registry — per-mode key binding resolution.
//!
//! Replaces the monolithic `KeyBindings` struct with a clean separation
//! between key representation (`KeyCombo`) and mode-specific maps.

use crate::input::WmAction;
use std::collections::HashMap;

/// The flat map's layer for what the **leader** key (`prefix`) leads to — `prefix+w`.
pub const LEADER_LAYER: &str = "normal";
/// The flat map's layer for what is bound **directly**, with no leader first — `Ctrl+q`.
pub const DIRECT_LAYER: &str = "global";

/// **A key as a binding writes it**: `prefix+w` (after the leader) or `Ctrl+q` (direct).
///
/// The one reader of the `prefix+` spelling. Config tables, a mode's trigger, an unbind and the
/// index all split it here; it used to be split by hand in ten places across five files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrittenKey {
    /// Whether it follows the leader key.
    pub after_leader: bool,
    /// The keys themselves, without the leader.
    pub combo: KeyCombo,
    /// **The keys pressed after it, in order** — a sequence. `prefix+Shift+s r` is `Shift+s` after
    /// the leader, then `r`. Empty for a single key.
    pub then: Vec<KeyCombo>,
}

impl WrittenKey {
    /// Read a key as written, `prefix+` and all. A space separates the steps of a sequence:
    /// `+` joins keys pressed together, a space says "then".
    pub fn parse(written: &str) -> Self {
        let steps = written_steps(written);
        let mut steps = steps.iter();
        let (after_leader, keys) = split_leader(steps.next().map_or("", String::as_str));
        Self {
            after_leader,
            combo: KeyCombo::parse(keys),
            then: steps.map(|step| KeyCombo::parse(step)).collect(),
        }
    }

    /// Every key of it, in order — the first and those pressed after it.
    pub fn keys(&self) -> Vec<KeyCombo> {
        std::iter::once(self.combo.clone())
            .chain(self.then.iter().cloned())
            .collect()
    }

    /// The flat layer this key binds in: [`LEADER_LAYER`] after the leader, else [`DIRECT_LAYER`].
    pub fn layer(&self) -> &'static str {
        if self.after_leader {
            LEADER_LAYER
        } else {
            DIRECT_LAYER
        }
    }
}

/// **The steps of a written key, in order** — a space says "then", so `prefix+Shift+s r` is two
/// steps; spaces beside a `+` belong to the step (`prefix+ Shift+w` is one). The one reader of that
/// rule, for binding a key and for drawing it.
pub fn written_steps(written: &str) -> Vec<String> {
    let mut joined = String::new();
    for word in written.split_whitespace() {
        if !joined.is_empty() && !joined.ends_with('+') && !word.starts_with('+') {
            joined.push(' ');
        }
        joined.push_str(word);
    }
    joined
        .split(' ')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// A written key split into whether it follows the leader and the keys themselves, as text — for a
/// surface that shows a binding rather than binds it. [`WrittenKey::parse`] is built on this.
pub fn split_leader(written: &str) -> (bool, &str) {
    let written = written.trim();
    match written.strip_prefix("prefix+") {
        Some(rest) => (true, rest.trim()),
        None => (false, written),
    }
}

/// Normalized representation of a key press.
/// Equality and hashing are case-insensitive on the `key` field so that
/// "Enter" and "enter" match the same binding.
#[derive(Clone, Debug, Eq)]
pub struct KeyCombo {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_: bool,
}

impl PartialEq for KeyCombo {
    fn eq(&self, other: &Self) -> bool {
        self.ctrl == other.ctrl
            && self.shift == other.shift
            && self.alt == other.alt
            && self.super_ == other.super_
            && self.key.eq_ignore_ascii_case(&other.key)
    }
}

impl std::hash::Hash for KeyCombo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ctrl.hash(state);
        self.shift.hash(state);
        self.alt.hash(state);
        self.super_.hash(state);
        self.key.to_lowercase().hash(state);
    }
}

impl KeyCombo {
    /// Parse a key string like "h", "Ctrl+h", "Ctrl+Shift+l", "Space".
    /// Character keys are lowercased so "Q" and "q" match the same binding.
    pub fn parse(s: &str) -> Self {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut super_ = false;
        let mut key = String::new();
        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                "super" | "win" | "cmd" => super_ = true,
                _ => {
                    let k = part.to_lowercase();
                    // Map shifted symbols to their base key + set shift flag.
                    // This lets config write `ctrl+{` instead of `ctrl+shift+[`.
                    let (base, is_shifted) = match k.as_str() {
                        "{" => ("[", true),
                        "}" => ("]", true),
                        ":" => (";", true),
                        "\"" => ("'", true),
                        "<" => (",", true),
                        ">" => (".", true),
                        "?" => ("/", true),
                        "+" => ("=", true),
                        "_" => ("-", true),
                        "|" => ("\\", true),
                        "!" => ("1", true),
                        "@" => ("2", true),
                        "#" => ("3", true),
                        "$" => ("4", true),
                        "%" => ("5", true),
                        "^" => ("6", true),
                        "&" => ("7", true),
                        "*" => ("8", true),
                        "(" => ("9", true),
                        ")" => ("0", true),
                        "~" => ("`", true),
                        _ => (k.as_str(), false),
                    };
                    key = base.to_string();
                    if is_shifted {
                        shift = true;
                    }
                }
            }
        }
        Self {
            key,
            ctrl,
            shift,
            alt,
            super_,
        }
    }
}

/// **Every resolved keymap**, as one thing — the layers a keypress is matched against, in the order
/// the input path consults them.
///
/// They are grouped because they are one artefact with one lifetime: all four are built from the
/// same config at load and rebuilt together on `prefix+Shift+r`, and every consumer that needs one
/// needs several. Threading them as four parameters had already pushed the window-event entry point
/// past what one function should take.
pub struct Keymaps {
    /// Prefix (`normal`) and direct (`global`) bindings — `[keys]`.
    pub flat: KeymapRegistry,
    /// One per custom input mode — `[[keys.mode]]`, plus the built-in `resize` / `sidebar` /
    /// `selection` / `focus` layers.
    pub modes: HashMap<String, KeymapRegistry>,
    /// One per component or narrowed placement — `[[keys.component]]`, consulted only while that
    /// component holds chrome focus.
    pub components: HashMap<String, KeymapRegistry>,
    /// Mode name → (trigger combo, sticky). A mode entered by focus rather than a key has none.
    pub triggers: HashMap<String, (KeyCombo, bool)>,
    /// **What key runs this action**, reversed out of the maps above.
    pub by_action: BindingIndex,
}

/// One place a key is bound, named the way the user's file names it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BoundKey {
    /// Which layer — `[keys]`, `[[keys.mode]] sidebar`, `[[keys.component]] workspaces`, `plugin`.
    pub layer: String,
    /// The combo exactly as it is typed, `prefix+` and all.
    pub key: String,
    /// **The arguments the key calls the action with** — `{mode = "session"}` for `prefix+#` on
    /// `command_palette`; empty for a plain binding. Kept so a surface asks for the key of the
    /// action *as it calls it*: the palette's "Command Palette" row is `command_palette` with no
    /// arguments and must not wear `prefix+#`, which opens the session list.
    pub args: BindingArgs,
    /// **The key that opens the mode this binding lives in**, as written (`prefix+r` for resize) —
    /// `None` for a key pressed straight away. A key inside a mode is reached by pressing this first,
    /// so a surface showing it shows both: `Ctrl+h` alone would read as a key that works anywhere.
    pub after: Option<String>,
}

impl BoundKey {
    /// A key bound in `layer`, pressed straight away and calling its action with no arguments.
    pub fn new(layer: &str, key: &str) -> Self {
        Self {
            layer: layer.to_string(),
            key: key.to_string(),
            args: BindingArgs::new(),
            after: None,
        }
    }

    /// The arguments the key calls its action with.
    pub fn args(mut self, args: &std::collections::HashMap<String, String>) -> Self {
        self.args = args.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        self
    }

    /// The key that opens the mode this one lives in — an empty one (a mode entered by focus)
    /// leaves the key pressed straight away.
    pub fn after(mut self, trigger: &str) -> Self {
        let trigger = trigger.trim();
        self.after = (!trigger.is_empty()).then(|| trigger.to_string());
        self
    }

    /// **The keys to press, in order**, as written and separated by a space — `prefix+r Ctrl+h`
    /// for a key inside the resize mode, the key alone otherwise. What every surface showing a
    /// binding draws.
    pub fn sequence(&self) -> String {
        match &self.after {
            Some(first) => format!("{first} {}", self.key),
            None => self.key.clone(),
        }
    }
}

/// A binding's arguments, sorted by name — one spelling for one set of arguments, whatever order
/// the config file wrote them in.
pub type BindingArgs = std::collections::BTreeMap<String, String>;

/// Action id → every key bound to it, across **every** layer.
///
/// **Why this and not the config file.** Reading `[keys]` answers for the flat map alone: it cannot
/// see a `[[keys.mode]]` binding, cannot see a `[[keys.component]]` one, and can never see a key a
/// plugin registered at runtime — which is exactly what the shortcut lookup used to do. This index
/// is filled while each layer is built, so it sees all of them uniformly, and is rebuilt with them.
///
/// It replaced `ActionMeta::default_binding`, which held a *second* copy of every key already in
/// `keybindings.default.toml` — one nothing compared against, so the two could drift silently.
pub type BindingIndex = std::collections::BTreeMap<String, Vec<BoundKey>>;

/// Record that `action` answers to `bound`, keeping the list free of duplicates.
pub fn index_binding(index: &mut BindingIndex, action: &str, bound: BoundKey) {
    let entry = index.entry(action.to_string()).or_default();
    if !entry.contains(&bound) {
        entry.push(bound);
    }
}

/// What a key is bound to — a built-in action, or a **name-keyed** one resolved at press time.
///
/// **The constraint that shapes this**: config is loaded *before* providers and
/// plugins register their actions. So a binding to `plugin.docker.restart` is **not resolvable at
/// load** — the action does not exist yet. An unknown name at load is therefore **not a config
/// error**; it becomes a [`Dynamic`](ActionRef::Dynamic) reference that resolves when the key is
/// actually pressed.
#[derive(Clone, Debug, PartialEq)]
pub enum ActionRef {
    /// Resolved at **load** (`action_from_name` / `build_action`) — today's behaviour exactly: the
    /// same arg-parsing errors surface at load, and a press costs nothing.
    Builtin(WmAction),
    /// Resolved at **press** — an `Intent { action, args }` naming an action that may not be
    /// registered yet (or ever). Still unknown when pressed ⇒ a debug warning, never a crash.
    Dynamic(crate::chrome::Intent),
}

/// Registry that maps key combinations to actions, organized by input mode.
pub struct KeymapRegistry {
    modes: HashMap<String, HashMap<KeyCombo, ActionRef>>,
    /// **Key sequences**, per mode — every key of each, in order, and what it runs. A sequence's
    /// first key may also be a key of its own (`prefix+s` and `prefix+s r`); see [`step`](Self::step).
    sequences: HashMap<String, Vec<(Vec<KeyCombo>, ActionRef)>>,
}

/// **Where some keys pressed in a row stand** among a mode's sequences.
#[derive(Clone, Debug, PartialEq)]
pub struct SequenceStep {
    /// A sequence these keys complete.
    pub complete: Option<ActionRef>,
    /// Whether a longer sequence starts with these keys — the next key may still continue it.
    pub continues: bool,
}

impl KeymapRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            modes: HashMap::new(),
            sequences: HashMap::new(),
        }
    }

    /// Bind a key combination to an action in a given mode.
    pub fn bind(&mut self, mode: &str, combo: KeyCombo, action: ActionRef) {
        self.modes
            .entry(mode.to_string())
            .or_default()
            .insert(combo, action);
    }

    /// **Bind a sequence** — `keys` pressed one after another, in `mode`. Binding the same keys
    /// again replaces what they run, as a single key does. Returns what they ran before.
    pub fn bind_sequence(
        &mut self,
        mode: &str,
        keys: Vec<KeyCombo>,
        action: ActionRef,
    ) -> Option<ActionRef> {
        let list = self.sequences.entry(mode.to_string()).or_default();
        match list.iter_mut().find(|(bound, _)| *bound == keys) {
            Some((_, old)) => Some(std::mem::replace(old, action)),
            None => {
                list.push((keys, action));
                None
            }
        }
    }

    /// Take a sequence out of `mode`.
    pub fn unbind_sequence(&mut self, mode: &str, keys: &[KeyCombo]) {
        if let Some(list) = self.sequences.get_mut(mode) {
            list.retain(|(bound, _)| bound != keys);
        }
    }

    /// **Where `keys`, pressed in a row, stand** in `mode` — a sequence they complete, and whether a
    /// longer one starts with them.
    pub fn step(&self, mode: &str, keys: &[KeyCombo]) -> SequenceStep {
        let list = self.sequences.get(mode).map_or(&[][..], Vec::as_slice);
        SequenceStep {
            complete: list
                .iter()
                .find(|(bound, _)| bound == keys)
                .map(|(_, action)| action.clone()),
            continues: list
                .iter()
                .any(|(bound, _)| bound.len() > keys.len() && bound.starts_with(keys)),
        }
    }

    /// Remove a binding from a mode. Keyed by the **combo**, so `[keys.unbind]` retires a binding
    /// whatever it points at — a built-in or a dynamic action id alike.
    pub fn unbind(&mut self, mode: &str, combo: &KeyCombo) -> Option<ActionRef> {
        self.modes.get_mut(mode)?.remove(combo)
    }

    /// Rebind an existing key to a new key within the same mode.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "used by tests and reserved for config reload and RPC workflows"
        )
    )]
    pub fn rebind(&mut self, mode: &str, old: &KeyCombo, new: KeyCombo) {
        if let Some(map) = self.modes.get_mut(mode)
            && let Some(action) = map.remove(old)
        {
            map.insert(new, action);
        }
    }

    /// Look up what `combo` is bound to in `mode`.
    pub fn resolve(&self, mode: &str, combo: &KeyCombo) -> Option<&ActionRef> {
        self.modes.get(mode)?.get(combo)
    }

    /// Look up `combo` and return it only if it is bound to a **built-in** — i.e. the binding was
    /// resolved at config load. `None` when unbound *or* when it points at a name-keyed action that
    /// resolves at press time.
    ///
    /// This is the introspection question "which `WmAction` does this key run?", which only has an
    /// answer for built-ins; a dynamic binding's answer depends on what is registered at the moment
    /// it is pressed.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "used by tests and reserved for config reload and RPC workflows"
        )
    )]
    pub fn resolve_builtin(&self, mode: &str, combo: &KeyCombo) -> Option<&WmAction> {
        match self.resolve(mode, combo)? {
            ActionRef::Builtin(a) => Some(a),
            ActionRef::Dynamic(_) => None,
        }
    }

    /// Return all bindings for a mode.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn bindings_in_mode(&self, mode: &str) -> Option<&HashMap<KeyCombo, ActionRef>> {
        self.modes.get(mode)
    }

    /// Check whether a mode exists (has any bindings).
    #[expect(
        dead_code,
        reason = "used by tests and reserved for config reload and RPC workflows"
    )]
    pub fn has_mode(&self, mode: &str) -> bool {
        self.modes.contains_key(mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A space says "then"; a space beside `+` does not.** `prefix+Shift+s r` is two steps,
    /// `prefix+ Shift+w` one.
    #[test]
    fn a_space_separates_steps_but_not_beside_a_plus() {
        assert_eq!(written_steps("prefix+Shift+s r"), ["prefix+Shift+s", "r"]);
        assert_eq!(written_steps(" prefix+ Shift+w "), ["prefix+Shift+w"]);
        let key = WrittenKey::parse("prefix+Shift+s r");
        assert!(key.after_leader);
        assert_eq!(
            key.keys(),
            [KeyCombo::parse("Shift+s"), KeyCombo::parse("r")]
        );
    }

    /// A written key knows which flat layer it binds in, and the leader never reaches the combo.
    #[test]
    fn a_written_key_names_its_layer() {
        let led = WrittenKey::parse(" prefix+ Shift+w ");
        assert!(led.after_leader);
        assert_eq!(led.layer(), LEADER_LAYER);
        assert_eq!(led.combo, KeyCombo::parse("Shift+w"));

        let direct = WrittenKey::parse("Ctrl+q");
        assert!(!direct.after_leader);
        assert_eq!(direct.layer(), DIRECT_LAYER);
        assert_eq!(direct.combo, KeyCombo::parse("Ctrl+q"));
    }

    /// Two spellings of one key are one key — what an unbind matches on.
    #[test]
    fn a_written_key_is_the_same_key_however_it_is_cased() {
        assert_eq!(WrittenKey::parse("prefix+W"), WrittenKey::parse("prefix+w"));
        assert_ne!(WrittenKey::parse("prefix+w"), WrittenKey::parse("w"));
    }

    #[test]
    fn test_key_combo_parse() {
        let c = KeyCombo::parse("h");
        assert_eq!(c.key, "h");
        assert!(!c.ctrl);
        assert!(!c.shift);

        let c = KeyCombo::parse("Ctrl+Shift+l");
        assert_eq!(c.key, "l");
        assert!(c.ctrl);
        assert!(c.shift);
        assert!(!c.alt);

        let c = KeyCombo::parse("Alt+Super+x");
        assert_eq!(c.key, "x");
        assert!(c.alt);
        assert!(c.super_);
        assert!(!c.ctrl);
        assert!(!c.shift);
    }

    #[test]
    fn test_bind_and_resolve() {
        let mut reg = KeymapRegistry::new();
        let combo = KeyCombo::parse("h");
        let action = WmAction::FocusLeft;
        reg.bind(
            LEADER_LAYER,
            combo.clone(),
            ActionRef::Builtin(action.clone()),
        );
        assert_eq!(reg.resolve_builtin(LEADER_LAYER, &combo), Some(&action));
        assert_eq!(reg.resolve_builtin("sidebar", &combo), None);
    }

    #[test]
    fn test_unbind() {
        let mut reg = KeymapRegistry::new();
        let combo = KeyCombo::parse("x");
        reg.bind(
            LEADER_LAYER,
            combo.clone(),
            ActionRef::Builtin(WmAction::ClosePane),
        );
        assert!(reg.resolve_builtin(LEADER_LAYER, &combo).is_some());
        reg.unbind(LEADER_LAYER, &combo);
        assert!(reg.resolve_builtin(LEADER_LAYER, &combo).is_none());
    }

    #[test]
    fn test_rebind() {
        let mut reg = KeymapRegistry::new();
        let old = KeyCombo::parse("h");
        let new = KeyCombo::parse("Left");
        reg.bind(
            LEADER_LAYER,
            old.clone(),
            ActionRef::Builtin(WmAction::FocusLeft),
        );
        reg.rebind(LEADER_LAYER, &old, new.clone());
        assert!(reg.resolve_builtin(LEADER_LAYER, &old).is_none());
        assert_eq!(
            reg.resolve_builtin(LEADER_LAYER, &new),
            Some(&WmAction::FocusLeft)
        );
    }

    #[test]
    fn test_multiple_modes() {
        let mut reg = KeymapRegistry::new();
        let combo = KeyCombo::parse("j");
        reg.bind(
            LEADER_LAYER,
            combo.clone(),
            ActionRef::Builtin(WmAction::FocusDown),
        );
        reg.bind(
            "selection",
            combo.clone(),
            ActionRef::Builtin(WmAction::FocusUp),
        );
        assert_eq!(
            reg.resolve_builtin(LEADER_LAYER, &combo),
            Some(&WmAction::FocusDown)
        );
        assert_eq!(
            reg.resolve_builtin("selection", &combo),
            Some(&WmAction::FocusUp)
        );
    }

    #[test]
    fn test_case_insensitive_named_key() {
        // Config stores "enter" (lowercased by parse), event sends "Enter".
        let mut reg = KeymapRegistry::new();
        let config_combo = KeyCombo::parse("Enter");
        let event_combo = KeyCombo {
            key: "Enter".to_string(),
            ctrl: false,
            shift: false,
            alt: false,
            super_: false,
        };
        reg.bind(
            LEADER_LAYER,
            config_combo,
            ActionRef::Builtin(WmAction::SplitHorizontal),
        );
        assert_eq!(
            reg.resolve_builtin(LEADER_LAYER, &event_combo),
            Some(&WmAction::SplitHorizontal)
        );
    }

    #[test]
    fn test_shift_q_matches_lowercase_q() {
        // Shift+Q produces "Q" but config stores "q".
        let mut reg = KeymapRegistry::new();
        let config_combo = KeyCombo::parse("Shift+q");
        let event_combo = KeyCombo {
            key: "Q".to_string(),
            ctrl: false,
            shift: true,
            alt: false,
            super_: false,
        };
        reg.bind(
            LEADER_LAYER,
            config_combo,
            ActionRef::Builtin(WmAction::SwapPane),
        );
        assert_eq!(
            reg.resolve_builtin(LEADER_LAYER, &event_combo),
            Some(&WmAction::SwapPane)
        );
    }
}
