# Modes

Modes are groups of bindings that stay active until `Escape` or `Enter` is pressed. The default resize mode is an example.

## Input Modes

```rust
pub enum InputMode {
    Normal,           // Forward keys to terminal, prefix triggers prefix mode
    Prefix,           // Waiting for action key after prefix
    PaneSelect { candidates: Vec<(char, u64)> },  // Quick-select overlay
    PaneSwap { candidates: Vec<(char, u64)>, focus_after: bool },  // Quick-swap overlay
    SidebarNav,       // Sidebar tree navigation
    Rename { target, buffer },  // Text input for renaming
    Chord { sequence },  // Multi-key chord (e.g., w → digit)
    Mode { name },     // Custom mode (resize, etc.)
}
```

## Mode Triggers

Config defines how to enter modes:

```toml
[[keys.mode]]
name = "resize"
trigger = "prefix+r"
sticky = true   # true = stay until Esc/Enter; false = one-shot (chord)
```

## Sticky vs Non-sticky Modes

- **Sticky** (`sticky = true`): Stay in mode until `Escape` or `Enter`. Resize mode is sticky.
- **Non-sticky** (`sticky = false`, or chord): Execute one action then exit. Like `prefix+w` → `1` creates workspace 1.

## Custom Modes

Define modes under `[[keys.mode]]`:

```toml
[[keys.mode]]
name = "my_mode"
trigger = "prefix+o"      # How to enter the mode
sticky = true             # true = stay until Esc/Enter

[[keys.mode.bindings]]
action = "focus_left"
keys = "h"

[[keys.mode.bindings]]
action = "focus_right"
keys = "l"
```

## Binding Precedence

heca checks bindings in this order:

1. **Mode bindings** — If in a custom mode, check mode-specific keymap
2. **Prefix mode** — If in prefix mode, check "normal" keymap for `prefix+key` combos
3. **Global bindings** — In Normal mode, check "global" keymap for direct `Alt+key` / `Super+key` combos
4. **Forward to terminal** — If no binding matched, send key to focused pane's backend

## KeyCombo

```rust
pub struct KeyCombo {
    pub key: String,      // Normalized key name (lowercase, "enter", "arrowleft")
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_: bool,
}
```

- **Case-insensitive equality**: `"h"` matches `"H"`
- **Shift inference**: `"{"` parses as `"["` + `shift=true`
- **macOS physical key fallback**: When `key_text` is empty (Ctrl produces control char), physical key maps back to printable key
- **Named keys**: `Enter`, `Tab`, `Escape`, `ArrowLeft`, etc.
