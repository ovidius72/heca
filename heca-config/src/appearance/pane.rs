//! **The pane's look** — `[appearance.pane]`: its header's parts and buttons.

use super::*;

/// The chips the pane info bar shows until the user says otherwise: `location` and `app_name`.
///
/// A chip is **named**, not typed: this list holds names, and which names exist is answered in the
/// app, where a program built on heca can add its own. The names heca ships are `location` (working
/// directory, home-relative), `app_name` (the running program — never a rename), `pane_name` (the
/// pane's own name: a rename wins, else the program), `git_branch` and `git_status` (`+A ~M -D`).
/// A chip with nothing to say for a pane is skipped.
pub fn default_pane_title_segments() -> Vec<String> {
    vec!["location".to_string(), "app_name".to_string()]
}

/// The action buttons the pane info bar shows until the user says otherwise: `split` and `close`.
///
/// Move-left/right are intentionally omitted — panes are already movable with the mouse (drag).
/// `move_left` / `move_right` remain valid names for users who want them.
///
/// A button is **named**, not typed: this list holds names, and which names exist is answered in the
/// app, where a program built on heca can add its own (`demo.show_notes`). The same spellings the
/// closed list used, so every existing config keeps working.
pub fn default_pane_title_actions() -> Vec<String> {
    vec!["split".to_string(), "close".to_string()]
}

/// Pane frame + info-bar appearance (`[appearance.pane]`). Every border field is
/// optional; when unset it inherits the global `[appearance]` default, which in
/// turn falls back to the theme. (Surface → global → theme.)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaneAppearance {
    /// Frame style. `None` → [`BorderStyle::Bordered`]. `none | bordered | bracketed`.
    #[serde(default)]
    pub border_style: Option<BorderStyle>,
    /// Border width (logical px). `None` → global `border_width` → theme. Clamped `[0, 10]`.
    #[serde(default)]
    pub border_width: Option<f32>,
    /// Inactive/unfocused border color. `None` → global `border_color` → `theme.border` @50%.
    #[serde(default)]
    pub border_color: Option<Color>,
    /// Corner radius (logical px). `None` → global `border_radius` → theme. Clamped `[0, 20]`.
    #[serde(default)]
    pub border_radius: Option<f32>,
    /// Focused/active border color. `None` → `theme.accent`.
    #[serde(default)]
    pub active_border_color: Option<Color>,
    /// Floating-pane border color (all floating panes, so they read as a distinct
    /// layer). `None` → `theme.float_accent`.
    #[serde(default)]
    pub floating_border_color: Option<Color>,
    /// Gap between tiled panes (logical px). `None` → 8.0.
    #[serde(default)]
    pub gap: Option<f32>,
    /// Internal padding (content inset from the border, logical px). `None` → theme
    /// `pane_padding`. Clamped `[0, 20]`.
    #[serde(default)]
    pub padding: Option<f32>,
    /// The pane name drawn over a pane while it is dragged: its size as a share of the pane's
    /// shorter side. `None` → 0.25.
    #[serde(default)]
    pub drag_label_size_factor: Option<f32>,
    /// Smallest size (logical px) of that name. `None` → 24.
    #[serde(default)]
    pub drag_label_min_size: Option<f32>,
    /// Largest size (logical px) of that name. `None` → 72.
    #[serde(default)]
    pub drag_label_max_size: Option<f32>,
    /// Info-bar chips (left), by **name**, in order. Empty hides the left side. See
    /// [`default_pane_title_segments`]. A name nothing provides is reported and skipped.
    #[serde(default = "default_pane_title_segments")]
    pub title_segments: Vec<String>,
    /// Info-bar action buttons (right), by **name**, in order. Empty hides the right side. The names
    /// heca ships are `split`, `move_left`, `move_right`, `close`, `zoom` and `float`; a program
    /// built on heca adds its own (`demo.show_notes`). A name nothing provides is reported and skipped.
    #[serde(default = "default_pane_title_actions")]
    pub title_actions: Vec<String>,
}

impl Default for PaneAppearance {
    fn default() -> Self {
        Self {
            border_style: None,
            border_width: None,
            border_color: None,
            border_radius: None,
            active_border_color: None,
            floating_border_color: None,
            gap: None,
            padding: None,
            drag_label_size_factor: None,
            drag_label_min_size: None,
            drag_label_max_size: None,
            title_segments: default_pane_title_segments(),
            title_actions: default_pane_title_actions(),
        }
    }
}
