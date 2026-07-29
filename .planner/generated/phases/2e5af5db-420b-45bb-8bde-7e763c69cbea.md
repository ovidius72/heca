# 2e5af5db-420b-45bb-8bde-7e763c69cbea — P086 — A component declares its keys, its mouse and its focus — generically, for any component

**Status:** 🚧 `in-progress`
**Created:** 2026-07-29T09:34:17.295Z
**Updated:** 2026-07-29T18:40:52.896Z

Agreed 2026-07-29: [[keys.component]], always-present container defaults, per-item mouse declarations, default_binding removed, and a chrome context that stops handing every component the workspaces model.

# Why

F003/P085 got the *shape* right — a component declares actions, chrome focus routes the keyboard —
but it was designed while looking at one component, and several of its decisions do not survive a
second one. This phase carries the design agreed with the user on **2026-07-29** and supersedes those
parts of P085.

**The rule everything here is judged against — the user said it three times, so it is written once,
at the top:**

> We are not building a workspaces container. The workspaces tree is **one** component that goes in
> a container; more will exist. Think generic and dynamic.

The working test: **a dock showing one number, declaring no actions and having no rows, must work
with nothing written.** And a Docker dock whose rows are not workspaces must be focusable, draggable,
right-clickable and keyboard-navigable through the *same* declarations, with no host code that knows
what Docker is.

# ✅ AGREED 2026-07-29 — the model

## Three separate things (do not collapse them again)

- **Sidebar** — a shell. Hosts containers, knows nothing about them.
- **Component** — its own thing, mounted in a shell opportunistically, may move elsewhere.
- **Context menu** — a host facility for **every** surface, not a sidebar feature.

## Container focus and its lifetime

`focused_container` already exists (`heca/src/chrome/state.rs`) — one container id or none, in the
**store**, not `AppState` (the store is what the drawing reads and what a plugin can read).

Focus **ends** only when: the component's own "activate and leave" key runs (`Enter`/`l` by
convention); `Esc`; a click lands outside every container; or another container takes it.

**Focusing a pane does NOT by itself end it.** F003/P085/T352 added exactly that rule to
`handle_focus_pane` (`heca/src/handlers.rs:113`) and it is what broke `Space`. It must come out.

## Select vs focus

- **`Space`** — the pane becomes active and comes to the front; the keyboard **stays** in the
  container, so `j`/`k` keep working. (Broken today by the rule above.)
- **`Enter`/`l`** — the pane is focused and the keyboard goes with it; typing reaches the terminal.

## How a component declares

- **Keys are flat at the container level.** One binding name, one key; the component decides what it
  means from the row the cursor is on. The user must know binding *names*, never the widget tree.
- **Mouse behaviour is declared per item kind** — a workspace row and a pane row do different things
  on click / double-click / right-click. **Not rebindable**, but still *named actions*, so a key, a
  menu entry and RPC reach the same thing.
- **Key lookup walks outward**: the row → its parent → the container → the paging/scroll keys. First
  match wins. **No "stop here" flag** until something needs one.
- **The container owns cursor movement** — it is the only thing that knows the order of its rows.
- Nothing is inherited between item kinds; each declares its own.

## Config shape

```toml
[[keys.component]]
name         = "workspaces"      # the component; a FIELD, not the table name
# id         = "ws2"             # optional — absent means every placement
global_focus = "prefix+Shift+e"
next_item    = "j"
prev_item    = "k"
focus        = ["l", "Enter"]
select       = "Space"
```

- Lives under `[keys]`, beside `[keys.widgets]` and `[[keys.mode]]`.
- **Array of tables**, so `name` + optional `id` distinguish entries and a component's name can never
  collide with a config keyword. Strictly better than what P085/T355 shipped (`[keys.<kind>]`, where
  the component's name IS the table name, so the parser had to tell a component from an action
  binding by *shape* — `BindingValue::Component` in `heca-config/src/keys.rs`).
- Binding names are short; the **action id** is `<component>.<name>`.
- `global_focus` applies **when the container does NOT have focus**; everything else applies while it
  does. Named so that is obvious.
- `global_focus` with an `id` focuses that placement; without an `id`, the placement you were in last
  — the rule `owning_mount` (`heca/src/providers/actions.rs`) already uses.
- **NOT a `[[keys.mode]]`.** A mode is one app-wide state; a container layer is decided by focus, and
  P085 settled that focus *is* the mode with nothing beside it that could disagree.

## Every container has two keys with nothing declared

- **`global_focus`** — per instance, so two placements can be reached separately.
- **`Esc` → focus the main region**, for every component. Nothing declares it; it is always there.

These are what make "a dock that declares nothing" usable, which is the test above.

## Defaults and discoverability

- **Core components** ship their keys in `keybindings.default.toml`.
- **Plugins** register actions and keys at runtime (`register_action` / `register_keybinding`); they
  have no file.
- **`ActionMeta.default_binding` is removed.** It has never been read for built-ins —
  `shortcut_for_action` (`heca/src/shortcut.rs:49`) reads the *config maps*, not this field — so it is
  a second copy of `keybindings.default.toml` that nothing verifies and that can drift. P085/T355 made
  it live for components only; that goes too.
- The **current key of any action** comes from a reverse lookup over the **built keymaps**
  (`keymap::Keymaps`), not from config and not from a metadata field. Only the built keymaps see every
  layer — flat, mode, component and plugin-registered — uniformly. Rebuilt on reload.
- **`heca --keys-show`** lists binding names and their resolved keys. The only discovery path that can
  work for a plugin.

## Keys the user chose for the workspaces component

`w` new workspace, `p` new pane, `c` new column, `x` delete what the cursor is on. This **changes**
today's `v` (new pane) and `d` (delete), so the "muscle memory survives" note in F003/P085/T356 no
longer holds.

# What in the tree still assumes one component

Each breaks when a second component exists. The first is worst — it sits in the middle of the
contract every component uses.

| what | why it breaks |
|---|---|
| `ChromeCtx::for_build` (`heca/src/providers/mod.rs`) | takes the **workspace tree** and hands it to every component's build hook. A Docker dock, a notes dock, a one-number label all receive a workspace model they have no use for. A component's own model must come from its own state. |
| `WorkspacesContainerState` (`heca/src/chrome/state.rs:142`) | a named field in the shared store for one component; a second has nowhere to put its state (F003/P085/T360) |
| `ChromeDragItem` (`heca/src/chrome/mod.rs:2661`) | closed: `Pane \| Column \| Workspace`. A Docker row cannot be dragged or dropped at all |
| `ContextTarget` | closed the same way — which is why a plugin row cannot be right-clicked |
| `SidebarSelection` (`heca/src/chrome/events.rs`) | a workspace/column/pane type in the **chrome event** vocabulary every component subscribes to |
| `sidebar_item_at`, `sidebar_drop_target` | named for a side, and they return `ChromeDragItem` |
| `pane_fallback_name`, `pane_custom_name` (`heca/src/chrome/mod.rs`) | the chrome reads the workspace model directly to draw labels |
| `mouse/surface_left.rs` | a whole mouse surface for one dock in one position |
| `region_on_screen` (`heca/src/chrome/focus.rs`) | only sidebars can hold containers; a component in a bar is reported as not on screen |

# What this undoes from F003/P085 (already committed)

- `[keys.<kind>]` → `[[keys.component]]`: the parsing and merging in `heca-config/src/keys.rs` +
  `build_component_keymaps` (`heca/src/app/registry.rs`) is rewritten. **The merge rules themselves
  stay** — table merges per key, the arg-carrying array merges by `keys`, `unbind` is by combo.
- The "focusing a pane releases container focus" rule comes out of `handle_focus_pane`.
- `activate` / `peek` are container-level declared actions today
  (`heca/src/providers/workspaces/mod.rs`); under this model they likely belong to the row kinds.
- `default_binding` comes out of the built-in descriptors, and `bind_component_default` with it.

# Behaviours to preserve

- An unbound key while a container has focus is **swallowed**, never forwarded to a backend.
- `prefix+…` keeps working and falls through to the component's layer when the global map misses.
- `state.focused_pane` is not cleared by container focus — only the keyboard is redirected.
- The conflict report (`heca/src/app/conflicts.rs`) must keep covering every binding path added here,
  at startup **and** after a reload.

# Still open — decide before building the affected task

1. Which level owns `w` / `p` / `c` / `x`. `x` is clearly the row's; `w` is about no row; `p` and `c`
   are in between.
2. Whether a key with no meaning on the current row does nothing, or falls through outward. The
   agreed lookup says fall through; confirm for keys a row deliberately does not want.
3. Whether item kinds ever get their own key blocks, or stay mouse-only.
4. Double-click — declared per item kind; nothing forced.

# Acceptance

- A container that declares **nothing** is focusable by its own `global_focus`, released by `Esc`,
  scrolls, and is clicked into — with no code written for it.
- A component's keys are written once under `[[keys.component]]`, per kind or per placement.
- No host code enumerates what kinds of row or component exist.
- `--keys-show` lists every binding name and its resolved key, for core and plugin alike.
- Whole workspace green, clippy 0 warnings. **The USER drives it in the app.**

## Tasks

### ✅ 11f2fe64-d9b2-4faf-83ce-7a89f61b8f1d — T362 — [[keys.component]] — a component's keys, by name and optional placement id

Status: ✅ `done`

# What

Replace `[keys.<kind>]` with `[[keys.component]]`, an array of tables whose `name` field says which
component and whose optional `id` narrows it to one placement.

```toml
[[keys.component]]
name      = "workspaces"     # every placement
next_item = "j"

[[keys.component]]
name   = "workspaces"        # this placement only, layered on top
id     = "ws2"
select = "Enter"
```

# Why the change

F003/P085/T355 shipped `[keys.<kind>]`, where the component's **name is the table name**. Nothing
under `[keys]` can then be told apart by name — a table might be a component or an action binding —
so `BindingValue` (`heca-config/src/keys.rs`) distinguishes them by **shape**: a table is a
component, a string or list is a binding. That works until a component is called `unbind` or
`widgets`. Making the name a *field* removes the guess, and reads like `[[keys.command]]` and
`[[keys.mode]]`, which already exist.

The **`id`** is new and is the reason for the array form: bindings belonged to the component *type*,
so both placements shared one set. Now a placement can differ.

# Current state

- `BindingValue::Component(ComponentKeysConfig)` + `ComponentKeysConfig { bindings, bind, unbind }`
  in `heca-config/src/keys.rs`.
- `build_component_keymaps` (`heca/src/app/registry.rs`) merges defaults with user config and builds
  one `KeymapRegistry` per kind, keyed by `kind`.
- `focus_layer_action` (`heca/src/app/input.rs`) resolves the focused mount's kind layer, then the
  host focus layer.

# Steps

1. New `ComponentKeysConfig { name, id: Option<String>, bindings, bind, unbind }` parsed from
   `[[keys.component]]`. Delete `BindingValue::Component` and the shape-based discrimination with it.
2. Merge in two passes: every entry with no `id` for that `name` first, then entries whose `id`
   matches the placement, layered on top. **Keep the existing merge rules** — table merges per key,
   `[[…bind]]` merges by its `keys` field, `unbind` is keyed by combo and applied last.
3. Key the built maps by **placement**, falling back to the kind's map — `focus_layer_action` asks
   for the focused mount first, then its kind.
4. Update `keybindings.default.toml` and `README.md`; both currently document `[keys.<kind>]`.

# Edge cases

- An `id` naming a placement that does not exist: not an error (config is read before anything
  mounts), like every other binding — it simply never applies.
- Two entries with the same `name` and the same `id`: the later merges over the earlier, per key.
- The conflict report (`heca/src/app/conflicts.rs`) must cover the new path, at startup and reload.

# Acceptance

- One component's keys can be written once for all placements, and overridden for one.
- No table under `[keys]` is interpreted by its name.
- Whole workspace green, clippy 0 warnings.

---
**Completion summary:**
DONE — commit `8916345`, together with F003/P086/T366 (one pass: splitting them would have moved the workspaces keys twice). Verified by the user in the app on 2026-07-29: with the dock focused, j/k/h/l/Space/Tab all work, now sourced from `keybindings.default.toml` instead of `default_binding`. 1056 tests (+10), clippy 0 warnings.

SHAPE
`ComponentKeysConfig` (`heca-config/src/keys.rs`) is now a standalone struct with `name: String` + `id: Option<String>` + the flattened bindings/bind/unbind, and `KeysConfig.component: Vec<ComponentKeysConfig>` parses `[[keys.component]]`. `BindingValue::Component` and the shape-based discrimination are gone; `BindingValue` is Single|Many again.

BUILD (`heca/src/app/registry.rs`)
`build_component_keymaps` runs two passes: every id-less entry merges into a per-name base (defaults then user), then every id-ful entry is seeded from that finished base and layered on top. Run as two passes deliberately so file order does not decide what a placement inherits — there is a test for exactly that. Output is keyed by component name AND by placement id; a placement id equal to a component name wins, since it already contains the base. `layer_component_keys` carries the merge rules over verbatim from the old shape (table per key, `bind` by `keys`, `unbind` by combo, applied last).

`component_action` is the qualification rule: a name that resolves to a built-in at load keeps its catalog id; anything else is `<component>.<name>`; an already-qualified name is left alone.

`focus_layer_action` (`heca/src/app/input.rs`) now tries the focused mount id, then the component's kind.

CONFLICT LABELS
`[[keys.component]] <name>` / `[[keys.component]] <name>:<id>` / `…​.bind`, so a report names the entry to edit.

NOT TESTABLE HERE
The placement-then-kind lookup in `focus_layer_action` needs an `AppState`, which only exists with a window (`heca/src/app/startup.rs:346`). The user drove it.

DOCS
`keybindings.default.toml` (the doc block near the top plus a real `[[keys.component]] name = "workspaces"` block at the bottom) and `README.md` both rewritten for the new shape.

### ✅ 8a2b2402-26b7-49d2-90f8-ac60b206ff77 — T363 — Two keys every container has with nothing declared: global_focus per instance, and Esc

Status: ✅ `done`

# What

Every mounted container gets two keys without declaring anything:

- **`global_focus`** — focuses **that instance**, and works while the container does *not* have
  focus. Per placement, so two seatings of one component are separately reachable.
- **`Esc`** — returns the keyboard to the main region. Same for every component, always present.

This is what makes the phase's test pass: a dock that declares no actions and has no rows is still
usable — you can reach it, scroll it, and leave it.

# Current state

- `focus_dock` (`heca/src/handlers.rs:1583`) takes an optional dock id; bare opens a letter pick,
  `dock = "…"` focuses one directly. It is bound to `prefix+Shift+e` in `keybindings.default.toml`.
- `unfocus_dock` (`handlers.rs`, added F003/P085/T352) clears focus; it is bound to `Escape` in the
  `focus` mode keymap, which is a **host-owned layer**, not something a container has of its own.
- `sidebar_focus` / `prefix+e` still exists as a built-in and enters `InputMode::SidebarNav`.

# Steps

1. `global_focus` is read from the component's `[[keys.component]]` entry (F003/P086/T362) and bound
   into the **global** map at mount, aimed at `focus_dock { dock: <placement id> }`.
   - with an `id` on the entry → that placement;
   - with no `id` → the placement of that kind focused most recently, which is the rule
     `owning_mount` (`heca/src/providers/actions.rs`) already uses.
   It cannot live in the container's own layer: that layer is only consulted while the container
   already has focus, so the key would never fire.
2. `Esc` stays where it is (the host `focus` layer) but must be documented as a guarantee rather than
   a default a user could delete and leave a container with no way out. Decide with the user whether
   `[keys.component] unbind` may remove it — recommendation: no.
3. `sidebar_focus` and its `prefix+e` binding are retired; the workspaces component declares
   `global_focus = "prefix+e"` instead, which is the same keystroke reaching the same dock through
   the generic path. (This is the P085/T356 deletion, done here instead.)
4. Both keys go through the conflict report — two components claiming one `global_focus` is exactly
   what it exists to say out loud.

# Edge cases

- A component with **no** `global_focus` in config or defaults: reachable only through the
  `focus_dock` letter pick. That is fine and must stay working.
- Two placements, one entry with no `id`: one key, and it lands on the last-focused of that kind.
  Pressing it twice could later swap between placements — do not build that now.
- `Esc` while nothing has container focus is a no-op, not an error.

# Acceptance

- A container that declares nothing is focusable by a key and released by `Esc`.
- Two placements of one component can be given different `global_focus` keys.
- `prefix+e` reaches the workspaces dock with no `sidebar_focus` built-in in the tree.
- Whole workspace green, clippy 0 warnings. **The USER drives it in the app.**

---
**Completion summary:**
DONE — commit `209adc3`. Verified by the user in the app on 2026-07-29 (dock pick → `Esc` returns the keyboard; `prefix+e` unchanged). 1068 tests (+8), clippy 0 warnings.

DECIDED WITH THE USER BEFORE BUILDING
1. **`Esc` cannot be unbound.** Step 2's open question, answered NO.
2. **`sidebar_focus` stays alive through this task** (the task text's step 3 moved out). Retiring it here would have left `w`/`c`/`v`/`z`/`d` keyless until F003/P086/T369 provides their replacements. Every commit stays in a working state instead.

CONSEQUENCE: **the workspaces component has no `global_focus` line yet.** `sidebar_focus` still owns `prefix+e`, and giving workspaces the same combo now would be a real conflict. That line and `sidebar_focus`'s deletion land together in T369, in one commit. `prefix+Shift+e` (the letter pick) worked throughout, so nothing regressed. The mechanism is proven by unit tests rather than by config.

WHAT LANDED
- `build_component_keymaps` now returns `(layers, Vec<GlobalFocus>)`. `global_focus` is pulled out of the entry's bindings and never reaches the component's own layer — that layer is consulted only while the container already has focus, so a key to *take* focus placed there could never fire. `GLOBAL_FOCUS` is a reserved binding name.
- `bind_global_focus` (`heca/src/app/registry.rs`) puts each into the flat map — `prefix+…` to `normal`, bare to `global`, the same rule as any flat binding — aimed at `WmAction::FocusDock { dock: <id or component> }`, through `bind_with_conflict_tracking`, so collisions report and `--keys-show` finds them under `focus_dock`.
- `placement_for` (`heca/src/chrome/focus.rs`) resolves a focus target as **id first, then component**: focused seating of that component → last focused → first mounted. `handle_focus_dock` (`heca/src/handlers.rs:1595`) uses it, so `dock = "…"` from a key, RPC or a menu all accept either.
- `build_modes` re-asserts `Escape → unfocus_dock` into the `focus` layer after the merge. Its line was **removed** from `keybindings.default.toml`: it looked editable and was not, and it made `--keys-show` list the key twice.

TWO THINGS THE TESTS CAUGHT
- The `Dock` stand-in in `chrome/focus.rs` tests never overrode `kind()`, so it defaulted to `id()` and no component name could ever match — `placement_for`'s whole fallback path was untestable until it got a fixed kind.
- Rebinding `Escape` in the focus layer produces **two** conflict lines, not one: the user's binding displaces the shipped default, then the guarantee displaces the user's. Both are true, so the test asserts the report names `Escape` rather than pinning a count.

NOT UNIT-TESTABLE
`handle_focus_dock` itself needs an `AppState`. `placement_for` was split out as a pure function precisely so the resolution order is covered without a window.

### ✅ 7bca3b5f-625f-449d-a761-9f225b164466 — T364 — Container focus ends only when something takes it — and Space stops stealing the keyboard

Status: ✅ `done`

# What

Container focus must survive focusing a pane. It ends only when: the component's "activate and
leave" runs, `Esc`, a click outside every container, or another container takes it.

# The bug this fixes

F003/P085/T352 added a rule to `handle_focus_pane` (`heca/src/handlers.rs:113`): focusing a pane
clears container focus unless the sidebar is being navigated. The intent was "clicking into a pane
takes the keyboard back". The effect is that **`Space` no longer works**: the workspaces component's
`peek` asks the host to focus the pane, the rule fires, and the keyboard leaves the container — so
`j`/`k` stop. The user reported this directly (2026-07-29): *"It was this way but now is not."*

The rule is wrong as written. Whether the keyboard leaves is not a property of "a pane got focused";
it is a property of **what the user asked for**.

# Steps

1. Remove the release from `handle_focus_pane`. Focusing a pane changes which pane is active and
   nothing else.
2. Make each way out explicit:
   - the component's activate-and-leave path already queues `unfocus_dock` — keep it;
   - `Esc` → `unfocus_dock` (F003/P086/T363);
   - a click that lands outside every container → release (F003/P086/T364);
   - another container taking focus → `set_focused_container` already replaces, no work.
3. The three explicit release sites added in T352 to `app/input.rs` (`release_chrome_focus` on the
   sidebar-nav exits) become unnecessary once `SidebarNav` is gone; check whether they can go with
   this change or must wait for F003/P085/T356 step 5.
4. Test: `peek` leaves `focused_container` set; activate clears it. Both are unit-testable through
   `ProviderCx` without a window — see the existing tests in `heca/src/providers/workspaces/mod.rs`.

# Behaviours to preserve

- `state.focused_pane` is not cleared by container focus; only the keyboard is redirected, so
  `prefix+Enter` still splits the pane you last worked in.
- An unbound key while a container has focus is still swallowed, never forwarded to a backend.

# Acceptance

- With a dock focused, `Space` brings a pane to the front and `j`/`k` keep working.
- `Enter`/`l` focuses the pane and typing reaches the terminal.
- Clicking into a pane returns the keyboard to it.
- Whole workspace green, clippy 0 warnings. **The USER drives it in the app** — this is exactly the
  behaviour that was broken by a change that passed its tests.

---
**Completion summary:**
DONE — commit `89745b7` "fix(chrome): container focus survives a pane getting focused". Verified by the user in the app on 2026-07-29: `prefix+Shift+e` → pick the dock → `Space` works and the cursor keeps moving. 1047 tests (+1), clippy 0 warnings.

FILES
- `heca/src/handlers.rs:113` `handle_focus_pane` — the F003/P085/T352 release is gone; the body is now just `focus_pane_by_id`. `AppState::sidebar_nav_active()` (`heca/src/app_state.rs:895`) survives, still read by `heca/src/chrome/mod.rs:2942` for the sidebar highlight while a menu is open.
- `heca/src/mouse.rs:338` — the left-press "content click → focus" arm calls `handlers::handle_unfocus_dock` before returning `WmAction::FocusPane`. This is the "click outside every container" release: every earlier arm in `on_mouse_input` has already resolved inside a container or a chrome widget.
- `heca/src/app/input.rs:785` — doc comment on `release_chrome_focus` records why its two call sites stay.
- `heca/src/providers/workspaces/mod.rs` — new test `peek_and_activate_differ_only_by_the_release_they_ask_for`.

STEP 3 RESOLVED — the release sites do NOT go yet
Two call sites, not three (`heca/src/app/input.rs:726`, `:740`), both inside `handle_sidebar_nav_mode` (`:703`). They release on the legacy `SidebarNav` mode's own exits; the mode is still live and nothing else covers them. They go out with F003/P085/T356 step 5 — tracked by F003/P086/T369.

DELIBERATE NON-CHANGE — right-click on content (`heca/src/mouse.rs:423`)
Written, then reverted. `open_context_menu_for` (`heca/src/chrome/context_menu.rs:241`) captures `restorable_mode(state.input_mode)` as the overlay's origin; releasing first rewrites `SidebarNav` to `Normal`, so the menu stops restoring sidebar nav on close. A comment at the site says so and hands it to F003/P086/T365, which rebuilds the mouse story once the legacy mode is gone. **T365 must pick this up.**

FOR WHOEVER WRITES THE NEXT HOST-LEVEL FOCUS RULE
`AppState` is constructed in exactly one place — `heca/src/app/startup.rs:346` — and needs a window. There is no test constructor in the tree, so nothing at the `handle_*` level is unit-testable and T352's rule shipped with no test that could have caught it. Put the assertion on the component through `ProviderCx` (no window needed), and expect the user to drive the rest in the app.

### 🚧 2086fbfc-1d37-4e4a-af69-536692ca12cf — T365 — The mouse: clicking a container focuses it, and each item kind declares its own click

Status: 🚧 `in-progress`

# What

Two halves of the same thing.

**Host side:** a click inside a container's area focuses that container — the same state a key
reaches. A click outside every container releases it. This is how a user "enters focus mode" with the
mouse (user, 2026-07-29).

**Component side:** each **item kind** declares what click, double-click and right-click do on it. A
workspace row and a pane row genuinely differ; a label has none. These are **not rebindable** (you do
not rebind a mouse click) but they are still *named actions*, so a key, a menu entry and RPC reach
the same thing.

# Agreed behaviour

- Click on a row = **`Space`**: the container takes focus, the container's **cursor moves to the
  clicked row**, and the row's declared click action runs (for a pane row: focus the pane). The
  keyboard stays in the container.
- Click on a container where there is no row: takes focus, changes nothing else.
- Right-click: the **host** opens the menu; the item supplies the entries.
- Double-click: declared per item kind. Nothing is forced — a component that declares none has none.

# Current state

- `chrome_dispatch_press` (`heca/src/chrome/mod.rs:3410`) sends the press into the retained tree and
  reports whether a widget consumed it. Nothing tells the host **which container** it landed in.
- Every mount is already wrapped per placement in `focus_and_pick` (`heca/src/chrome/mod.rs:1522`),
  which is where a container's area is known.
- `nav::nav_key_at` (`heca-grid-ui/src/nav.rs`) resolves a point to the topmost, deepest row — the
  walk this needs a twin of.
- Row clicks today run a closure: `.on_activate(move || emit(InteractionIntent::FocusPane {…}))` in
  `heca/src/providers/workspaces/mod.rs`. That is native-only and unnamed, so RPC and menus cannot
  reach it and a plugin cannot express it.

# Steps

1. `container_at(pos)` in `heca-grid-ui` — the twin of `nav_key_at`, returning the innermost
   container id under a point. Generic: any container, including a plugin's, then gets click-to-focus
   with nothing declared.
2. On press: resolve the container, set focus to it; resolve the row, move that container's cursor to
   it (`ProviderCx::set_selected` / `set_container_cursor`). A press resolving to no container
   releases focus.
3. Give a row a way to declare its gestures by **name** rather than as a closure, so the same id is
   reachable from a key, a menu and RPC. `ViewNode` already carries `events: { press → Intent }`
   (`heca-view/src/lib.rs`), which is the described half; the native builder needs the same shape.
4. Right-click resolution moves onto row identity — this is F003/P085/T354 steps 5–7, currently
   parked in F003/P085/T356 step 4: `ContextTarget::Row { container, key }`, and the
   `ChromeDragItem` match in `heca/src/mouse.rs:143` deleted.

# Edge cases

- Clicking a row in container B while container A has focus: focus moves to B, B's cursor moves.
- The cursor and the click must be the **same** thing — click row 5 then press `j` and it must go to
  row 6. They are unrelated today.
- A press consumed by a widget inside the container (a scrollbar thumb) still focuses the container.
- Drag start must not be read as a click-to-focus twice.

# Acceptance

- Clicking any container focuses it; clicking a pane returns the keyboard to it.
- Clicking a row moves that container's cursor there, and `j` continues from it.
- A component declares its rows' click behaviour by name, and a plugin can do the same.
- No host code enumerates row kinds.
- Whole workspace green, clippy 0 warnings. **The USER drives it in the app.**

**Checklist:**
- [ ] DONE — scope_key/scope_at in heca-grid-ui; a click inside a container focuses it, outside releases it; documented in docs/widgets.md (12ef6ac)
- [ ] DONE — the click moves the container's cursor to the clicked row, via Provider::cursor_moved; every seating of a component shows the same cursor (24043d4)
- [ ] DONE — a focused container decides the context menu instead of InputMode::SidebarNav; that mode and its dead handlers deleted (ba52150)
- [ ] DONE — item(), item_running() and usize_arg() are pub(crate): shared menu-entry vocabulary, since the sidebar builders move out and build_pane_menu stays (edfef48)
- [ ] DONE — the three row menus moved into the component as Provider::context_menus; paths are workspaces.pane / .column / .workspace; ContextPath::SIDEBAR_* and their with_builtins registrations deleted; ContextPath::PANE stays host-owned; stale sidebar.* references swept from app_state.rs, contribution.rs and docs/widgets.md (05f3ac4)
- [ ] ContextTarget::Row { container, key } replaces SidebarPane/SidebarColumn/SidebarWorkspace. NOTE: with the builders now living inside the component, the host-resolved facts those variants carry (ws_idx, col_idx, custom_name) are redundant — the component reads its own model. Check ContextTarget::pane_id() (:84) and ws_idx() (:95), which retarget pane/workspace actions and have callers outside the menu code. The ChromeDragItem match in mouse.rs:143 goes with it.
- [ ] Named gestures per item kind: row clicks still run an unnamed closure (.on_activate), so RPC, menus and plugins cannot reach them; ViewNode already carries events { press -> Intent } and the native builder needs the same shape
- [ ] Rebuild the right-click content path (mouse.rs:455), which deliberately does not release container focus; the restorable_mode constraint that forced that is gone, and its comment is stale
- [ ] Clear the 9 remaining SidebarNav mentions, all in comments (mouse.rs:455, app_state.rs:680/729, chrome/state.rs:181, chrome/mod.rs:2952)

### ✅ 872713aa-1c7c-48c5-ac8c-d0561866eca7 — T366 — Delete default_binding; a key comes from the built keymaps, and heca --keys-show prints them

Status: ✅ `done`

# What

Remove `ActionMeta.default_binding` / `ActionDescriptor.default_binding`, answer "what key runs this
action" from the **built keymaps**, and add a command that lists every binding name and its key.

# Why

`default_binding` holds a key string on every action's metadata (`"prefix+h"`, `"Shift+PageUp"`).
For built-ins it has **never been read**: tooltips call `shortcut_for_action`
(`heca/src/shortcut.rs:49`), which reads the *config maps* — the user's file first, the bundled
defaults second. So the descriptors carry a second copy of every key in `keybindings.default.toml`
that nothing compares against; a descriptor could say `prefix+h` while the file says `prefix+k` and
nothing would notice. The only test on it asserts it is non-empty (`heca/src/actions.rs:2857`).

F003/P085/T355 made it live for components (`bind_component_default`, `heca/src/app/registry.rs`),
which is what created a real second source. Under the model agreed 2026-07-29 — core components ship
keys in the file, plugins register them at runtime — the field has no remaining job.

# Why the built keymaps are the right source

`shortcut_for_action` only sees the flat `[keys]` map. It cannot see `[[keys.mode]]` bindings, will
not see `[[keys.component]]` (F003/P086/T362), and can never see a plugin's runtime-registered key.
`keymap::Keymaps` (`heca/src/keymap.rs`) holds combo → action for **every** layer after everything
has registered. Reversing it gives action → keys, covering all of them uniformly, and it is already
rebuilt on reload.

# Steps

1. Add a reverse lookup over `Keymaps`: action id → the combos bound to it, per layer, with the layer
   named (flat / mode `<name>` / component `<name>[:<id>]`).
2. Point `ActionShortcuts` (`heca/src/chrome/`, built at load and reload) at it, so tooltips and
   `prefix+/` hints report the real binding including component and plugin layers.
3. Delete `default_binding` from `ActionMeta`, `ActionDescriptor` and every literal in
   `ActionRegistry::ALL`; delete `bind_component_default` and its call in
   `heca/src/providers/actions.rs`; delete the tests asserting the field is non-empty.
4. Core components' keys move into `keybindings.default.toml` under `[[keys.component]]`.
5. Plugins get `register_keybinding` beside `register_action` — the runtime path, since a plugin has
   no file.
6. `heca --keys-show`: binding name, action id, resolved key(s), and which layer. This is how a user
   discovers names now that they are not all in one file. Consider a `--json` form for scripting.

# Edge cases

- An action bound in several layers must list all of them, not the first — a component's `j` and a
  mode's `j` are both real.
- An action bound nowhere prints nothing rather than an empty string.
- The lookup must run **after** components register, and be rebuilt on reload — the same lifecycle as
  the conflict report (`heca/src/app/conflicts.rs`).

# Acceptance

- `default_binding` does not exist.
- A tooltip shows the right key for an action bound in a component layer.
- `--keys-show` lists core and plugin bindings alike, with their layer.
- Whole workspace green, clippy 0 warnings.

---
**Completion summary:**
DONE — commit `8916345`, in the same pass as F003/P086/T362. Verified by the user in the app on 2026-07-29. 1056 tests, clippy 0 warnings.

WHAT WENT
`ActionMeta.default_binding`, `ActionDescriptor.default_binding`, `ActionInfo.default_binding`, all 157 literals in `ActionRegistry::ALL`, `bind_component_default` + its two call sites, `rebind_provider_defaults`, `shortcut_for_action` (`heca/src/shortcut.rs`, now dead), and the three tests asserting the field was non-empty — including the `UNBOUND` list they needed.

WHAT REPLACED IT
`Keymaps::by_action: BindingIndex` (`heca/src/keymap.rs`) — `BTreeMap<action id, Vec<BoundKey { layer, key }>>`. Filled by `bind_with_conflict_tracking` itself, so it cannot describe a layer that was not built, and `unbind_and_deindex` removes an entry when its combo is retired (matched on the parsed combo, not the literal string). `build_keymaps` is the single entry point that builds all four layers plus the index; startup and reload both call it, so they cannot diverge.

**The reason it is not derived from the finished keymaps by reverse-mapping:** there is no `WmAction → name` function in the tree, and writing one for ~150 variants with args would be a second source that could drift. The bind sites already know the id as written, so the index is exact for free.

`ActionShortcuts::from_index` (`heca/src/chrome/mod.rs`) replaces `from_config`. It is now built in `resumed` and after reload, once every layer including plugins has bound — the first moment a tooltip can be told the truth. An action bound in several layers shows all of them joined.

`heca --keys-show` / `--keys-show --json` — new module `heca/src/app/keys_show.rs`, run from `main()` before the event loop, so a script needs no GPU. `render()` is pure and unit-tested (every layer listed, an action bound twice shows both, an unbound action prints nothing, JSON carries the same three facts). This is the first CLI flag in the binary.

`Provider::keybindings() -> Vec<(String, String)>` (`heca/src/providers/mod.rs`) — the plugin path, defaulting to empty. `bind_provider_keybindings` applies it at mount and after reload. **User config always wins:** `register_component_keybinding` skips a combo the layer already bound, and skips the action entirely if the index already holds a key for it in ANY layer — the index is built from config first, so an entry there means the user has spoken wherever they wrote it.

Core components' keys moved into `keybindings.default.toml` under `[[keys.component]] name = "workspaces"`. That file and `README.md` document the new shape and `--keys-show`.

WHAT THIS UNBLOCKS
F003/P085/T356 steps 3 and 5 — tracked by F003/P086/T369, which is now free to run once F003/P086/T363 lands `global_focus`/`Esc`.

### 📋 ec4ff2d9-f5d6-4ae1-b5ca-cda6fb0fd47c — T367 — The chrome context stops handing every component the workspaces model

Status: 📋 `planned`

# What

`ChromeCtx::for_build` takes the **workspace tree** and passes it to every component's build hook.
A Docker dock, a notes dock, a label showing one number — all receive a workspaces model they have no
use for, through the one context that is supposed to be component-agnostic.

This is the single most component-specific thing in the shared contract, and it is in the middle of
the seam every component uses.

# Current state

```rust
// heca/src/providers/mod.rs
pub fn for_build(
    app: App,
    tree: &'a WorkspaceTree,      // ← one component's model, for everybody
    programs: &'a ProgramsConfig,
    theme: &'a GuiTheme,
    emit: &'a ChromeIntentEmitter,
) -> Self
```

- `RenderInputs { tree, programs, theme, emit }` (`heca/src/providers/mod.rs`) and `ChromeCtx::tree()`
  expose it.
- The one caller that needs it is `build_body` in `heca/src/providers/workspaces/mod.rs`, which
  immediately projects it into rows.
- The tree now lives on `WorkspacesContainerState` (F003/P085/T356 step 1), reachable as
  `ctx.state().workspaces().tree()` — so the component can read its **own** model without the host
  threading it in.
- `programs` has the same smell one level down: the program catalog is a workspaces/pane concern, not
  every component's.

# Steps

1. Drop `tree` from `RenderInputs` / `for_build` / `ChromeCtx::tree()`; the workspaces component
   reads its model from its own state instead.
2. Look at `programs` with the same question: does a component that is not about panes need it? If
   not, it moves the same way — the workspaces state already mirrors per-pane runtime
   (`PaneRuntimeSignals` in `heca/src/chrome/state.rs`).
3. What is left in the context should be true for **any** component: the host facade, the theme, and
   the intent sink. Write that rule into the type's doc comment so the next addition has to justify
   itself against it.
4. Check `BuildCx` (`heca/src/chrome/mod.rs`) the same way — it carries the signal / drag / hint
   registries, which are genuinely host-owned and generic, plus `container_id`. That one looks right;
   confirm rather than assume.

# Edge cases

- A context built to read metadata rather than render (`ChromeCtx::new`) already has no render
  inputs — that stays.
- The workspaces build hook must keep working with the tree borrowed from the store, and the borrow
  must not be held across anything that reaches back into the store (it is a `RefCell`).

# Acceptance

- Nothing in `ChromeCtx` names a workspace, a column or a pane.
- A component that knows nothing about panes can be built with the same context.
- Whole workspace green, clippy 0 warnings.

### ✅ 756e787b-3a0d-4c42-8007-88562969d12b — T369 — Unpark F003/P085/T356 — P086 is not done until the workspaces container is on the contract

Status: ✅ `done`

# What this task is

A **pointer, not new work.** The work itself lives in **F003/P085/T356 `F3J4W`**, which was set to
`blocked` on 2026-07-29 because it cannot proceed until this phase lands its key plumbing. This task
exists so P086 cannot be marked done while T356 sits forgotten. Do it **last**.

# Why T356 is parked (verified in the code, not inferred)

`keybindings.default.toml` ships **no** `[keys.workspaces]` block. Every key the user verified in the
app — `j`/`k`/`h`/`l`/`Space`/`Tab` — comes from `ActionMeta.default_binding` (`heca/src/actions.rs:313`)
via `bind_component_default` (`heca/src/providers/actions.rs:57`), re-applied after a config reload by
`rebind_provider_defaults` (`heca/src/providers/actions.rs:83`).

That field and that function are exactly what **F003/P086/T366** deletes, and **F003/P086/T362**
replaces the config shape that must carry the keys instead. So T356's remaining steps had no way to
give an action a key that survives this phase.

# What is still outstanding in T356

- **Step 3** — move the five mutation built-ins into the component's `actions()` / `perform()`
  (`heca/src/providers/workspaces/mod.rs:154` and `:214`, which today declare only `cursor_up`,
  `cursor_down`, `collapse_row`, `activate_selected`, `peek_selected`, `toggle_selected`):
  `handle_sidebar_create_workspace` (`heca/src/handlers.rs:1870`), `handle_sidebar_create_column`
  (`:1884`), `handle_sidebar_split_in_column` (`:1904`), `handle_sidebar_zoom_selected_column`
  (`:1914`), `handle_sidebar_delete_selected` (`:2145`). Their two helpers,
  `sidebar_selected_workspace_idx` and `sidebar_selected_column_target`, are pure reads of the
  component's own model and move with them.
  The user's letters are **`w`** new workspace, **`p`** new pane, **`c`** new column, **`x`** delete
  what the cursor is on — this replaces today's `v` and `d`, so the "muscle memory survives" note in
  T356's own text no longer holds. Ship them under `[[keys.component]]` (T362's shape), never
  `default_binding`.
- **Step 5** — delete `InputMode::SidebarNav` (`heca/src/app_state.rs:58`), `handle_sidebar_nav_mode`
  (`heca/src/app/input.rs`), `restorable_mode` (`heca/src/chrome/context_menu.rs:334`), the eleven
  `sidebar_*` built-ins with their `WmAction` variants / `action_from_name` arms / `action_priority`
  arms / handlers / registrations / descriptors / RPC commands, and the
  `[[keys.mode]] name = "sidebar"` block (`keybindings.default.toml:448`). ~140 references, still live
  across `heca/src/{providers/mod.rs,actions.rs,input.rs,handlers.rs,app/input.rs,app/registry.rs}`.
  Also delete the single bridge `publish_sidebar_selection` → `selection_nav_key`, which T354 left
  behind precisely so there is one place to remove here — do not add a second.
- **Step 6** — docs + tests.

Step 4 (context menu → `ContextTarget::Row`) already moved out to **F003/P086/T365**; it is not
T356's any more.

# Prerequisites — all inside this phase

1. **T362** `[[keys.component]]` — the shape that carries w/p/c/x.
2. **T366** delete `default_binding`; a key comes from the built keymaps.
3. **T363** `global_focus` + `Esc` — needed before `sidebar_focus` can be retired in step 5.

# Steps

1. Confirm T362, T366 and T363 are done.
2. `planner-task-update F003/P085/T356 --status in-progress` and finish steps 3, 5, 6 there.
3. Only then close this task and the phase.

# Edge cases

- Deleting the `sidebar_*` built-ins before step 3 has working keys removes `w`/`c`/`v`/`z`/`d`
  outright — order matters, step 3 first.
- `WorkspacesContainerState::tree()` is a `RefCell`: end the borrow before any call that reaches back
  into the store, or it panics at runtime instead of failing to compile. Expect more of these as step
  3 moves handlers in.
- BSD `sed` has no `\b` — use Python for the word-boundary renames step 5 needs.

# Acceptance

- F003/P085/T356 is `done`, not `blocked`.
- No `SidebarNav`, no `sidebar_*` action, no fixed-row geometry anywhere in the tree.
- Whole workspace green, clippy 0 warnings. **The USER drives it in the app.**

---
**Completion summary:**
DONE — the pointer did its job. F003/P085/T356 is `done` (commit `7cc63b1`), not blocked.

Its three prerequisites all landed in this phase first, exactly as this task required: T362 (`[[keys.component]]`), T366 (`default_binding` deleted; keys from the built keymaps), T363 (`global_focus` + `Esc`).

See T356 for the full record. Two things it left behind, both now owned elsewhere:

1. **`InputMode::SidebarNav`'s husk** — nothing enters it, but `resolve_context_for` reads it to decide which menu `prefix+>` opens. The replacement signal is container focus, which is F003/P086/T365's design work; written up there in full, together with the resulting live regression and the settled rule that a container's context menu is always provided by the component itself.
2. **RPC parity** — the twelve `sidebar-*` commands were removed and their replacements are unreachable, because `parse_rpc_command` returns a closed `WmAction` enum and a component's action is an `Intent`. Affects every component and plugin action. **Still needs its own task.**

### ✅ 8517a679-984d-4562-a5cb-0980cab2defa — T370 — One word for closing a pane: the confirm dialog and its toggle say Close, like the action already does

Status: ✅ `done`

# What

Make every user-facing name for closing a pane agree on **Close**. Surfaced by the user reading
`heca --keys-show` output during F003/P086/T366 and asking what `close` was — because the confirm
dialog they knew said "Delete".

# The drift

One action carries three words today:

| where | reads |
|---|---|
| action id + `[keys]` binding name | `close` |
| catalog label (palette, tooltip, context menu) — `heca/src/actions.rs:920` | "Close Pane" |
| **confirm dialog button** — `heca/src/actions.rs:2414` | **"Delete"** |
| **confirm toggle in config** — `[confirm]` | **`delete_pane`** |

Three of the four already say close, so one value and one verb change. Going the other way would
mean renaming the action itself, which breaks the binding name in every user's `keybindings.toml`.

Note `heca-config/src/confirm.rs` already documents its keys as "`close` / `delete_column` /
`delete_workspace`" and its tests already use `close` — so the code disagreed with its own docs.

# ✅ DECIDED with the user (2026-07-29)

**Close for the pane, Delete stays for the column and the workspace.** Not an inconsistency: "close"
carries a reversible connotation that is true of a pane (one process ends, the layout absorbs the
gap) and false of a workspace (every column, pane and process inside it is destroyed, recursively).
The split is information — **Close** for the thing you work *in*, **Delete** for the containers you
*manage*. `delete_column` / `delete_workspace` are also existing action ids and binding names
(`delete_current_column = "prefix+Shift+x"`), so renaming them would break configs for no gain.

# Steps

1. `heca/src/actions.rs:2414` — `("close", mk("delete_pane", "Delete"))` → `("close", mk("close", "Close"))`.
2. Reword the doc comments that explain the split as a deliberate `close` → `delete_pane` divergence:
   `heca/src/actions.rs` (the `ActionMeta::confirm` field, `with_builtins`, `confirm_spec`,
   `builtin_confirm_specs`), `heca/src/handlers.rs` (`confirm_owner_name`, `maybe_confirm_destructive`).
   **Keep the mechanism**: `ConfirmSpec::config_name` stays a separate field, because one spec still
   governs both `ClosePane` and `ClosePaneById` (§5.1). Only heca's own divergent *value* goes.
3. Tests: `heca/src/actions.rs` `builtin_confirm_specs_are_declared_for_the_destructive_actions`
   (`("close", "delete_pane")` → `("close", "close")`) and `heca/src/rpc.rs:684`
   (`describe-action close` → `Some("close")`).
4. `ConfirmConfig::enabled` (`heca-config/src/confirm.rs`) — accept `delete_pane` as a retired alias
   for `close`. **Required, not optional:** a silent rename turns a user's disabled prompt back on.
5. Docs: `config.default.toml:112`, `README.md:836` + `:849`, `AGENTS.md:1063`, `docs/widgets.md:3784`.
   `BACKLOG.md:1635` is stale planning text — leave it.

# Edge cases

- A config with **both** `delete_pane` and `close` set: the current name wins.
- `[confirm] delete_pane = false` alone must still disable the prompt, or the rename is a regression.

# Acceptance

- The dialog button reads **Close**; the palette, the context menu, the binding name and the toggle
  all read `close`.
- An existing `[confirm] delete_pane = false` still disables the prompt.
- Whole workspace green, clippy 0 warnings. **The USER sees the dialog in the app.**

---
**Completion summary:**
DONE — commit `5c21f97`. 1057 tests (+1), clippy 0 warnings. **Not yet seen in the app**: the user should trigger a pane close once and check the dialog button reads "Close".

DECIDED WITH THE USER (2026-07-29)
Close for the pane; Delete stays for the column and the workspace; nothing whose rename would break a config was renamed. `close`, `delete_column` and `delete_workspace` all keep their action ids and `[keys]` binding names.

WHAT CHANGED
- `heca/src/actions.rs:2418` — `("close", mk("close", "Close"))`. The two sibling rows are untouched.
- `heca-config/src/confirm.rs` — `RENAMED: &[(&str, &str)]` with `("delete_pane", "close")`, consulted by `ConfirmConfig::enabled` after the current name and before the declared default. Required, not cosmetic: without it a user who had written `[confirm] delete_pane = false` silently gets the prompt back. Test `a_retired_toggle_name_still_disables_its_prompt` covers the alias and the both-set case (current name wins).
- Doc comments reworded in `heca/src/actions.rs` (the `ActionMeta::confirm` field, `with_builtins`, `confirm_spec`, `builtin_confirm_specs`) and `heca/src/handlers.rs` (`confirm_owner_name`, `maybe_confirm_destructive`). **The mechanism stayed**: `ConfirmSpec::config_name` is still a separate field because one spec still governs both `ClosePane` and `ClosePaneById` (§5.1) — only heca's own divergent *value* went.
- Tests: `builtin_confirm_specs_are_declared_for_the_destructive_actions` now expects `("close", "close")`; `heca/src/rpc.rs:684` expects `Some("close")`.
- Docs: `config.default.toml`, `README.md` (the `[confirm]` block plus a note on the rename and on why Close/Delete differ), `AGENTS.md:1063`, `docs/widgets.md`. `BACKLOG.md` left alone as stale planning text.

NOTE FOR LATER
`heca-config/src/confirm.rs` already documented its keys as "close / delete_column / delete_workspace" and its own tests already used `close` — the code had drifted from its own docs. The `RENAMED` table is the place to add any future toggle rename.

---
**Completion summary:**
DONE — commits `5c21f97` (the spec, the toggle, the docs) and `080473a` (the dialog title and the menus). Verified by the user in the app on 2026-07-29. 1060 tests (+4), clippy 0 warnings.

THE FINAL VOCABULARY
`close` — action id, `[keys]` binding name, `[confirm]` toggle. "Close Pane" — catalog label. "Close pane" — both context menus (content and sidebar). "Close Pane?" over a **Close** button — the confirm dialog. A column and a workspace keep **Delete**: they destroy every pane and process inside them, which is a different act and should not read the same.

IT TOOK THREE PASSES BECAUSE THE WORDING HAS FOUR INDEPENDENT SOURCES
None of them referenced each other, so changing one moved nothing else:
1. `ActionDescriptor.label` (`heca/src/actions.rs:920`) — palette, tooltips.
2. `ConfirmSpec.buttons[].label` (`heca/src/actions.rs:2418`) — the dialog **button**.
3. `confirm_title` (`heca/src/handlers.rs:2054`) — the dialog **title**, computed from the resolved `WmAction` and never from the spec.
4. Hand-written `DropdownItem` labels (`heca/src/chrome/context_menu.rs`) — one per menu, `:432` content pane and `:507` sidebar pane, which had drifted apart from each other.
The user found (2) → (3) → (4) by reading the app each time. **Anyone changing an action's verb must touch all four.**

WHAT NOW GUARDS IT
- `handlers.rs` `mod confirm_wording_tests` — `the_title_and_the_button_use_the_same_verb` asserts each destructive title starts with its own button's verb; `a_pane_closes_and_a_container_deletes` pins the split so the first cannot pass by both saying the same word. `confirm_title_for(action, target)` was extracted as a pure fn precisely so this is testable without an `AppState`.
- `context_menu.rs` `a_destructive_entry_uses_the_same_verb_as_the_action_it_names` — compares menu label to catalog label for `close` / `delete_column` / `delete_workspace` only. **Deliberately not every entry**: a first attempt did that and failed on "Zoom / unzoom" vs "Toggle Column Zoom", which is better menu wording, not drift. Only the verb is compared — menus are sentence case, the catalog is title case.

COMPAT
`ConfirmConfig::enabled` (`heca-config/src/confirm.rs`) has a `RENAMED` table, `("delete_pane", "close")`, so a config that had switched the prompt off does not silently get it back. That table is where any future toggle rename goes.

PROCESS NOTE FOR THE NEXT SESSION
The user stopped this task twice for writing code while a question was still open. **When a question is on the table — theirs or mine — reply in prose and wait for the answer.** A defect report on just-shipped work is not authorisation either: say what is wrong and what the fix would be, then wait.

### 📋 195bc451-0ad1-402e-996a-c2ec28b28955 — T371 — The focus domain has four states, not two — and an overlay that covers the panes is one of them

Status: 📋 `planned`

# Why

`ActionPolicy` is judged on the wrong axis, and a plugin overlay is what makes it obvious.

`policy_allows` (`heca/src/app/interaction.rs:536`) takes `&Session` and nothing else, and
`FocusDomain` is `Tiled | Floating`, read off the active workspace (`is_floating_domain`, `:591`).
So the six policy values can only ever answer **"is a pane floating?"**. They cannot see:

- **container focus** — it lives in `chrome_state`, not the session;
- **overlays** — they live in `state.layers`.

Everything else was handled *outside* the policy system by ad-hoc checks:

- `route_interaction` (`:431`) blocks **everything** while `top_modal` is up — total, and blunt.
- For a focused container there is **no gate at all**. Component actions happen to be unreachable by
  keyboard unless the dock is focused, because their keys live in a layer only consulted then — but
  that is a property of key *routing*, not a declared rule. The palette, RPC and context menus reach
  them from anywhere. That is exactly how F003/P085/T356 shipped five mutations as
  `ActionPolicy::Global` with nothing to catch it.

**The user's case (2026-07-29):** a plugin opens a non-modal overlay covering the scrolling area and
the scrollbars. Today nothing stops `prefix+Enter` adding a pane you cannot see, or `zoom`/`close`
acting on panes underneath. A modal overlay is already safe by the blunt rule above; a non-modal one
is not.

# ✅ AGREED — a domain, not a new policy value

Policy says **what kind of act this is** (`TiledOnly`, `FocusedPaneLocal`, `WorkspaceLevel`) and is
already fine. Domain says **what owns the screen right now**, and is the half that is under-modelled.

| domain | meaning |
|---|---|
| `Tiled` | a pane has the keyboard |
| `Floating` | a floating pane is active |
| **`Container`** | a dock has the keyboard |
| **`Overlay`** | something covers the tiled area |

Everything then falls out of rules that already exist:

- `TiledOnly` blocks in `Floating` **and** `Overlay` — the user's `prefix+Enter` case, solved with no
  plugin declaring anything about actions it does not own.
- **`ActionPolicy::ContainerFocused`** — allowed only in the `Container` domain. This is what every
  component action actually means, and it makes the palette/RPC path safe for free. It was first
  proposed as a standalone policy value; under this framing it is one row in the same table, not a
  special case.
- A plugin overlay declares **its coverage** — does it obscure the tiled area, yes or no. One flag on
  the overlay. **A plugin must never declare policy for actions it does not own**, or every plugin
  ends up knowing about `split_horizontal`.

# Steps

1. Widen `FocusDomain` (or introduce an app-level `Domain` beside it) to the four states. Container
   focus comes from `chrome_state.focused_container()`, overlay from `state.layers`.
2. Compute the domain in `route_interaction` from `&AppState` and **pass the value** into
   `route_interaction_for_session`. Keep that function pure — it is session-only today "extracted for
   testability" and must stay testable; it just needs the truth handed to it.
3. Add `ActionPolicy::ContainerFocused` and move the component-declared actions onto it where it is
   more accurate than `TiledOnly` (`heca/src/providers/workspaces/mod.rs`, the `mutates` group).
4. Give an overlay a **covers-the-tiled-area** flag; derive the `Overlay` domain from it.
5. Retire the `top_modal → Block` special case in `route_interaction` once the domain covers it —
   a modal is simply an overlay with coverage.
6. Export `ActionPolicy` beyond `pub(crate)`. It is documented as "**private** to the interaction
   module", so a plugin outside the crate cannot name the thing it is required to declare.

# What is NOT in scope

- **Config-assignable policy.** Discussed and rejected 2026-07-29: policy is a correctness property
  the action's author knows and the user does not; setting it in config unlocks misbehaviour, not a
  feature. Named policy sets were considered too — a set assigned per component still has to name
  actions inside it, so the indirection buys nothing until several components share one rule. If a
  concrete case appears, the defensible version is **deny-only**: config may make an action more
  restricted, never less, and a loosening attempt is refused with a startup report line.
- **Dynamic availability** ("a row is selected", "the daemon is up") is already solved: `perform`
  returns `Handled::No` and a silent decline is a real answer. Policy is static domain gating; the
  two must not be conflated.

# Edge cases

- Container focus **and** a floating pane at once: the container has the keyboard, so `Container`
  wins for keyboard-sourced interactions. Decide explicitly rather than by match order.
- An overlay that covers only part of the screen (a dropdown, a tooltip) is **not** `Overlay` — the
  flag is about the tiled area specifically.
- `Global` must keep meaning "always", including `Overlay`, or `reload_config` stops working.

# Acceptance

- A non-modal overlay covering the panes blocks `prefix+Enter`, zoom and close, with no plugin code.
- A component action declaring `ContainerFocused` cannot be run from the palette or RPC while its
  container does not hold focus.
- `route_interaction_for_session` is still a pure, unit-tested function.
- Whole workspace green, clippy 0 warnings. **The USER drives it in the app.**

### 📋 9ba34c54-cdc5-4225-9a7a-41eb356239b1 — T372 — RPC can only reach built-ins — a component's or plugin's declared action has no path in

Status: 📋 `planned`

# Why

`parse_rpc_command` (`heca/src/rpc.rs:146`) returns a **`WmAction`** — a closed enum. A component's or
plugin's declared action is not a variant of it and never can be: that is the whole reason
[`ActionRef::Dynamic`] and name-keyed dispatch exist. So **no component or plugin action is reachable
by RPC at all**, today.

That contradicts what the contract promises. `Provider::actions`'s own doc says the declaration alone
buys "the palette entry, the icon and label, `describe-action` and **RPC**" — and `describe-action`
does work, because it reads the catalog. *Running* the action does not.

**How it surfaced.** F003/P085/T356 removed the twelve `sidebar-*` RPC commands along with the
built-ins they named. Their replacements are the workspaces component's declared actions
(`workspaces.cursor_up`, `workspaces.delete_selected`, …), which this parser cannot express — so a
capability that a script could drive is now unreachable. The comment at the site says so and
`test_sidebar_commands` asserts the old names are gone. The gap is **not** specific to those twelve:
it is every component and plugin action.

# What already exists to build on

The dispatch half is done. `dispatch_view_intent` (`heca/src/app/interaction.rs`) already resolves a
name **either** to a built-in (via `build_action` / `action_from_name`, args and all) **or** to a
name-keyed action registered at runtime, and routes both through the same `policy_allows`. It takes an
`Intent` — an action name plus args — which is exactly what an RPC caller has.

So what is missing is a **parse** path that can yield an `Intent`, not another `WmAction`.

# Steps

1. Give the RPC surface a generic command — `action <name> [key=value …]` — that builds an `Intent`
   rather than a `WmAction`.
2. Return a type that can carry either: `enum RpcCommand { Builtin(WmAction), Intent(Intent) }`, or
   have the parser produce an `Intent` for **every** command and let the existing built-in resolution
   in `dispatch_view_intent` do the rest. The second is smaller and removes a whole parallel
   vocabulary, but it changes `parse_rpc_command`'s signature, which many tests assert on.
3. Judge args against the declaration, as the other doors do — `report_arg_problems` already exists
   and is what makes a misspelled argument say so instead of silently defaulting.
4. `--keys-show` has the same shape of answer for keys; consider whether `list-actions` should mark
   which actions RPC can actually run, so the gap cannot silently reappear.

# Edge cases

- An action whose component is **not mounted**: not an error. Like a binding, it resolves at call
  time and simply does nothing — but RPC should say so in its reply rather than succeeding silently.
- Policy still applies: a `TiledOnly` action called over RPC while a float owns the domain is blocked,
  exactly as a keypress would be.
- The removed `sidebar-*` command names must **not** come back as aliases; their replacements are the
  component's action ids.

# Acceptance

- `heca action workspaces.cursor_down` moves the dock cursor.
- A plugin action registered at runtime is callable the same way, with args.
- Calling an action whose component is unmounted reports that, rather than reporting success.
- Whole workspace green, clippy 0 warnings.
