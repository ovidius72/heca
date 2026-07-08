//! On-screen formatting for keybindings.
//!
//! The keymap/config store the literal token `prefix` (e.g. `"prefix+f"`); this is
//! the **single place** that turns a binding into the string users *see*, so the
//! prefix renders as a symbol and the formatting lives in one function instead of
//! being re-spelled at every call site.

/// How the tmux-style `prefix` key renders in the UI. The config/parse token stays
/// `"prefix"` everywhere — only display substitutes this. Change the symbol here.
pub(crate) const PREFIX_SYMBOL: &str = "λ";

/// Separator between the prefix symbol and the key (a space reads as "prefix, then
/// key" for a leader-style chord; modifier chords inside `keys` keep their own `+`).
const PREFIX_JOIN: &str = " ";

/// Format a keybinding for display.
///
/// `keys` is the key portion — either bare (`"f"`, `"Shift+c"`, `"$"`) or the full
/// config form (`"prefix+f"`); a leading `prefix+` is stripped so callers can pass
/// the raw binding straight from the keymap. When `with_prefix` is `true` (most heca
/// commands go through the prefix) the [`PREFIX_SYMBOL`] is prepended.
///
/// ```ignore
/// format_shortcut("f", true)        // "λ f"
/// format_shortcut("prefix+f", true) // "λ f"   (config form accepted)
/// format_shortcut("Shift+c", true)  // "λ Shift+c"
/// format_shortcut("Ctrl+H", false)  // "Ctrl+H"   (global binding, no prefix)
/// ```
pub(crate) fn format_shortcut(keys: &str, with_prefix: bool) -> String {
    let keys = keys.strip_prefix("prefix+").unwrap_or(keys);
    if with_prefix {
        format!("{PREFIX_SYMBOL}{PREFIX_JOIN}{keys}")
    } else {
        keys.to_string()
    }
}

/// Resolve the **display shortcut(s)** for an action by its config name — the
/// single, centralized seam every button uses so no surface hand-picks or hardcodes
/// a shortcut. Reads the *effective* binding: the user's override in `user` when
/// present (so a rebind is honored — **not** the bundled default), else `defaults`,
/// mirroring `build_keymap`'s per-action merge. An action may carry **several**
/// bindings (`"prefix+x, prefix+X"` or a list); all are rendered and joined with
/// `" / "`. The leader renders through [`PREFIX_SYMBOL`] (never a literal). Returns
/// `None` when the action is unbound (caller then shows the label alone).
///
/// Each binding string carries whether it's a leader chord (`"prefix+x"`) or a
/// global one (`"Alt+1"`), so a name maps to exactly what the user presses.
pub(crate) fn shortcut_for_action(
    name: &str,
    user: &heca_config::keys::KeybindingMap,
    defaults: &heca_config::keys::KeybindingMap,
) -> Option<String> {
    let value = user.get(name).or_else(|| defaults.get(name))?;
    let all: Vec<String> = value
        .keys()
        .into_iter()
        .map(|key| format_shortcut(key, key.starts_with("prefix+")))
        .collect();
    if all.is_empty() {
        None
    } else {
        Some(all.join(" / "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepends_prefix_symbol() {
        assert_eq!(format_shortcut("f", true), format!("{PREFIX_SYMBOL} f"));
        assert_eq!(format_shortcut("Shift+c", true), format!("{PREFIX_SYMBOL} Shift+c"));
    }

    #[test]
    fn accepts_config_form() {
        // A raw keymap binding ("prefix+f") renders the same as the bare key.
        assert_eq!(format_shortcut("prefix+f", true), format_shortcut("f", true));
    }

    #[test]
    fn global_binding_has_no_prefix() {
        assert_eq!(format_shortcut("Ctrl+H", false), "Ctrl+H");
    }
}
