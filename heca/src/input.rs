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
    ResizeIncrease,
    ResizeDecrease,
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
    MovePaneLeft,
    MovePaneRight,
    PaneHeightIncrease,
    PaneHeightDecrease,
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
        "resize_increase" => Some(WmAction::ResizeIncrease),
        "resize_decrease" => Some(WmAction::ResizeDecrease),
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
        "move_pane_left" => Some(WmAction::MovePaneLeft),
        "move_pane_right" => Some(WmAction::MovePaneRight),
        "pane_height_increase" => Some(WmAction::PaneHeightIncrease),
        "pane_height_decrease" => Some(WmAction::PaneHeightDecrease),
        _ => None,
    }
}

/// Binding priority: lower = checked first. Focus wins over resize on conflicts.
fn action_priority(action: WmAction) -> u8 {
    match action {
        // Navigation (highest priority)
        WmAction::FocusLeft | WmAction::FocusRight |
        WmAction::FocusUp | WmAction::FocusDown |
        WmAction::NextPane | WmAction::PrevPane => 0,
        // Pane management
        WmAction::SplitHorizontal | WmAction::SplitVertical |
        WmAction::Float | WmAction::Scratchpad |
        WmAction::Hide | WmAction::ClosePane |
        WmAction::PaneSelect | WmAction::SwapSelect => 1,
        // Swap
        WmAction::SwapLeft | WmAction::SwapRight |
        WmAction::SwapUp | WmAction::SwapDown |
        WmAction::MovePaneLeft | WmAction::MovePaneRight => 2,
        // Resize (lowest priority — checked last)
        WmAction::ResizeLeft | WmAction::ResizeRight |
        WmAction::ResizeUp | WmAction::ResizeDown |
        WmAction::ResizeIncrease | WmAction::ResizeDecrease |
        WmAction::PaneHeightIncrease | WmAction::PaneHeightDecrease => 3,
        // Other
        _ => 4,
    }
}

#[derive(Clone, Debug)]
struct Binding {
    action: WmAction,
    key: String,
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
        // Sort by priority so focus is checked before resize on conflicts
        bindings.sort_by_key(|b| action_priority(b.action));
        Self { bindings }
    }

    /// Parse a key string like "h", "H", "Ctrl+h", "Ctrl+Shift+l", "Space".
    /// Preserves key case exactly; shift ONLY from explicit "Shift+" modifier.
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
                _ => key = part.to_string(),
            }
        }
        (ctrl, shift, alt, key)
    }

    pub fn resolve(
        &self,
        key_text: &str,
        ctrl: bool,
        _alt: bool,
        shift: bool,
        named: &winit::keyboard::Key,
        phys: &winit::keyboard::PhysicalKey,
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

        // macOS winit often doesn't report shift in modifiers.
        // Infer shift from key_text being any uppercase ASCII character (A-Z, +, _, etc).
        // Physical key names like "KeyH" -> "H" are ALWAYS uppercase — never infer shift from them.
        let key_implies_shift = key.len() == 1
            && key.chars().next().unwrap().is_ascii_uppercase();
        let effective_shift = shift || key_implies_shift;

        for b in &self.bindings {
            let key_match = if b.key.len() == 1 {
                // Single-char: case-insensitive match (handles Shift+Q vs q).
                // Physical key fallback ALWAYS (macOS layouts may produce odd key_text).
                let phys_char = phys_name.chars().next();
                // Map physical key names to their unshifted character (for Shift+ bindings).
                let phys_as_char = match phys_name.as_str() {
                    "Equal" => Some('='),
                    "Minus" => Some('-'),
                    "Comma" => Some(','),
                    "Period" => Some('.'),
                    "Slash" => Some('/'),
                    "Semicolon" => Some(';'),
                    "Quote" => Some('\''),
                    "BracketLeft" => Some('['),
                    "BracketRight" => Some(']'),
                    "Backslash" => Some('\\'),
                    "Backquote" => Some('`'),
                    "Space" => Some(' '),
                    _ => None,
                };
                b.key.eq_ignore_ascii_case(&key)
                    || b.key.eq_ignore_ascii_case(&named_key)
                    || phys_char.map_or(false, |pc| b.key.eq_ignore_ascii_case(&pc.to_string()))
                    || phys_as_char.map_or(false, |c| b.key.eq_ignore_ascii_case(&c.to_string()))
                    || (key.is_empty() && b.key.eq_ignore_ascii_case(&phys_name))
            } else {
                b.key.eq_ignore_ascii_case(&key)
                    || b.key.eq_ignore_ascii_case(&named_key)
                    || b.key.eq_ignore_ascii_case(&phys_name)
            };

            // Exact modifier match using effective_shift (inferred from key text/phys).
            let mod_match = b.ctrl == ctrl && b.shift == effective_shift;

            if key_match && mod_match {
                return Some(b.action);
            }
        }
        None
    }
}
