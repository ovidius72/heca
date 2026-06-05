use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  ModifierKey
// ═══════════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════════
//  Default-value helpers (used by serde attributes on SettingsConfig)
// ═══════════════════════════════════════════════════════════════════════════════

fn default_theme() -> String {
    "mocha".to_string()
}

fn default_mouse() -> bool {
    true
}

fn default_window_width() -> u32 {
    1280
}

fn default_window_height() -> u32 {
    800
}

fn default_auto_scroll_edge() -> bool {
    true
}

fn default_always_center_single_column() -> bool {
    false
}

// ═══════════════════════════════════════════════════════════════════════════════
//  SettingsConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// User-configurable settings that control behaviour and appearance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_mouse")]
    pub mouse: bool,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    /// Automatically scroll the workspace view when the pointer hovers near the left/right edge.
    #[serde(default = "default_auto_scroll_edge")]
    pub auto_scroll_edge: bool,
    /// Modifier key that must be held to initiate an interactive pane drag with the mouse.
    #[serde(default)]
    pub interactive_move_modifier: ModifierKey,
    /// Center a single column even when it fits within the viewport.
    #[serde(default = "default_always_center_single_column")]
    pub always_center_single_column: bool,
}

impl Default for SettingsConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            mouse: default_mouse(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            auto_scroll_edge: default_auto_scroll_edge(),
            interactive_move_modifier: ModifierKey::default(),
            always_center_single_column: default_always_center_single_column(),
        }
    }
}
