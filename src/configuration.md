# Configuration

heca loads config from `~/.config/heca/config.toml` (Linux/macOS) or `%APPDATA%\heca\config.toml` (Windows).

## Config File Format

The config uses TOML with a flat `[keys]` table:

```toml
# ~/.config/heca/config.toml

prefix = "ctrl+a"   # Prefix key (default: "ctrl+b")
theme = "mocha"     # Theme name

[settings]
window_width = 1280
window_height = 800
mouse = true
focus_follows_mouse = true
auto_scroll_edge = true
interactive_move_modifier = "Super"

[keys]
# Prefix bindings (checked in Prefix mode)
focus_left = "prefix+h"
focus_right = "prefix+l"

# Global bindings (checked in Normal mode, before terminal forwarding)
Alt+Enter = "spawn_terminal"

# Multiple bindings
focus_left = ["prefix+h", "prefix+ArrowLeft"]

# Unbind defaults
[keys.unbind]
"prefix+f" = true

# Spawn external commands
[[keys.command]]
keys = "prefix+g"
command = "lazygit"

# Custom modes
[[keys.mode]]
name = "resize"
trigger = "prefix+r"
sticky = true

[[keys.mode.bindings]]
action = "resize"
keys = "h"
args = { target = "column", axis = "x", amount = "-50" }
```

## Config Loading

1. `~/.config/heca/config.toml` (user config, optional)
2. Built-in defaults from `heca-config/src/theme.rs`
3. User config **overrides** defaults (same key replaces)
4. `keys.unbind` removes specific defaults
5. Default modes are **always merged** with user modes (user modes override same name)

## Unbinding Keybindings

To remove a default binding, add it to `[keys.unbind]`:

```toml
[keys.unbind]
"prefix+f" = true        # Remove float toggle
"prefix+q" = true        # Remove pane select
"prefix+Shift+q" = true  # Remove swap pane
```

**How it works:**
- During config loading, all defaults are bound first
- Then `[keys.unbind]` entries are processed
- `keymap.unbind("normal", &combo)` removes the binding from the normal mode keymap
- If the combo was also bound in global mode, it is removed from there too
- The action itself still exists — you can rebind it to a different combo

**Use cases:**
- Free up a key for a custom binding
- Disable features you don't use
- Resolve conflicts between default and custom bindings

## Config Reload

`WmAction::ReloadConfig` triggers `reload_config()` on `HecaApp`:
- Rebuilds keymaps from config file
- Reloads theme
- Updates settings (mouse, focus_follows_mouse, etc.)
- Does NOT restart the app

## Theme

Built-in themes: `mocha` (dark), `latte` (light). Create custom themes in `~/.config/heca/themes/mytheme.toml`:

```toml
name = "My Theme"
background = "#1e1e2e"
foreground = "#cdd6f4"
border = "#313244"
accent = "#89b4fa"
border_radius = 6.0
border_width = 1.0

[shadow]
color = "#000000"
alpha = 0.3
blur = 8.0
```

Fonts are **not** part of the color theme (they are system-local, not
theme-portable). Configure them in the dedicated `[font]` block:

```toml
[font.family.ui]
normal = "Geist Mono"

[font.family.terminal]
normal = "Maple Mono Normal NF"

[font.size]
ui = 15.0
terminal = 14.0
```

## Settings

```toml
[settings]
window_width = 1280           # Initial window width
window_height = 800           # Initial window height
mouse = true                  # Enable mouse interactions
focus_follows_mouse = true    # Focus pane on hover
auto_scroll_edge = true       # Auto-scroll near edges
interactive_move_modifier = "Super"  # Modifier for drag-and-drop
```
