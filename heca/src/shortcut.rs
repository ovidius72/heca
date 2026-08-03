//! On-screen formatting for keybindings.
//!
//! The keymap/config store the literal token `prefix` (e.g. `"prefix+f"`); this is
//! the **single place** that turns a binding into the string users *see*, so the
//! prefix renders as a symbol and the formatting lives in one function instead of
//! being re-spelled at every call site.

use heca_grid_ui::widgets::{KeyCap, NfGlyph};

/// How the tmux-style `prefix` key renders in the UI. The config/parse token stays
/// `"prefix"` everywhere — only display substitutes this. Change the symbol here.
pub(crate) const PREFIX_SYMBOL: &str = "λ";

/// Separator between the prefix symbol and the key (a space reads as "prefix, then
/// key" for a leader-style chord; modifier chords inside `keys` keep their own `+`).
const PREFIX_JOIN: &str = " ";

/// How a binding is spelled on screen.
///
/// Two, because a shortcut is shown on two kinds of surface and they have different powers: a
/// **keycap** can draw a Nerd Font glyph ([`chord_caps`]), a **line of text** — a tooltip, a menu
/// row, a terminal — cannot, since it is shaped in the UI face, which has no `⌃ ⌥ ⌘ ⎋`. So the
/// glyphs live with the caps and this enum stays about words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum KeyStyle {
    /// Words, as the config file spells them: `Shift+e`, `ArrowLeft`, `Escape`. The safe spelling
    /// anywhere, and the one a script or `--keys-show` should get — it is also what the user would
    /// type back into `config.toml`.
    #[default]
    Plain,
    /// The **arrows** as symbols (`←` for `ArrowLeft`), words for everything else. Only the arrows,
    /// because they are the only long key names the UI face can actually draw a picture for, and
    /// `λ ArrowLeft` was wider than the command it belonged to.
    Compact,
}

/// Which **Nerd Font glyph** a key token is drawn as — **the single table**, beside
/// [`PREFIX_SYMBOL`] and for the same reason: a surface names the binding, never the glyph, so
/// tooltips, the context menu, the command palette and any future one cannot drift.
///
/// Matched case-insensitively against the config token. Anything absent — a letter, a digit, `[`,
/// `F5`, `PageUp` — is drawn as its own text, so the table names only the keys that have a picture.
///
/// The glyphs come from the embedded Nerd Font rather than plain Unicode because the UI face has no
/// `⌃ ⌥ ⌘ ⎋`: spelling a shortcut with those renders half of it as empty boxes. That is measured,
/// not assumed — `heca-renderer/tests/font_coverage.rs` fails if it ever stops being true.
const KEY_GLYPHS: &[(&str, NfGlyph)] = &[
    // Modifiers.
    ("shift", NfGlyph::Shift),
    ("ctrl", NfGlyph::Control),
    ("control", NfGlyph::Control),
    ("alt", NfGlyph::Option),
    ("option", NfGlyph::Option),
    ("cmd", NfGlyph::Command),
    ("meta", NfGlyph::Command),
    ("super", NfGlyph::Command),
    ("capslock", NfGlyph::CapsLock),
    // Arrows.
    ("arrowup", NfGlyph::ArrowUp),
    ("arrowdown", NfGlyph::ArrowDown),
    ("arrowleft", NfGlyph::ArrowLeft),
    ("arrowright", NfGlyph::ArrowRight),
    ("up", NfGlyph::ArrowUp),
    ("down", NfGlyph::ArrowDown),
    ("left", NfGlyph::ArrowLeft),
    ("right", NfGlyph::ArrowRight),
    // Editing / whitespace.
    ("enter", NfGlyph::Enter),
    ("return", NfGlyph::Enter),
    ("escape", NfGlyph::Escape),
    ("esc", NfGlyph::Escape),
    ("space", NfGlyph::Space),
    ("tab", NfGlyph::Tab),
    ("backspace", NfGlyph::Backspace),
];

/// The glyph for one key token, if it has one.
fn key_glyph(token: &str) -> Option<NfGlyph> {
    let lower = token.to_ascii_lowercase();
    KEY_GLYPHS
        .iter()
        .find(|(name, _)| *name == lower)
        .map(|(_, glyph)| *glyph)
}

/// One binding as the **caps of its chord** — `"prefix+Shift+e"` → `[λ] [⇧] [e]`.
///
/// This is the display path for a surface that draws chips; [`format_shortcut`] is the one for a
/// surface that draws a line of text. Both split the binding the same way, here, so a tooltip and a
/// keycap can never disagree about what the keys are.
///
/// The prefix leads as its own cap, and stays **text** (`λ`): it is not a physical key with a
/// picture, and the UI face has the letter.
pub(crate) fn chord_caps(binding: &str) -> Vec<KeyCap> {
    let (prefixed, keys) = match binding.strip_prefix("prefix+") {
        Some(rest) => (true, rest),
        None => (false, binding),
    };
    let mut caps = Vec::new();
    if prefixed {
        caps.push(KeyCap::Text(PREFIX_SYMBOL.to_string()));
    }
    caps.extend(keys.split('+').filter(|t| !t.is_empty()).map(|token| {
        match key_glyph(token) {
            Some(glyph) => KeyCap::Nf(glyph),
            None => KeyCap::Text(token.to_string()),
        }
    }));
    caps
}

/// Format a keybinding for display as **text** — a tooltip, a menu row, a terminal line. A surface
/// that draws keycaps takes [`chord_caps`] instead.
///
/// `keys` is the key portion — either bare (`"f"`, `"Shift+c"`, `"$"`) or the full config form
/// (`"prefix+f"`); a leading `prefix+` is stripped so callers can pass the raw binding straight from
/// the keymap. When `with_prefix` is `true` (most heca commands go through the prefix) the
/// [`PREFIX_SYMBOL`] is prepended.
///
/// `KeyStyle::Compact` substitutes the four arrows only: `"prefix+ArrowLeft"` → `"λ ←"`. Every other
/// key keeps its word, because the UI face cannot draw the rest (verified in
/// `heca-renderer/tests/font_coverage.rs`) and a tooltip has nowhere else to go.
///
/// ```ignore
/// format_shortcut_styled("f", true, KeyStyle::Plain)          // "λ f"
/// format_shortcut_styled("prefix+f", true, KeyStyle::Plain)   // "λ f"   (config form accepted)
/// format_shortcut_styled("Ctrl+H", false, KeyStyle::Plain)    // "Ctrl+H" (global, no prefix)
/// format_shortcut_styled("prefix+ArrowLeft", true, KeyStyle::Compact) // "λ ←"
/// ```
pub(crate) fn format_shortcut_styled(keys: &str, with_prefix: bool, style: KeyStyle) -> String {
    let keys = keys.strip_prefix("prefix+").unwrap_or(keys);
    let keys = match style {
        KeyStyle::Plain => keys.to_string(),
        KeyStyle::Compact => keys
            .split('+')
            .filter(|t| !t.is_empty())
            .map(|token| match token.to_ascii_lowercase().as_str() {
                "arrowup" | "up" => "↑".to_string(),
                "arrowdown" | "down" => "↓".to_string(),
                "arrowleft" | "left" => "←".to_string(),
                "arrowright" | "right" => "→".to_string(),
                _ => token.to_string(),
            })
            .collect::<Vec<_>>()
            .join("+"),
    };
    if with_prefix {
        format!("{PREFIX_SYMBOL}{PREFIX_JOIN}{keys}")
    } else {
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(keys: &str, with_prefix: bool) -> String {
        format_shortcut_styled(keys, with_prefix, KeyStyle::Plain)
    }

    #[test]
    fn prepends_prefix_symbol() {
        assert_eq!(plain("f", true), format!("{PREFIX_SYMBOL} f"));
        assert_eq!(plain("Shift+c", true), format!("{PREFIX_SYMBOL} Shift+c"));
    }

    #[test]
    fn accepts_config_form() {
        // A raw keymap binding ("prefix+f") renders the same as the bare key.
        assert_eq!(plain("prefix+f", true), plain("f", true));
    }

    #[test]
    fn global_binding_has_no_prefix() {
        assert_eq!(plain("Ctrl+H", false), "Ctrl+H");
    }

    /// A **text** surface gets the arrows and keeps every other word — the six symbols the UI face
    /// lacks (`⌃ ⌥ ⌘ ⎋ ⇞ ⇟`) would be empty boxes, which is the whole reason keycaps exist.
    #[test]
    fn compact_substitutes_the_arrows_and_nothing_else() {
        assert_eq!(
            format_shortcut_styled("prefix+ArrowLeft", true, KeyStyle::Compact),
            format!("{PREFIX_SYMBOL} ←"),
        );
        assert_eq!(
            format_shortcut_styled("Ctrl+Shift+Escape", false, KeyStyle::Compact),
            "Ctrl+Shift+Escape",
            "no ⌃ ⇧ ⎋ in a line of text — the UI font has none of them",
        );
    }

    /// A binding becomes **caps**: the prefix as its own text chip, a Nerd Font glyph for every key
    /// that has a picture, the literal text for the rest.
    #[test]
    fn a_chord_becomes_one_cap_per_key() {
        assert_eq!(
            chord_caps("prefix+Shift+e"),
            vec![
                KeyCap::Text(PREFIX_SYMBOL.to_string()),
                KeyCap::Nf(NfGlyph::Shift),
                KeyCap::Text("e".to_string()),
            ],
        );
        assert_eq!(
            chord_caps("prefix+ArrowLeft"),
            vec![
                KeyCap::Text(PREFIX_SYMBOL.to_string()),
                KeyCap::Nf(NfGlyph::ArrowLeft),
            ],
        );
        assert_eq!(
            chord_caps("Escape"),
            vec![KeyCap::Nf(NfGlyph::Escape)],
            "a global binding has no prefix cap",
        );
        assert_eq!(
            chord_caps("prefix+]"),
            vec![
                KeyCap::Text(PREFIX_SYMBOL.to_string()),
                KeyCap::Text("]".to_string()),
            ],
            "a key with no picture is spelled",
        );
    }

    /// Case is the config author's business, not the table's: `ctrl`, `Ctrl` and `CTRL` are one key.
    #[test]
    fn key_names_match_whatever_the_config_capitalised() {
        for spelling in ["ctrl+a", "Ctrl+a", "CTRL+a"] {
            assert_eq!(
                chord_caps(spelling),
                vec![KeyCap::Nf(NfGlyph::Control), KeyCap::Text("a".to_string())],
                "{spelling}",
            );
        }
    }
}
