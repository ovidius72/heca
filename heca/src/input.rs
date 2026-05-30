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
    #[allow(dead_code)]
    SidebarRight,
    NextPane,
    PrevPane,
    PaneSelect,
}

#[allow(dead_code)]
pub struct KeyBindings {
    pub prefix: String,
    pub bindings: Vec<(String, String, WmAction)>, // (key, mods, action)
}

#[allow(dead_code)]
impl KeyBindings {
    pub fn load(_app_config: &AppConfig) -> Self {
        // Default bindings (Ctrl+B prefix, then single key)
        Self {
            prefix: "Ctrl+B".to_string(),
            bindings: vec![
                ("h".to_string(), "".to_string(), WmAction::FocusLeft),
                ("j".to_string(), "".to_string(), WmAction::FocusDown),
                ("k".to_string(), "".to_string(), WmAction::FocusUp),
                ("l".to_string(), "".to_string(), WmAction::FocusRight),
                ("-".to_string(), "".to_string(), WmAction::SplitHorizontal),
                ("v".to_string(), "".to_string(), WmAction::SplitVertical),
                ("f".to_string(), "".to_string(), WmAction::Float),
                ("s".to_string(), "".to_string(), WmAction::Scratchpad),
                ("z".to_string(), "".to_string(), WmAction::Hide),
                ("x".to_string(), "".to_string(), WmAction::ClosePane),
                ("]".to_string(), "".to_string(), WmAction::TabNext),
                ("[".to_string(), "".to_string(), WmAction::TabPrev),
                ("h".to_string(), "Shift".to_string(), WmAction::ResizeLeft),
                ("l".to_string(), "Shift".to_string(), WmAction::ResizeRight),
                ("k".to_string(), "Shift".to_string(), WmAction::ResizeUp),
                ("j".to_string(), "Shift".to_string(), WmAction::ResizeDown),
                ("Space".to_string(), "".to_string(), WmAction::SidebarLeft),
                ("n".to_string(), "".to_string(), WmAction::NextPane),
                ("p".to_string(), "".to_string(), WmAction::PrevPane),
                ("q".to_string(), "".to_string(), WmAction::PaneSelect),
            ],
        }
    }

    pub fn resolve(
        &self,
        key_text: &str,
        _ctrl: bool,
        _alt: bool,
        shift: bool,
        named: &winit::keyboard::Key,
    ) -> Option<WmAction> {
        let mod_str = if shift { "Shift" } else { "" };
        let key = key_text.to_string();

        // Direct text match first
        for (k, m, action) in &self.bindings {
            if *k == key && *m == mod_str {
                return Some(*action);
            }
        }

        // Named keys
        if let winit::keyboard::Key::Named(named_key) = named {
            let name = match named_key {
                NamedKey::Space => "Space".to_string(),
                _ => format!("{:?}", named_key),
            };
            for (k, m, action) in &self.bindings {
                if *k == name && *m == mod_str {
                    return Some(*action);
                }
            }
        }

        None
    }
}
