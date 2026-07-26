# heca — Agent Guide

> Everything an AI coding agent needs to work effectively on the heca project.
> Last updated: 2026-06-15

---

## ⛔ STOP — read this before writing code (the two mistakes that get work rejected)

These are made over and over. **Violating either = redo.**

### 1. UI work → use the existing `heca-grid-ui` widgets. They exist. There is a showcase.
- **Before building ANY UI**, look at what already exists:
  - **Widget catalog + recipes:** [`docs/widgets.md`](docs/widgets.md) (every widget + a "Drag and drop" section + patterns).
  - **Layering / overlays / KeyHint visibility:** the planner (F003/P019) — see the planner (F003/P019) — the surface-tree model that decides which layers/buttons are interactive. **Required reading before adding any layer, surface, overlay/modal, exposé, or a button on a new surface.**
  - **The living reference:** run the showcase — `cargo run -p heca-renderer --example showcase` —
    it exercises **every** widget + chrome recipes. Look at it before hand-rolling anything.
  - Widgets available today (non-exhaustive): `Flex`, `Surface`, `Row`, `Item`, `ItemGroup`,
    `DockFrame`, `MarkerGroup`, `ChromeRegion`, `RailCell`, `KeyHint`, `Grid`, `Icon`, `Badge`, `Tag`, `Button`,
    `Label`, `Input`, `Select`, `Choice`, `Overlay`, `Dialog`, `CommandPalette`, `Toast`, `Tabs`, `Pane`, …
    (There is **no `Modal` widget** — it was deleted; `Overlay` is the base overlay layer
    (blocking = a layer property) and `Dialog` — which composes it — is the confirm/modal widget;
    the app builds one from a `ModalSpec` via `OverlayHost::open_modal`. The app-side `ModalSpec` /
    `ModalResult` / `ModalAction` types and the `Modal` layer *band* are still real.)
- **New UI = a proper, GENERIC, theme-driven `heca-grid-ui` widget** — embed `Base`, impl
  `Component` + builder traits, read **ALL** styling from `Theme` (colors/font/radius/border).
  **NEVER** ad-hoc inline `Flex`/`Surface` with hardcoded sizes/colors in the app. Domain-neutral
  (never name a widget for workspace/column/pane). Full rule: **§ "Creating new widgets"** below.

#### FUNDAMENTAL — the styling/layout contract (violating any of these = redo)
- **Don't invent widgets.** Use the library widgets. Never hand-roll UI in the app.
- **Only library-provided values.** No hardcoded size/padding/alpha/color/spacing **anywhere** —
  every value comes from the `Theme` or a widget **variant**. Magic numbers are a bug.
- **The caller only picks a semantic variant; the widget owns its styling.** e.g.
  `IconButton::new(icon).size(WidgetSize::Small)` — the widget derives icon px, padding, cell from
  the theme font internally. Widgets **must expose methods/properties for their variants**.
- **No calculations on size / layout / position / style — not in call sites, not in `paint`.**
  The **parent (`Flex`) handles positioning/layout**; the **widget handles its own styling**. No
  `header_icon_size(font)` / `padding_xy(font*0.5, …)` math at call sites or in rendering.
- **The library is fixed ONLY on a proposal the maintainer reviews and accepts.** If a widget/theme
  lacks a variant, token, or method you need, do **NOT** hand-calculate a workaround and do **NOT**
  change `heca-grid-ui` unilaterally — **propose** the addition (what + why), get the OK, then
  implement. Track pending gaps in the plan (e.g. BACKLOG `gridui-styling-foundation`).

### 2. Behavior/keys → register through the registries. NEVER hardcode.
- **Every action goes through `ActionRegistry`** (`heca/src/actions.rs`): `registry.register(...)` +
  `registry.execute(...)`. **Registry bypasses are bugs.** No direct state mutation from input code.
- **Every keybinding goes through `KeymapRegistry`** (`heca/src/keymap.rs`) and is **configurable in
  `config.toml`** — never hardcode a key→behavior mapping in handlers. heca is tmux-like: bindings
  go through the prefix (see § Keybinding Style).
- **EVERY setting → `config.default.toml`, EVERY keybound action → `keybindings.default.toml`.**
  These embedded files are the **single source of truth** for defaults. Adding a `SettingsConfig`
  field (`heca-config/src/settings.rs`) or a bound `WmAction` but **not** writing it into the
  matching default file is a **bug** — the default then lives only in code (`#[serde(default=…)]`),
  invisible and undiscoverable to the user. Whenever you add/rename a setting, add it (with a
  comment) under the right `[section]` in `config.default.toml`; whenever you add a keybound action,
  add its default in `keybindings.default.toml`. Parameterized, non-config-bindable actions (e.g.
  `SubmitOverlay`) are the only exception to the keybinding file. When in doubt, **audit** that every
  `SettingsConfig` field appears in `config.default.toml` and every default binding is present.
- Adding an action? Follow the **"Adding New Actions" checklist** (§ below) end to end — `WmAction`
  variant → `action_from_name` → priority → handler → `build_registry` → default binding → descriptor
  → **declared `args`**.
- Rule: a capability must be reachable from **mouse + keybinding/action + RPC**, never one surface only.

> If a change touches UI or input and you didn't open `docs/widgets.md`/the showcase, or didn't go
> through the registries, stop and redo it.

### 3. No "for now" fixes left behind.
- Do **not** land temporary workarounds, degraded fallbacks, or "we'll fix this later" code as the final state of a task.
- If a bug needs a real architectural fix, implement that fix in the same task before closing it.
- If work truly cannot be completed in the task, record the follow-up explicitly in the repo's tracked plan/task files **before** stopping. Untracked cleanup debt is a bug.

---

## What This Is

**heca** is a native, GPU-accelerated tiling workspace compositor for developers. Think tmux meets NIRI meets Neovide — but rendered in a single GPU window via `wgpu` and `cosmic-text`, not in a terminal.

### Core Value

A keyboard-native workspace where every tool lives in a tiled, floating, or scratchpad pane — all rendered in one GPU-accelerated window, fully restorable across sessions.

### Key Differentiators

| Feature | Why It Matters |
|---------|---------------|
| GPU-native text rendering | Sharp fonts, ligatures, smooth animations. Terminal emulators can't match this. |
| Single-frame compositing | All panes + chrome render in one GPU pass. No window seams between panes. |
| NIRI-inspired scrolling columns | Horizontal scrollable columns (not BSP tree). View offset animates when switching focus. |
| tmux-style prefix bindings | `Ctrl+B → key` avoids conflicts with hosted terminal applications. |
| Cross-platform from day one | Linux, macOS, Windows. All GPU via wgpu. |
| Registry-based action dispatch | Every WM action goes through `ActionRegistry` — traceable, hookable, scriptable. |
| Config reload at runtime | `prefix+Shift+r` reloads keymaps, themes, settings without restart. |

---

## Architecture (NIRI Scrolling Layout)

```
Session                          ← manages all workspaces + overview/expose mode
├── workspaces: Vec<Workspace>   ← arranged VERTICALLY (discrete switching)
│   └── Workspace
│       ├── scrolling: ScrollingSpace   ← horizontal COLUMNS (continuous scroll)
│       │   ├── view_offset: ViewOffset ← animated horizontal scroll + snap
│       │   ├── columns: Vec<Column>
│       │   │   ├── width: ColumnWidth  ← Proportion | Fixed
│       │   │   ├── panes: Vec<Pane>    ← vertical stack within column
│       │   │   └── pane_sizes: Vec<Size>
│       │   └── active_column_idx
│       └── floating_panes: Vec<FloatingPane>
├── overview: OverviewState      ← zoom progress, open/closed
├── workspace_switch: WorkspaceSwitch  ← animated vertical transitions
└── active_workspace_idx
```

### Layout Rules (from NIRI)

1. **Opening a new window does not affect sizes of existing windows.**
2. **The focused window does not move around on its own.**
3. **Column widths are NOT normalized** — each column keeps its own width; columns overflow the viewport (that's the point of scrolling).
4. **Only the active column's width changes on resize.** Other columns are untouched.
5. **Animations are driven by easing functions** (spring physics planned).

### Scrolling Model

| Axis | Container | Scroll Type | Mechanism |
|------|-----------|-------------|-----------|
| **Horizontal** | `ScrollingSpace.columns` | Continuous scroll + snap | `ViewOffset` (animated `f64`) |
| **Vertical** | `Session.workspaces` | Discrete switch + animation | `WorkspaceSwitch` (animated index) |

### Keybinding Style (tmux-style prefix)

```
Ctrl+B → h / ←    Focus column left (animated scroll)
Ctrl+B → l / →    Focus column right (animated scroll)
Ctrl+B → j / ↓    Focus pane down
Ctrl+B → k / ↑    Focus pane up
Ctrl+B → Enter Split horizontal (new column to the right)
Ctrl+B → v    Split vertical (new pane in current column)
Ctrl+B → x    Close active pane
Ctrl+B → f    Toggle pane floating
Ctrl+B → q    Quick-select pane (overlay letters, all workspaces)
Ctrl+B → Shift+q  Quick-swap pane (stay at current position)
Ctrl+B → m    Swap and focus (follow to destination)
Ctrl+B → =    Increase column width
Ctrl+B → -    Decrease column width
Ctrl+B → [    Move pane to column left
Ctrl+B → ]    Move pane to column right
Ctrl+B → e    Enter sidebar navigation mode
Ctrl+B → w    Create workspace + pane
Ctrl+B → Shift+w  Rename workspace
Ctrl+B → Shift+c  Rename active column
Ctrl+B → $    Rename active pane/tab
Ctrl+B → i    Toggle focus (local, same workspace)
Ctrl+B → Shift+l  Toggle focus (global, cross-workspace)
Ctrl+B → b    Toggle left sidebar
Ctrl+B → r    Enter resize mode (sticky)
Ctrl+B → Shift+r  Reload config at runtime
Ctrl+B → p    Command palette (backend ready, UI pending)
```

**Key rules:**

- Prefix mode is intentional (like tmux), NOT a bug. This avoids conflicts with hosted apps.
- The prefix key is **configurable** via `prefix = "ctrl+b"` in config.toml.
- All keybingings should be configurable in config.toml.
- All actions should be registered in the action registry and accessible with keybindings and from the RPC.
- Important app-wide rule: actions must not be trapped behind a single input surface. Design app features so they are reachable through mouse/UI, keyboard via actions/keybindings, and RPC whenever they are meaningful on those surfaces. Example: moving a container from the left sidebar to the right sidebar must be doable by mouse interaction, by keybinding via an action, and by RPC.
- Actions can be assigned to more keys in config.toml.
- No harcoded keybinging or color, style and theme related data must be defined in the code. They must be configurable in config.toml.
- In Normal mode, all key events are forwarded to the focused backend (terminal/nvim).
- Only the prefix key and explicitly bound keys trigger WM actions.
- **Prefix timeout:** auto-exits Prefix mode after 500ms of inactivity.
- **Pane letter limit:** PaneSelect/Swap modes use a-z, A-Z (52 unique labels). Sessions with >52 panes/columns fall back to sidebar navigation.
- **Actions** should not be harcode. ActionRegistry should be used to register new actions (`registry.register`) and execute them (`registry.execute`).

---

## Action System Architecture

### Three-Layer Dispatch

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

### ActionRegistry (`heca/src/actions.rs`)

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

### KeymapRegistry (`heca/src/keymap.rs`)

Mode-specific keymap lookup:

```rust
pub struct KeymapRegistry {
    keymaps: HashMap<String, HashMap<KeyCombo, WmAction>>,
}
```

- **Bind**: `keymap.bind("normal", combo, action)`
- **Resolve**: `keymap.resolve("normal", &combo)` → `Option<&WmAction>`
- **Modes**: `"normal"` (prefix bindings), `"global"` (direct bindings), `"sidebar"` (sidebar nav), custom mode names

### WmAction Enum (`heca/src/input.rs`)

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

### Input Modes (`heca/src/app_state.rs`)

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

**Mode triggers**: Config defines how to enter modes:

```toml
[[keys.mode]]
name = "resize"
trigger = "prefix+r"
sticky = true   # true = stay until Esc/Enter; false = one-shot (chord)
```

### KeyCombo (`heca/src/keymap.rs`)

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

### Interaction Policy (`heca/src/app/interaction.rs`)

Every user-initiated action that changes WM state flows through `dispatch_action()` — the single chokepoint that decides **Allow vs Block** based on the active **focus domain** (`Tiled`/`Floating`), input mode, and **interaction source**. Handler-to-handler calls bypass the router (`registry.execute()` directly); only user-initiated actions are policy-routed.

```
User input → InteractionIntent → route_interaction() → RouteDecision
                                                       ├─ Allow(intent) → registry.execute()
                                                       └─ Block          → silently discarded (debug log)
```

**Rule: every `WmAction` variant MUST be classified in `action_policy()`.** The match is exhaustive (no wildcard) and verified by the `action_policy_covers_all_variants` test. Adding a `WmAction` without classifying it = compile error.

**The 6 `ActionPolicy` variants** (and what they mean for the Floating domain):

| Policy | Tiled | Floating | Examples |
|--------|-------|----------|----------|
| `Global` | Allow | **Allow** | `ReloadConfig` — true app-level, no layout impact, must work even when floating |
| `AlwaysAllowed` | Allow | **Block** (current sources) | `CommandPalette`, `SpawnCommand`, `EnterMode` — app-level but layout-affecting |
| `TiledOnly` | Allow | Block | `Focus*`, `Split*`, `ZoomColumn`, `Resize*`, `Swap*`, `Move*`, `Sidebar*`, `PaneSelect/Swap/Take`, `FloatAt`, `RenameColumn`, `DeleteColumn`, collapse/expand workspace+column |
| `FocusedPaneLocal` | Allow | **Allow** | `Float`, `ClosePane`, `ClosePaneById`, `RenamePane`, `RenameTarget`, `Selection*` (`EnterSelectionMode`, `Selection*`, `ClearSelection`, `CopySelection`, `PasteClipboard`, `BeginSelection`, `ToggleSelectionEndpoint`) — operate on the focused pane in either domain |
| `WorkspaceLevel` | Allow | Block | `WorkspaceNext/Prev`, `FocusWorkspace`, `CreateWorkspace`, `RenameWorkspace`, `DeleteWorkspace` |
| `SourceDependent` | Allow | depends | `FocusPane` — allowed only if it targets the active floating pane |

**Floating-domain policy:** when `FocusDomain::Floating` is active, only `FocusedPaneLocal` + `Global` actions pass from `Keyboard`/`MouseContent`/`MouseLeftSidebar`. Everything else is blocked. The only escape from floating is `prefix+f` (Float toggle) or `ClosePane`.

**`Global` vs `AlwaysAllowed` — do NOT conflate.** `AlwaysAllowed` is a misnomer: the router *blocks* it when floating. `Global` is the only policy that is truly always allowed. Use `Global` for app-level actions with **zero layout impact** that must stay reachable while floating (e.g. `ReloadConfig`). The hot-reload bug (config/style only applied on full restart, not on `prefix+Shift+r`, whenever a floating pane was active) was exactly `ReloadConfig` being mis-classified as `AlwaysAllowed` — fixed 2026-06-18 by moving it to `Global`.

**Interaction sources:** `Keyboard`, `MouseContent`, `MouseLeftSidebar` (future: `MouseRightSidebar`, `MouseTopMenu`, `MouseStatusBar`, `Rpc`). Source matters for `SourceDependent` actions and for future chrome sources that may allow `AlwaysAllowed` actions even while floating.

**Adding a new action — policy step (in addition to the "Adding New Actions" checklist above):**
1. Classify the variant in `action_policy()` under the right policy arm — choose carefully using the table above. When in doubt, ask: "should this work while a floating pane is active?" → if yes and it has no layout impact, `Global`; if yes and it's pane-local, `FocusedPaneLocal`; if no, `TiledOnly`/`WorkspaceLevel`/`AlwaysAllowed`.
2. If you introduce a **new `ActionPolicy` variant**, handle it in the exhaustive `match policy` in `route_action()`.
3. Add a spot-check assertion (`assert_eq!(action_policy(&WmAction::X), ActionPolicy::Y)`) and routing tests (tiled allows it, floating blocks/allows it as appropriate).

**Do NOT** conflate `ActionPolicy` (interaction.rs — Allow/Block per focus domain) with `action_priority()` (input.rs — keybinding resolution priority). They are unrelated systems.

See `.planning/interaction-policy-plan.md` for the intent-routing roadmap (Phase B/C).

---

## Config System (`heca-config/src/theme.rs`)

### Config File Format

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
focus_left = ["prefix+h", "prefix+ArrowLeft"]
focus_right = ["prefix+l", "prefix+ArrowRight"]
focus_up = ["prefix+k", "prefix+ArrowUp"]
focus_down = ["prefix+j", "prefix+ArrowDown"]
# ...

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

### Program Catalog

- Foreground-program presentation uses canonical `[program.<id>]` entries with
  optional raw-process aliases via `processes = [...]`.
- `icon` values in the program catalog are semantic Phosphor icon names
  (`terminal`, `file_code`, `folder_open`, `git_branch`, …), not raw glyph strings.
- Built-in catalog defaults live in `config.default.toml` `[program]` (the single
  source — `ProgramsConfig::default()` parses it); if you change them, update
  `README.md` in the same patch.
- Users remove a built-in program mapping with `disabled = true`. Do not invent
  empty-string semantics for inherited defaults.

### Pane Info Bar (segments + action buttons)

Each pane renders an in-pane **info bar**: ordered **segments** (left, "what the
pane is") and **action buttons** (right). Both are driven by config lists under
`[appearance.pane]` — `title_segments` and `title_actions` (fields of
`PaneAppearance`). Order in the list = render order. Empty list hides that side;
both empty ⇒ no bar and no reserved space. User-facing list of supported values
lives in `README.md` ("Pane Info Bar"); keep it in sync when you change the enums.

Both are typed enums in `heca-config/src/appearance.rs`, `#[serde(rename_all = "snake_case")]`:
- `PaneSegment` — `Location`, `AppName`, `GitBranch`, `GitStatus`.
- `PaneAction` — `Split`, `MoveLeft`, `MoveRight`, `Close`.

**To add a new segment kind:**
1. Add the variant to `PaneSegment` (`heca-config/src/appearance.rs`); document the doc-comment.
2. Render it in the segment match in `heca/src/chrome/mod.rs` (around the
   `PaneSegment::Location =>` arm) — pull from the pane's `PaneRuntime` projection;
   a segment with no data must be **skipped** (no empty pill).
3. Update `README.md` (supported-segments table) + `config.default.toml`.

**To add a new action kind:**
1. Add the variant to `PaneAction` (`heca-config/src/appearance.rs`).
2. Map it in `pane_action_spec()` (`heca/src/chrome/mod.rs`) → `(Glyph icon,
   WmAction, label, needs_focus)`, and give it a config **name** in
   `pane_action_name()` (same file). Reuse an **existing** `WmAction` (e.g. `Float`
   → `WmAction::Float`, `zoom` → `WmAction::ZoomColumn`); do not invent a parallel
   code path. The tooltip (and its keybind) is then automatic — see **§ Chrome
   buttons** below. No new keymap entry needed if the action already has a binding.
3. Per the action checklist, the action must already be reachable from keyboard +
   RPC; the button just adds the mouse/UI path.
4. Update the showcase pane-header demo + `docs/widgets.md` (grid-ui rule),
   `README.md` (supported-actions table), and `config.default.toml`.

### Chrome buttons → action, tooltip, KeyHint (centralized — do NOT hand-roll)

Every clickable chrome button is tied to the **`WmAction` it triggers**, and both
its tooltip and its `prefix+/` hint are derived **from that action** — a caller (or
plugin) never picks a shortcut string, hardcodes the leader symbol, or hand-builds a
tip. This is the one pattern; follow it for any new button.

> **Which hints are actually shown** is decided by the layered **surface compositor**, not
> per-feature: a button inherits its layer from the surface it lives in, and one uniform
> rule (context activation + geometric occlusion, no hardcoded z) picks the visible set.
> **Read the planner (F003/P019) — see the planner (F003/P019) before adding any new
> layer, surface, overlay/modal, or a button on a new surface.** Never add a bespoke
> visibility filter — model the surface instead.

**1. Tooltip with the live keybinding — `action_tooltip(...)`** (`heca/src/chrome/mod.rs`):
```rust
row = row.child(action_tooltip(button, action_name, label, &state.action_shortcuts));
```
- `action_name` is the action's **config name** (`"close"`, `"sidebar_left"`, …) — the
  canonical identity. The emitted `WmAction` may be a button-only variant
  (`ClosePaneById`, `AddPaneToColumn`) that isn't itself bound, so the *name* is the key.
- `ActionShortcuts` (on `AppState`, built at load **and** reload from
  `ActionShortcuts::from_config`) resolves the shortcut once per config via
  `shortcut::shortcut_for_action(name, user, defaults)`. That reads the **user's real
  binding when overridden** (falling back to the bundled default only when the user
  hasn't rebound it), supports **multiple** bindings (joined ` / `), and renders the
  leader through `shortcut::PREFIX_SYMBOL` (`λ`) — **never** a literal symbol, never
  the macOS `⌃⌥⇧⌘` form. Rebinding in `config.toml` + reload updates every tooltip.
- Result: `tip = "<label>  <shortcut(s)>"`, or the label alone when unbound.

**2. KeyHint (vimium-style `prefix+/` pick)** — register the **same intent** the click
sends and attach it to the widget so the picker can target it by letter:
```rust
let hint_id = hints.register(InteractionIntent::ActivateAction(action.clone()));
let button = IconButton::new(icon).hint_target(hint_id).on_click(move || {
    emit(InteractionIntent::ActivateAction(action.clone()));
});
```
`hints` is a `&mut HintTargetRegistry` — the **shared** allocator that lives on
`AppState` and spans **every** retained tree that carries hint targets. It is threaded
through `build_chrome_root` **and** `build_pane_header`. Ids are **monotonic** (never
reused), so targets from trees that rebuild on different cadences (the chrome tree vs.
each per-pane header tree) never collide; each tree records the contiguous id **range**
it registered and calls `hint_targets.remove_range(range)` when it is rebuilt or pruned
(see `render.rs` for the chrome tree, `sync_pane_headers` for the headers). The pick
path (`handle_hint_pick` + `paint_hint_targets`) walks the chrome tree **and** all
`state.pane_headers` trees; resolution reads the one shared map. See
`sidebar_toggle_button` for a complete example (tooltip + hint together).

- **Active-targeted buttons must focus first.** A pane button whose action acts on the
  *focused* pane (zoom/float — no pane id in the `WmAction`) registers
  `InteractionIntent::FocusPaneThenAction { pane_id, action }`, which `dispatch_intent`
  expands into a `FocusPane` then the action — mirroring what the click does across two
  events. Pane-parameterized actions (close/move/split carry the pane) just use
  `ActivateAction`.
- **The button set is a dynamic vector, never a hardcoded switch.** Pane-header buttons
  come from `pane_header_buttons(content, ctx) -> Vec<PaneHeaderButton>` (config's
  `[pane] title_actions` today; the documented **plugin seam** appends there later). The
  build loop only reads the descriptor fields (glyph / action name / `WmAction` / focus),
  so config-added, config-hidden, and future plugin-added buttons are tooltip'd + hinted
  automatically. Do **not** re-introduce a per-action `match` in the render loop.

### Planned parameterized binding contract

When implementing richer spawning / geometry-aware bindings, keep these rules:

- **Both** normal keybindings and mode bindings should support parameterized actions.
- Keep simple flat bindings for unit actions:

```toml
[keys]
focus_left = "prefix+h"
float = "prefix+f"
```

- Add `[[keys.bind]]` for parameterized non-mode bindings:

```toml
[[keys.bind]]
keys = "prefix+z"
action = "zoom_column"

[[keys.bind]]
keys = "prefix+g"
action = "spawn_pane"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "80%", height = "80%" }

[[keys.bind]]
keys = "prefix+Shift+b"
action = "spawn_pane"
args = { kind = "browser", float = true, width = "1200", height = "800" }

[[keys.bind]]
keys = "prefix+Shift+f"
action = "float_active_at"
args = { width = "95%", height = "95%" }
```

- Mode bindings should keep using `[[keys.mode.bindings]]` with `args`:

```toml
[[keys.mode]]
name = "spawn"
trigger = "prefix+s"
sticky = true

[[keys.mode.bindings]]
action = "spawn_pane"
keys = "n"
args = { kind = "terminal", program = "nvim", argv = ["."], float = true, width = "800", height = "400" }
```

- Size parsing contract:
  - `800` → `800px`
  - `800px` → explicit pixels
  - `80%` → percentage of available content area
- Floating spawns should open **centered by default** when no `x/y` are provided.
- `spawn_pane` should be **future-ready by kind**:
  - `terminal`
  - `browser`
  - `nvim_gui`
  - temporary/mock fallback when a real backend is not implemented yet
- For terminal-like panes, use structured command fields:
  - `program = "nvim"`
  - `argv = ["."]`
  instead of shell-only strings.

### Config Loading

Defaults are the **embedded** `config.default.toml` (settings/appearance/font/
program) + `keybindings.default.toml` (keys) — the single source of truth, parsed
at every startup (`heca-config/src/loader.rs`). User files are deep-merged on top:

1. Embedded defaults (`config.default.toml` ⊕ `keybindings.default.toml`)
2. `~/.config/heca/config.toml` (user general config, optional)
3. `~/.config/heca/keybindings.toml` (user keybindings, optional)

Deep-merge rules: tables merge **per-key** (a partial user file overrides only the
values it sets and keeps every other default); scalars and **arrays are replaced**
wholesale (so a user `[[keys.command]]`/`[[keys.mode]]` list replaces the default
list). `keys.unbind` removes specific bindings. The three built-in modes
(resize/sidebar/selection) are **always merged back** by name in the app layer
(`build_modes`), so defining your own mode never drops sidebar/selection nav.
`prefix+Shift+r` reloads both files at runtime.

### Unbinding Keybindings

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

### Config Reload

`WmAction::ReloadConfig` triggers `reload_config()` on `HecaApp`:

- Rebuilds keymaps from config file
- Reloads theme
- Updates settings (mouse, focus_follows_mouse, etc.)
- Does NOT restart the app

---

## Stack

| Layer | Crate | Version | Why |
|-------|-------|---------|-----|
| Windowing | `winit` | 0.30+ | De-facto Rust standard, cross-platform, HiDPI. |
| GPU API | `wgpu` | 0.25+ | Cross-platform (Vulkan/Metal/DX12/WebGPU), safe Rust. |
| Text layout | `cosmic-text` | 0.14+ | Best pure-Rust text stack; atlas caching, ligatures, variable fonts. |
| Terminal emulation | `alacritty_terminal` | 0.25+ | Battle-tested VT parser library. |
| PTY spawning | `portable-pty` | 0.9+ | Cross-platform PTY creation. |
| Async runtime | `tokio` | 1.40+ | All IO (PTY, Neovim socket, RPC server). |
| MsgPack | `rmpv` | 1.3+ | Neovim msgpack-RPC protocol. |
| Config | `serde` + `toml` | latest | TOML parsing. |
| Session IO | `bincode` or JSON | — | Session persistence format. |
| Plugin runtime | WASM host/runtime (planned) | — | Preferred long-term plugin boundary for pluggable chrome containers and actions; safer than native Rust dylibs. |

### What NOT to use

| Technology | Reason |
|------------|--------|
| Dioxus / Tauri / Electron | WebView-based; can't own the GPU render loop freely. |
| GTK / Qt | Fight you for custom GPU surfaces. Heavy cross-platform packaging. |
| egui / iced | Immediate-mode or over-constrained layout; we need to own pane rectangle assignment. |
| Bevy | Game engine ECS fights traditional GUI event loops. |
| skia-safe | Proven (Neovide) but requires C++ toolchain. Rust-native is lighter. |

### z=0 background frost model (heca-owned, cross-platform)

Frosted-glass frost is **heca-owned**, not OS-dependent. heca renders a blurred
vertical gradient as the bottom-most (z=0) layer in `render_frame`, and panes
composite translucently over it. This is the canonical background; there is no
second background-tint layer and no OS-vibrancy blur dependency (vibrancy is an
optional platform backdrop material, `Vibrancy::None` by default — grill-me Q6:
try z=0 without macOS vibrancy first).

- **`BackgroundLayer`** (`heca-renderer/src/background.rs`) is a **heca-renderer
  GPU primitive**, headless and unit-testable like the rest of `heca-renderer`.
  It is **NOT a `heca-grid-ui` widget** — it owns GPU textures (`z0_tex` gradient
  render target + `cache_tex` blurred result) and a static cached blur. It
  recomputes only when dirty (resize or `set_params` change) and snapshots the
  blurred result into its own cache before returning, so the shared `state.blur`
  is free to be reused afterwards by the floating-pane frost pass.
- **Render order** (see `heca/src/app/render.rs`): `clear → z=0 blit (pre-stencil,
  `Backdrop::draw` fullscreen at `background_alpha()`, `stencil = None`) → tiled
  stencil → tiled content (translucent `surface_alpha` over z=0) → borders →
  floating blur capture → floating stencil → floating backdrop(1.0) + content →
  grid-ui chrome → present`. z=0 is pre-stencil so the tiled content-clip never
  clips the background.
- **Translucency channel (grill-me Q1, Option A):** theme `terminal_background`
  values are **opaque**; `[appearance.terminal] transparency` → `surface_alpha` is
  the **only** translucency channel. Never ship a theme `terminal_background` with alpha 0
  (the `latte` `#e6e9ef00` bug) — that bypasses the knob and re-introduces the
  "no blur/transparency" regression.
- **Removed knobs:** `terminal_blur`, `terminal_frost_color`, `terminal_frost_opacity()`,
  `effective_terminal_frost_color()` are gone. Tiled frost = z=0 showing through
  `surface_alpha`; use `background_blur` for frost strength. Floating panes keep
  `terminal_floating_blur` + `terminal_floating_transparency`.
- **No hardcoded color/style (reinforced):** the gradient colors come from
  `Theme::effective_background_gradient_top/bottom()` (config → theme field →
  derived fallback), read at paint time — never literal `[f32;4]` in `render.rs`.
  This is the same rule as for `heca-grid-ui` widgets: every visual value reads
  from `heca-theme::Theme` / config, responds to `prefix+Shift+r` reload, and has
  no `Color::new(...)` / `with_alpha(28)` magic numbers in the app.

### Effect tokens — `glow_size` vs `intensity` (separate dimensions)

The two effect tokens are **independent**; do not conflate them:

- **`glow_size`** (`GlowLevel`: `none | thin | medium | large`) — the **sole
  owner of glow**: presence + halo radius (`radius_scale()`) + strength
  (`strength_scale()`, curve `0.0/0.5/1.0/1.6`). `MarkerGroup` reads
  `t.glow_size.strength_scale()` for its glow alpha; `component.rs::scaled_glow`
  reads `glow_size.radius_scale()` for the halo size.
- **`intensity`** (`Intensity`: `off | low | medium | heavy`) — owns
  **scanline/CRT overlay opacity only** (`scanline_opacity()`). It does **not**
  affect glow. The older `Intensity::glow_scale()` was removed; the curve was
  preserved exactly by moving it onto `GlowLevel::strength_scale()` so
  MarkerGroup's glow strength is unchanged. `intensity` is tied to the
  compositor's z=0.5 CRT scanline overlay pass (Q2-extra in
  `compositor-blur-refactor-plan.md`).

Both are `[appearance]`-overridable (`AppearanceConfig::effective_glow_size` /
`effective_intensity`; unset → theme wins, set → overrides) and applied at the
single choke point `chrome_gui_theme(state)` in `heca/src/chrome/mod.rs`.

---

## heca-grid-ui — Grid UI Component Library

> Full plan: `grid-ui-plan.md`. Developed on the **`heca-grid-ui`** branch.

`heca-grid-ui` is a **GPU-free, signal-driven, composable component library** that gives heca a *Tron/GridCN* visual identity (glow, corner brackets, scanlines, HUD typography). It is a *component framework*, not a theme.

### Rules

- **No GPU in `heca-grid-ui`.** It emits a `Scene` (a `DrawCommand` display list); `heca-renderer` rasterizes it. `heca-grid-ui` must NOT depend on `wgpu`, `winit`, or `heca-renderer`. Same boundary as `heca-core` — headless and unit-testable.
- **Composition, not inheritance.** Every component embeds a `Base` struct and implements the `Component` trait. "Extends base" = embed `Base` + impl trait, with builder-style styling.
- **Reactivity** via `floem_reactive`, hidden behind the `heca_grid_ui::reactive` facade — component code never names the dependency (swappable).
- **Component layout** via `taffy` (Flexbox/Grid/Block). This is *intra-component* layout (widgets inside a sidebar/panel/pane). It is **NOT** a second WM layout engine — `taffy` never positions panes or columns; the niri scrolling engine remains canonical for that.
- **Coordinates**: `Scene` carries `f32` logical pixels; the renderer scales to physical by `scale_factor` (HiDPI crispness preserved at the GPU boundary).
- The existing chrome becomes a **consumer** of `heca-grid-ui`; over time this should evolve toward a pluggable chrome host with left/right/top/bottom regions.
- Important separation: the `Sidebar` in `heca-grid-ui` is a **shell/layout widget**, while the current workspace tree should evolve into a built-in `WorkspacesContainer` mounted inside that shell.

### ⭐ THE WIDGET ARCHITECTURE — `ViewNode` + composition (READ FIRST; applies to EVERY widget change)

**heca's UI is a declarative, compositional tree — the same shape SwiftUI/Flutter use — and this is the target architecture for EVERY widget.** Two layers, one shape:

- **`ViewNode`** (`heca/src/chrome/view.rs`) — the **serializable declarative model**: `ViewNode { kind: WidgetKind, props: Map<name, PropValue>, events: { press|change → Intent }, children: Vec<ViewNode> }`. `WidgetKind` is the **closed vocabulary of the WHOLE library** (containers `VStack`/`HStack`/`Row`/`Grid`/`Card`/`Scroll`/`Panel`/`Surface`/`ItemGroup`/`DockFrame`/`MarkerGroup`; leaves `Label`/`Button`/`IconButton`/`Badge`/`BadgeButton`/`Tag`/`Icon`/`Input`/`Select`/`Toggle`/`Checkbox`/`StatusDot`/`Gauge`/`ScrollBar`/`Alert`/`Toast`/`RailCell`/`Item`/`Tabs`). **Styling IS a prop (changed 2026-07-26)** — the `Theme` gives the default and code may override it with a string; the old "styling is not a prop" rule is dead. See [`docs/chrome-and-ui.md`](docs/chrome-and-ui.md). **Behaviour is an `Intent`** (action id + args) — never a closure — so it serializes for native code, RPC, and WASM plugins alike.
- **`realize(&ViewNode, …) -> Box<dyn Component>`** (`heca/src/chrome/realize.rs`) — the recursive host mapper: build the `heca-grid-ui` widget for `kind`, resolve props against `Theme`, wire events to intents, recurse `children`, attach via `.child(...)`. It **translates**; it never re-implements layout/paint/focus.

**THE RULE (mandatory, every task): a widget's content is COMPOSED from child components — the very tree `realize` produces — never hand-drawn in `paint`.** A widget draws its own *chrome* (background/border/glow/focus ring, from `Theme`); its *content* (labels, icons, rows) must be child `Component`s laid out by the engine, so that:
- it is **realizable via `ViewNode`** (a `WidgetKind` + props + events + a `realize` arm), and
- it is **extended by composition, not rewrite** — a new affordance (e.g. a button's accelerator = an `Icon(CaretUp) + Label`) is a **child/slot**, not a hand-positioned `cx.icon`/`cx.text` call.

**When you touch a widget, you MUST refactor it toward this.** Any leaf that hand-draws its content (e.g. `Button` drawing its label text directly, so it can't hold an `Icon`) is a **refactor target**: make it compose its content as children (a content slot: leading / label / trailing, like `Item` already does) before adding to it. Do not bolt a hand-drawn extra onto a hand-drawn widget — that is the anti-pattern this rule exists to kill. Extending the vocabulary (a new `WidgetKind`, a new prop) is **host-side** work (widget + `realize` arm + showcase + `docs/widgets.md`); plugins only *compose* existing kinds.

#### ⛔ SETTLED — never re-derive or re-propose these

> These are **decided**. They have been re-explained to agents many times; re-opening them wastes
> the maintainer's time. They are written **here**, in the file you read every session, on purpose
> — not behind a link, because you will not open the link.

**The dependency rule — this is why the model looks the way it does:**

```
heca (app)  ──depends on──▶  heca-grid-ui (library)      # NEVER the reverse
```

| Question | Answer — do NOT re-propose |
|---|---|
| Should `heca-grid-ui` own `ViewNode`? | **NO.** It inverts the crate graph (the library would then need `realize`, which needs the app's `InteractionIntent` / `HintTargetRegistry` / theme wiring). |
| Can a widget constructor take a `ViewNode` — `Button::new(ViewNode)`? | **NO — impossible.** `ViewNode` lives in the **app** (`heca/src/chrome/view.rs`); the library cannot see it. |
| Then move `ViewNode` down into the library? | **NO.** `ViewNode` **cannot carry closures or signals** (it must serialize for WASM). The native chrome depends on both — `.on_activate(move \|\| …)`, `row.state().set(true)`, `label.text_signal().set(…)` — which update **in place, with no rebuild**. Routing all native UI through `ViewNode` turns every state change into a full rebuild and fights the reactive chrome store. **The library keeps its builder API.** |
| Is `realize` the only `ViewNode`→widget path? | **YES.** One bridge, app-side (`heca/src/chrome/realize.rs`). |
| Do widgets hold children, or hand-draw content? | **CHILDREN.** Hand-drawn content is a refactor target (THE RULE above). |
| What type is a slot / child? | **`impl Component`** — any widget. **Never** narrow it to a closed `Icon\|Label` enum. |
| How does a **realized** subtree enter a widget? | Via a **`*_boxed` setter**: `realize` returns `Box<dyn Component>`, which is not itself `Component`, so it cannot go through `Parent::child`. `Dialog::body_boxed(Box<dyn Component>)` is the precedent. |
| How does behaviour cross the plugin boundary? | As an **`Intent`** (action id + args), never a callback. Click, KeyHint pick, and RPC all fire the same intent. |
| Is styling a prop? | **YES — changed 2026-07-26.** The `Theme` gives the default; code may override it with a string (`"#ff8800"` or a theme name like `"muted"`). Set nothing and you follow the theme, which is what most widgets should do — a literal colour will not follow a theme reload, and that is the author's trade to make. **The old rule ("NO — the host resolves the pixels") is dead; do not restore it.** **Built 2026-07-27 (F003/P017/T7):** the whole of `Visual` (fill, border, glow, radius, font size) serializes and merges through the same generic path as `Layout`; a token name resolves against the theme the tree is built with, and a theme reload rebuilds the trees, so the token follows. Full model: [`docs/chrome-and-ui.md`](docs/chrome-and-ui.md). |

**Both authoring paths converge on the same retained tree — that is the whole point:**

```rust
// Declarative (plugins, RPC, modal/menu bodies) — arbitrary tree, arbitrary depth:
ViewNode::new(WidgetKind::Button)
    .prop("variant", PropValue::Variant(ViewVariant::Destructive))
    .on_press(Intent::new("confirm_ok"))
    .child(ViewNode::new(WidgetKind::HStack)
        .child(ViewNode::new(WidgetKind::Icon).prop("icon", PropValue::Glyph("trash".into())))
        .child(ViewNode::new(WidgetKind::Label).text("Delete")))

// …realize() produces exactly this — which native code (chrome: closures + signals) writes directly:
Button::destructive("Delete")
    .child(Icon::new(Glyph::Trash))
    .on_click(move || emit(intent))
```

**Genuinely OPEN (the live design space):** a typed builder SDK over `ViewNode`; and `Label`
truncation/ellipsis + wrapping (a long label overflows its box today).

**SETTLED since (do not re-open):**
- **`realize` coverage is COMPLETE.** Every `WidgetKind` maps to a live widget, and a test
  (`every_widget_kind_realizes_to_a_live_widget_except_the_host_only_ones`) walks `WidgetKind::ALL`
  and fails if one doesn't — so a new kind cannot silently render an empty container. Do **not**
  re-derive "which kinds are missing"; the answer is none.
- **`Button`, `Item`, `Select`, `Tabs` compose their content** (a hand-drawn leaf is a refactor
  target, not a style).
- **An option is a node with a value and arbitrary content, and options are CHILDREN** — the
  `Choice` widget — never a `props["options"]` list of strings. `realize` maps the widget's index
  back through the options' `value` props, so a `change` intent carries `{"value": "high"}`, never
  an opaque index.
- **A widget with several places for children takes a `slot` prop on the CHILD** (`header`,
  `leading`, `trailing`). `ViewNode.children` stays one flat vector — no slots map, no second child
  vector.
- A child inherits its parent's per-state **content color** via `PaintCx::with_content_color` (the
  control publishes one value per frame; unstyled `Label`/`Icon` children pull it).
- **A widget whose state is a live host signal is HOST-ONLY** — static serializable data cannot
  drive a signal, so a declarative one would be a dead control. `ScrollBar` is the case (plugins use
  `Scroll`); it also applies to individual builders, e.g. `DockFrame::rail(..)`.

Background reading (the rules above are self-contained — you do **not** need these to avoid the
mistakes): `docs/widget-architecture.md` (same content, with rationale); `docs/chrome-and-ui.md` §2.6.2 + §2.7.2; `docs/widgets.md` → "Declarative UI model (`ViewNode`)"; `docs/chrome-and-ui.md`.

---

### Creating new widgets / components (MANDATORY — read before adding ANY UI element)

Any new visual or interactive element belongs in **`heca-grid-ui` as a proper widget** — **never** as ad-hoc inline composition in the app (`heca/src/chrome.rs`, sidebar, …) with hardcoded sizes/colors/alphas. **Plan the widget, build it in `heca-grid-ui`, integrate it into the catalog — then have the app compose it.** It must also follow **THE WIDGET ARCHITECTURE** above — composed content, `ViewNode`-realizable, refactor-toward-composition when touched.

A new widget **MUST**:

- **Be domain-neutral / GENERIC.** `heca-grid-ui` widgets must **never** encode an app domain — never name or couple a widget to `workspace`/`column`/`pane` (nor `docker`/`agent`/`git`). They are generic primitives (frames, groups, rows, rails, marker bars, target hints, regions); the **domain meaning is applied app-side** by the mounted container/provider. The chrome regions (left/right/top/bottom) host *generic containers* — `WorkspacesContainer` today, but also Docker instances, AI agents, git status, notes, plugin-defined containers (see `docs/chrome-and-ui.md`). So a widget built to render the workspace "columns" must be a **generic grouping/marker primitive that ANY container can reuse** — e.g. not `ColumnGroup`, but a generic `MarkerGroup`/`RailGroup` whose left bar + target-hint mean nothing in particular until a container gives them meaning. The existing widgets model this: `DockFrame`/`ItemGroup`/`Row`/`RailCell`/`ChromeRegion`/`KeyHint` are all domain-free. **Read how they are built — and run the live showcase (`cargo run -p heca-renderer --example showcase`, `heca-renderer/examples/showcase.rs`) — before adding a new one** (it demonstrates the widgets + chrome recipes to take inspiration from).
- **Embed `Base` and implement `Component`** (+ builder traits `LayoutExt`/`StyleExt`/`Parent` as appropriate). This gives it — for free and uniformly with every other widget — `visible`/`disabled`/`focused`/`focus_visible`, `bounds` (so hit-testing + event dispatch work), `tab_index`, children, `mark_needs_paint`, and `tick(dt)` animation. Inline `Flex`+`Surface` blobs inherit **none** of this.
- **Read ALL styling from the `Theme` (read at paint via `cx.theme()`) — hardcode nothing.** Colors, font family/size, border width, radius, glow, and transparency come from theme tokens, **not** literal `Color::new(...)` / `with_alpha(28)` / `Length::Px(3.0)` magic numbers in the app. Core project rule (see "No hardcoded color/style/theme" above): widgets must respond to `config.toml`, runtime theme reload (`prefix+Shift+r`), font changes, and the `[appearance]` transparency settings.
- **Drive state styling from the theme**: active/inactive border + color, border width, radius, hover/press/focus — all from theme tokens, so behaviour is consistent across the library.
- **Be planned + integrated**: export in `widgets/mod.rs`, add to the catalog in this file + `docs/widgets.md`, and add unit tests. No one-off escape hatches bolted on under deadline — if a variant is needed, design it as a proper, documented, theme-driven widget option.

#### Documentation is MANDATORY — update **both**, in the same change

Whenever you fix, create, update, or extend a widget **or the declarative UI model** (`ViewNode`,
props, `Intent`s), you MUST update **both** of these (not one or the other):

1. **Rustdoc — mandatory and complete.** Full in-code doc comments on the type / fields / methods.
   This is the authoritative in-code reference; **never reduce it to a bare pointer.**
2. **`docs/widgets.md` — supplementary human documentation.** The reader-friendly catalog entry.

**Both must be EXHAUSTIVE, and both must carry CODE EXAMPLES for BOTH audiences.** A widget's entry
is incomplete — and the task is not done — unless it documents:

- **every** builder / property / event / signal accessor it exposes (not a subset, not "the main
  ones"), what each does, and its default;
- a **runnable native example** — the Rust builder API (`Button::destructive("Delete").icon(Glyph::Trash)`);
- a **runnable declarative example** — the `ViewNode` form a plugin/RPC would author (`WidgetKind`
  + props + `children` + `Intent`s), including which props `realize` reads and any **precedence**
  rules (e.g. a Button's `children` win over its `text`/`icon` sugar);
- how to **compose/extend** it (what may go inside it, and what the widget owns vs. what the caller
  provides).

Keep the two in sync (cross-reference them). This is in addition to the showcase-demo requirement
above. A widget landed with thin docs, or documented for only one audience, is unfinished work.

The app side (`heca/src/chrome.rs`, sidebar) must **only compose existing widgets and project app state into them** — it must not invent visual primitives or hardcode styling inline.

**Why this is non-negotiable (a real mistake made 2026-06-15):** the sidebar column "marker bar" + pane cards were built as inline `Flex`/`Surface` composition in `heca/src/chrome.rs` with hardcoded widths/alphas/colors (e.g. `Surface::new().width(Length::Px(3.0))…with_alpha(90)`, `theme.accent.with_alpha(28)`). Result: they do **not** inherit `Base` props, do **not** read font/theme/colors from config, **ignore** the `[appearance]` transparency, and have **no** consistent active/inactive border/width/radius — silently breaking theming, font changes, and transparency, and bloating the codebase with un-reusable, untested one-offs. Always build the widget properly in `heca-grid-ui` instead. The ad-hoc `.frameless()` added to `DockFrame` is the kind of unplanned escape-hatch to avoid; widget options must be deliberate + theme-driven.

### Using the widgets

**Live showcase: [`heca-renderer/examples/showcase.rs`](heca-renderer/examples/showcase.rs)** — run `cargo run -p heca-renderer --example showcase`. It exercises **every** widget (and chrome recipes: the EXPLORER `DockFrame`→`ItemGroup` tree, PANES cards, the `ChromeRegion` sidebar shell, `RailCell`/`KeyHint` rail, command palette, toasts) with real interaction. **Always look at the showcase first** — both to see how to *use* a widget and to take inspiration / copy patterns when building a new one. **Full per-widget API reference + examples: [`docs/widgets.md`](docs/widgets.md)** — read it before using the library. Quick orientation:

- Import via `use heca_grid_ui::prelude::*;` (widgets, builder traits, `Theme`, `Color`, signals, events).
- Build a retained tree (`Flex`/`Card`/`Button`/…), then each frame: `LayoutEngine::new().compute(&mut root, size)` → paint into a `Scene` with `PaintCx` → `heca_renderer::scene::enqueue_scene(grid, text, &scene)`.
- Widgets opt into builder methods via marker traits: **`LayoutExt`** (`.width/.height/.padding/.gap/.justify/.align/.grow/.disabled/.tab_index`), **`StyleExt`** (surfaces only: `.background/.border/.glow/.radius`), **`Parent`** (`.child`).
- **Change widgets report via callbacks**, not return values: `.on_change(|action: Action| …)` carrying `Action::value("<name>-change", SignalData::…)` (`toggle-change`/Bool, `checkbox-change`/Bool, `input-change`/String, `tab-change`/Usize). Buttons use `.on_click(|| …)`.
- **`Base.disabled`** (dim+inert+unfocusable) and **`Base.tab_index`** are common to all widgets. Focus via one `FocusManager` (Tab/Shift+Tab, click-focus, `deliver_key`). Animations via `tick(dt) -> bool`.

**Catalog (implemented):** layout `Flex`/`Container`, `Surface`, `Card`, `ScrollRegion`; text `Label`; interactive `Button` (6 variants × 3 sizes), `Toggle`, `Checkbox` (optional clickable label), `Input` (full keyboard/selection model), `Tabs`, `Select`; overlays `Overlay` (base layer; blocking = a layer property), `Dialog`, `CommandPalette`, `ContextMenu`, `Tooltip`, `ToastStack`; display `Badge`, `StatusDot`, `Separator`, `Spinner`, `Alert`, `ProgressBar`, `Gauge`. Foundations: `Base`, `Component`, `Theme`/`Intensity`, `Color`, `Action`/`SignalData`, `GridKey`/`Modifiers`/`Event`, `FocusManager`, `Flash`, `Scene`/`DrawCommand`/`PaintCx`.

### Gotchas

- **Don't run `cargo fmt`** in this repo — the local rustfmt reflows many files (no pinned `rustfmt.toml`); hand-format to match and verify with `cargo clippy --all-targets`.
- **Renderer text API**: `TextRenderer::queue_text(text, x, y, size, color)` positions at a top-left point (used across the app); `queue_text_in_box(text, x, y, w, h, size, color, bold, align)` centers within a box (used by the grid scene). Don't conflate them.
- **Never hard-code font family/size** — read them from the dedicated `[font]` config (`state.font_config.family.ui_normal()` / `state.font_config.size.ui`), not the color `Theme` (fonts are system-local, not theme-portable — see `compositor-04c`). `normal` is optional: omitting it falls back to the surface-correct embedded font (`ui_normal()` → Geist Mono, `terminal_normal()` → Maple Mono Normal NF).

### Extra dependencies (in `heca-grid-ui` only)

| Crate | Purpose |
|-------|---------|
| `floem_reactive` | Fine-grained signals/memos/effects (behind facade). |
| `taffy` | Flexbox/Grid/Block component layout. |

---

## Project Structure

```
myvim/
├── AGENTS.md              ← This file
├── README.md              ← User-facing documentation
├── config.default.toml        ← Default settings/appearance/font/program (embedded, single source)
├── keybindings.default.toml   ← Default keybindings (embedded, single source)
├── Cargo.toml             ← Workspace root
├── heca/                  ← Main binary (event loop, app state, rendering)
│   ├── src/
│   │   ├── main.rs        ← HecaApp, ApplicationHandler, render(), registry setup
│   │   ├── app_state.rs   ← AppState, InputMode, DragState, SidebarState
│   │   ├── input.rs       ← WmAction enum, action_from_name(), action_priority()
│   │   ├── keymap.rs      ← KeymapRegistry, KeyCombo, event_combo_matches()
│   │   ├── actions.rs     ← ActionRegistry, ActionDescriptor, ActionCategory
│   │   ├── handlers.rs    ← All action handlers (handle_focus_pane, handle_swap, etc.)
│   │   ├── sidebar.rs     ← Current workspace-tree container façade (`model`, `hit_test`, `render`, `tests`); future built-in `WorkspacesContainer`
│   │   └── chrome.rs      ← Current chrome config; future pluggable chrome host will generalize left/right/top/bottom regions
│   └── Cargo.toml
├── heca-core/             ← Layout engine + backends (no GPU code)
│   ├── src/
│   │   ├── layout/
│   │   │   ├── mod.rs     ← Module re-exports
│   │   │   ├── types.rs   ← Shared geometry types (Point, Size, Rectangle, ColumnWidth, etc.)
│   │   │   ├── animation.rs  ← Animation, SwipeTracker, easing functions
│   │   │   ├── view_offset.rs  ← ViewOffset (Static/Animation/Gesture)
│   │   │   ├── column.rs  ← Column, Pane, height distribution
│   │   │   ├── scrolling.rs  ← ScrollingSpace, focus, add/remove, view positions
│   │   │   ├── workspace.rs  ← Workspace, FloatingPane
│   │   │   └── session.rs ← Session, OverviewState, WorkspaceSwitch
│   │   ├── backend/
│   │   │   ├── mod.rs     ← PaneBackend trait, BackendRenderData
│   │   │   ├── terminal.rs  ← TerminalBackend (PTY + vte)
│   │   │   └── fake.rs    ← FakeBackend (no PTY, for layout testing)
│   │   ├── pane.rs        ← DEPRECATED old BSP tree code (kept for reference)
│   │   └── types.rs       ← Old Rect type (for BSP legacy)
│   └── Cargo.toml
├── heca-renderer/          ← GPU rendering (wgpu)
│   ├── src/
│   │   ├── lib.rs         ← Renderer init
│   │   ├── primitive.rs   ← PrimitiveRenderer (rects, borders)
│   │   └── text.rs        ← TextRenderer (cosmic-text atlas)
│   └── Cargo.toml
├── heca-config/            ← Configuration loading
│   ├── src/
│   │   ├── lib.rs         ← Module exports
│   │   └── theme.rs       ← Config, AppConfig, Theme, keybinding defaults
│   └── Cargo.toml
├── heca-grid-ui/                ← Grid UI component library (GPU-free, signals + taffy) [heca-grid-ui branch]
│   ├── src/
│   │   ├── reactive/      ← Signal/Memo/Effect facade over floem_reactive
│   │   ├── scene.rs       ← Scene + DrawCommand display list (visual vocabulary)
│   │   ├── style.rs       ← Style (taffy::Style wrapper) + Theme tokens + builders
│   │   ├── component.rs   ← Component trait + Base struct + Layout/Paint contexts
│   │   ├── layout.rs      ← taffy tree sync + compute → Base.bounds
│   │   └── widgets/       ← Flex, Grid, Stack, Label, Button, Card, Hud, Gauge,
│   │                        CornerBrackets, StatusBar, Sidebar, Pane, ...
│   └── Cargo.toml
├── docs/
│   └── niri-wiki/          ← NIRI documentation (structured wiki reference)
├── .planning/              ← GSD planning artifacts
│   ├── PROJECT.md          ← Project overview
│   ├── REQUIREMENTS.md     ← v1/v2 requirements
│   ├── ROADMAP.md          ← Phase roadmap
│   ├── STATE.md            ← Current execution state
│   └── research/           ← Research docs (NIRI analysis, architecture, stack, etc.)
├── .agents/
│   └── skills/
│       └── niri/SKILL.md   ← NIRI knowledge skill
└── review.md               ← Previous code review
```

---

## Available Skills

### `niri` (`.agents/skills/niri/SKILL.md`)

**Activate when:** Working on layout engine, ViewOffset, scrolling, workspaces, overview mode, or any feature inspired by niri's scrollable-tiling model.

Contains:

- Complete niri architecture reference (ScrollingSpace, Column, ViewOffset, Workspace)
- Scrolling model (horizontal continuous + snap, vertical discrete)
- Column width management (no normalization)
- Workspace system (dynamic, named, addressing)
- Animation types (spring vs easing) and parameters
- Full action reference for keybindings
- Window rules, layer rules, output config
- IPC protocol
- Fractional layout (physical pixel alignment)
- Animation timing (LazyClock) and redraw loop (RedrawState)
- Source file references into niri's actual codebase

### `pi-intercom` (for multi-session coordination)

Use when delegating tasks to other pi sessions.

### `pi-subagents` (for subagent workflows)

Use for multi-step analysis, advisory review, or parallel implementation tasks.

---

## Coding Conventions

### Do

- Use `f64` for all layout coordinates (logical pixels). Convert to `f32` only at the GPU render boundary.
- Put layout logic in `heca-core/src/layout/`. No GPU code in the core crate.
- Put GPU rendering in `heca-renderer/src/`. No layout logic in the renderer.
- Put input handling and WM actions in `heca/src/`. This is the orchestrator.
- Use `PaneBackend` trait for all pane content sources (terminal, neovim, browser).
- **Route ALL WM actions through `registry.execute()`**. Direct function calls are registry bypasses.
- Use `Animated<T>` for any value that should animate smoothly over time.
- Store column widths as `ColumnWidth::Proportion(f64)` or `ColumnWidth::Fixed(f64)`. NEVER normalize column widths.
- Store `working_area` in the layout engine; apply chrome offsets in the renderer.
- Treat the current workspace/sidebar tree as the future built-in `WorkspacesContainer`, not as the final definition of the Sidebar shell.
- Keep Sidebar-shell concerns separate from mounted-container concerns: shell = framing/visibility/collapsed mode; container = tree semantics, search, DnD, provider-specific actions.
- Design compatible container placement/move behavior as host-managed chrome behavior, not as container-internal DnD.
- Add `#[cfg(debug_assertions)]` for debug logging.
- Use `expect("descriptive message")` instead of `unwrap()` for initialization code.

### Don't

- **Do NOT add a second WM layout engine.** The NIRI-inspired scrolling-column engine is canonical for arranging panes/columns. Old BSP code in `heca-core/src/pane.rs` is kept for reference only — do not wire it in. (Note: `taffy` in `heca-grid-ui` is *component-internal* widget layout — a different altitude — and does not count; it never positions panes/columns.)
- **Do NOT call `update_all_column_widths()` more than necessary.** Prefer stored column widths.
- **Do NOT use BSP tree concepts** (split direction, child ratios, etc.). NIRI layout is a flat column list with vertical pane stacks.
- **Do NOT hardcode `ctrl=false` in prefix mode.** The prefix key is a mechanism, not a modifier eraser.
- **Do NOT use the old `Rect` type** from `heca-core/src/types.rs`. Use `Rectangle` from `heca-core/src/layout/types.rs` for new code.
- **Do NOT add a webview.** All chrome renders via `wgpu` primitives.
- **Do NOT add tokio to the main event loop** without careful thought. winit events must not block. Use `pollster` for async init.
- **Do NOT create registry bypasses.** All focus/workspace/layout changes must go through `registry.execute()`.
- **Do NOT treat the current workspace tree as the final Sidebar abstraction.** The Sidebar should evolve into a shell/host; workspace tree behavior belongs to the built-in `WorkspacesContainer`.
- **Do NOT put provider-specific semantics in the Sidebar shell.** Expand/collapse rules, search, row actions, and pane DnD belong to the mounted container/provider, not the shell.
- **Do NOT model future plugins as native Rust dylibs by default.** Prefer a host-controlled WASM boundary for external chrome/container extensions.
- **Do NOT repeat yourself.** Prefer reusable components, modules, and functions. If you find yourself writing the same pattern multiple times (e.g., button rendering, hit testing, animation logic), extract it into a shared function or struct. Duplication breeds bugs and makes maintenance harder.

### Prefix Mode Design Rules

The project deliberately uses tmux-style prefix architecture (`Ctrl+B → key`). This is NOT a bug to be fixed. However:

- ✅ Pass real modifier state (`state.modifiers.control_key()`) in prefix mode — don't hardcode `false`.
- ✅ Add a prefix timeout (~500ms) so the user can't get stuck in prefix mode.
- ✅ Make the prefix key configurable (via `prefix = "ctrl+b"` in config).
- ✅ Forward the literal configured prefix key on double-press (e.g. `Ctrl+B Ctrl+B` → `Ctrl+B`, `Ctrl+A Ctrl+A` → `Ctrl+A`).
- ❌ Do not eliminate prefix mode — it prevents conflicts with hosted terminal apps.
- ❌ Do not make prefix mode modeless — that defeats the purpose.
- ❌ Do not forget that Ctrl-modified bindings (`Ctrl+h`, `Ctrl+]`) should work after prefix.

### Keybinding System Rules

- `WmAction` enum: one variant per action. Add new variants as needed.
- `action_from_name()`: maps config string names to actions. Keep in sync.
- `action_priority()`: **do NOT use `_ =>` catch-all** — explicitly match every variant.
- `resolve()`: case-insensitive key matching, modifier-exact. Physical key fallback for macOS.
- **All WM state changes go through `registry.execute()`** — no direct `focus_pane_by_id()` calls outside handlers.
- **No hardcoded feature keys in input handlers.** Any user-triggerable keyboard behavior must go through:
  - `WmAction`
  - `ActionRegistry`
  - `KeymapRegistry`
  - config-driven bindings (`[keys]` or `[[keys.mode.bindings]]`)
- This includes mode-local behavior such as selection, resize, sidebar navigation, pane manipulation, and future browser / Neovim GUI interactions.
- The only acceptable hardcoded keys in mode handlers are universal control keys:
  - `Esc`
  - `Enter`
  - the configured prefix key
  - raw text-entry primitives for explicit text-input modes
- Do not match raw feature keys like `h/j/k/l`, arrows, `y`, `p`, etc. inside mode handlers unless they are resolved through the mode keymap and action system.
- Important app-wide rule: design actions so they are reachable through mouse/UI, keyboard/action dispatch, and RPC whenever that capability makes sense on those surfaces.
- Default keybindings live in `keybindings.default.toml` (embedded single source): add new bindings here.

### Adding New Actions

1. Add variant to `WmAction` in `heca/src/input.rs`
2. Add string mapping in `action_from_name()` — **only if the bare name says everything.** An action
   that needs a target does *not* belong there: it is built from its arguments in `build_action()`.
   Putting it in `action_from_name()` with placeholder fields means the name silently resolves to
   index 0, which is how a bare `delete_workspace` used to delete workspace 0.
3. Add builder support in `build_action()` when the action is parameterized
4. Add priority in `action_priority()`
5. Create handler in `heca/src/handlers.rs`
6. Register in `build_registry()` in `heca/src/app/registry.rs`
7. Add default binding in `keybindings.default.toml` (the embedded default keymap). An action with a
   **required argument gets none** — a key cannot supply a pane id.
8. Add descriptor in `ActionRegistry::ALL` in `heca/src/actions.rs`
8b. **Declare its arguments** in that descriptor's `args` — name, kind, required, and for a
   vocabulary argument the list from `EnumArg::VALUES` beside its own parser, never a copy. The field
   has no default, so you cannot skip the question; `args: &[]` means it genuinely takes none.
   This is what lets heca say *which* argument a caller got wrong instead of the call vanishing —
   and the tests `every_declared_argument_is_read_by_the_action`,
   `every_required_argument_is_actually_required` and `every_optional_argument_is_actually_optional`
   fail if the declaration and the `build_action()` arm disagree.
9. Add RPC parser support in `heca/src/rpc.rs`
9b. **Classify the interaction policy** in `action_policy()` (`heca/src/app/interaction.rs`) — the match is exhaustive, so a new variant **won't compile** until you do. (`Global` = always allowed incl. floating; `AlwaysAllowed` is a misnomer — blocked when floating. See § Interaction Policy.)
9c. **If it's destructive, declare a confirm** as data on its `ActionMeta.confirm` (a `ConfirmSpec`), not at the call site — the central gate then confirms it on *every* surface. The toggle key is `ConfirmSpec.config_name` (may differ from the action name, as `close` → `delete_pane`); users toggle it under `[confirm]`.
10. Make sure the capability is not trapped behind one surface: route it through the action model so it can be reached from mouse/UI, keyboard/action dispatch, and RPC whenever appropriate. Metadata is discoverable via RPC introspection (`list-actions` / `describe-action <name>`, `ActionCatalog::describe_all/describe`).
11. Document examples in `README.md` and `keybindings.default.toml`

> A **name-keyed** action (contributed by a provider/plugin, no `WmAction` variant) skips steps 1–4/8: register it at runtime with `register_dynamic(registry, catalog, meta, handler)`; a native built-in can use `register(ActionSpec { action, handler, meta })` to wire handler + metadata in one call. Full guide with examples: `docs/widgets.md` → "Registering a custom (name-keyed) action".

For planned richer actions like `zoom_column`, `float_active_at`, and `spawn_pane`, prefer domain-friendly arguments over ad hoc strings. Example target shape:

```rust
WmAction::ZoomColumn
WmAction::FloatActiveAt { width: SizeSpec, height: SizeSpec }
WmAction::SpawnPane {
    kind: PaneKind,
    program: Option<String>,
    argv: Vec<String>,
    float: bool,
    width: Option<SizeSpec>,
    height: Option<SizeSpec>,
}
```

Behavior contract for float/unfloat:

- `prefix+f` stays the float toggle
- if a pane was originally tiled, unfloat restores it
- if it was spawned directly as floating with no original slot, unfloat should place it into a **new column**
- `prefix+z` should be reserved for column zoom toggle, not float/unfloat

---

## Commands

```bash
# Build
cargo build
cargo build --release

# Check (no codegen, fast)
cargo check

# Run
cargo run -p heca

# Test
cargo test
cargo test -p heca-core
cargo test -p heca-renderer

# Lint (before committing)
cargo clippy --workspace --all-targets --all-features

# Fix auto-fixable issues
cargo clippy --fix --workspace --all-targets --all-features

# Watch (auto-rebuild on changes)
cargo watch -x check
```

---

## Planning

Outstanding work lives in **the planner** (single source of truth). See it for priorities and status.

---

## Known Issues (from code review)

See `niri-compatibility-review.md` for full details. Key issues:

| ID | Issue | Severity | Status |
|----|-------|----------|--------|
| K1 | Prefix mode hardcodes `ctrl=false` | Critical | ✅ **FIXED** — passes real modifier state |
| K2 | Prefix key not configurable | High | ✅ **FIXED** — `prefix = "ctrl+b"` in config |
| K3 | No prefix timeout | Medium | ✅ **FIXED** — 500ms auto-exit |
| K4 | Shift+special-char bindings fail on some layouts | High | ✅ **FIXED** — `KeyCombo::parse()` maps shifted symbols |
| K5 | Registry bypasses (direct function calls) | High | ✅ **FIXED** — all routing through `registry.execute()` |
| L1 | `update_all_column_widths()` on every mutation | Critical | ✅ **FIXED** — removed from float/unfloat path |
| L2 | Proportion widths not persistent | High | Open |
| L4 | Focus up/down conflated with workspace switch | Medium | ✅ **FIXED** — `j/k` stay within workspace; `u/d` switch |
| L6 | Tabbed display, maximize, fullscreen dead code | Medium | Open |
| N1 | PaneSelect/Swap limited to 52 labels | Low | By design — use sidebar for >52 panes |

---

## Key NIRI References

- `docs/niri-wiki/` — Structured NIRI wiki documentation (architecture, config, usage, development)
- `.agents/skills/niri/SKILL.md` — NIRI knowledge skill with source references
- `docs/niri-wiki/architecture.md` — Full architecture reference (layout engine internals, data flow)
- `docs/niri-wiki/04-development/design-principles.md` — NIRI's 5 design principles
- `docs/niri-wiki/04-development/fractional-layout.md` — Physical pixel alignment strategy

### NIRI Terminology Mapping

| NIRI term | heca equivalent | Notes |
|-----------|----------------|-------|
| `Tile<W>` | `Pane` | Content leaf node |
| `Column<W>` | `Column` | Vertical stack of panes |
| `ScrollingSpace<W>` | `ScrollingSpace` | Horizontal column strip |
| `Workspace<W>` | `Workspace` | Contains scrolling + floating |
| `Layout<W>` | `Session` | Top-level with overview + workspace switch |
| `ViewOffset` | `ViewOffset` | Three-state horizontal scroll |
| `Monitor` | (not implemented) | heca is single-window; monitor = output is future |

---

## Session Addendum — 2026-06-09

Track 2 — Surface-agnostic DnD architecture completed on `feature/gpt-refactoring`. PR #36 ready to merge.

### Work completed

**Track 1 — Rust code hygiene (PR #34, merged):**
- Remove dead `mouse/drop.rs`, clean `InputMode::Chord` allow
- Descriptive messages to 4 `unreachable!()` calls
- `Rectangle` type instead of `(f32,f32,f32,f32)` tuples
- Chrome constants (`DEFAULT_TAB_BAR_HEIGHT`, `DEFAULT_STATUS_BAR_HEIGHT`) into `chrome.rs`
- Split 163-line `on_cursor_moved()` into 4 named helpers
- Extract mouse release handlers into `mouse/release.rs`

**Track 2 — Surface-agnostic DnD (PR #36, open):**
- `heca-grid-ui/src/drag/` framework types (5 files, 430+ lines):
  - `DragSurfaceId` (enum), `DragItemId` (newtype), `DragContext` (per-surface state), `SurfaceDragPhase` (state machine)
  - `rubberband()` math with unit tests
- **⚠️ SUPERSEDED (2026-06-15) — generic DnD refactor (WS-A):** the framework is now
  **domain-neutral + generic over an app payload `P`**: `DragContext<P>` /
  `SurfaceDragState<P>` / `DragPhase<P>` (was `SurfaceDragPhase`); `DragItemKind`/`DragItem`
  removed (payload lives in the app's `AppDragPayload`). Added universal `DragExt`
  (`.draggable`/`.drop_target`), tree-geometry `drag::resolve_at`/`source_at`
  (`DropHit`/`DropSide`), and `PaintCx::drag_ghost`/`drop_indicator`. **Docs:
  `docs/widgets.md` §"Drag and drop"; design: `dnd-framework-refactor-plan.md`.** Never
  put pane/workspace/column concepts in the `drag` module.
- App integration: replace `DragState` with `DragContext` + `InteractiveMovePhase` (13 files)
- Enum dispatch: `mouse/target.rs` — compiler exhaustiveness when adding surfaces
- `mouse/surface_left.rs` — left sidebar handler; deleted `sidebar.rs`/`sidebar_drop.rs` (528 lines removed)
- `mouse/interactive.rs` — content-area drag extracted; `drag.rs` shrinks 45%
- Render: `Option<DragItemId>` instead of raw `usize`
- Dispatch wired into app: `mouse.rs` + `release.rs` route through `target::surface_*()`
- 3 Rust skill findings fixed (private field, redundant clear, `_pane_id` rename)

**DnD plan files deleted** — `.planning/dnd-*.md` and `.planning/refactoring-and-dnd-plan.md` removed.

### Remaining in original refactoring plan

- Phase 3.2: `handle_swap_param()` still needs delegation to shared helpers (partial progress)
- Phase 3.3: Reduce cross-file ad hoc search logic — not started
- Phase 4-10: Not started beyond what Track 1/2 incidentally touched

### Next start point

The refactoring track (Phases 0–10) is **complete**. All checklist items are done.
See `.planning/interaction-policy-plan.md` for remaining intent-routing work (Phase B/C).
See `docs/chrome-and-ui.md` for the future chrome/plugin architecture.

## Session Addendum — 2026-06-05

This addendum captures important project-specific rules and outcomes established during the current refactor session. Treat these as active working rules unless the user explicitly overrides them.

### Workflow rules for future phases

- Work **solo** by default — do not use intercom/subagent delegation unless the user explicitly asks for it again.
- **Before each new phase or major sub-phase, use the `/grill-me` skill** to acquire as much missing behavioral/product detail as possible before implementing.
- Before starting a new phase slice, explicitly read:
  - `AGENTS.md`
  - all directly affected code files
- **Pull/rebase from `origin/main` before starting each new task or phase slice.**
- Keep work in **small, behavior-preserving slices** with clean commits.
- After each meaningful slice, update:
  - `session-resume-handoff.md`
  - `.planning/STATE.md`
- when the user gives you hint or observation mark them in the agent-rules.md file (create if needed):
  - record what the user want you to do and what not to do
  - record important things to remember
  - try to follow coding standard and best practices and if you get scolted ask the user solutions and how they want to be implemented. Write in the file the user choice so you remeber next times.
  
### Action-system rules reinforced in this session

For any new app behavior that should be user-visible or scriptable:

- add a `WmAction` variant
- add `action_from_name()` mapping
- update `action_priority()` explicitly
- register the handler in `build_registry()`
- add metadata in `ActionRegistry::ALL` when user-facing
- make it bindable from config when appropriate

Do **not** introduce ad hoc behavior that bypasses the action system when the feature should be reachable from:

- keyboard
- mouse/UI
- RPC / future RPC

### Sidebar Phase 1.5 semantic rules already settled

These were clarified in detail with `/grill-me`; do not casually re-decide them:

- Sidebar mode is **selection-driven**.
- `j/k` and `Up/Down` move sidebar cursor only.
- Main scrolling/focus state does **not** auto-follow sidebar cursor movement.
- `h/l` and `Left/Right` are tree-navigation keys on structural rows.
- Pane / floating-pane leaf activation (`Enter`, `Right`, `l`, or second click in sidebar mode) focuses the leaf and exits `SidebarNav`.
- `Esc` exits sidebar mode and focuses contextual content.
- Sidebar-mode mutation keys are sidebar-only.
- Global prefix collapse actions use **active main-view state**, not sidebar selection.
- Sidebar collapse in current 1.5 work is **UI-tree collapse only**, not compositor/layout collapse.
- Explicit expand/collapse/toggle action families should exist when preparing for future RPC friendliness, even if only toggle variants get default bindings initially.

### Important reference files

Planning / rules:

- `docs/chrome-and-ui.md`
- `session-resume-handoff.md`
- `.planning/STATE.md`
- `.planning/ROADMAP.md`
- `.planning/interaction-policy-plan.md`

Default keybinding reference:

- `keybindings.default.toml`
- `README.md`

Sidebar/action implementation files:

- `heca/src/input.rs`
- `heca/src/actions.rs`
- `heca/src/app/registry.rs`
- `heca/src/app/input.rs`
- `heca/src/handlers.rs`
- `heca/src/mouse.rs`
- `heca/src/mouse/sidebar.rs`
- `heca/src/mouse/hit_test.rs`
- `heca/src/sidebar/model.rs`
- `heca/src/sidebar/hit_test.rs`
- `heca/src/sidebar/render.rs`
- `heca/src/sidebar/tests.rs`

Current `heca-config` split reference:

- `heca-config/src/color.rs`
- `heca-config/src/settings.rs`
- `heca-config/src/keys.rs`
- `heca-config/src/loader.rs`
- `heca-config/src/theme.rs`
- `heca-config/src/defaults.rs`

### Work completed in this session

Already completed:

- `heca-config` Phase 1.4 split work:
  - `color.rs`
  - `settings.rs`
  - `keys.rs`
  - `loader.rs`
  - `defaults.rs`
  - slimmed `theme.rs`
- Sidebar Phase 1.5 completed slices so far:
  - `1.5.1` normalize sidebar navigation contract
  - `1.5.2` add sidebar-only mutation keymap
  - `1.5.3` make sidebar actions selection-driven
  - `1.5.4` add mouse semantics for entering/exiting sidebar mode
  - `1.5.5` add disclosure hit targets and visual symbols for workspace + column rows

### Planned incoming phases / slices

Immediate remaining 1.5 work:

- `1.5.6` global sidebar-tree collapse action family
- `1.5.7` preserve public config/action surface for future RPC work
- `1.5.8` add/update focused sidebar tests
- `1.5.9` update docs/defaults

After that:

- proceed to sidebar intent routing (Phase B/C in `.planning/interaction-policy-plan.md`)
- future chrome/plugin architecture work is planned in `docs/chrome-and-ui.md`

## Workflow Rules for Future Phases

- Before starting a new phase, use the `/grill-me` skill to acquire as much information as possible and have a clear plan.
- At the end of tasks, wait for user approval before committing and creating a PR.
- When the session is about to run out of tokens (70/80%), write a detailed handoff with all information for restart without losing context.

## Agent Rules

### Workflow / Step Control

1. Before each step or sub-step, explicitly verify what has already been done, what will change next, and why.
2. Before making changes, state the next action and the validation you will run after it.
3. Do not start a new phase or major sub-phase until the current one is clearly complete and the user has approved the next step.
4. Before any new phase or major sub-phase, use the `/grill-me` skill first.
5. Double-check all details before updating checklists, handoffs, commits, or PRs.

### When Reading Code

1. Read `.planning/research/ARCHITECTURE.md` and `.planning/PROJECT.md` for context first.
2. Read `.agents/skills/niri/SKILL.md` when working on layout features.
3. Check `heca/src/input.rs` and `heca-config/src/theme.rs` for keybinding concerns.
4. Check `heca/src/main.rs` for registry setup and bypasses.
5. Run `cargo check` before and after changes — the project must compile.

### When Writing Code

1. Use the NIRI layout engine, not BSP (`pane.rs` is dead reference code).
2. Always use `Rectangle` from `layout/types.rs`, not `Rect` from `types.rs`.
3. **Every WM action goes through `registry.execute()`** — no direct function calls in event handlers.
4. Add new keybindings to both `keybindings.default.toml` (defaults) and `heca/src/input.rs` (action enum + parser + priority); every keybinding and theme variable must be configurable from `config.toml`/`keybindings.toml`.
5. Test prefix mode: verify both plain key and Ctrl-modified key bindings work.
6. Do NOT remove or refactor layout code without consulting the NIRI skill.
7. **NEVER add `#[allow(dead_code)]` without a clear reason.** Remove dead code instead. If a lint must be suppressed, add a `//` comment explaining why right above the attribute.
8. **After every task, run `cargo clippy --workspace --all-targets --all-features` and fix all warnings.** The codebase must stay clippy-clean. Use `cargo clippy --fix` for auto-fixable issues.
9. **Load `/Users/antonio/.agents/skills/rust/SKILL.md` and run a formal review against its rules before EVERY commit.** This is non-negotiable. Then run clippy, then commit. Never skip this.

### When Reviewing

1. Check for BSP tree references that should be NIRI scrolling columns.
2. Verify `update_all_column_widths()` isn't called unnecessarily.
3. **Verify no registry bypasses** — all state changes go through `registry.execute()`.
4. Check prefix mode passes real modifier state, not hardcoded `false`.
5. Verify column widths are stored per-column, not normalized.
6. Check `action_priority()` explicitly matches all variants.
