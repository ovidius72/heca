# Actions System

Every WM command in heca is an **action**. Actions are the core abstraction — everything from focusing a pane to resizing a column to spawning an application is an action.

## Three-Layer Dispatch

```
Keyboard Input → KeyCombo → KeymapRegistry → WmAction → ActionRegistry → Handler
     │                                                            │
     │                                                            ├── handle_focus_pane()
     │                                                            ├── handle_swap_pane()
     │                                                            ├── handle_resize()
     │                                                            └── ...
     │
     └── Physical key normalization
         ├── macOS: Ctrl+letter → control char fallback
         ├── Shift+symbol → unshifted base key
         └── NamedKey mapping (Enter, Tab, ArrowLeft, ...)
```

## ActionRegistry (`heca/src/actions.rs`)

The central dispatch for all WM actions:

```rust
pub struct ActionRegistry {
    handlers: HashMap<Discriminant<WmAction>, ActionHandler>,
}

type ActionHandler = fn(&mut AppState, &WmAction);
```

- **Register** a handler: `registry.register(&WmAction::FocusPane { pane_id: 0 }, handle_focus_pane)`
- **Execute** an action: `registry.execute(&action, state)` — routes to the correct handler by discriminant
- **Parameterized variants** share one handler — the handler destructures the action to get arguments

**Registry bypasses are bugs.** All WM state changes must go through `registry.execute()`. Direct calls like `focus_pane_by_id(state, id)` are only allowed inside handlers (as part of their implementation).

## KeymapRegistry (`heca/src/keymap.rs`)

Mode-specific keymap lookup:

```rust
pub struct KeymapRegistry {
    keymaps: HashMap<String, HashMap<KeyCombo, WmAction>>,
}
```

- **Bind**: `keymap.bind("normal", combo, action)`
- **Resolve**: `keymap.resolve("normal", &combo)` → `Option<&WmAction>`
- **Modes**: `"normal"` (prefix bindings), `"global"` (direct bindings), `"sidebar"` (sidebar nav), custom mode names

## WmAction Enum (`heca/src/input.rs`)

```rust
pub enum WmAction {
    // Unit actions (no arguments)
    FocusLeft, FocusRight, FocusUp, FocusDown,
    SplitHorizontal, SplitVertical,
    ClosePane, Float, PaneSelect, SwapPane, SwapAndFocusPane,
    CreateWorkspace, RenameWorkspace, RenamePane,
    SidebarLeft, SidebarRight, SidebarFocus,
    CommandPalette, ReloadConfig, ...

    // Parameterized actions (arguments)
    FocusPane { pane_id: u64 },
    FocusWorkspace { ws_idx: usize },
    Swap { a_id: u64, b_id: u64 },
    Move { pane_id: u64, target_col: usize },
    Resize { target: ResizeTarget, axis: ResizeAxis, amount: f64 },
    ResizeTo { target: ResizeTarget, width: f64, height: f64 },
    FloatAt { pane_id: u64, x: f64, y: f64, width: f64, height: f64 },
    ClosePaneById { pane_id: u64 },
    RenameTarget { pane_id: u64, name: String },
    SpawnCommand { command: String },
    EnterMode { name: String },
}
```

## Action Types

**Unit actions** — Simple commands with no arguments:
- `focus_left`, `focus_right`, `split_horizontal`, `close`, `float`

**Parameterized actions** — Commands with arguments:
- `FocusPane { pane_id }` — Focus a specific pane by ID
- `FocusWorkspace { ws_idx }` — Focus a workspace by index
- `Swap { a_id, b_id }` — Swap two panes
- `Resize { target, axis, amount }` — Resize column or pane
- `SpawnCommand { command }` — Run an external command

## Registry Dispatch

All actions go through a central registry:

```
Keyboard input → KeyCombo → KeymapRegistry → WmAction → ActionRegistry → Handler
```

This design means:
- Every action is traceable and hookable
- Future scripting/IPC can trigger any action by name
- Actions can be composed and chained

## Creating Custom Actions

To add a new action to heca:

1. **Add to `WmAction` enum** in `heca/src/input.rs`:
```rust
pub enum WmAction {
    // ... existing variants
    MyCustomAction,
}
```

2. **Add name mapping** in `action_from_name()`:
```rust
"my_custom_action" => Some(WmAction::MyCustomAction),
```

3. **Add priority** in `action_priority()`:
```rust
WmAction::MyCustomAction => 1,  // Lower = higher priority
```

4. **Create handler** in `heca/src/handlers.rs`:
```rust
pub fn handle_my_custom_action(state: &mut AppState, _action: &WmAction) {
    // Your logic here
    state.needs_redraw = true;
}
```

5. **Register** in `build_registry()` in `heca/src/main.rs`:
```rust
registry.register(&WmAction::MyCustomAction, handle_my_custom_action);
```

6. **Add default binding** in `heca-config/src/theme.rs`:
```rust
bindings.insert("my_custom_action".to_string(), Single("prefix+y".to_string()));
```
