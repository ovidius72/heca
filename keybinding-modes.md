# Keybinding Modes — Implementation Analysis

> i3-style persistent modes for heca (e.g., resize mode, move mode)
> Last updated: 2026-05-31

---

## Overview

This document describes how to implement **persistent modal keybinding** in heca, similar to i3's resize mode. The user enters a mode via a keybinding (e.g., `r`), stays in that mode performing mode-specific actions, and exits with `Escape`.

---

## Current Architecture

### InputMode Enum (`app_state.rs`)

```rust
pub enum InputMode {
    Normal,
    Prefix,
    PaneSelect { candidates: Vec<(char, u64)> },  // one-shot
    PaneSwap { candidates: Vec<(char, u64)> },     // one-shot
}
```

### Current Flow

| Mode | Behavior | Exit Condition |
|------|----------|----------------|
| `Normal` | Forward all keys to terminal backend | — |
| `Prefix` | Wait for action key after `Ctrl+B` | Action executed or unbound key |
| `PaneSelect` | Letter overlay; next keypress selects pane | Single keypress |
| `PaneSwap` | Letter overlay; next keypress swaps pane | Single keypress |

### Limitations

- **No persistent modes**: PaneSelect/PaneSwap are one-shot (exit after one action)
- **No mode-specific bindings**: Same keybindings everywhere
- **No visual mode indicator**: Status bar shows "SELECT" but no dedicated feedback

---

## i3-Style Mode Requirements

| Feature | Description |
|---------|-------------|
| **Persistent** | Stay in mode until explicitly exited with `Escape` |
| **Mode-specific bindings** | `h/j/k/l` do resize in resize mode, not focus |
| **Visual feedback** | Status bar shows mode name (e.g., "RESIZE") |
| **No terminal forwarding** | Keys intercepted by WM, not sent to terminal |
| **Multiple modes** | Could have resize, move, scratchpad, etc. |

---

## Proposed Implementation

### 1. Extend InputMode Enum

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Prefix,
    PaneSelect { candidates: Vec<(char, u64)> },
    PaneSwap { candidates: Vec<(char, u64)> },
    /// Persistent mode: stays until Escape is pressed
    PersistentMode {
        mode_name: String,  // "resize", "move", etc.
    },
}
```

**Why `PersistentMode` is separate from `PaneSelect/PaneSwap`:**
- PaneSelect/PaneSwap are one-shot (exit after one keypress)
- PersistentMode stays active until Escape
- Different key resolution logic (mode-specific bindings vs. global bindings)

---

### 2. Add Mode-Specific Keybinding Storage

```rust
pub struct KeyBindings {
    normal_bindings: Vec<Binding>,               // existing
    mode_bindings: HashMap<String, Vec<Binding>>, // NEW: mode_name -> bindings
}
```

**Benefits:**
- Clean separation between normal and mode-specific bindings
- Each mode has its own binding set (no conflicts)
- Easy to add new modes (just add entries to HashMap)

---

### 3. Add New WmAction Variants

```rust
pub enum WmAction {
    // ... existing variants ...

    // Mode entry/exit
    EnterResizeMode,
    EnterMoveMode,
    EnterScratchpadMode,
    // Or more generic:
    // EnterMode(String),  // mode_name
    ExitMode,

    // Resize mode actions
    ResizePaneLeft,
    ResizePaneRight,
    ResizePaneUp,
    ResizePaneDown,

    // Move mode actions
    MovePaneLeft,
    MovePaneRight,
    MovePaneUp,
    MovePaneDown,
}
```

**Why explicit variants instead of `EnterMode(String)`:**
- Type safety: compiler catches missing match arms
- Clear action names in config (`enter_resize_mode` vs `enter_mode("resize")`)
- Easier to document and discover

---

### 4. Config Format

```toml
[keybindings]
# Entry points (Normal mode only)
enter_resize_mode = "r"
enter_move_mode = "m"
exit_mode = "Escape"

[modes.resize]
# Mode-specific bindings
h = "resize_pane_left"
l = "resize_pane_right"
k = "resize_pane_up"
j = "resize_pane_down"
Left = "resize_pane_left"
Right = "resize_pane_right"
Up = "resize_pane_up"
Down = "resize_pane_down"
# Escape is handled by exit_mode binding (or built-in)

[modes.move]
h = "move_pane_left"
l = "move_pane_right"
k = "move_pane_up"
j = "move_pane_down"
Escape = "exit_mode"
```

**Config structure:**
```rust
pub struct Config {
    pub theme: String,
    pub general: GeneralConfig,
    pub keybindings: KeybindingMap,           // Normal mode bindings
    pub modes: HashMap<String, KeybindingMap>, // Mode-specific bindings
}
```

---

### 5. KeyBinding Resolution

```rust
impl KeyBindings {
    /// Resolve a keypress in normal mode (existing)
    pub fn resolve(
        &self,
        key_text: &str,
        ctrl: bool,
        alt: bool,
        shift: bool,
        named: &winit::keyboard::Key,
        phys: &winit::keyboard::PhysicalKey,
    ) -> Option<WmAction> {
        // ... existing logic ...
    }

    /// Resolve a keypress in a specific mode (NEW)
    pub fn resolve_mode(
        &self,
        mode_name: &str,
        key_text: &str,
        ctrl: bool,
        alt: bool,
        shift: bool,
        named: &winit::keyboard::Key,
        phys: &winit::keyboard::PhysicalKey,
    ) -> Option<WmAction> {
        let bindings = self.mode_bindings.get(mode_name)?;
        // Same resolution logic as resolve(), but against mode-specific bindings
        for b in bindings {
            // ... key matching ...
            if key_match && mod_match {
                return Some(b.action);
            }
        }
        None
    }
}
```

---

### 6. Event Handler Changes

```rust
// In window_event match:
InputMode::PersistentMode { mode_name } => {
    // Always check for Escape first
    if matches!(event.logical_key, Key::Named(NamedKey::Escape)) {
        state.input_mode = InputMode::Normal;
        state.needs_redraw = true;
        return;
    }

    // Resolve against mode-specific bindings
    let action = self.bindings.resolve_mode(
        mode_name,
        &key_text,
        is_ctrl,
        false,
        is_shift,
        &event.logical_key,
        &event.physical_key,
    );

    if let Some(act) = action {
        execute_action(act, state.focused_pane, state);
        // Stay in mode! Don't reset to Normal
    }
    // Ignore unbound keys (don't forward to terminal)
}
```

**Key differences from Prefix mode:**
- **No auto-exit on unbound keys**: Stay in mode until Escape
- **No double-prefix handling**: Modes aren't composable
- **Mode-specific resolution**: Use `resolve_mode()` instead of `resolve()`

---

### 7. Status Bar Integration

```rust
let mode_str = match &state.input_mode {
    InputMode::Normal => "NORMAL",
    InputMode::Prefix => "PREFIX",
    InputMode::PaneSelect { .. } => "SELECT",
    InputMode::PaneSwap { .. } => "SWAP",
    InputMode::PersistentMode { mode_name } => {
        // Capitalize mode name for display
        mode_name.to_uppercase()
    }
};
```

**Optional visual enhancements:**
- Change status bar background color in mode (e.g., red for resize)
- Show mode name in center overlay (like i3)
- Add mode indicator to tab bar

---

## Example: Resize Mode Flow

```
┌─────────────────────────────────────────────────────────────┐
│ 1. User presses 'r' in Normal mode                          │
│    → EnterResizeMode action triggered                       │
│    → InputMode::PersistentMode { mode_name: "resize" }     │
│    → Status bar shows "RESIZE"                              │
├─────────────────────────────────────────────────────────────┤
│ 2. User presses 'h'                                         │
│    → resolve_mode("resize", "h", ...) → ResizePaneLeft     │
│    → execute_action(ResizePaneLeft, ...)                    │
│    → Pane width decreases by 5%                             │
│    → Stay in resize mode                                    │
├─────────────────────────────────────────────────────────────┤
│ 3. User presses 'j'                                         │
│    → resolve_mode("resize", "j", ...) → ResizePaneDown     │
│    → execute_action(ResizePaneDown, ...)                    │
│    → Pane height decreases                                  │
│    → Stay in resize mode                                    │
├─────────────────────────────────────────────────────────────┤
│ 4. User presses Escape                                      │
│    → ExitMode action (or built-in escape handling)          │
│    → InputMode::Normal                                      │
│    → Status bar shows "NORMAL"                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Edge Cases to Handle

| Case | Question | Recommendation |
|------|----------|----------------|
| **Prefix → Mode** | Should `Ctrl+B → r` enter resize mode? | No. Prefix is for one-shot actions. Only bare `r` in Normal mode. |
| **Mode → Prefix** | Can user press `Ctrl+B` while in resize mode? | No. Modes are modal, not composable. |
| **Mode → Mode** | Can user enter move mode from resize mode? | No. Must exit to Normal first. |
| **Timeout** | Should modes auto-exit after inactivity? | No. User explicitly exits with Escape. |
| **Escape in Normal** | Should Escape do anything in Normal mode? | No. Only exits modes. |

---

## Implementation Steps

| Step | Task | Files | Notes |
|------|------|-------|-------|
| 1 | Extend `InputMode` enum with `PersistentMode` | `app_state.rs` | Add variant with `mode_name: String` |
| 2 | Add `mode_bindings` HashMap to `KeyBindings` | `input.rs` | `HashMap<String, Vec<Binding>>` |
| 3 | Add `resolve_mode()` method to `KeyBindings` | `input.rs` | Same logic as `resolve()`, different bindings |
| 4 | Add mode parsing in `Config` and `AppConfig` | `theme.rs` | `modes: HashMap<String, KeybindingMap>` |
| 5 | Add `EnterResizeMode`, `ExitMode` to `WmAction` | `input.rs` | Plus mode-specific actions |
| 6 | Implement `action_from_name()` for new variants | `input.rs` | Keep in sync with config |
| 7 | Implement mode entry/exit in `execute_action()` | `main.rs` | Set `input_mode` to `PersistentMode` |
| 8 | Add mode-specific keybinding resolution in event loop | `main.rs` | Handle `PersistentMode` case |
| 9 | Update status bar to show mode name | `main.rs` | Match on `PersistentMode` |
| 10 | Add default mode configs (resize, move) | `theme.rs` | Ship with sensible defaults |
| 11 | Test prefix → mode → escape flow | Manual | Verify no conflicts |

---

## Benefits of This Design

1. **Familiar**: Works exactly like i3/sway
2. **Extensible**: Easy to add new modes (move, scratchpad, etc.)
3. **Clean separation**: Mode bindings don't pollute normal bindings
4. **Type-safe**: Each mode has its own binding set; compiler catches errors
5. **Configurable**: Users can customize mode keybindings in TOML
6. **Visual feedback**: Status bar clearly shows current mode

---

## Future Extensions

### Additional Modes

| Mode | Entry Key | Actions |
|------|-----------|---------|
| **resize** | `r` | h/j/k/l → resize pane dimensions |
| **move** | `m` | h/j/k/l → move pane between columns/positions |
| **scratchpad** | `s` | s → show/hide scratchpad, number keys select scratchpad |
| **layout** | `L` | h/v/t → horizontal/vertical/tabbed layout |

### Advanced Features

- **Mode indicator overlay**: Show "RESIZE MODE" in center of screen (toggle)
- **Mode timeout**: Auto-exit after N seconds of inactivity (optional)
- **Nested modes**: Allow `resize → move` without exiting (complex, optional)
- **Mode-specific cursor shapes**: Change mouse cursor in modes (optional)

---

## Config Example (Full)

```toml
[theme]
name = "mocha"

[general]
window_width = 1280
window_height = 800

[keybindings]
# Normal mode bindings
focus_left = "h"
focus_right = "l"
focus_up = "k"
focus_down = "j"
split_horizontal = "Enter"
split_vertical = "v"
close = "x"
float = "f"
pane_select = "q"
swap_select = "Shift+q"

# Mode entry points
enter_resize_mode = "r"
enter_move_mode = "m"

[modes.resize]
h = "resize_pane_left"
l = "resize_pane_right"
k = "resize_pane_up"
j = "resize_pane_down"
Left = "resize_pane_left"
Right = "resize_pane_right"
Up = "resize_pane_up"
Down = "resize_pane_down"

[modes.move]
h = "move_pane_left"
l = "move_pane_right"
k = "move_pane_up"
j = "move_pane_down"
Left = "move_pane_left"
Right = "move_pane_right"
Up = "move_pane_up"
Down = "move_pane_down"
```

---

## Summary

Implementing i3-style persistent modes requires:

1. **Extend `InputMode`** with `PersistentMode { mode_name }`
2. **Add mode-specific bindings** in `KeyBindings.mode_bindings`
3. **Add `resolve_mode()`** for mode-aware key resolution
4. **Handle `PersistentMode`** in event loop (stay in mode until Escape)
5. **Update status bar** to show current mode name

The result is a clean, extensible system that feels familiar to i3/sway users while fitting naturally into heca's existing architecture.
