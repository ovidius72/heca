use heca_config::theme::AppConfig;
use winit::keyboard::NamedKey;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WmAction {
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    SplitHorizontal,
    SplitVertical,
    Float,
    Scratchpad,
    Hide,
    ClosePane,
    TabNext,
    TabPrev,
    ResizeLeft,
    ResizeRight,
    ResizeUp,
    ResizeDown,
    SidebarLeft,
    SidebarRight,
    NextPane,
    PrevPane,
    PaneSelect,
    SwapSelect,
    SwapLeft,
    SwapRight,
    SwapUp,
    SwapDown,
}

fn action_from_name(name: &str) -> Option<WmAction> {
    match name {
        "focus_left" => Some(WmAction::FocusLeft),
        "focus_right" => Some(WmAction::FocusRight),
        "focus_up" => Some(WmAction::FocusUp),
        "focus_down" => Some(WmAction::FocusDown),
        "split_horizontal" => Some(WmAction::SplitHorizontal),
        "split_vertical" => Some(WmAction::SplitVertical),
        "float" => Some(WmAction::Float),
        "scratchpad" => Some(WmAction::Scratchpad),
        "hide" => Some(WmAction::Hide),
        "close" => Some(WmAction::ClosePane),
        "tab_next" => Some(WmAction::TabNext),
        "tab_prev" => Some(WmAction::TabPrev),
        "resize_left" => Some(WmAction::ResizeLeft),
        "resize_right" => Some(WmAction::ResizeRight),
        "resize_up" => Some(WmAction::ResizeUp),
        "resize_down" => Some(WmAction::ResizeDown),
        "sidebar_left" => Some(WmAction::SidebarLeft),
        "sidebar_right" => Some(WmAction::SidebarRight),
        "next_pane" => Some(WmAction::NextPane),
        "prev_pane" => Some(WmAction::PrevPane),
        "pane_select" => Some(WmAction::PaneSelect),
        "swap_select" => Some(WmAction::SwapSelect),
        "swap_left" => Some(WmAction::SwapLeft),
        "swap_right" => Some(WmAction::SwapRight),
        "swap_up" => Some(WmAction::SwapUp),
        "swap_down" => Some(WmAction::SwapDown),
        _ => None,
    }
}

/// Parsed keybinding entry.
#[derive(Clone, Debug)]
struct Binding {
    action: WmAction,
    key: String,          // e.g. "h", "Space", "-"
    ctrl: bool,
    shift: bool,
    #[allow(dead_code)]
    alt: bool,
}

pub struct KeyBindings {
    bindings: Vec<Binding>,
}

impl KeyBindings {
    pub fn load(app_config: &AppConfig) -> Self {
        let mut bindings = Vec::new();
        for (name, key_str) in &app_config.config.keybindings {
            if let Some(action) = action_from_name(name) {
                for part in key_str.split(',') {
                    let part = part.trim();
                    if part.is_empty() { continue; }
                    let (ctrl, shift, alt, key) = Self::parse_key(part);
                    bindings.push(Binding { action, key, ctrl, shift, alt });
                }
            }
        }
        Self { bindings }
    }

    /// Parse a key string like "h", "H", "Ctrl+h", "Ctrl+Shift+l", "Space".
    fn parse_key(s: &str) -> (bool, bool, bool, String) {
        let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut key = String::new();
        for part in &parts {
            match part.to_lowercase().as_str() {
                "ctrl" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                other => key = other.to_string(),
            }
        }
        // Single uppercase letter implies shift unless explicit
        if key.len() == 1 && key.chars().next().unwrap().is_ascii_uppercase() && parts.len() == 1 {
            shift = true;
        }
        (ctrl, shift, alt, key)
    }

    /// Resolve a key press to an action.
    /// `in_prefix`: when true, also tries with ctrl=false if exact match fails
    ///              (handles macOS where modifier-release event may be delayed).
    pub fn resolve(
        &self,
        key_text: &str,
        ctrl: bool,
        _alt: bool,
        shift: bool,
        named: &winit::keyboard::Key,
        phys: &winit::keyboard::PhysicalKey,
        in_prefix: bool,
    ) -> Option<WmAction> {
        let key = key_text.to_string();
        let named_key = match named {
            winit::keyboard::Key::Named(NamedKey::Space) => "Space".to_string(),
            winit::keyboard::Key::Named(n) => format!("{:?}", n),
            _ => key.clone(),
        };

        let phys_name = match phys {
            winit::keyboard::PhysicalKey::Code(c) => {
                let s = format!("{:?}", c);
                s.strip_prefix("Key").unwrap_or(&s).to_string()
            }
            _ => String::new(),
        };

        // Helper: try once with given ctrl value
        let try_resolve = |try_ctrl: bool| -> Option<WmAction> {
            for b in &self.bindings {
                // Single-char bindings: case-sensitive text, case-insensitive phys fallback
                let key_match = if b.key.len() == 1 {
                    b.key == key || b.key.eq_ignore_ascii_case(&phys_name)
                } else {
                    b.key.eq_ignore_ascii_case(&key)
                        || b.key.eq_ignore_ascii_case(&named_key)
                        || b.key.eq_ignore_ascii_case(&phys_name)
                };
                let ctrl_match = b.ctrl == try_ctrl;
                let shift_match = b.shift == shift;
                if key_match && ctrl_match && shift_match {
                    return Some(b.action);
                }
            }
            None
        };

        // First: exact match (Ctrl+hjkl → swap)
        if let Some(action) = try_resolve(ctrl) {
            return Some(action);
        }
        // Second: in prefix mode, try with ctrl=false (hjkl → focus)
        // This handles macOS where modifier-release may be delayed after prefix
        if in_prefix && ctrl {
            if let Some(action) = try_resolve(false) {
                return Some(action);
            }
        }
        None
    }
}
