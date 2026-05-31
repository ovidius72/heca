# Plan 02: Action Registry + RPC Foundation

**Created:** 2026-06-01
**Status:** Draft
**Depends on:** Phase 6 (Command Palette Backend, `dd55ec7`)
**Goal:** Replace the monolithic `execute_action()` match with a dispatch registry, separate keybinding resolution from action handling, and lay the foundation for RPC commands with arguments.

---

## Motivation

The current architecture has three problems:

1. **`execute_action()` is a 500-line match statement.** Adding an action requires touching `input.rs` (enum), `theme.rs` (default binding), `main.rs` (match arm), and `actions.rs` (metadata). This is error-prone and doesn't scale.
2. **Keybinding logic is mixed with action logic.** `KeyBindings::resolve()` lives in `input.rs` but calls `execute_action()` in `main.rs`. There's no clean separation between "what key was pressed" and "what should happen."
3. **RPC is impossible with unit variants.** Future commands like `swap 1 2` or `resize 1 +10` need to carry arguments. The current `WmAction` enum is all unit variants, so args have no place to live.

This plan solves all three by:
- Making `WmAction` a **mixed enum** (some unit, some parameterized)
- Creating an `ActionRegistry` that maps `WmAction` → named `fn` handlers
- Creating a `KeymapRegistry` that maps key combos → `WmAction` variants
- Adding an RPC parser that constructs parameterized `WmAction` variants and dispatches through the same registry

---

## Design Principles

1. **One action, one handler.** Every `WmAction` variant has exactly one handler function. No closure duplication.
2. **Keybindings dispatch unit variants.** Keys like `h` or `x` trigger parameterless actions.
3. **RPC dispatch parameterized variants.** Commands like `swap 1 2` construct variants with embedded args.
4. **Interactive and direct variants coexist.** `PaneSelect` (interactive, unit) and `FocusPane { pane_id }` (direct, parameterized) can both exist. The command palette UI will show both and let the user pick.
5. **No dynamic dispatch.** Handlers are `fn` pointers, not `Box<dyn Fn>`. This keeps the registry zero-cost and avoids lifetime/closure issues.

---

## Architecture

```
┌─────────────────────────────────────────┐
│            WmAction (enum)               │
│  ┌─────────────────────────────────────┐│
│  │ Unit variants (keybindings)         ││
│  │   FocusLeft, ClosePane, PaneSelect  ││
│  ├─────────────────────────────────────┤│
│  │ Parameterized variants (RPC)        ││
│  │   Swap { a_id, b_id }               ││
│  │   Move { pane_id, target_col }      ││
│  │   Resize { target, delta }          ││
│  │   FloatAt { pane_id, x, y, w, h }   ││
│  └─────────────────────────────────────┘│
└─────────────────────────────────────────┘
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
┌───────────────┐       ┌───────────────┐
│ KeymapRegistry│       │  RPC Parser   │
│               │       │               │
│  "h" → FocusL │       │ "swap 1 2"    │
│  "x" → Close  │       │  → Swap{a,b}  │
│  "q" → PaneSel│       │               │
└───────┬───────┘       └───────┬───────┘
        │                       │
        └───────────┬───────────┘
                    ▼
        ┌───────────────────────┐
        │    ActionRegistry      │
        │  ┌───────────────────┐ │
        │  │ handlers:         │ │
        │  │   FocusLeft  → fn │ │
        │  │   ClosePane  → fn │ │
        │  │   Swap       → fn │ │
        │  │   ...             │ │
        │  └───────────────────┘ │
        │  ┌───────────────────┐ │
        │  │ metadata:         │ │
        │  │   label, category │ │
        │  │   default_binding │ │
        │  │   current_binding │ │
        │  └───────────────────┘ │
        └───────────────────────┘
                    │
                    ▼
        ┌───────────────────────┐
        │   Named Handlers       │
        │  fn handle_focus_left  │
        │  fn handle_close_pane  │
        │  fn handle_swap        │
        │  ...                   │
        └───────────────────────┘
```

---

## New / Modified Data Structures

### 1. `WmAction` — Mixed Enum

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WmAction {
    // ── Navigation (unit variants) ──
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    NextPane,
    PrevPane,
    WorkspaceNext,
    WorkspacePrev,
    FocusToggleLocal,
    FocusToggleGlobal,

    // ── Navigation (parameterized) ──
    FocusPane { pane_id: u64 },
    FocusWorkspace { ws_idx: usize },

    // ── Layout (unit) ──
    SplitHorizontal,
    SplitVertical,
    ResizeIncrease,
    ResizeDecrease,
    PaneHeightIncrease,
    PaneHeightDecrease,
    SwapLeft,
    SwapRight,
    SwapUp,
    SwapDown,
    MovePaneLeft,
    MovePaneRight,

    // ── Layout (parameterized) ──
    Swap { a_id: u64, b_id: u64 },
    Move { pane_id: u64, target_col: usize },
    Resize { target: ResizeTarget, delta: i32 },
    ResizeTo { target: ResizeTarget, width: f64, height: f64 },

    // ── Pane (unit) ──
    Float,
    ClosePane,
    PaneSelect,
    SwapSelect,
    SwapAndFocus,
    RenamePane,

    // ── Pane (parameterized) ──
    FloatAt { pane_id: u64, x: f64, y: f64, width: f64, height: f64 },
    ClosePaneById { pane_id: u64 },
    RenameTarget { target: RenameTarget, name: String },

    // ── Workspace (unit) ──
    CreateWorkspace,
    RenameWorkspace,

    // ── Sidebar / Chrome (unit) ──
    SidebarLeft,
    SidebarRight,
    SidebarFocus,
    SidebarUp,
    SidebarDown,
    SidebarLeftNav,
    SidebarRightNav,
    SidebarExpandToggle,
    TabNext,
    TabPrev,

    // ── System ──
    CommandPalette,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResizeTarget {
    Column,
    Pane,
}
```

**Key rule:** Unit variants are for keybindings. Parameterized variants are for RPC. The command palette UI can show both — unit variants execute immediately, parameterized variants open a prompt for args.

### 2. `ActionRegistry` — Handler Dispatch

```rust
pub struct ActionRegistry {
    handlers: HashMap<WmActionDiscriminant, fn(&mut AppState, &WmAction)>,
    metadata: HashMap<WmActionDiscriminant, ActionMetadata>,
}

impl ActionRegistry {
    pub fn register(
        &mut self,
        discriminant: WmActionDiscriminant,
        handler: fn(&mut AppState, &WmAction),
        default_binding: &str,
    );

    pub fn execute(&self, action: &WmAction, state: &mut AppState);
    pub fn find_meta(&self, action: &WmAction) -> Option<&ActionMetadata>;
}
```

**Why `WmActionDiscriminant`?** We use `std::mem::discriminant(&action)` as the HashMap key. This means `FocusLeft`, `FocusPane { pane_id: 1 }`, and `FocusPane { pane_id: 2 }` all share one handler.

**Handler signature:** `fn(&mut AppState, &WmAction)`. The handler pattern-matches on the variant to extract args:

```rust
fn handle_swap(state: &mut AppState, action: &WmAction) {
    let WmAction::Swap { a_id, b_id } = action else { return };
    swap_panes(state, *a_id, *b_id);
}
```

### 3. `KeymapRegistry` — Key Resolution

```rust
pub struct KeymapRegistry {
    modes: HashMap<String, HashMap<KeyCombo, WmAction>>,
}

impl KeymapRegistry {
    pub fn bind(&mut self, mode: &str, key: KeyCombo, action: WmAction);
    pub fn unbind(&mut self, mode: &str, key: &KeyCombo);
    pub fn rebind(&mut self, mode: &str, old: &KeyCombo, new: KeyCombo);
    pub fn resolve(&self, mode: &str, key: &KeyCombo) -> Option<&WmAction>;
}
```

`KeyCombo` is a normalized representation of a keypress (key string + ctrl + shift + alt + super). Modes are `"normal"`, `"sidebar"`, etc.

### 4. RPC Parser — String → WmAction

```rust
pub fn parse_rpc_command(cmd: &str) -> Result<WmAction, RpcError> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    match parts.as_slice() {
        ["focus", "left"] => Ok(WmAction::FocusLeft),
        ["focus", "right"] => Ok(WmAction::FocusRight),
        ["focus-pane", id] => Ok(WmAction::FocusPane { pane_id: id.parse()? }),
        ["focus-ws", idx] => Ok(WmAction::FocusWorkspace { ws_idx: idx.parse()? }),
        ["close"] => Ok(WmAction::ClosePane),
        ["close-pane", id] => Ok(WmAction::ClosePaneById { pane_id: id.parse()? }),
        ["swap", a, b] => Ok(WmAction::Swap { a_id: a.parse()?, b_id: b.parse()? }),
        ["move", pane, col] => Ok(WmAction::Move { pane_id: pane.parse()?, target_col: col.parse()? }),
        ["resize", target, delta] if delta.starts_with('+') || delta.starts_with('-') => {
            Ok(WmAction::Resize { target: parse_target(target)?, delta: delta.parse()? })
        }
        ["resize-to", target, dims] => {
            let (w, h) = parse_dimensions(dims)?;
            Ok(WmAction::ResizeTo { target: parse_target(target)?, width: w, height: h })
        }
        ["float", pane, x, y, w, h] => Ok(WmAction::FloatAt {
            pane_id: pane.parse()?,
            x: x.parse()?, y: y.parse()?,
            width: w.parse()?, height: h.parse()?,
        }),
        _ => Err(RpcError::UnknownCommand(cmd.to_string())),
    }
}
```

---

## Phase Breakdown

### Phase 1: Refactor WmAction to Mixed Enum

**Goal:** Add parameterized variants without breaking existing keybinding flow.

**Tasks:**
1. Add parameterized variants to `WmAction` (see list above)
2. Derive `Clone` on `WmAction` (needed because `String` fields in `RenameTarget` prevent `Copy`)
3. Create `ResizeTarget` enum
4. Update `action_from_name()` to return unit variants only (keybindings never carry args)
5. Add `action_discriminant()` helper that returns `Discriminant<WmAction>`

**Files to modify:**
- `heca/src/input.rs` — `WmAction` enum, `action_from_name()`

**Tests:**
- `test_wm_action_discriminant` — verify FocusPane{1} and FocusPane{2} share discriminant
- `test_wm_action_clone` — verify parameterized variants clone correctly

---

### Phase 2: Build ActionRegistry

**Goal:** Create the dispatch table and migrate all existing handlers.

**Tasks:**
1. Create `ActionRegistry` struct in `heca/src/actions.rs`
2. Define handler signature: `pub type ActionHandler = fn(&mut AppState, &WmAction);`
3. Write named handler functions for all 37 existing actions
4. Implement `register()` and `execute()`
5. Wire registry initialization in `HecaApp::init_state()`

**Handler examples:**

```rust
fn handle_focus_left(state: &mut AppState, _action: &WmAction) {
    state.session.focus_left();
    sync_focus(state);
    state.needs_redraw = true;
}

fn handle_pane_select(state: &mut AppState, _action: &WmAction) {
    let candidates = collect_all_pane_candidates(&state.session);
    state.input_mode = InputMode::PaneSelect { candidates };
    state.needs_redraw = true;
}

fn handle_swap(state: &mut AppState, action: &WmAction) {
    let WmAction::Swap { a_id, b_id } = action else { return };
    swap_panes(state, *a_id, *b_id);
}

fn handle_focus_pane(state: &mut AppState, action: &WmAction) {
    let WmAction::FocusPane { pane_id } = action else { return };
    focus_pane_by_id(state, *pane_id);
}
```

**Files to modify:**
- `heca/src/actions.rs` — ActionRegistry, handlers
- `heca/src/main.rs` — init registry, replace execute_action() call
- `heca/src/app_state.rs` — add `ActionRegistry` to AppState (or keep it in HecaApp)

**Note on `focus_pane_by_id`:** This function is currently called directly from mouse handlers and sidebar nav. It stays as a direct function (not in registry). But the registry gets a `FocusPane { pane_id }` handler that calls it.

**Tests:**
- `test_registry_executes_handler` — register a test handler, call execute, verify side effect
- `test_registry_missing_handler` — execute unregistered action, verify graceful no-op

---

### Phase 3: Build KeymapRegistry

**Goal:** Separate key resolution from action execution. Replace `KeyBindings`.

**Tasks:**
1. Define `KeyCombo` struct: `{ key: String, ctrl: bool, shift: bool, alt: bool, super_: bool }`
2. Define `KeymapRegistry` with per-mode maps
3. Implement `bind`, `unbind`, `rebind`, `resolve`
4. Parse key strings like `"h"`, `"Ctrl+h"`, `"Shift+q"` into `KeyCombo`
5. Build keymap from `ActionRegistry` metadata at init time
6. Override with user config
7. Replace `self.bindings.resolve()` calls in `window_event` handler

**File:** `heca/src/keymap.rs` (NEW)

**Files to modify:**
- `heca/src/input.rs` — deprecate old `KeyBindings`, or keep as shim
- `heca/src/main.rs` — use KeymapRegistry in keyboard handler

**Tests:**
- `test_keymap_bind_resolve` — bind "h" → FocusLeft, resolve "h", verify
- `test_keymap_mode_switch` — bind "j" in normal vs sidebar modes
- `test_keymap_rebind` — change binding, verify old key misses, new key hits

---

### Phase 4: Wire RPC Parser

**Goal:** Parse string commands into WmAction variants.

**Tasks:**
1. Create `heca/src/rpc.rs` with `parse_rpc_command()`
2. Define `RpcError` enum: `UnknownCommand`, `ParseIntError`, `MissingArgs`
3. Implement command handlers for:
   - `focus left/right/up/down`
   - `focus-pane <id>`
   - `focus-ws <idx>`
   - `close` / `close-pane <id>`
   - `swap <a> <b>`
   - `move <pane> <col>`
   - `resize <target> <+/-delta>`
   - `resize-to <target> <WxH>`
   - `float <pane> <x> <y> <w> <h>`
4. Add `execute_rpc_command(cmd: &str, state: &mut AppState, registry: &ActionRegistry)` convenience function
5. Hook into a debug input path (e.g., `:` key in Prefix mode to type commands)

**File:** `heca/src/rpc.rs` (NEW)

**Tests:**
- `test_rpc_focus_left` → `WmAction::FocusLeft`
- `test_rpc_swap` → `WmAction::Swap { a_id: 1, b_id: 2 }`
- `test_rpc_resize` → `WmAction::Resize { target: Column, delta: 10 }`
- `test_rpc_unknown` → `RpcError::UnknownCommand`
- `test_rpc_missing_args` → `RpcError::MissingArgs`

---

### Phase 5: Integration & Cleanup

**Goal:** Remove old `execute_action()` match, ensure all paths use registry.

**Tasks:**
1. Delete the old `execute_action()` match in `main.rs`
2. Remove deprecated `KeyBindings` code (or keep as thin shim for one commit)
3. Update `ActionDescriptor` in `actions.rs` to link to `WmAction` discriminant
4. Update command palette backend to read `current_binding` from registry metadata
5. Verify all existing tests still pass
6. Run clippy, fix warnings

**Files to modify:**
- `heca/src/main.rs` — remove old execute_action
- `heca/src/actions.rs` — link descriptors to discriminant
- `heca/src/input.rs` — remove old KeyBindings

**Tests:**
- All 45 existing tests must pass
- New integration test: simulate keypress → keymap resolves → registry executes

---

## New Files

| File | Purpose |
|------|---------|
| `heca/src/keymap.rs` | KeyCombo, KeymapRegistry, key string parsing |
| `heca/src/rpc.rs` | RPC command parser, execute_rpc_command |

## Files to Modify

| File | Changes |
|------|---------|
| `heca/src/input.rs` | WmAction mixed enum, action_from_name, deprecate KeyBindings |
| `heca/src/actions.rs` | ActionRegistry with handler dispatch, metadata linking |
| `heca/src/main.rs` | Init registry + keymap, remove execute_action() match |
| `heca/src/app_state.rs` | Add registry/keymap fields to AppState |

---

## Keybinding Table (unchanged behavior)

All existing keybindings keep working. The registry just changes how they're dispatched internally.

| Key | Action | Variant |
|-----|--------|---------|
| `h` | Focus Column Left | `FocusLeft` (unit) |
| `l` | Focus Column Right | `FocusRight` (unit) |
| `q` | Quick-Select Pane | `PaneSelect` (unit) |
| `x` | Close Pane | `ClosePane` (unit) |
| `]` | Move Pane Right | `MovePaneRight` (unit) |
| `Ctrl+h` | Swap Column Left | `SwapLeft` (unit) |

---

## RPC Command Reference (new)

| Command | Action | Variant |
|---------|--------|---------|
| `focus left` | Focus Column Left | `FocusLeft` |
| `focus-pane <id>` | Focus specific pane | `FocusPane { pane_id }` |
| `focus-ws <idx>` | Focus workspace | `FocusWorkspace { ws_idx }` |
| `swap <a> <b>` | Swap two panes | `Swap { a_id, b_id }` |
| `move <pane> <col>` | Move pane to column | `Move { pane_id, target_col }` |
| `resize <target> <±delta>` | Resize by delta | `Resize { target, delta }` |
| `resize-to <target> <WxH>` | Resize to exact | `ResizeTo { target, width, height }` |
| `float <pane> <x> <y> <w> <h>` | Float pane at position | `FloatAt { pane_id, x, y, width, height }` |
| `close` | Close active pane | `ClosePane` |
| `close-pane <id>` | Close specific pane | `ClosePaneById { pane_id }` |

---

## Effort Estimate

| Phase | Tasks | Est. Effort | Risk |
|-------|-------|-------------|------|
| 1 — Mixed Enum | Add variants, update parser | Small | Low |
| 2 — ActionRegistry | Write 37 handlers, init wiring | Medium | Medium (tedious) |
| 3 — KeymapRegistry | KeyCombo, bind/unbind/rebind | Medium | Low |
| 4 — RPC Parser | 10 command parsers, error handling | Small | Low |
| 5 — Integration | Delete old code, fix tests | Medium | Medium (regression risk) |
| **Total** | | **~2–3 days** | |

---

## Test Plan

| Phase | Test |
|-------|------|
| 1 | `test_wm_action_discriminant`, `test_wm_action_clone` |
| 2 | `test_registry_executes_handler`, `test_registry_missing_handler` |
| 3 | `test_keymap_bind_resolve`, `test_keymap_rebind`, `test_keymap_mode_switch` |
| 4 | `test_rpc_focus_left`, `test_rpc_swap`, `test_rpc_resize`, `test_rpc_errors` |
| 5 | All 45 existing tests + new integration test |

---

## Future Work (not in this plan)

- **Command Palette UI** — iterate `ActionRegistry::ALL`, show `label` + `current_binding`, filter by name
- **Settings View** — call `keymap.rebind()` with conflict detection, persist to `config.toml`
- **Full RPC Server** — Unix socket / TCP server that accepts commands and dispatches through registry
- **Action Arguments UI** — for parameterized variants in the palette, show input fields for args

---

## Dependency Graph

```
Phase 1 ──► Phase 2 ──► Phase 5
  │            │
  ▼            ▼
Phase 3 ──────► Phase 4
```

Execute in order: **1 → 2 → 3 → 4 → 5**
