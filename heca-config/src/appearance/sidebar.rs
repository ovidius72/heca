//! **The sidebar's look** — `[appearance.sidebar]`.

use super::*;

/// Sidebar shell appearance (`[appearance.sidebar]`). Border fields unset →
/// global `[appearance]` default → theme. (Surface → global → theme.)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SidebarAppearance {
    /// Frame style. `None` → [`BorderStyle::Bracketed`]. `none | bordered | bracketed`.
    #[serde(default)]
    pub border_style: Option<BorderStyle>,
    /// Border width (logical px). `None` → global `border_width` → theme. Clamped `[0, 10]`.
    #[serde(default)]
    pub border_width: Option<f32>,
    /// Frame color, for every `border_style`: the line of `bordered`, the brackets of
    /// `bracketed`. `None` → `bordered` takes global `border_color` → `theme.border`; `bracketed`
    /// takes the theme accent.
    #[serde(default)]
    pub border_color: Option<Color>,
    /// Corner radius (logical px). `None` → global `border_radius` → theme. Clamped `[0, 40]`.
    #[serde(default)]
    pub border_radius: Option<f32>,
    /// Shell background fill. `None` → the theme-derived sidebar surface color. The
    /// window/chrome transparency still applies on top.
    #[serde(default)]
    pub background_color: Option<Color>,
    /// Gap between the sidebar and the content area (logical px). `None` → 12.0.
    #[serde(default)]
    pub gap: Option<f32>,
    /// Sidebar width (logical px). `None` → 300; clamped to `[MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH]`.
    #[serde(default)]
    pub width: Option<f32>,
    /// The lines under a pane's name in the sidebar, by **name**, in order. Empty shows the name
    /// alone. See [`default_sidebar_pane_lines`]. A name nothing provides is reported and skipped.
    #[serde(default = "default_sidebar_pane_lines")]
    pub pane_lines: Vec<String>,
}

impl Default for SidebarAppearance {
    fn default() -> Self {
        Self {
            border_style: None,
            border_width: None,
            border_color: None,
            border_radius: None,
            background_color: None,
            gap: None,
            width: None,
            pane_lines: default_sidebar_pane_lines(),
        }
    }
}

/// The pane-row line list from the embedded configuration, until the user supplies their own.
///
/// A line is **named**, not typed: which names exist is answered in the app, where a program built
/// on heca can add its own (`demo.status`).
pub fn default_sidebar_pane_lines() -> Vec<String> {
    static LINES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    LINES.get_or_init(|| {
        let defaults: toml::Value = toml::from_str(crate::loader::CONFIG_DEFAULT)
            .expect("embedded config.default.toml must be valid TOML");
        defaults["appearance"]["sidebar"]["pane_lines"].clone().try_into()
            .expect("embedded appearance.sidebar.pane_lines must be a list of names")
    }).clone()
}
