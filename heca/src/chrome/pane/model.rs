//! One pane, reduced to the plain data its shell renders from.
//!
//! Nothing here knows about `AppState`, a window or a GPU: [`super::mod`] gathers these from the
//! session and hands them down, which is what lets every component below be tested headless
//! (AGENTS.md § 0b-bis rule 4).

use heca_core::layout::PaneId;

/// Everything the pane shell draws from, for one pane.
///
/// The rect is **given**, never computed: zoom, float, split and remove are performed by
/// `heca-core`'s layout engine through `ActionRegistry` handlers, and the shell only reflects the
/// result. A component that computed its own geometry would be taking over the layout engine's job
/// (AGENTS.md § 0b).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PaneShellModel {
    pub(crate) pane_id: PaneId,
    /// Absolute logical-pixel rect the layout engine placed this pane at.
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
    /// This pane holds the keyboard.
    pub(crate) active: bool,
    /// Frame style from `[appearance.pane] border_style`.
    pub(crate) frame: heca_config::appearance::BorderStyle,
    pub(crate) border_color: [f32; 4],
    pub(crate) border_width: f32,
    pub(crate) border_radius: f32,
    /// Inner padding reserved for the content that draws inside the frame.
    pub(crate) content_inset: f32,
    /// The **theme accent**, for the pick keycap.
    ///
    /// Carried explicitly because the pane is painted with a doctored theme whose `accent` is set
    /// to *this pane's border colour* — so a keycap left on the default tint comes out dim on an
    /// inactive pane and unreadable. A letter says "press this to go here"; it is not a pane-state
    /// colour, so it must not follow the border.
    pub(crate) accent: [f32; 4],
}

impl PaneShellModel {
    /// **What the built tree depends on**, so the retained tree is rebuilt only when it changes.
    ///
    /// The rect is deliberately absent: a moved or resized pane is re-laid-out and re-positioned
    /// every frame, which is far cheaper than rebuilding, and rebuilding on every pixel of a drag
    /// would throw away the widget signals mid-gesture. Same split
    /// [`crate::chrome::pane_header_key`] makes.
    pub(crate) fn key(&self) -> String {
        format!(
            "{}|{}|{:?}|{:?}|{}|{}|{}|{:?}",
            self.pane_id.0,
            self.active,
            self.frame,
            self.border_color,
            self.border_width,
            self.border_radius,
            self.content_inset,
            self.accent,
        )
    }
}
