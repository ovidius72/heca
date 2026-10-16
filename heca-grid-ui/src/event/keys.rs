//! The keyboard's vocabulary: keys, the modifier keys held right now, and which key presses are
//! typing.

// Named only in the docs below.
#[cfg(doc)]
use super::Event;

/// A renderer-agnostic keyboard key. No `winit` types leak into this crate; the
/// host maps its platform keys onto this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GridKey {
    Char(char),
    Enter,
    Space,
    Tab,
    Escape,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    /// A function key, `1..=24`.
    Function(u8),
}

impl GridKey {
    /// **The key a name means** — the one list, so no surface keeps its own.
    ///
    /// A key has more than one spelling in the wild (a config file writes `esc`, a platform reports
    /// `Escape`, a user types `left`), and every surface that mapped platform input to a `GridKey`
    /// wrote the list out again: heca's app kept one keyed by lowercase config name and its
    /// showcase kept another keyed by `winit::NamedKey`, twelve entries each, free to drift.
    /// Aliases are accepted here so a caller normalises nothing.
    ///
    /// A single character is that key, **read in lowercase**: `"a"` and `"A"` are both
    /// [`Char('a')`](GridKey::Char). Whether Shift was down travels in the [`Modifiers`] beside the
    /// key, and what was typed keeps its case in the text — so a binding for `ctrl+a` matches
    /// whichever the platform reported, and a terminal gets the key the way it expects one. Anything
    /// longer that is not named here is `None` — a dead key, a modifier on its own.
    ///
    /// ```
    /// use heca_grid_ui::GridKey;
    ///
    /// assert_eq!(GridKey::from_name("Escape"), Some(GridKey::Escape));
    /// assert_eq!(GridKey::from_name("esc"), Some(GridKey::Escape));
    /// assert_eq!(GridKey::from_name("ArrowLeft"), Some(GridKey::ArrowLeft));
    /// assert_eq!(GridKey::from_name("left"), Some(GridKey::ArrowLeft));
    /// assert_eq!(GridKey::from_name("a"), Some(GridKey::Char('a')));
    /// assert_eq!(GridKey::from_name("A"), Some(GridKey::Char('a')));
    /// assert_eq!(GridKey::from_name("F5"), Some(GridKey::Function(5)));
    /// assert_eq!(GridKey::from_name("F99"), None);
    /// ```
    pub fn from_name(name: &str) -> Option<Self> {
        let lowered = name.to_lowercase();
        Some(match lowered.as_str() {
            "enter" | "return" => GridKey::Enter,
            "space" => GridKey::Space,
            "tab" => GridKey::Tab,
            "escape" | "esc" => GridKey::Escape,
            "backspace" => GridKey::Backspace,
            "delete" | "del" => GridKey::Delete,
            "arrowleft" | "left" => GridKey::ArrowLeft,
            "arrowright" | "right" => GridKey::ArrowRight,
            "arrowup" | "up" => GridKey::ArrowUp,
            "arrowdown" | "down" => GridKey::ArrowDown,
            "home" => GridKey::Home,
            "end" => GridKey::End,
            "pageup" => GridKey::PageUp,
            "pagedown" => GridKey::PageDown,
            "insert" => GridKey::Insert,
            s => {
                // `f1` … `f24`: a function key is a letter and a number, not a character.
                if let Some(n) = s.strip_prefix('f').and_then(|n| n.parse::<u8>().ok())
                    && (1..=24).contains(&n)
                {
                    return Some(GridKey::Function(n));
                }
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => GridKey::Char(c),
                    _ => return None,
                }
            }
        })
    }
}

/// Keyboard modifier state, renderer-agnostic. The host maps its platform
/// modifiers onto this and broadcasts changes via [`Event::ModifiersChanged`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// The Cmd/Super/Windows key.
    pub meta: bool,
}

thread_local! {
    /// What is held down right now. Updated from the [`Event::ModifiersChanged`] broadcast, which
    /// every host already sends, and read by everything that needs to know.
    static HELD: std::cell::Cell<Modifiers> = const {
        std::cell::Cell::new(Modifiers { ctrl: false, alt: false, shift: false, meta: false })
    };
}

/// **What is held down right now.**
///
/// The modifiers are *device* state, not a property of each event, so the framework keeps them in
/// one place and fills them in on every event it delivers. Nothing has to carry them, and nothing
/// has to remember them.
///
/// It was the other way round and it was wrong twice over. A pointer event carried its own
/// modifiers, which meant every host call site had to remember to attach them — and a host has more
/// than one call site, so the ones that forgot sent "nothing held" with no way to tell: a drop
/// resolved as a plain move however hard Shift was pressed. Meanwhile three widgets each kept a
/// private copy of the same broadcast, which is the same rule written four times counting the
/// plugin that comes next.
pub fn modifiers() -> Modifiers {
    HELD.with(|m| m.get())
}

/// Record what the host just announced. Called by `dispatch` for every
/// [`Event::ModifiersChanged`], so a widget never has to track this itself.
pub(crate) fn remember_modifiers(m: Modifiers) {
    HELD.with(|c| c.set(m));
}

/// **The text a keystroke committed, if it committed any** — the one rule that decides whether a
/// key is *typing* or a *shortcut*.
///
/// A host has the platform's two facts (the key, and the text it produced) and has to turn them
/// into [`Event::TextInput`] plus [`Event::Key`]. Which chords count as typing is not a decision
/// each host should make: heca's app and its showcase made it separately, and the showcase simply
/// never made it at all — so the identical `CommandPalette` typed in one and was deaf in the other.
///
/// Space **is** typing: a field must be able to type one. A focused button still activates on it,
/// because a button does not consume `TextInput` and the key follows right behind.
///
/// ```
/// use heca_grid_ui::{typed_text, Modifiers};
///
/// let plain = Modifiers::default();
/// assert_eq!(typed_text(Some("a"), plain).as_deref(), Some("a"));
/// assert_eq!(typed_text(Some(" "), plain).as_deref(), Some(" "));
/// // A shortcut is not typing, and neither is a control character.
/// assert!(typed_text(Some("a"), Modifiers { ctrl: true, ..plain }).is_none());
/// assert!(typed_text(Some("\u{1}"), plain).is_none());
/// assert!(typed_text(None, plain).is_none());
/// ```
pub fn typed_text(key_text: Option<&str>, mods: Modifiers) -> Option<String> {
    let text = key_text?;
    if mods.ctrl || mods.meta || text.is_empty() {
        return None;
    }
    text.chars()
        .all(|c| !c.is_control())
        .then(|| text.to_string())
}
