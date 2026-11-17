# Actions

Every WM command in heca is an **action**. Actions are the core abstraction — everything from focusing a pane to resizing a column to spawning an application is an action.

## Action Types

**Unit actions** — Simple commands with no arguments:

- `focus_left`, `focus_right`, `split_horizontal`, `close`, `float`

**Parameterized actions** — Commands with arguments:

- `FocusPane { pane_id }` — Focus a specific pane by ID
- `FocusWorkspace { ws_idx }` — Focus a workspace by index
- `Swap { a_id, b_id }` — Swap two panes
- `Resize { target, amount, edge }` — Move a boundary of the focused column or pane. The target decides the axis; `edge` picks which of its two edges moves (default: the one it already owned)
- `SpawnCommand { command, kind, float, close_policy }` — Run an external command in a new pane

**Name-keyed actions** — contributed at **runtime** by a provider or plugin, identified by a stable
string id (`plugin.docker.restart`). They have no `WmAction` variant — the enum is closed — but they
are otherwise ordinary actions: same catalog, same metadata, same policy, same dispatch.

## Registry Dispatch

Every action goes through one dispatch door, whatever surface triggered it:

```
Keyboard  → KeyCombo → KeymapRegistry → ActionRef ─┐
Mouse/UI  → Intent {action, args} ─────────────────┤
RPC       → command ───────────────────────────────┤
                                                   ▼
                                          [interaction policy]
                                                   │
                        built-in ──► WmAction ──► native handler
                        name-keyed ─────────────► registered handler
```

A binding resolves to an `ActionRef`: a **built-in** is resolved when config loads (same cost and same
arg-parsing errors as before), while a **name-keyed** id is resolved at **press** time — because heca
reads your config before any provider has registered its actions.

This design means:

- Every action is traceable and hookable, and is judged by the same interaction policy
- Any action can be triggered **by name** — from a keybinding, a menu, a plugin, or RPC
- Arguments take one path: a menu item, an RPC call and a config binding construct the identical action
- Important capabilities should not be trapped behind one surface: when meaningful, the same action should be reachable from mouse/UI, keybindings, and RPC

## Creating Custom Actions

There are **two** ways to add an action, and which one you want depends on where the code lives.

- **A built-in action** — you're editing heca itself. It gets a `WmAction` variant and a native
  handler. This is the path for a new WM capability.
- **A name-keyed action** — a provider or (later) a WASM plugin contributes an action at **runtime**.
  `WmAction` is a closed enum, so it *cannot* get a variant; it is identified by a stable **string
  id** instead. This is the path for anything that isn't core heca.

Both end up equal citizens: same catalog, same metadata, same interaction policy, same single dispatch
door — so a menu item, a keybinding and an RPC call all reach either one identically.

### A. A built-in action (in-tree)

Say you want `my_custom_action`.

**1. Add the variant** — `heca/src/input/action.rs`:

```rust
pub enum WmAction {
    // ... existing variants
    MyCustomAction,
}
```

**2. Give it a name.** A unit action goes in `action_from_name()`; one that takes arguments goes in
`build_action()` instead (that's what parses a binding's `args`, an `Intent`'s args, and RPC args —
one constructor for all three):

```rust
// unit:
"my_custom_action" => Some(WmAction::MyCustomAction),

// or parameterized, in build_action():
"my_custom_action" => Some(WmAction::MyCustomAction { amount: get_usize(args, "amount")? }),
```

**3. Classify it — `action_policy()` in `heca/src/app/interaction.rs`.** This match is **exhaustive**:
if you skip this step, **it will not compile.** That is deliberate — it forces you to answer "should
this run while a floating pane owns the focus domain?"

```rust
WmAction::MyCustomAction => ActionPolicy::TiledOnly,
```

> ⚠️ `AlwaysAllowed` is a **misnomer** — the router *blocks* it when a floating pane is active.
> `Global` is the only policy that is truly always allowed. See
> [Interaction Policy](#interaction-policy) below.

**4. Write the handler** — in the file under `heca/src/handlers/` for what it acts on:

```rust
pub fn handle_my_custom_action(state: &mut AppState, _action: &WmAction) {
    // Your logic here — only the action. The dispatcher asks for the frame, re-syncs focus and
    // refreshes what lists the session afterwards, for every action.
}
```

**5. Register the handler** in `build_registry()` — `heca/src/app/registry/actions/mod.rs`:

```rust
registry.register(WmActionKind::MyCustomAction, handle_my_custom_action);
```

**6. Describe it** in the file for its category under `heca/src/actions/builtins/` (`pane.rs`, `layout.rs`, …) — the file is its category. This is what gives the action its
label, icon and category everywhere it is shown (context menu, tooltip, command palette), **and what
says which arguments it takes**:

```rust
ActionDescriptor {
    name: "my_custom_action",
    label: "My Custom Action",
    description: "What it does, in one line.",
    icon: Some(Glyph::Gear),   // any Glyph; None if it has no icon yet
    args: &[],                 // takes none — see below if it does
},
```

`args` has no default: a struct literal has to fill every field, so you cannot add an action without
answering the question. **Empty means it takes none**, and every argument handed to it is then an
argument nobody declared — which is reported, not ignored.

An action that reads arguments in `build_action()` declares each one here, in the same order:

```rust
args: &[
    ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to act on."),
    ArgDescriptor::optional("focus_after", ArgKind::Bool, "Focus it afterwards. Default false."),
    // A vocabulary argument points at the list beside its own parser, never a copy of it:
    ArgDescriptor::required_enum("step", <FontZoomStep as EnumArg>::VALUES, "Which way to step."),
],
```

This is what makes a mistake audible. heca compares every call against this list — a `config.toml`
binding when config is read, an `Intent` when it is dispatched — and names what is wrong:

```
[heca] binding 'my_custom_action': unknown argument 'pane_di' — did you mean 'pane_id'?
[heca] binding 'my_custom_action': missing required argument 'pane_id'
[heca] binding 'my_custom_action' cannot be built and will do nothing when pressed
```

A missing required argument stops the action (nothing can build it). An unknown or malformed one
costs only itself: the rest of the call still stands. Three tests keep the declaration and the arm
that reads it in step, so a list that stops matching the code fails the build rather than misleading
someone later.

> **An action that requires an argument gets no default keybinding**, and cannot be reached from its
> bare name at all. A key supplies no pane id, so a name that needs one would have to guess — which
> heca used to do, answering `delete_workspace` with *workspace 0*. Bind such an action with an
> explicit `args` table (see [`[keys.mode]`](keybindings.md#3-a-mode--a-sub-keymap-behind-one-key)) or dispatch it from a menu, a plugin or RPC.

**7. Bind it by default** in **`keybindings.default.toml`** — *not* in Rust. The embedded default
files are the single source of truth for defaults; a default that lives only in code is invisible to
users:

```toml
[keys]
my_custom_action = "prefix+y"
```

**8. RPC parity is automatic.** The RPC looks every name up in the same catalog (step 6), so
`my_custom_action` — with its arguments positionally in declared order, or as `key=value` — works over
RPC the moment it is described. There is nothing to add in `heca/src/rpc.rs`.

**9. Optional — make it confirm first.** Destructive actions declare a `ConfirmSpec`, and *every*
surface that triggers them confirms identically, because the guard lives on the **action**, not the
call site. Users toggle it in the [`[confirm]`](configuration.md#confirmation-prompts) table.

### B. A name-keyed action (provider / plugin)

Registered at runtime, identified by a **stable string id**. Because `WmAction` is closed, this is the
only way for code outside core heca to contribute an action.

Metadata and handler are registered together but stored apart, for a borrow reason rather than a
design one: a handler is `fn(&mut AppState, …)` and never receives the registry, so **metadata** must
be reachable from `AppState` (the `ActionCatalog`) while the **handler** table must be borrowable
alongside `&mut AppState` (the `ActionRegistry`).

```rust
use crate::actions::{register_dynamic, unregister_dynamic, ActionCategory, ActionMeta};
use crate::app::interaction::ActionPolicy;
use crate::chrome::PropValue;
use heca_grid_ui::Glyph;
use std::rc::Rc;

// At mount:
let handle = register_dynamic(
    registry,   // &mut ActionRegistry
    catalog,    // &mut ActionCatalog
    ActionMeta {
        name: "plugin.docker.restart".into(),   // the id bindings / menus / RPC name
        label: "Restart Container".into(),
        description: "Restart the selected Docker container.".into(),
        category: ActionCategory::System,
        default_binding: String::new(),
        icon: Some(Glyph::Play),
        policy: ActionPolicy::Global,           // REQUIRED — no permissive default exists
    },
    // Receives the Intent, so its args arrive as plain data — no closure ever crosses a plugin
    // boundary. `&mut AppState` is the sanctioned write path for an action.
    Some(Rc::new(|state, intent| {
        let Some(PropValue::Text(container)) = intent.args.get("container") else {
            return;
        };
        restart_container(state, container);
        // No redraw here: the dispatcher asks for one after every action.
    })),
);

// At unmount — retires the handler AND the metadata:
unregister_dynamic(registry, catalog, &handle.0);
```

`policy` is a **required field with no default**. A built-in is forced to classify itself by an
exhaustive match; a name-keyed action has no variant, so nothing else would force the question — and a
default would mean "the author forgot" silently resolves to the most permissive setting in the system.

Once registered, the id behaves like any other action. Bind it:

```toml
[keys]
"prefix+d" = "plugin.docker.restart"

# With arguments — use a mode binding (flat bindings take no args):
[[keys.mode.bindings]]
action = "plugin.docker.restart"
keys = "w"
args = { container = "web" }
```

**A binding to an action that doesn't exist yet is not an error.** heca reads your config *before* any
provider or plugin has registered, so a dynamic id simply cannot resolve at load; it resolves when the
key is actually pressed. The flip side is that a **typo in an action name** is not an error either —
it just never fires. If a binding seems dead, check the name.

A typo in an **argument** name is different: heca knows what each built-in action takes, so it says so
at startup rather than leaving you to work it out from a key that does nothing.

```
[heca] binding 'delete_workspace': unknown argument 'ws_idxx' — did you mean 'ws_idx'?
[heca] binding 'delete_workspace': missing required argument 'ws_idx'
[heca] binding 'delete_workspace' cannot be built and will do nothing when pressed
```

The `describe-action <name>` introspection command lists an action's arguments, their types and which
are required — including the ones a provider or plugin contributed. See
**[widgets.md → Discovering actions at runtime](widgets.md#discovering-actions-at-runtime--introspection)**.

**Running one over RPC — `action <name> [key=value …]`.** Every named built-in keeps its own RPC
spelling (`focus-left`, `resize target=column amount=-50`), but a component's or plugin's
declared action has no such spelling and never can: it is not a variant of any enum. The generic
verb takes any action **id** instead, so the whole catalog is reachable:

```
action workspaces.cursor_down          # a component's own verb
action close_pane_by_id pane_id=7      # a built-in, by id, with arguments
```

Arguments are judged against the action's declaration, so a misspelled one says so rather than
silently defaulting. A call is policy-routed exactly like a keypress — a `tiled_only` action is
refused while a floating pane owns the screen, and a component's cursor verb is refused unless that
dock holds the keyboard. And an action whose component is **not mounted** is reported as such rather
than answered with success.

**Passing `None` instead of a handler** registers a *declarative* action: it has an id, metadata and a
policy, and it appears in menus and introspection, but the host cannot run it — it is forwarded to its
owner across the plugin boundary (this is how WASM plugin actions will work).

For how a widget or a plugin's UI fires one of these, see
**[widgets.md → The other half of an `Intent`](widgets.md#the-other-half-of-an-intent--the-action-it-names)**.

## Interaction Policy

Not every action is allowed in every context. heca tracks a per-workspace **focus domain** — `Tiled` (the scrolling column layout) or `Floating` (a detached floating pane is active) — and an interaction policy layer decides whether each action is allowed before it runs.

**When a floating pane is active**, most actions are blocked so the floating pane stays put and the tiled layout isn't disturbed. These still work while floating:

- **Pane-local actions** — `float` (toggle back to tiled), `close`, `rename`, and text selection.
- **Global app actions** — `reload_config` (hot-reload always works, even with a floating pane open).

Blocked while floating: focus/split/resize/swap/move, sidebar navigation, workspace switching, command palette, spawn, and pane select/swap overlays. The only ways to leave the floating domain are `prefix+f` (toggle float) or closing the floating pane.

**An overlay that covers the panes blocks the same things.** Whether an action may run is judged
against what owns the screen: a pane (`Tiled`), a floating pane (`Floating`), a **dock** holding the
keyboard (`Container`), or something **covering the tiled area** (`Overlay`) — a confirm dialog, or a
plugin panel that declared it obscures the panes. In that last one only truly global actions
(`reload_config`) get through, so nothing splits, zooms or closes a pane you cannot see. A focused
dock changes nothing about the app's own keys: `prefix+Enter` still splits the pane you last worked
in — what it adds is that a component's own cursor verbs (`r`, `x`, …) are reachable *only* while its
dock is being driven, from a key, the command palette or RPC alike.

**The action flow:**

```
Keyboard → KeyCombo → WmAction → dispatch_action() ──[policy]──► registry.execute() → handler
                                          └─ blocked → no-op
```

This policy layer is why `prefix+Shift+r` (reload config) hot-applies appearance/keymap/theme changes without a restart — `reload_config` is a **global** action that stays reachable even when a floating pane is active. (This was a real bug: reload used to be silently blocked while floating, so style only applied on a full restart.)

For the full policy table (all six policy categories) and how to classify a new action, see [`AGENTS.md`](../AGENTS.md) → "Interaction Policy".

## Planned `spawn_pane` action contract

> ⚠️ **`spawn_pane` DOES NOT EXIST.** Nothing below this line is implemented — not the action, not
> `float_active_at`, not the `program` / `argv` / `width` / `height` arguments, and not the
> `browser` or `nvim_gui` pane kinds. A binding to an unknown action is not an error: it simply
> never fires, so copying one of these produces a key that silently does nothing.
>
> What exists today is **`spawn_command`** (`command`, `kind`, `float`, `close_pane`,
> `keep_on_error`, `keep_on_success`) and **`[[keys.command]]`** — see
> [Keybindings that carry arguments](keybindings.md#2-a-key-that-carries-arguments--keysbind). This section is
> kept as the agreed design for the sized, multi-kind spawn.

The agreed future-ready action model is `spawn_pane`, designed for multiple pane kinds:

- `terminal`
- `browser`
- `nvim_gui`
- temporary/mock fallback while a backend is not implemented

Examples:

```toml
# Terminal-like pane, tiled
[[keys.bind]]
keys = "prefix+g"
action = "spawn_pane"
args = { kind = "terminal", program = "lazygit", argv = [] }

# Terminal-like pane, floating, centered, 800x400 with implicit px
[[keys.bind]]
keys = "prefix+Shift+t"
action = "spawn_pane"
args = { kind = "terminal", program = "btm", argv = [], float = true, width = "800", height = "400" }

# Terminal-like pane, floating, centered, 800px x 400px with explicit px suffix
[[keys.bind]]
keys = "prefix+Shift+y"
action = "spawn_pane"
args = { kind = "terminal", program = "htop", argv = [], float = true, width = "800px", height = "400px" }

# Terminal-like pane, floating, centered, 80% of content area
[[keys.bind]]
keys = "prefix+Shift+v"
action = "spawn_pane"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

# Future browser pane
[[keys.bind]]
keys = "prefix+Shift+b"
action = "spawn_pane"
args = { kind = "browser", float = true, width = "1200", height = "800" }

# Future nvim GUI pane
[[keys.bind]]
keys = "prefix+Shift+n"
action = "spawn_pane"
args = { kind = "nvim_gui", float = true, width = "1000", height = "700" }

# Float the active pane using explicit geometry
[[keys.bind]]
keys = "prefix+Shift+f"
action = "float_active_at"
args = { width = "95%", height = "95%" }

# Zoom toggle on a normal binding
[[keys.bind]]
keys = "prefix+z"
action = "zoom_column"

# Same spawn ideas inside a mode
[[keys.mode]]
name = "spawn"
trigger = "prefix+s"
sticky = true

[[keys.mode.bindings]]
action = "spawn_pane"
keys = "g"
args = { kind = "terminal", program = "lazygit", argv = [] }

[[keys.mode.bindings]]
action = "spawn_pane"
keys = "n"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

[[keys.mode.bindings]]
action = "zoom_column"
keys = "z"
```

## Float / unfloat contract

The agreed behavior for float toggling is:

- `prefix+f` remains the float/unfloat toggle
- if a pane was originally tiled, unfloat restores it to its original tiled position
- if a pane was spawned directly as floating with no original tiled slot, unfloat should place it into a **new column**
- `prefix+z` is reserved for **column zoom toggle**
