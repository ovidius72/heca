# Handoff — Zoom Column Toggle + Parameterized Spawn/Float Bindings

**Date:** 2026-06-04  
**Branch:** `feature/improve-sidebar`  
**Base synced:** merged `origin/main` into branch before pausing  
**Purpose:** resume implementation in a fresh session without re-discovery

---

## 1. Current state

### Already done in this session
- merged `origin/main` into `feature/improve-sidebar`
- updated documentation in:
  - `AGENTS.md`
  - `README.md`
  - `keybindings.toml`
- implemented **zoom active column toggle** end-to-end:
  - new `WmAction::ZoomColumn`
  - default keybinding `prefix+z`
  - layout-level toggle / restore behavior in `ScrollingSpace`
  - handler + action registry wiring
  - RPC commands: `zoom-column`, `zoom-col`
  - tests for input parsing, RPC parsing, config defaults, and scrolling behavior

### Existing recent fixes on this branch
- fixed floating pane border draw order
- fixed active floating pane normalization
- fixed unfloat bug where restoring into a split column could place a pane off-screen

---

## 2. Agreed feature scope

## A. Column zoom toggle

### Status
**Implemented.**

### Final behavior
- zooms the **active column** to cover the full available content width
- aligns it to the **left**
- keeps `prefix+z` as its default binding
- is available via:
  - action registry
  - config keybindings
  - RPC (`zoom-column`, `zoom-col`)
- behaves as a **toggle**:
  - first press zooms full-width
  - second press restores previous width
- each column tracks zoom independently, so multiple columns can stay zoomed at once

### Implementation notes
- uses a dedicated `WmAction::ZoomColumn`
- stores previous width in `Column.zoom_restore_width: Option<ColumnWidth>`
- does **not** rely on the dormant maximize/fullscreen state machine
- zoom width is resolved dynamically from the viewport, so it stays correct across viewport-size recomputation

---

## B. Parameterized keybindings in both normal and mode bindings

### Agreed config contract
Support parameterized actions in:
- normal bindings via `[[keys.bind]]`
- mode bindings via `[[keys.mode.bindings]]`

Keep simple unit bindings in the flat `[keys]` table.

### Agreed syntax

#### Simple unit actions
```toml
[keys]
focus_left = "prefix+h"
float = "prefix+f"
```

#### Parameterized normal bindings
```toml
[[keys.bind]]
keys = "prefix+z"
action = "zoom_column"

[[keys.bind]]
keys = "prefix+g"
action = "spawn_pane"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }
```

#### Parameterized mode bindings
```toml
[[keys.mode]]
name = "spawn"
trigger = "prefix+s"
sticky = true

[[keys.mode.bindings]]
action = "spawn_pane"
keys = "t"
args = { kind = "terminal", program = "lazygit", argv = [] }
```

### Exhaustive example matrix
```toml
[keys]
focus_left = "prefix+h"
float = "prefix+f"

# Unit action via parameterized normal binding syntax
[[keys.bind]]
keys = "prefix+z"
action = "zoom_column"

# Tiled terminal spawn
[[keys.bind]]
keys = "prefix+g"
action = "spawn_pane"
args = { kind = "terminal", program = "lazygit", argv = [] }

# Floating terminal spawn with implicit px values
[[keys.bind]]
keys = "prefix+Shift+t"
action = "spawn_pane"
args = { kind = "terminal", program = "btm", argv = [], float = true, width = "800", height = "400" }

# Floating terminal spawn with explicit px suffix
[[keys.bind]]
keys = "prefix+Shift+y"
action = "spawn_pane"
args = { kind = "terminal", program = "htop", argv = [], float = true, width = "800px", height = "400px" }

# Floating terminal spawn with percent sizing
[[keys.bind]]
keys = "prefix+Shift+g"
action = "spawn_pane"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

# Floating browser spawn
[[keys.bind]]
keys = "prefix+Shift+b"
action = "spawn_pane"
args = { kind = "browser", float = true, width = "1200", height = "800" }

# Floating nvim GUI spawn
[[keys.bind]]
keys = "prefix+Shift+n"
action = "spawn_pane"
args = { kind = "nvim_gui", float = true, width = "1000", height = "700" }

# Float active pane with geometry helper
[[keys.bind]]
keys = "prefix+Shift+f"
action = "float_active_at"
args = { width = "95%", height = "95%" }

[[keys.mode]]
name = "spawn"
trigger = "prefix+s"
sticky = true

# Tiled spawn from a mode
[[keys.mode.bindings]]
action = "spawn_pane"
keys = "g"
args = { kind = "terminal", program = "lazygit", argv = [] }

# Floating spawn from a mode with implicit px values
[[keys.mode.bindings]]
action = "spawn_pane"
keys = "t"
args = { kind = "terminal", program = "btm", argv = [], float = true, width = "800", height = "400" }

# Floating spawn from a mode with explicit px suffix
[[keys.mode.bindings]]
action = "spawn_pane"
keys = "y"
args = { kind = "terminal", program = "htop", argv = [], float = true, width = "800px", height = "400px" }

# Floating spawn from a mode with percent sizing
[[keys.mode.bindings]]
action = "spawn_pane"
keys = "n"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

# Browser / nvim_gui from a mode
[[keys.mode.bindings]]
action = "spawn_pane"
keys = "b"
args = { kind = "browser", float = true, width = "1200", height = "800" }

[[keys.mode.bindings]]
action = "spawn_pane"
keys = "v"
args = { kind = "nvim_gui", float = true, width = "1000", height = "700" }

# Float helper / zoom from a mode
[[keys.mode.bindings]]
action = "float_active_at"
keys = "f"
args = { width = "95%", height = "95%" }

[[keys.mode.bindings]]
action = "zoom_column"
keys = "z"
```

---

## C. SpawnPane action design

### Product intent
The system should be future-ready to open panes that correspond to supported pane kinds, not just terminal strings.

### Agreed pane kinds
- `terminal`
- `browser`
- `nvim_gui`
- temporary/mock fallback where real backend support is missing

### Agreed command shape
For terminal-like panes, use structured command fields:
- `program = "nvim"`
- `argv = ["."]`

Not just a single shell string.

### Agreed spawn behavior
- always creates a **new pane**
- if `float = true`, open it **centered** by default
- width/height may be provided
- for now, unsupported kinds can be mocked

---

## D. Size parsing contract

### Agreed rules
- `800` => `800px`
- `800px` => explicit pixels
- `80%` => percentage of available content area
- `%` must be explicit
- px suffix is optional

### Strong recommendation
Introduce a typed size representation, e.g.:
```rust
enum SizeSpec {
    Px(f64),
    Percent(f64),
}
```

---

## E. Float/unfloat behavior contract

### Agreed behavior
- `prefix+f` remains the float/unfloat toggle
- if a pane was originally tiled, unfloat restores it to its original tiled location
- if a pane was spawned directly as floating and has **no original tiled slot**, unfloat should place it into a **new column**
- `prefix+z` remains reserved for **column zoom toggle**, not float toggle

This means spawned floating panes should carry enough metadata to distinguish:
- originally tiled -> restore
- born-floating -> create new column on unfloat

---

## 3. Files already identified as needing changes

## For `zoom_column`
Completed in:
- `heca/src/input.rs`
- `heca/src/handlers.rs`
- `heca/src/main.rs`
- `heca/src/actions.rs`
- `heca-config/src/theme.rs`
- `heca/src/rpc.rs`
- `heca-core/src/layout/scrolling.rs`
- `heca-core/src/layout/column.rs`

## For parameterized normal bindings
- `heca-config/src/theme.rs`
  - add new config shape for `[[keys.bind]]`
- `heca/src/main.rs`
  - update `build_keymap()` to load `[[keys.bind]]`
- `heca/src/input.rs`
  - ensure `build_action()` supports the new actions

## For `spawn_pane`
- `heca/src/input.rs`
  - add new action type(s)
- `heca/src/handlers.rs`
  - add spawn handler
- maybe `heca-core/src/backend/mod.rs`
  - if introducing typed pane-kind/runtime mapping
- maybe `heca/src/main.rs` or runtime helper module
  - for backend insertion and pane creation orchestration
- `heca/src/rpc.rs`
  - add RPC parser support if desired immediately

---

## 4. Existing implementation constraints discovered earlier

### Current parameterized action support already exists only for mode bindings
- `build_modes()` in `heca/src/main.rs` uses:
  - `action_from_name()` for unit actions
  - `build_action()` for parameterized actions
- flat `[keys]` currently only supports `action_from_name()`

### Existing custom command bindings are limited
- `[[keys.command]]` only maps to `WmAction::SpawnCommand { command }`
- `command_type` exists in config but appears unused
- current spawn paths still mostly create `FakeBackend`s in handlers/runtime code

### `float_at` exists but is not ergonomic for config
- `FloatAt` requires a concrete `pane_id`
- not suitable for “float current pane with geometry” config UX
- likely need a new action like `FloatActiveAt`

---

## 5. Recommended implementation order

### Step 1 — `zoom_column` toggle
Done.

### Step 2 — config support for `[[keys.bind]]`
Once `zoom_column` exists, wire the new parameterized normal-binding format.

### Step 3 — introduce `SizeSpec` parsing
Do this before `spawn_pane` and `float_active_at`.

### Step 4 — add `FloatActiveAt`
Useful independent action and a good stepping stone before full spawn support.

### Step 5 — add `SpawnPane`
Start with mock-capable behavior, terminal-first semantics, and floating support.

---

## 6. Suggested action shapes

### Minimal likely additions
```rust
WmAction::ZoomColumn
WmAction::FloatActiveAt {
    width: SizeSpec,
    height: SizeSpec,
}
WmAction::SpawnPane {
    kind: PaneKind,
    program: Option<String>,
    argv: Vec<String>,
    float: bool,
    width: Option<SizeSpec>,
    height: Option<SizeSpec>,
}
```

### Possible helper enums
```rust
enum PaneKind {
    Terminal,
    Browser,
    NvimGui,
    Mock,
}

enum SizeSpec {
    Px(f64),
    Percent(f64),
}
```

---

## 7. Docs already updated

These files now document the agreed contract, so use them as the implementation source of truth:
- `AGENTS.md`
- `README.md`
- `keybindings.toml`

They include:
- `[[keys.bind]]` planned syntax
- `spawn_pane` examples
- `%` vs px size rules
- float/unfloat behavior contract
- `prefix+z` reserved for zoom toggle

---

## 8. Resume checklist for next session

1. confirm branch status after merge/docs/zoom changes
2. run:
   - `cargo check`
   - `cargo clippy --workspace --all-targets --all-features`
3. start config schema changes for `[[keys.bind]]`
4. add `SizeSpec`
5. implement `FloatActiveAt` / `SpawnPane`
6. keep changes small and test after each slice

---

## 9. Important note

This handoff now includes:
- the agreed product contract
- the finished `zoom_column` implementation status
- the remaining implementation map for `[[keys.bind]]`, `SizeSpec`, `FloatActiveAt`, and `SpawnPane`

The next session should not need to re-analyze requirements.