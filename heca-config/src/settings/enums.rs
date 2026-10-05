//! The small enums a setting can be: a modifier key, when to centre a column, how big the palette
//! is, how a search treats case.

use serde::{Deserialize, Serialize};

/// Modifier keys that can be used for mouse-driven interactive actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ModifierKey {
    /// Super / Command / Windows key.
    #[default]
    Super,
    /// Alt / Option key.
    Alt,
    /// Control key (also accepts "Control" in config).
    #[serde(alias = "Control")]
    Ctrl,
    /// Shift key.
    Shift,
}

/// When the workspace view centres the focused column, mirroring
/// `heca_core::layout::CenterFocusedColumn`.
///
/// Mirrored rather than shared because `heca-config` depends on nothing but the theme — the same
/// arrangement every other layout value here uses, with `startup.rs` converting at the boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CenterFocusedColumn {
    /// Never centre: focusing an off-screen column scrolls it to the nearest edge. **The default**,
    /// matching niri's, and the one that keeps the view where you put it.
    #[default]
    Never,
    /// Centre a column only when it cannot fit on screen beside the previously focused one.
    OnOverflow,
    /// The focused column is always centred.
    Always,
}

/// How roomy the command palette is — `small` / `normal` / `large`, as in Zed.
///
/// It decides the panel's **maximum width and how many rows it shows**, not the text size: the
/// palette is read at the same size whatever its width. On a screen too small for the chosen
/// variant the window wins — the panel is capped at a fraction of the viewport, so `large` on a
/// laptop is simply as large as fits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaletteSize {
    /// 560px, 6 rows.
    Small,
    /// 700px, 8 rows.
    #[default]
    Normal,
    /// 1000px, 12 rows.
    Large,
}

/// How a search query's case is treated — the command palette today, and any search surface that
/// follows it.
///
/// Three, because the right answer is a habit rather than a fact: `smart` is what fzf and ripgrep
/// do and suits most people, `sensitive` suits anyone whose command names differ only by case, and
/// `insensitive` suits anyone who never wants Shift to change what they find.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchCase {
    /// Insensitive until the query contains an uppercase character, then sensitive.
    #[default]
    Smart,
    /// Always case-sensitive.
    Sensitive,
    /// Never case-sensitive.
    Insensitive,
}
