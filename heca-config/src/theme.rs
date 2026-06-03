use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

// ═══════════════════════════════════════════════════════════════════════════════
//  Color
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_f32x4(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }
}

impl FromStr for Color {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if !s.starts_with('#') { return Err(format!("Color must start with #: got {}", s)); }
        let hex = &s[1..];
        if hex.len() != 6 && hex.len() != 8 {
            return Err(format!("Color hex must be 6 or 8 chars: got {}", hex.len()));
        }
        let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| format!("Invalid red: {}", e))?;
        let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| format!("Invalid green: {}", e))?;
        let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| format!("Invalid blue: {}", e))?;
        let a = if hex.len() >= 8 {
            u8::from_str_radix(&hex[6..8], 16).map_err(|e| format!("Invalid alpha: {}", e))?
        } else { 255 };
        Ok(Color { r, g, b, a })
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
    }
}

impl From<Color> for String { fn from(c: Color) -> Self { c.to_string() } }
impl TryFrom<String> for Color { type Error = String; fn try_from(s: String) -> Result<Self, Self::Error> { Color::from_str(&s) } }

// ═══════════════════════════════════════════════════════════════════════════════
//  Shadow & Theme
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub color: String,
    pub alpha: f32,
    pub blur: f32,
}

impl Default for Shadow {
    fn default() -> Self { Self { color: "#000000".to_string(), alpha: 0.3, blur: 8.0 } }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaneStyle {
    pub border_width: f32,
    pub active_border_width: f32,
    pub border_radius: f32,
    pub border_color: Color,
    pub active_border_color: Color,
    pub glow_color: Color,
    pub glow_radius: f32,
    pub bg_gradient_start: Color,
    pub bg_gradient_end: Color,
    pub shadow_gradient_start: Color,
    pub shadow_gradient_end: Color,
    pub show_pane_title: bool,
    pub pane_title_font_size: f32,
    pub pane_title_color: Color,
    pub pane_title_active_color: Color,
    pub padding: f32,
    pub close_btn_color: Color,
    pub close_btn_hover_color: Color,
}

impl Default for PaneStyle {
    fn default() -> Self {
        Self {
            border_width: 0.2,
            active_border_width: 1.0,
            border_radius: 1.5,
            border_color: Color::new(49, 50, 68, 255),
            active_border_color: Color::new(137, 180, 250, 255),
            glow_color: Color::new(137, 180, 250, 90),
            glow_radius: 5.0,
            bg_gradient_start: Color::new(20, 20, 30, 255),
            bg_gradient_end: Color::new(30, 30, 46, 255),
            shadow_gradient_start: Color::new(0, 0, 0, 150),
            shadow_gradient_end: Color::new(0, 0, 0, 0),
            show_pane_title: true,
            pane_title_font_size: 11.0,
            pane_title_color: Color::new(150, 150, 180, 255),
            pane_title_active_color: Color::new(205, 214, 244, 255),
            padding: 6.0,
            close_btn_color: Color::new(100, 100, 120, 255),
            close_btn_hover_color: Color::new(255, 100, 100, 255),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
    pub accent: Color,
    pub font_family: String,
    pub font_size: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub shadow: Shadow,
    pub pane_style: PaneStyle,
    #[serde(default = "default_float_bg")]     pub float_background: Color,
    #[serde(default = "default_float_accent")] pub float_accent: Color,
    #[serde(default = "default_float_focus")]  pub float_focus: Color,

    // ── Sidebar drag-and-drop colors ──
    #[serde(default = "default_drag_ghost_bg")]    pub sidebar_drag_ghost_bg: Color,
    #[serde(default = "default_drag_ghost_fg")]    pub sidebar_drag_ghost_fg: Color,
    #[serde(default = "default_drag_source_bg")]   pub sidebar_drag_source_bg: Color,
    #[serde(default = "default_drag_source_border")] pub sidebar_drag_source_border: Color,
}

fn default_float_bg()    -> Color { Color::new(49, 50, 68, 255) }
fn default_float_accent() -> Color { Color::new(137, 180, 250, 255) }
fn default_float_focus()  -> Color { Color::new(250, 179, 135, 255) }
fn default_drag_ghost_bg()    -> Color { Color::new(137, 180, 250, 217) } // accent @ 85%
fn default_drag_ghost_fg()    -> Color { Color::new(255, 255, 255, 255) } // white
fn default_drag_source_bg()   -> Color { Color::new(137, 180, 250, 38)  } // accent @ 15%
fn default_drag_source_border() -> Color { Color::new(137, 180, 250, 255) } // accent

impl Default for Theme { fn default() -> Self { Self::catppuccin_mocha() } }

impl Theme {
    pub fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".to_string(),
            background: Color::new(30, 30, 46, 255),
            foreground: Color::new(205, 214, 244, 255),
            border: Color::new(49, 50, 68, 255),
            accent: Color::new(137, 180, 250, 255),
            font_family: "JetBrainsMono Nerd Font".to_string(),
            font_size: 32.0,
            border_radius: 6.0,
            border_width: 1.0,
            shadow: Shadow::default(),
            pane_style: PaneStyle {
                active_border_color: Color::new(137, 180, 250, 255),
                glow_color: Color::new(137, 180, 250, 60),
                bg_gradient_start: Color::new(20, 20, 35, 255),
                bg_gradient_end: Color::new(35, 35, 55, 255),
                ..PaneStyle::default()
            },
            float_background: Color::new(49, 50, 68, 255),
            float_accent: Color::new(137, 180, 250, 255),
            float_focus: Color::new(250, 179, 135, 255),

            sidebar_drag_ghost_bg: Color::new(137, 180, 250, 217),
            sidebar_drag_ghost_fg: Color::new(255, 255, 255, 255),
            sidebar_drag_source_bg: Color::new(137, 180, 250, 38),
            sidebar_drag_source_border: Color::new(137, 180, 250, 255),
        }
    }

    pub fn catppuccin_latte() -> Self {
        Self {
            name: "Catppuccin Latte".to_string(),
            background: Color::new(239, 241, 245, 255),
            foreground: Color::new(76, 79, 105, 255),
            border: Color::new(204, 208, 218, 255),
            accent: Color::new(30, 102, 245, 255),
            font_family: "JetBrainsMono Nerd Font".to_string(),
            font_size: 32.0,
            border_radius: 6.0,
            border_width: 1.0,
            shadow: Shadow { color: "#000000".to_string(), alpha: 0.15, blur: 8.0 },
            pane_style: PaneStyle {
                active_border_color: Color::new(30, 102, 245, 255),
                glow_color: Color::new(30, 102, 245, 60),
                bg_gradient_start: Color::new(245, 245, 250, 255),
                bg_gradient_end: Color::new(230, 230, 240, 255),
                ..PaneStyle::default()
            },
            float_background: Color::new(204, 208, 218, 255),
            float_accent: Color::new(30, 102, 245, 255),
            float_focus: Color::new(230, 126, 34, 255),

            sidebar_drag_ghost_bg: Color::new(30, 102, 245, 217),
            sidebar_drag_ghost_fg: Color::new(255, 255, 255, 255),
            sidebar_drag_source_bg: Color::new(30, 102, 245, 38),
            sidebar_drag_source_border: Color::new(30, 102, 245, 255),
        }
    }

    pub fn load(name: &str) -> Self {
        Self::load_from_disk(name).unwrap_or_else(|| Self::load_bundled(name).unwrap_or_default())
    }

    fn load_from_disk(name: &str) -> Option<Self> {
        let path = config_dir().join("themes").join(format!("{}.toml", name));
        std::fs::read_to_string(path).ok().and_then(|c| toml::from_str(&c).ok())
    }

    fn load_bundled(name: &str) -> Option<Self> {
        let bundled: HashMap<&str, &str> = [
            ("mocha", include_str!("themes/mocha.toml")),
            ("latte", include_str!("themes/latte.toml")),
        ].into_iter().collect();
        toml::from_str(bundled.get(name)?).ok()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Settings
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

fn default_theme()        -> String { "mocha".to_string() }
fn default_mouse()        -> bool   { true }
fn default_window_width() -> u32    { 1280 }
fn default_window_height()-> u32    { 800 }
fn default_prefix_key()   -> String { "ctrl+b".to_string() }
fn default_auto_scroll_edge()    -> bool { true }
fn default_always_center_single_column()  -> bool { false }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsConfig {
    #[serde(default = "default_theme")]         pub theme: String,
    #[serde(default = "default_mouse")]         pub mouse: bool,
    #[serde(default = "default_window_width")]  pub window_width: u32,
    #[serde(default = "default_window_height")] pub window_height: u32,
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
            auto_scroll_edge: true,
            interactive_move_modifier: ModifierKey::default(),
            always_center_single_column: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Keybinding types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BindingValue {
    Single(String),
    Many(Vec<String>),
}

impl BindingValue {
    pub fn keys(&self) -> Vec<&str> {
        match self {
            BindingValue::Single(s) => s.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()).collect(),
            BindingValue::Many(v)   => v.iter().map(|s| s.as_str()).filter(|p| !p.is_empty()).collect(),
        }
    }
}

pub type KeybindingMap = HashMap<String, BindingValue>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandKeybindConfig {
    pub key: String,
    pub command: String,
    #[serde(default = "default_command_type")] pub command_type: String,
}

fn default_command_type() -> String { "pane".to_string() }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ModeBindingConfig {
    pub action: String,
    /// The key(s) for this binding. Also accepts `key` (singular) for convenience.
    #[serde(alias = "key")]
    pub keys: String,
    /// Arguments for parameterized actions.
    /// e.g. `args = { target = "column", axis = "x", amount = "50" }`
    /// → WmAction::Resize { target: Column, axis: X, amount: 50.0 }
    #[serde(default)]
    pub args: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyModeConfig {
    pub name: String,
    pub trigger: String,
    /// If true, stay in this mode until Esc or another mode trigger.
    /// If false, execute one binding and exit to Normal.
    #[serde(default = "default_mode_sticky")]
    pub sticky: bool,
    #[serde(default)] pub bindings: Vec<ModeBindingConfig>,
}

fn default_mode_sticky() -> bool { true }

// ═══════════════════════════════════════════════════════════════════════════════
//  KeysConfig — holds prefix, flat action bindings, commands, and modes
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeysConfig {
    /// The prefix key that activates prefix mode.
    /// Accepts both `prefix` and `prefix_key` for compatibility.
    #[serde(default = "default_prefix_key", alias = "prefix_key")]
    pub prefix: String,
    /// Flat action bindings (any key not named "prefix", "command", "mode", or "unbind").
    #[serde(flatten)]
    pub bindings: KeybindingMap,
    /// Key combos to remove from the keymap (e.g. to free a default binding).
    /// Each entry is a combo string like "prefix+w" or "Alt+1".
    #[serde(default)]
    pub unbind: HashMap<String, bool>,
    /// Custom command bindings.
    #[serde(default)]
    pub command: Vec<CommandKeybindConfig>,
    /// Custom input modes.
    #[serde(default)]
    pub mode: Vec<KeyModeConfig>,
}

impl Default for KeysConfig {
    fn default() -> Self {
        let mut bindings = HashMap::new();
        use BindingValue::*;

        // ── Navigation ──
        bindings.insert("focus_left".to_string(),  Single("prefix+h".to_string()));
        bindings.insert("focus_right".to_string(), Single("prefix+l".to_string()));
        bindings.insert("focus_up".to_string(),    Single("prefix+k".to_string()));
        bindings.insert("focus_down".to_string(),  Single("prefix+j".to_string()));

        // ── Splits ──
        bindings.insert("split_horizontal".to_string(), Single("prefix+Enter".to_string()));
        bindings.insert("split_vertical".to_string(),   Single("prefix+v".to_string()));

        // ── Resize ──
        bindings.insert("resize_increase".to_string(),      Single("prefix+=".to_string()));
        bindings.insert("resize_decrease".to_string(),      Single("prefix+-".to_string()));
        bindings.insert("pane_height_increase".to_string(), Single("prefix+Shift+=".to_string()));
        bindings.insert("pane_height_decrease".to_string(), Single("prefix+Shift+-".to_string()));

        // ── Pane operations ──
        bindings.insert("close".to_string(),  Single("prefix+x".to_string()));
        bindings.insert("float".to_string(),  Single("prefix+f".to_string()));

        // ── Quick select / swap ──
        bindings.insert("pane_select".to_string(),     Single("prefix+q".to_string()));
        // Swap (keyboard): Shift+M swaps panes
        bindings.insert("swap_pane".to_string(),     Single("prefix+Shift+m".to_string()));
        // Swap + focus (keyboard): M swaps and then focuses the swapped pane
        bindings.insert("swap_and_focus_pane".to_string(),  Single("prefix+m".to_string()));

        // ── Tabs ──
        bindings.insert("next_pane".to_string(), Single("prefix+n".to_string()));
        bindings.insert("prev_pane".to_string(), Single("prefix+p".to_string()));

        // ── Sidebars ──
        // Note: sidebar navigation (h/j/k/l) is ONLY active in SidebarNav mode.
        // Do NOT bind them in the normal prefix map — they conflict with focus_left/right/up/down.
        bindings.insert("sidebar_left".to_string(),  Single("prefix+b".to_string()));
        bindings.insert("sidebar_right".to_string(), Single("prefix+.".to_string()));
        bindings.insert("sidebar_focus".to_string(), Single("prefix+e".to_string()));
        bindings.insert("sidebar_expand_toggle".to_string(), Single("prefix+Tab".to_string()));

        // ── Workspace navigation ──
        bindings.insert("workspace_prev".to_string(), Single("prefix+u".to_string()));
        bindings.insert("workspace_next".to_string(), Single("prefix+d".to_string()));

        // ── Focus toggle ──
        bindings.insert("focus_toggle_local".to_string(),  Single("prefix+i".to_string()));
        bindings.insert("focus_toggle_global".to_string(), Single("prefix+Shift+l".to_string()));

        // ── Workspace / naming ──
        bindings.insert("create_workspace".to_string(),  Single("prefix+w".to_string()));
        bindings.insert("rename_workspace".to_string(),  Single("prefix+Shift+w".to_string()));
        bindings.insert("rename_pane".to_string(),       Single("prefix+Shift+p".to_string()));

        // ── Command palette ──
        bindings.insert("command_palette".to_string(), Single("prefix+p".to_string()));

        // ── Config reload ──
        bindings.insert("reload_config".to_string(), Single("prefix+Shift+r".to_string()));

        // ── Move pane to column (NIRI-style) ──
        bindings.insert("move_pane_left".to_string(),  Single("prefix+Ctrl+[".to_string()));
        bindings.insert("move_pane_right".to_string(), Single("prefix+Ctrl+]".to_string()));

        // ── Move column to workspace (vertical movement) ──
        bindings.insert("move_column_up".to_string(),   Single("prefix+Ctrl+k".to_string()));
        bindings.insert("move_column_down".to_string(), Single("prefix+Ctrl+j".to_string()));

        // ── Swap position (Ctrl+nav) ──
        bindings.insert("swap_left".to_string(),  Single("prefix+Ctrl+h".to_string()));
        bindings.insert("swap_right".to_string(), Single("prefix+Ctrl+l".to_string()));
        bindings.insert("swap_up".to_string(),    Single("prefix+Ctrl+k".to_string()));
        bindings.insert("swap_down".to_string(),  Single("prefix+Ctrl+j".to_string()));

        // ── Default resize mode ──
        let mut mode = Vec::new();
        let mut resize_bindings = Vec::new();
        use ModeBindingConfig as Mbc;
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "h".to_string(),
            args: [
                ("target".to_string(), "column".to_string()),
                ("axis".to_string(), "x".to_string()),
                ("amount".to_string(), "-50".to_string()),
            ].into_iter().collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "l".to_string(),
            args: [
                ("target".to_string(), "column".to_string()),
                ("axis".to_string(), "x".to_string()),
                ("amount".to_string(), "50".to_string()),
            ].into_iter().collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "j".to_string(),
            args: [
                ("target".to_string(), "pane".to_string()),
                ("axis".to_string(), "y".to_string()),
                ("amount".to_string(), "-40".to_string()),
            ].into_iter().collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "k".to_string(),
            args: [
                ("target".to_string(), "pane".to_string()),
                ("axis".to_string(), "y".to_string()),
                ("amount".to_string(), "40".to_string()),
            ].into_iter().collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "Left".to_string(),
            args: [
                ("target".to_string(), "column".to_string()),
                ("axis".to_string(), "x".to_string()),
                ("amount".to_string(), "-50".to_string()),
            ].into_iter().collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "Right".to_string(),
            args: [
                ("target".to_string(), "column".to_string()),
                ("axis".to_string(), "x".to_string()),
                ("amount".to_string(), "50".to_string()),
            ].into_iter().collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "Down".to_string(),
            args: [
                ("target".to_string(), "pane".to_string()),
                ("axis".to_string(), "y".to_string()),
                ("amount".to_string(), "-40".to_string()),
            ].into_iter().collect(),
        });
        resize_bindings.push(Mbc {
            action: "resize".to_string(),
            keys: "Up".to_string(),
            args: [
                ("target".to_string(), "pane".to_string()),
                ("axis".to_string(), "y".to_string()),
                ("amount".to_string(), "40".to_string()),
            ].into_iter().collect(),
        });
        mode.push(KeyModeConfig {
            name: "resize".to_string(),
            trigger: "prefix+r".to_string(),
            sticky: true,
            bindings: resize_bindings,
        });

        Self { prefix: default_prefix_key(), bindings, unbind: HashMap::new(), command: Vec::new(), mode }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Config
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)] pub settings: SettingsConfig,
    #[serde(default)] pub keys: KeysConfig,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  AppConfig
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub config: Config,
    pub theme: Theme,
}

impl AppConfig {
    pub fn load() -> Self {
        let config = Self::load_config_file().unwrap_or_else(|e| {
            eprintln!("[config] failed to load: {e}; using defaults");
            Config::default()
        });
        let theme = Theme::load(&config.settings.theme);
        Self { config, theme }
    }

    fn load_config_file() -> Result<Config, String> {
        let paths = [
            dirs::home_dir().map(|h| h.join(".config").join("heca").join("config.toml")),
            Some(config_dir().join("config.toml")),
        ];
        for path in paths.into_iter().flatten() {
            eprintln!("[config] trying: {}", path.display());
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    eprintln!("[config] found file at: {}", path.display());
                    match toml::from_str::<Config>(&content) {
                        Ok(config) => {
                            eprintln!("[config] parsed successfully");
                            eprintln!("[config] prefix='{}' bindings_count={} command_count={} mode_count={}",
                                config.keys.prefix,
                                config.keys.bindings.len(),
                                config.keys.command.len(),
                                config.keys.mode.len());
                            return Ok(config);
                        }
                        Err(e) => {
                            eprintln!("[config] PARSE ERROR in {}: {}", path.display(), e);
                            return Err(format!("parse error in {}: {}", path.display(), e));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[config] not found: {} ({e})", path.display());
                }
            }
        }
        Err("no config file found".to_string())
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("heca")
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex() {
        let c: Color = "#1e1e2e".parse().unwrap();
        assert_eq!(c.r, 30); assert_eq!(c.g, 30); assert_eq!(c.b, 46); assert_eq!(c.a, 255);
    }

    #[test]
    fn test_color_with_alpha() {
        let c: Color = "#ff000080".parse().unwrap();
        assert_eq!(c.r, 255); assert_eq!(c.g, 0); assert_eq!(c.b, 0); assert_eq!(c.a, 128);
    }

    #[test]
    fn test_color_roundtrip() {
        let c1 = Color::new(30, 30, 46, 255);
        let s: String = c1.into();
        let c2 = Color::from_str(&s).unwrap();
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_default_theme_is_mocha() {
        let theme = Theme::default();
        assert_eq!(theme.name, "Catppuccin Mocha");
    }

    #[test]
    fn test_bundled_mocha_theme() {
        assert_eq!(Theme::load("mocha").name, "Catppuccin Mocha");
    }

    #[test]
    fn test_bundled_latte_theme() {
        assert_eq!(Theme::load("latte").name, "Catppuccin Latte");
    }

    #[test]
    fn test_fallback_when_config_missing() {
        let app = AppConfig::load();
        assert_eq!(app.config.settings.theme, "mocha");
        assert_eq!(app.theme.name, "Catppuccin Mocha");
    }

    #[test]
    fn test_binding_value_single() {
        let v = BindingValue::Single("h, Left".to_string());
        assert_eq!(v.keys(), vec!["h", "Left"]);
    }

    #[test]
    fn test_binding_value_many() {
        let v = BindingValue::Many(vec!["h".to_string(), "Left".to_string()]);
        assert_eq!(v.keys(), vec!["h", "Left"]);
    }

    #[test]
    fn test_binding_value_empty_ignored() {
        let v = BindingValue::Single("h, , Left".to_string());
        assert_eq!(v.keys(), vec!["h", "Left"]);
    }

    #[test]
    fn test_config_default_prefix() {
        let cfg = Config::default();
        assert_eq!(cfg.keys.prefix, "ctrl+b");
    }

    #[test]
    fn test_config_has_default_bindings() {
        let cfg = Config::default();
        assert!(cfg.keys.bindings.contains_key("focus_left"));
        assert!(cfg.keys.bindings.contains_key("split_horizontal"));
        assert!(cfg.keys.bindings.contains_key("close"));
    }

    #[test]
    fn test_parse_toml_config() {
        let toml = r#"
[settings]
theme = "mocha"
mouse = true

[keys]
prefix = "ctrl+a"
focus_left = ["h", "Left"]
focus_right = "l"

[[keys.command]]
key = "prefix+Shift+g"
command = "lazygit"

[[keys.mode]]
name = "resize"
trigger = "prefix+r"

[[keys.mode.bindings]]
action = "resize_increase"
keys = "="
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.keys.prefix, "ctrl+a");
        assert_eq!(cfg.keys.bindings.get("focus_left").unwrap().keys(), vec!["h", "Left"]);
        assert_eq!(cfg.keys.bindings.get("focus_right").unwrap().keys(), vec!["l"]);
        assert_eq!(cfg.keys.command.len(), 1);
        assert_eq!(cfg.keys.mode.len(), 1);
        assert_eq!(cfg.keys.mode[0].name, "resize");
        assert_eq!(cfg.keys.mode[0].bindings.len(), 1);
    }
}
