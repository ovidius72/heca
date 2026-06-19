# Keybindings Reference

This file documents ALL configurable keybindings in heca. See `default-keybindings.toml` for the repo reference copy and copy the bindings you want into your `~/.config/heca/config.toml`.

## Syntax

```toml
action_name = "combo"                    # Single binding
action_name = ["combo1", "combo2"]       # Multiple bindings
```

## Combo Syntax

```
prefix+h           → Press prefix, then h
prefix+Shift+q     → Press prefix, then Shift+q
prefix+Ctrl+h      → Press prefix, then Ctrl+h
Alt+Enter          → Global Alt+Enter (no prefix)
Super+t            → Global Super+t (no prefix)
F1                 → Global F1
```

**Modifiers:** `Ctrl`, `Shift`, `Alt`, `Super` (also accepts `Win`, `Cmd`)

**Named keys:** `Enter`, `Tab`, `Escape`, `Backspace`, `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`, `Space`, `Delete`

---

## Prefix Key

The prefix key triggers prefix mode. All prefix bindings start with `"prefix+"`.

```toml
# Default: "ctrl+b"
prefix = "ctrl+a"
```

---

## Navigation (within workspace)

```toml
[keys]
focus_left  = "prefix+h"   # Focus column left (animated scroll)
focus_right = "prefix+l"   # Focus column right (animated scroll)
focus_up    = "prefix+k"   # Focus pane up within column
focus_down  = "prefix+j"   # Focus pane down within column
```

---

## Splits

```toml
[keys]
split_horizontal = "prefix+Enter"   # New column to the right
split_vertical   = "prefix+v"       # New pane in current column
```

---

## Resize (column width / pane height)

```toml
[keys]
resize_increase      = "prefix+="       # Widen active column
resize_decrease      = "prefix+-"       # Narrow active column
pane_height_increase = "prefix+Shift+=" # Grow active pane height
pane_height_decrease = "prefix+Shift+-" # Shrink active pane height
```

---

## Pane Operations

```toml
[keys]
close = "prefix+x"   # Close active pane
float = "prefix+f"   # Toggle pane floating (detach/attach)
```

---

## Quick Select / Swap

```toml
[keys]
pane_select         = "prefix+q"        # Show letters on panes, pick to focus
swap_pane           = "prefix+Shift+q"  # Show letters, swap panes (stay here)
swap_and_focus_pane = "prefix+m"        # Show letters, swap panes (follow)
```

---

## Pane Cycling (within column)

```toml
[keys]
next_pane = "prefix+n"   # Next pane in column
prev_pane = "prefix+p"   # Previous pane in column
```

---

## Sidebars

```toml
[keys]
sidebar_left         = "prefix+b"    # Toggle left sidebar
sidebar_right        = "prefix+."    # Toggle right sidebar
sidebar_focus        = "prefix+e"    # Focus sidebar (enter nav mode)
sidebar_expand_toggle = "prefix+Tab" # Expand/collapse sidebar tree
```

### Sidebar Navigation (only active IN sidebar mode)

| Key | Action |
|-----|--------|
| `h` / `l` | Collapse / expand tree node |
| `j` / `k` | Move cursor down / up |
| `Enter` | Activate selected item (pane/workspace) |
| `Escape` | Exit sidebar mode |

---

## Workspace Navigation

```toml
[keys]
workspace_prev = "prefix+u"   # Previous workspace
workspace_next = "prefix+d"   # Next workspace
```

### Focus workspace by number (chord: prefix → w → digit)

```
prefix+w, then 1-9 → focus workspace 1-9
```

---

## Focus Toggle

```toml
[keys]
focus_toggle_local  = "prefix+i"        # Toggle last pane (same workspace)
focus_toggle_global = "prefix+Shift+l"  # Toggle last pane (cross-workspace)
```

---

## Workspace Management

```toml
[keys]
create_workspace  = "prefix+w"        # Create new workspace + pane
rename_workspace  = "prefix+Shift+w"  # Rename current workspace
rename_pane       = "prefix+Shift+p"  # Rename current pane
```

---

## Move / Swap (adjacent)

```toml
[keys]
move_pane_left  = "prefix+Ctrl+["
move_pane_right = "prefix+Ctrl+]"
swap_left       = "prefix+Ctrl+h"
swap_right      = "prefix+Ctrl+l"
swap_up         = "prefix+Ctrl+k"
swap_down       = "prefix+Ctrl+j"
```

---

## System

```toml
[keys]
command_palette = "prefix+p"        # Open command palette (backend ready)
reload_config   = "prefix+Shift+r"  # Reload config at runtime
```

---

## Custom Modes

Modes are groups of bindings active until `Esc`/`Enter` is pressed. Define modes under `[[keys.mode]]`:

```toml
[[keys.mode]]
name = "resize"
trigger = "prefix+r"
sticky = true

[[keys.mode.bindings]]
action = "resize"
keys = "h"
args = { target = "column", axis = "x", amount = "-50" }

[[keys.mode.bindings]]
action = "resize"
keys = "l"
args = { target = "column", axis = "x", amount = "50" }
```

---

## Spawn Commands

Launch external applications in new panes:

```toml
[[keys.command]]
keys = "prefix+g"
command = "lazygit"

[[keys.command]]
keys = "Alt+Enter"
command = "alacritty"
```

---

## Unbind (remove default bindings)

To remove a default binding, add it to `[keys.unbind]`. The action still exists — you can rebind it to a different key in `[keys]`.

Example:

```toml
[keys.unbind]
"prefix+f" = true        # Remove default float toggle (prefix+f)
"prefix+q" = true        # Remove default pane select (prefix+q)
```

Then rebind to different keys:

```toml
[keys]
float      = "prefix+Shift+f"   # Float is now prefix+Shift+f
pane_select = "prefix+Shift+q"  # Pane select is now prefix+Shift+q
```

**Why unbind?**
- Free up keys for custom bindings
- Disable features you don't use
- Resolve conflicts between defaults and custom bindings
