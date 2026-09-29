//! The small word lists an action argument can take — each an enum, its accepted spellings
//! ([`EnumArg::VALUES`]) and its parser, read and changed together.

use crate::args::EnumArg;

/// Target for resize actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeTarget {
    Column,
    Pane,
}

impl EnumArg for ResizeTarget {
    const VALUES: &'static [&'static str] = &["column", "col", "pane"];
}

impl std::str::FromStr for ResizeTarget {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "column" | "col" => Ok(ResizeTarget::Column),
            "pane" => Ok(ResizeTarget::Pane),
            _ => Err(format!("unknown resize target: {}", s)),
        }
    }
}

/// **Which edge of the target a resize moves.**
///
/// A pane has two horizontal edges and a resize has to move one of them. Until now it was always
/// the one below (the one above for the last pane, which has nothing below it) — so `j`/`k` in
/// resize mode could grow a pane downwards but never move its top edge (Antonio, 2026-09-03:
/// *"so he can choose which edge moves"*).
///
/// It is an **argument on the existing `resize` action**, not four new actions. `resize_top` and
/// friends would each re-state what `resize` already does and then diverge; one action with an edge
/// keeps a single code path, and a binding says which edge it wants:
///
/// ```toml
/// [[keys.mode.bindings]]
/// action = "resize"
/// keys = "Shift+k"
/// args = { target = "pane", axis = "y", amount = "-50", edge = "top" }
/// ```
///
/// Omit it and nothing changes — [`Auto`](Self::Auto) is what every existing binding gets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ResizeEdge {
    /// The edge the target already owned: for a pane, the one below it — or above, when it is last
    /// and has nothing below. The default, so an unchanged binding behaves exactly as before.
    #[default]
    Auto,
    /// The pane's **upper** edge. Positive is still *down the screen*, so a positive amount shrinks
    /// the pane from the top and a negative one grows it upwards. No-op for the first pane.
    Top,
    /// The pane's **lower** edge — the same boundary [`Auto`](Self::Auto) takes, said explicitly,
    /// and without the last pane's fallback to the edge above.
    Bottom,
    /// The target's **left** edge. A column's left edge is the right edge of the column before it,
    /// so this moves that boundary: positive is still *right*, shrinking the active column from the
    /// left. No-op for the first column, which has nothing to its left.
    Left,
    /// The target's **right** edge — a column's own, which is the one [`Auto`](Self::Auto) moves.
    Right,
}

impl EnumArg for ResizeEdge {
    const VALUES: &'static [&'static str] = &["auto", "top", "bottom", "left", "right"];
}

impl std::str::FromStr for ResizeEdge {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(ResizeEdge::Auto),
            "top" | "up" => Ok(ResizeEdge::Top),
            "bottom" | "down" => Ok(ResizeEdge::Bottom),
            "left" => Ok(ResizeEdge::Left),
            "right" => Ok(ResizeEdge::Right),
            _ => Err(format!("unknown resize edge: {}", s)),
        }
    }
}

/// Direction of a terminal font-zoom step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontZoomStep {
    /// Increase the font size by one step.
    In,
    /// Decrease the font size by one step.
    Out,
    /// Reset to the base size (global → configured size; pane → follow global).
    Reset,
}

impl EnumArg for FontZoomStep {
    const VALUES: &'static [&'static str] =
        &["in", "increase", "+", "out", "decrease", "-", "reset", "0"];
}

impl std::str::FromStr for FontZoomStep {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "in" | "increase" | "+" => Ok(FontZoomStep::In),
            "out" | "decrease" | "-" => Ok(FontZoomStep::Out),
            "reset" | "0" => Ok(FontZoomStep::Reset),
            _ => Err(format!("unknown font zoom step: {}", s)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpawnKind {
    Terminal,
    App,
    Plugin,
}

impl EnumArg for SpawnKind {
    const VALUES: &'static [&'static str] = &["terminal", "app", "plugin"];
}

impl std::str::FromStr for SpawnKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "terminal" => Ok(SpawnKind::Terminal),
            "app" => Ok(SpawnKind::App),
            "plugin" => Ok(SpawnKind::Plugin),
            _ => Err(format!("unknown spawn kind: {}", s)),
        }
    }
}

/// **Shown, hidden, or the other of the two** — what `set_region_visible` does to a region.
/// Omitted, it toggles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RegionVisibility {
    Show,
    Hide,
    #[default]
    Toggle,
}

impl RegionVisibility {
    /// Whether the region is shown afterwards, given whether it is shown now.
    pub fn apply(self, shown: bool) -> bool {
        match self {
            RegionVisibility::Show => true,
            RegionVisibility::Hide => false,
            RegionVisibility::Toggle => !shown,
        }
    }
}

impl EnumArg for RegionVisibility {
    const VALUES: &'static [&'static str] = &["show", "hide", "toggle"];
}

impl std::str::FromStr for RegionVisibility {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "show" => Ok(RegionVisibility::Show),
            "hide" => Ok(RegionVisibility::Hide),
            "toggle" => Ok(RegionVisibility::Toggle),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every spelling an [`EnumArg`] advertises is one its own parser accepts.
    ///
    /// The vocabulary an action declares comes from `VALUES`, and `VALUES` sits beside the
    /// `FromStr` it describes — but "beside" is a habit, not a guarantee. This is the guarantee.
    #[test]
    fn every_enum_arg_value_parses() {
        fn check<T: EnumArg>(what: &str) {
            for value in T::VALUES {
                assert!(
                    value.parse::<T>().is_ok(),
                    "{what} advertises {value:?}, which its own FromStr rejects",
                );
            }
        }
        check::<ResizeTarget>("ResizeTarget");
        check::<FontZoomStep>("FontZoomStep");
        check::<SpawnKind>("SpawnKind");
        check::<crate::chrome::RegionId>("RegionId");
    }
}
