# Plan: Sidebar Tree + Naming + Navigation + Action Registry

**Created:** 2026-05-31
**Status:** Draft
**Depends on:** Phase 2 (NIRI layout engine done), Phase 3 (PaneBackend trait exists)

---

## Overview

This plan adds the left sidebar as a **live workspace/pane tree explorer**, workspace and pane naming, sidebar keyboard navigation, enhanced pane select/swap, and an **action registry** that will power a future command palette.

These features turn heca from a "layout you can see" into a "layout you can navigate and manage."

---

## Requirements

### New Actions

| Action | Default Binding | Description |
|--------|----------------|-------------|
| `sidebar_focus` | `e` | Focus the left sidebar (enter SidebarNav mode) |
| `create_workspace` | `w` | Create a new empty workspace |
| `rename_workspace` | `Shift+w` | Prompt to rename the current/selected workspace |
| `rename_pane` | `Shift+p` | Prompt to rename the selected pane |
| `toggle_sidebar_expand` | `Tab` or `Enter` | Collapse/expand a tree node in sidebar |
| `command_palette` | `p` | Open command palette overlay |
| `sidebar_up` | `k` or `ArrowUp` | Move selection up in sidebar |
| `sidebar_down` | `j` or `ArrowDown` | Move selection down in sidebar |
| `sidebar_left` | `h` or `ArrowLeft` | Collapse node / move selection to parent |
| `sidebar_right` | `l` or `ArrowRight` | Expand node / enter selected node |

### Modified Actions

| Action | Change |
|--------|--------|
| `pane_select` | Also show selection letters on sidebar tree items |
| `swap_select` | Include sidebar panes in candidate pool |
| `close_pane` | After removing last pane, auto-remove the workspace if empty |

### New State / Data Structures

- **SidebarTreeState** — tree model with expand/collapse, selection cursor
- **ActionRegistry** — static list of all `(name, description, default_binding, category)` for all actions
- **InputMode::SidebarNav** — keyboard navigation within the sidebar
- **InputMode::Rename** — text input for renaming workspaces/panes

---

## Phase 1: Sidebar Tree Data Model

**Goal:** Add a tree model to `AppState` that represents the sidebar's view of workspaces/panes, with expand/collapse state and selection tracking.

### Tasks

| # | Task | Status | Est. Effort |
|---|------|--------|-------------|
| 1.1 | Define `SidebarTree` struct in `app_state.rs` (or new `sidebar.rs` module) | Pending | Small |
| 1.2 | Store collapsed state per workspace node | Pending | Small |
| 1.3 | Store selection cursor (current hovered/selected item) | Pending | Small |
| 1.4 | Method to rebuild the tree from `Session` state | Pending | Small |
| 1.5 | Method to find pane/workspace ID from cursor position | Pending | Small |
| 1.6 | Auto-expand when entering SidebarNav mode | Pending | Small |
| 1.7 | Add `last_visited_ws_idx: Option<usize>` to Session/AppState for visited tracking | Pending | Small |
| 1.8 | Track workspace visits: update `last_visited_ws_idx` on every workspace switch | Pending | Small |
| 1.9 | Track pane focus visits: maintain `last_visited_pane_id` per workspace for dim highlight | Pending | Small |

### Details

The tree view mirrors the layout hierarchy:

```
 (focused workspace, highlighted)
├─ Workspace 1  (collapsible)
│  ├─ Column 0
│  │  ├─ Pane "main"        (← active pane, highlighted)
│  │  ├─ Pane "logs"
│  │  └─ Pane "debug"
│  └─ Column 1
│     └─ Pane "browser"
├─ Workspace 2              (collapsed — children hidden)
│  └─ ...
└─ Workspace 3
   └─ ...
```

**Data structure (in `heca/src/sidebar.rs`):**

```rust
#[derive(Debug, Clone)]
pub struct SidebarTree {
    /// Workspace entries in order.
    pub workspaces: Vec<SidebarWsEntry>,
    /// Index into the flat list of all selectable items.
    pub cursor: usize,
    /// Total number of selectable items (for cursor clamping).
    pub item_count: usize,
}

#[derive(Debug, Clone)]
pub struct SidebarWsEntry {
    pub ws_idx: usize,
    pub collapsed: bool,
    pub columns: Vec<SidebarColEntry>,
}

#[derive(Debug, Clone)]
pub struct SidebarColEntry {
    pub col_idx: usize,
    pub panes: Vec<SidebarPaneEntry>,
}

#[derive(Debug, Clone)]
pub struct SidebarPaneEntry {
    pub pane_id: u64,
    pub name: String,
}
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItemState {
    /// Currently active (focused workspace / pane).
    Active,
    /// Was visited previously this session (last focused before current).
    Visited,
    /// Not visited this session.
    None,
}
```

Each tree entry stores an `item_state` field computed from the session's active + visited indices.

A flat navigation list is built from the tree for cursor movement:

```rust
enum SidebarItem {
    Workspace { ws_idx: usize },
    Column { ws_idx: usize, col_idx: usize },
    Pane { pane_id: u64 },
}
```

This drives the keyboard cursor (`sidebar_up` / `sidebar_down` move through the flat list, skipping collapsed items).

**Visited tracking** lives in `AppState`:

```rust
pub struct AppState {
    // ... existing fields ...
    /// Last visited workspace index (for dim highlight in sidebar).
    pub last_visited_ws_idx: Option<usize>,
    /// Per-workspace last-visited pane IDs (for dim highlight).
    /// Key = workspace index, Value = pane ID.
    pub last_visited_pane_per_ws: Vec<Option<u64>>,
}
```

Updated every time the user switches workspace or focuses a different pane:
- On workspace switch: push `current_ws_idx` → `last_visited_ws_idx`, set new `current_ws_idx`
- On pane focus within same workspace: update `last_visited_pane_per_ws[ws_idx]`

```rust
// In focus_pane_by_id() or execute_action():
if let Some(old_ws_idx) = state.last_visited_ws_idx {
    // Record which pane was active in the departing workspace
    if let Some(ws) = state.session.workspaces.get(old_ws_idx) {
        if let Some(pane) = ws.active_pane() {
            state.last_visited_pane_per_ws[old_ws_idx] = Some(pane.id.0);
        }
    }
}
state.last_visited_ws_idx = Some(state.session.active_workspace_idx);
```

### Key Files to Modify

- `heca/src/app_state.rs` — add `sidebar_tree: SidebarTree` field to `AppState`, add `InputMode::SidebarNav(SidebarNavState)`
- `heca/src/sidebar.rs` — NEW file: `SidebarTree`, `SidebarItem`, `SidebarNavState`
- `heca/src/main.rs` — initialize sidebar tree, rebuild when layout changes

---

## Phase 2: Sidebar Rendering

**Goal:** Replace the static "Sessions (empty)" text with a rendered workspace/pane tree that respects expand/collapse and highlights the focused/active items.

### Tasks

| # | Task | Status | Est. Effort |
|---|------|--------|-------------|
| 2.1 | Render workspace entries with indentation and collapse arrows (`v`/`>`) | Pending | Medium |
| 2.2 | Render column headings for columns with multiple panes | Pending | Small |
| 2.3 | Render pane entries with name and active indicator | Pending | Medium |
| 2.4 | Highlight the sidebar cursor selection | Pending | Small |
| 2.5 | Highlight the currently focused pane/workspace in sidebar | Pending | Small |
| 2.6 | Scroll sidebar content if tree exceeds available height | Pending | Medium |
| 2.7 | Rebuild sidebar tree and re-render on layout changes | Pending | Small |
| 2.8 | Show selection letters during PaneSelect/PaneSwap mode in sidebar | Pending | Small |
| 2.9 | Collapsed sidebar rendering (activity strip at 40px) with workspace number + pane letters | Pending | Medium |
| 2.10 | Activity bar indicators (active/visited/never) with accent colors and opacity | Pending | Small |
| 2.11 | Increase collapsed sidebar width from 32px to 40px in ChromeConfig | Pending | Small |
| 2.12 | Active pane highlighted letter, last-visited pane subtle indicator | Pending | Small |

### Details

Rendering happens in the same `render()` function in `main.rs`, in the left sidebar section.

**Layout estimation (expanded sidebar, 200px):**
- Each item: 1 line of text (24px height at default chrome_text=14px)
- Indentation per level: 16px
- Collapse indicator: `▶` (collapsed) / `▼` (expanded)
- Active item: bold/highlighted with accent color
- Cursor item: inverted background or different border

**Scroll offset:** Track a `scroll_offset: usize` in `SidebarNavState` so the tree can scroll.

**Selection letter rendering:** During `PaneSelect`/`PaneSwap` mode, render the assigned letter next to each sidebar pane entry, so the user can see all panes including ones scrolled off-screen in the main view.

---

### Collapsed Sidebar Design ("Activity Strip")

When the sidebar is collapsed (`left_visible = false`), it shrinks to 40px (up from the current 32px to give room for identifiers). This becomes an **activity strip** — a compact workspace/pane overview that fits in a narrow column.

**Visual layout (40px collapsed):**

```
┌──────┐
│▓ 1  │  ← active WS (solid accent left bar, bold number)
│ a   │  ← active pane (highlighted letter)
│ b   │  ← other pane
│──────│
│▣ 2  │  ← last visited WS (50% accent left bar, dimmer)
│ c   │  ← pane in visited workspace
│──────│
│ 3   │  ← never-visited WS (no left bar)
│ d   │
└──────┘
```

**Elements:**

| Element | Width | Description |
|---------|-------|-------------|
| Activity bar | 4px | Left-edge colored stripe per workspace section |
| Gap | 4px | Space between bar and text |
| Identifier | ~24px | Workspace number or pane letter |
| Right pad | 8px | Breathing room and border |

**Activity bar indicators:**

| State | Visual | Meaning |
|-------|--------|---------|
| Active workspace | ▓ Solid 4px accent bar, full height | The workspace currently displayed |
| Last visited | ▣ 4px accent bar at 50% opacity | The workspace most recently active (before current) |
| Never visited | (no bar) | Workspace hasn't been switched to this session |

**Identifier scheme:**

- **Workspace numbers**: `1`, `2`, `3`... `99` (fits in 2 chars = ~17px at 14px font)
- **Pane letters**: `a`, `b`, `c`... (single char = ~8.4px, abundant room)
- **Workspace name fallback**: if workspace has a custom name, show first 2 characters instead of number
- **Pane name fallback**: if pane has a custom name, show first character instead of auto-letter

**Active pane indicator:**
- The currently focused pane's letter is rendered in **accent color**, bold if possible
- The last focused pane (if different from current) gets a subtle underline or a trailing `·` dot

**Interaction in collapsed state:**
- `Prefix+e` still enters SidebarNav mode — the collapsed strip becomes navigable with j/k
- Selecting a workspace toggles the sidebar to expanded view (auto-expand on Enter)
- Mouse click on a pane letter focuses that pane
- Mouse click on a workspace number switches to that workspace

**Implementation details:**
- The collapsed sidebar width changes from 32px → 40px in `chrome.rs`
- The activity bar is drawn as a `PrimitiveRenderer::draw_rect()` colored rect
- The identifiers are drawn via `TextRenderer::queue_text()` at a compact font size (12-13px to fit, or the standard chrome_text=14px)
- The tree data model (`SidebarTree`) is still built — it's always built, just rendered differently based on width
- `scroll_offset` still applies for trees that exceed the sidebar height

```rust
// Pseudo-code for collapsed rendering:
fn render_sidebar_collapsed(tree: &SidebarTree, ...) {
    for ws_entry in &tree.workspaces {
        // Draw activity bar
        let bar_color = match ws_entry.state {
            SidebarItemState::Active => ACCENT_COLOR,
            SidebarItemState::Visited => ACCENT_COLOR.dim(0.5),  // 50% opacity
            SidebarItemState::None => TRANSPARENT,
        };
        draw_rect(x, y, 4, section_height, bar_color);
        
        // Draw workspace number
        draw_text(x + 8, y, &ws_entry.label, chrome_text, DEFAULT_FG);
        
        // Draw pane letters below
        for pane in &ws_entry.panes {
            y += LINE_HEIGHT;
            let color = if pane.is_active { ACCENT_COLOR } else { DEFAULT_FG };
            draw_text(x + 8, y, &pane.label, chrome_text, color);
        }
    }
}
```

**Why 40px vs the current 32px:**
- 32px leaves ~22px usable after borders → only 2.5 characters → can't fit `2` + padding
- 40px leaves ~30px usable → comfortably fits `12` (2 chars) + padding
- 40px is still narrow enough to feel "collapsed" and not compete with content
- Matches common collapsed sidebar widths (VS Code's activity bar is 48px, many designs use 36-44px)

**Reference:** [Sidebar Nav — UI Anatomy](https://uianatomy.dev/components/sidebar-nav) describes the standard patterns this design follows — persistent navigation rail with expanded/collapsed variants, active-item differentiation via accent strip + background + text color (triple-layer), scrollable item region, and collapsed-state icons replaced by compact identifiers.

### Key Files to Modify

- `heca/src/main.rs` — replace sidebar rendering section (around line 470) with tree renderer
- `heca/src/sidebar.rs` — add `render_sidebar()` function that uses `TextRenderer` and `PrimitiveRenderer`
- `heca-renderer/src/text.rs` — if color-per-character rendering is needed, add simple colored text batches

---

## Phase 3: Naming and Workspace Creation

**Goal:** Let users create workspaces, rename workspaces, and rename panes. The `Pane.title` field already exists — this adds UI to set it.

### Tasks

| # | Task | Status | Est. Effort |
|---|------|--------|-------------|
| 3.1 | Add `WmAction::CreateWorkspace`, `RenameWorkspace`, `RenamePane` to enum | Pending | Small |
| 3.2 | Add `InputMode::Rename { target: RenameTarget, buffer: String }` | Pending | Small |
| 3.3 | Implement `create_workspace`: add empty WS + activate it | Pending | Small |
| 3.4 | Implement `rename_workspace`: enter Rename mode for workspace name | Pending | Small |
| 3.5 | Implement `rename_pane`: enter Rename mode for pane name | Pending | Small |
| 3.6 | Rename mode handling: Type text → Enter confirms, Esc cancels | Pending | Medium |
| 3.7 | Add `set_workspace_name()` / `set_pane_name()` to Session/Workspace/Pane | Pending | Small |
| 3.8 | Add default keybindings to config | Pending | Small |

### Details

**Rename Input Mode:**

```rust
InputMode::Rename {
    target: RenameTarget,
    buffer: String,
}

enum RenameTarget {
    Workspace(usize),      // ws_idx
    Pane(u64),            // pane_id
}
```

The rename mode captures all keyboard input as text:
- Alphanumeric keys → append to `buffer`
- Backspace → pop last char
- Enter → commit rename, return to Normal
- Escape → cancel, return to Normal
- The status bar shows "Rename: <buffer>_" with cursor

**Workspace::name** is already `Option<String>`. The session already has `add_workspace()` and `remove_workspace()`. Need to add `set_workspace_name(ws_idx, name)` method.

**Pane.title** already exists — just need a method to set it.

A `WorkspaceId(0)` format: the workspace's display name is its `name` field if set, or `"Workspace {idx+1}"` as fallback.

### Key Files to Modify

- `heca/src/input.rs` — add `CreateWorkspace`, `RenameWorkspace`, `RenamePane` to enum + parser
- `heca/src/main.rs` — add execute_action cases + rename mode handling in KeyboardInput
- `heca/src/app_state.rs` — add `InputMode::Rename`, `RenameTarget`
- `heca-core/src/layout/session.rs` — add `set_workspace_name()`, `set_pane_name()`
- `heca-config/src/theme.rs` — add default keybindings

---

## Phase 4: Sidebar Navigation Mode

**Goal:** `Prefix + e` focuses the sidebar, letting the user navigate workspaces/panes with keyboard (j/k/l/h) and mouse, and press Enter to switch to the selected item.

### Tasks

| # | Task | Status | Est. Effort |
|---|------|--------|-------------|
| 4.1 | Add `WmAction::SidebarFocus` to enum + parser + execute_action | Pending | Small |
| 4.2 | Enter SidebarNav mode: rebuild tree, set cursor on current workspace | Pending | Small |
| 4.3 | `sidebar_up/down`: move cursor in flat item list with clamping | Pending | Small |
| 4.4 | `sidebar_left`: collapse parent / move to parent | Pending | Small |
| 4.5 | `sidebar_right`: expand node / activate item | Pending | Small |
| 4.6 | Enter on a pane: focus that pane in the layout, exit SidebarNav | Pending | Medium |
| 4.7 | Enter on a workspace: switch to that workspace, exit SidebarNav | Pending | Small |
| 4.8 | Mouse click on sidebar item: focus that item | Pending | Medium |
| 4.9 | Exit SidebarNav via Escape or clicking outside sidebar | Pending | Small |
| 4.10 | Scroll the tree view when cursor goes out of visible bounds | Pending | Small |
| 4.11 | Add WmActions for sidebar_up/down/left/right + enter/expand | Pending | Small |

### Details

**SidebarNavState:**

```rust
#[derive(Debug, Clone)]
pub struct SidebarNavState {
    pub cursor: usize,
    pub scroll_offset: usize,
    pub item_count: usize,
}
```

The flat item list (built from the tree, skipping collapsed items):

```rust
enum FlatItem {
    Workspace { ws_idx: usize },
    Column { ws_idx: usize, col_idx: usize },
    Pane { pane_id: u64 },
}
```

**Navigation semantics:**
- `sidebar_up` (j/↓): cursor-- (skip collapsed workspace headers' children)
- `sidebar_down` (k/↑): cursor++ (skip collapsed workspace headers' children)
- `sidebar_right` (l/→): if cursor is on a collapsed workspace → expand it; if on a pane → focus it and exit sidebar mode
- `sidebar_left` (h/←): if cursor is on a child of a workspace → collapse that workspace; if on workspace header → collapse it
- `Enter`: same as right on the current item
- `Escape`: exit SidebarNav back to Normal

**Mouse hit-testing:** The render function stores the pixel rects of each sidebar item. On mouse click, compute which item was clicked and either select it (single click) or activate it (double click / click in SidebarNav mode).

### Key Files to Modify

- `heca/src/sidebar.rs` — `SidebarNavState`, `FlatItem`, navigation methods
- `heca/src/input.rs` — new WmAction variants
- `heca/src/main.rs` — input handling for SidebarNav mode, execute_action cases
- `heca-config/src/theme.rs` — add default bindings

---

## Phase 5: Enhanced Pane Select/Swap

**Goal:** `Prefix+q` (PaneSelect) and `Prefix+Shift+q` (SwapSelect) should show selection letters not just on rendered panes but also in the sidebar, so the user can pick panes that are scrolled off-screen.

### Tasks

| # | Task | Status | Est. Effort |
|---|------|--------|-------------|
| 5.1 | When computing candidates for PaneSelect, include ALL panes across all workspaces (not just active) | Pending | Small |
| 5.2 | Render selection letters in sidebar during PaneSelect/Swap | Pending | Small |
| 5.3 | Letter-to-pane mapping stays visible until selection | Pending | Small |
| 5.4 | After selection, focus the target pane in its workspace (switch workspace if needed) | Pending | Medium |
| 5.5 | SwapSelect: include all panes across all workspaces | Pending | Small |

### Details

Currently `PaneSelect` only collects panes in the active workspace. Change to collect ALL panes in ALL workspaces:

```rust
for ws in &state.session.workspaces {
    for col in &ws.scrolling.columns {
        for pane in &col.panes {
            let ch = (b'a' + candidates.len() as u8) as char;
            candidates.push((ch, pane.id.0));
        }
    }
    for float in &ws.floating_panes {
        ...
    }
}
```

When the user selects a pane from a different workspace:
1. Switch to that workspace
2. Focus the pane
3. Close PaneSelect mode

### Key Files to Modify

- `heca/src/main.rs` — PaneSelect candidate collection, selection handling
- `heca/src/sidebar.rs` — render selection letters in sidebar

---

## Phase 6: Command Palette Backend

**Goal:** Build the action registry and command palette data model. The command palette itself (UI overlay) will be implemented in a follow-up plan, but the data source is built here.

### Tasks

| # | Task | Status | Est. Effort |
|---|------|--------|-------------|
| 6.1 | Define `ActionDescriptor { name, description, default_binding, category }` | Pending | Small |
| 6.2 | Build static `ActionRegistry` with ALL actions documented | Pending | Medium |
| 6.3 | Each action has: config name, human-readable description, category, default bindings | Pending | Medium |
| 6.4 | Add `WmAction::CommandPalette` variant (no-op until UI built) | Pending | Small |
| 6.5 | Action registry exported from `heca/src/actions.rs` | Pending | Small |

### Details

**ActionDescriptor:**

```rust
pub struct ActionDescriptor {
    /// Config key name (e.g. "focus_left").
    pub name: &'static str,
    /// Human-readable label for the command palette.
    pub label: &'static str,
    /// Short description.
    pub description: &'static str,
    /// Category for grouping in palette.
    pub category: ActionCategory,
    /// Default keybinding string (e.g. "h,ArrowLeft").
    pub default_binding: &'static str,
}

pub enum ActionCategory {
    Navigation,     // focus, sidebar nav
    Layout,         // splits, resize, move
    Pane,           // float, hide, scratchpad, close, rename
    Workspace,      // create, rename, switch
    Session,        // save, load, overview
    Chrome,         // sidebar toggle, tab management
    System,         // command palette, quit
}
```

**All actions to register (31 existing + 6 new = 37):**

| Config Name | Label | Category | Default |
|-------------|-------|----------|---------|
| `focus_left` | Focus Column Left | Navigation | `h,ArrowLeft` |
| `focus_right` | Focus Column Right | Navigation | `l,ArrowRight` |
| `focus_up` | Focus Pane Up | Navigation | `k,ArrowUp` |
| `focus_down` | Focus Pane Down | Navigation | `j,ArrowDown` |
| `next_pane` | Next Pane in Column | Navigation | `n` |
| `prev_pane` | Previous Pane in Column | Navigation | `p` |
| `sidebar_focus` | Focus Sidebar | Navigation | `e` |
| `sidebar_up` | Sidebar Cursor Up | Navigation | `k` |
| `sidebar_down` | Sidebar Cursor Down | Navigation | `j` |
| `sidebar_left` | Sidebar Collapse/Out | Navigation | `h` |
| `sidebar_right` | Sidebar Expand/Enter | Navigation | `l` |
| `split_horizontal` | New Column (HSplit) | Layout | `Enter` |
| `split_vertical` | New Pane in Column (VSplit) | Layout | `v` |
| `resize_increase` | Increase Column Width | Layout | `=` |
| `resize_decrease` | Decrease Column Width | Layout | `-` |
| `pane_height_increase` | Increase Pane Height | Layout | `Shift+=` |
| `pane_height_decrease` | Decrease Pane Height | Layout | `Shift+-` |
| `swap_left` | Swap Column Left | Layout | `Ctrl+h` |
| `swap_right` | Swap Column Right | Layout | `Ctrl+l` |
| `swap_up` | Swap Pane Up | Layout | `Ctrl+k` |
| `swap_down` | Swap Pane Down | Layout | `Ctrl+j` |
| `move_pane_left` | Move Pane to Column Left | Layout | `[` |
| `move_pane_right` | Move Pane to Column Right | Layout | `]` |
| `close` | Close Pane | Pane | `x` |
| `float` | Toggle Float | Pane | `f` |
| `pane_select` | Quick-Select Pane | Pane | `q` |
| `swap_select` | Quick-Swap Pane | Pane | `Shift+q` |
| `rename_pane` | Rename Pane | Pane | `Shift+p` |
| `create_workspace` | Create Workspace | Workspace | `w` |
| `rename_workspace` | Rename Workspace | Workspace | `Shift+w` |
| `toggle_overview` | Toggle Overview | Session | (new) |
| `sidebar_left` | Toggle Left Sidebar | Chrome | `b` |
| `sidebar_right` | Toggle Right Sidebar | Chrome | `.` |
| `tab_next` | Next Tab | Chrome | `Ctrl+]` |
| `tab_prev` | Previous Tab | Chrome | `Ctrl+[` |
| `command_palette` | Command Palette | System | `p` |
| `toggle_overview` | Toggle Overview | Layout | (add to config) |

### Key Files to Modify

- `heca/src/actions.rs` — NEW file: `ActionRegistry`, `ActionDescriptor`, `ActionCategory`
- `heca/src/main.rs` — stub execute_action for CommandPalette
- `heca/src/input.rs` — add `CommandPalette` variant
- `heca-config/src/theme.rs` — add `toggle_overview` default binding

---

## Phase 7: Workspace Auto-Removal

**Goal:** When the last pane in a workspace is closed, remove the workspace automatically. Currently `remove_workspace()` exists but may not be called after every pane removal.

### Tasks

| # | Task | Status | Est. Effort |
|---|------|--------|-------------|
| 7.1 | After every pane removal (ClosePane, remove_pane, etc.), check if active workspace became empty | Pending | Small |
| 7.2 | If empty and not pinned, call `Session::remove_workspace()` | Pending | Small |
| 7.3 | Ensure there's always at least one workspace (create default if last removed) | Pending | Small |
| 7.4 | Update sidebar tree after workspace removal | Pending | Small |

### Details

After the pane removal logic in `execute_action` (ClosePane case), add:

```rust
// After removing pane, check if workspace is empty
if let Some(ws) = state.session.active_workspace() {
    if !ws.has_panes() && !ws.is_pinned {
        let idx = state.session.active_workspace_idx;
        state.session.remove_workspace(idx);
        // Ensure at least one workspace exists
        if state.session.workspaces.is_empty() {
            let working_area = Rectangle::new(Point::default(), state.session.viewport_size);
            state.session.add_workspace(working_area);
        }
        sync_focus(state);
    }
}
```

### Key Files to Modify

- `heca/src/main.rs` — add workspace cleanup after ClosePane and after move-to-workspace operations
- `heca-core/src/layout/session.rs` — ensure `remove_workspace` updates `active_workspace_idx` correctly (already done)

---

## Phase 8: Config Extensions

**Goal:** Add new config options for the sidebar and new keybindings.

### Tasks

| # | Task | Status | Est. Effort |
|---|------|--------|-------------|
| 8.1 | Add default keybindings for all new actions in theme.rs | Pending | Small |
| 8.2 | Add `sidebar_default_width` and `sidebar_min_width` to config | Pending | Small |
| 8.3 | Add `auto_remove_empty_workspaces` toggle (default: true) | Pending | Small |

### Default Keybindings Table

```rust
keybindings.insert("sidebar_focus".to_string(), "e".to_string());
keybindings.insert("sidebar_up".to_string(), "k,ArrowUp".to_string());
keybindings.insert("sidebar_down".to_string(), "j,ArrowDown".to_string());
keybindings.insert("sidebar_left".to_string(), "h,ArrowLeft".to_string());
keybindings.insert("sidebar_right".to_string(), "l,ArrowRight,Enter".to_string());
keybindings.insert("create_workspace".to_string(), "w".to_string());
keybindings.insert("rename_workspace".to_string(), "Shift+w".to_string());
keybindings.insert("rename_pane".to_string(), "Shift+p".to_string());
keybindings.insert("command_palette".to_string(), "p".to_string());
keybindings.insert("toggle_overview".to_string(), "o".to_string());
```

### Key Files to Modify

- `heca-config/src/theme.rs` — new bindings + new config fields
- `heca-config/src/lib.rs` — serde defaults

---

## Phase 9: Update Input Mode State Machine

**Goal:** Update the keyboard event handler to handle all new input modes correctly.

### Tasks

| # | Task | Status | Est. Effort |
|---|------|--------|-------------|
| 9.1 | Add `SidebarNav` to match arms in `window_event` KeyboardInput handler | Pending | Medium |
| 9.2 | Add `Rename` to match arms, forwarding printable keys to buffer | Pending | Medium |
| 9.3 | Ensure PaneSelect/Swap candidates don't interfere with other modes | Pending | Small |
| 9.4 | Add prefix mode timeout (500ms) so user doesn't get stuck | Pending | Small |

### Key Files to Modify

- `heca/src/main.rs` — extend the big match on `state.input_mode`

---

## Effort Estimate Summary

| Phase | Tasks | Est. Effort | Risk Level |
|-------|-------|-------------|------------|
| 1 — Sidebar Tree Data Model | 6 | Small | Low |
| 2 — Sidebar Rendering | 8 | Medium | Medium |
| 3 — Naming & Creation | 8 | Medium | Low |
| 4 — Sidebar Navigation Mode | 11 | Medium | Medium |
| 5 — Enhanced Pane Select/Swap | 5 | Small | Low |
| 6 — Command Palette Backend | 5 | Medium | Low |
| 7 — Workspace Auto-Removal | 4 | Small | Low |
| 8 — Config Extensions | 3 | Small | Low |
| 9 — Input Mode State Machine | 4 | Medium | Medium |
| **Total** | **63** | **~3-4 weeks** | |

---

## Files to Create

| File | Purpose |
|------|---------|
| `heca/src/sidebar.rs` | SidebarTree, SidebarNavState, sidebar rendering, tree navigation |
| `heca/src/actions.rs` | ActionRegistry, ActionDescriptor, ActionCategory |

## Files to Modify

| File | Changes |
|------|---------|
| `heca/src/app_state.rs` | Add sidebar_tree, SidebarNavState, InputMode variants, last_visited_ws_idx, last_visited_pane_per_ws |
| `heca/src/input.rs` | Add 12 new WmAction variants + parser entries + priority entries |
| `heca/src/main.rs` | Execute_action for new actions, SidebarNav input handling, rename mode, sidebar rendering, workspace auto-remove |
| `heca-core/src/layout/session.rs` | add set_workspace_name(), set_pane_name() helpers |
| `heca-core/src/layout/workspace.rs` | Maybe set_name() method |
| `heca-core/src/layout/column.rs` | Pane.set_title() method |
| `heca-config/src/theme.rs` | All new default keybindings, sidebar config fields |

## Test Plan

| Phase | Test Scenario |
|-------|---------------|
| 1 | Build tree from session with 2 workspaces, 3 panes each → verify tree structure matches |
| 1b | After switching from WS1 to WS2, `last_visited_ws_idx == Some(0)` → sidebar shows visited indicator on WS1 |
| 2 | Render sidebar with 3 workspaces, one collapsed → verify descendants hidden |
| 2b | Collapse sidebar (`Prefix+b`) → 40px strip shows workspace numbers + pane letters. Active WS has solid accent bar. Last visited has 50% bar. |
| 3 | Rename workspace → verify Session.workspaces[i].name updated. Rename pane → verify pane.title updated. |
| 4 | Prefix+e → sidebar focused. j/k moves cursor. l enters workspace. h collapses. Escape exits. |
| 5 | Prefix+q in a 2-workspace session → letters shown in sidebar for ALL panes. Selecting cross-workspace pane switches workspace. |
| 6 | `ActionRegistry::all()` returns 37 actions. Each has label, category, description. |
| 7 | Close last pane in workspace → workspace disappears. Close pane in last workspace → workspace stays (min 1). |
| 8 | Config file with custom bindings for new actions → bindings override defaults. |
| 9 | Press prefix then wait 500ms → auto-exits prefix. Press q after prefix → PaneSelect shows letters. |

---

## Future: Command Palette UI (Next Plan)

This plan builds the action registry and the `CommandPalette` action hook. The actual command palette overlay **UI** (text input, fuzzy matching, keyboard navigation of results, Enter to execute) will be in the next plan (`plans/02-command-palette-ui.md`). The action registry created here is the data source for that feature.

---

## Requirements Added/Updated

These should be merged into `.planning/REQUIREMENTS.md`:

### New Layout Requirements

- [ ] **LAY-13**: Left sidebar displays workspace tree with expand/collapse and active indicators
- [ ] **LAY-14**: Sidebar tree scrolls when content exceeds sidebar height
- [ ] **LAY-15**: Clicking an item in the sidebar focuses the corresponding pane/workspace
- [ ] **LAY-16**: Collapsed sidebar (40px activity strip) shows workspace numbers + pane letters with activity bar indicators
- [ ] **LAY-17**: Active workspace gets solid accent activity bar; last visited gets 50% opacity bar; never visited gets none
- [ ] **LAY-18**: Last visited workspace tracked and visually distinguished from current active workspace
- [ ] **LAY-19**: Last visited pane tracked per workspace for subtle highlight indicator

### New Input Requirements

- [ ] **INP-11**: Sidebar navigation mode (Prefix+e) with j/k/l/h keyboard navigation
- [ ] **INP-12**: Workspace creation (Prefix+w), rename workspace (Prefix+Shift+w), rename pane (Prefix+Shift+p)
- [ ] **INP-13**: Rename input mode captures typed text, Enter to confirm, Escape to cancel
- [ ] **INP-14**: Pane select (Prefix+q) and swap select (Prefix+Shift+q) show letters on sidebar items
- [ ] **INP-15**: Pane select across ALL workspaces (not just active)
- [ ] **INP-16**: Command palette trigger (Prefix+p) — stub until UI is built

### New Session Requirements

- [ ] **SESS-08**: Empty workspaces auto-removed when last pane is closed
- [ ] **SESS-09**: At least one workspace always exists

### New Config Requirements

- [ ] **CONF-06**: Sidebar width configurable in config
- [ ] **CONF-07**: `auto_remove_empty_workspaces` option (default true)

### Updated Requirements

- [ ] **LAY-10**: Toggle pane between embedded (tiling) and floating — ADD: "with workspace content area placement"
- [ ] **INP-05**: Keyboard pane operations — ADD: "rename, create workspace, rename workspace"

---

## Dependency Graph

```
Phase 1 ──► Phase 2 ──► Phase 4 ──► Phase 9
  │                       │
  ▼                       ▼
Phase 3 ──────────────► Phase 5 ──► Phase 9
  │                       │
  ▼                       ▼
Phase 6 ◄───────────────────────► Phase 8
  │
  ▼
Phase 7 ──► Phase 9
  │
  ▼
Phase 8
```

Phases can be executed in this order: **1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9**
Or in parallel: **1+3+6** (data model), then **2+4+5+7** (UI + behavior), then **8+9** (config + polish).
