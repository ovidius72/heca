# Phase 2: The Workspace — Plan

**Phase:** 2
**Name:** The Workspace
**Created:** 2026-05-29
**Waves:** 4
**Requirements Addressed:** COMP-05,06,07,11,12 | LAY-01 through LAY-12 | INP-01 through INP-10 | CONF-02 | PANE-10

---

## Must-Haves (Goal-Backward Verification)

1. Panes can be split horizontally and vertically via keyboard
2. Keyboard navigates between panes (HJKL)
3. Splits resize via keyboard and mouse drag
4. A pane can float, be embedded back, hidden, or sent to scratchpad
5. Tab bar displays tabs and allows switching
6. Left/right sidebars exist, are collapsible, and show placeholder content
7. Status bar shows pane count + focused title + mode
8. Ctrl+B prefix captures next key as WM command
9. Mouse click focuses a pane; mouse drag on border resizes splits

---

## Wave 1: Core Layout Engine + Pane State Machine

### Plan 01: BSP Tree + Pane State

**Objective:** Replace `MockLayout` with a proper binary BSP tree data structure and a `Pane` type with explicit state (Embedded, Floating, Scratchpad, Hidden).

**Requirements:** LAY-01, LAY-02, LAY-03, LAY-11, LAY-12, PANE-10

**Tasks:**

#### Task 1.1: Define Pane and PaneTree types in heca-core
<action>
Create `heca-core/src/pane.rs`:

```rust
use crate::types::Rect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Disposition {
    Embedded,         // in the BSP tree
    Floating(Rect),   // absolute position
    Scratchpad,       // hidden until toggled
    Hidden,           // alive but not displayed
}

#[derive(Clone, Debug)]
pub struct Pane {
    pub id: u64,
    pub title: String,
    pub disposition: Disposition,
    pub background: [f32; 4],
}

#[derive(Clone, Debug)]
pub enum LayoutNode {
    Split {
        dir: SplitDirection,
        ratio: f32, // 0.0–1.0, fraction for left/top child
        left: Box<LayoutNode>,
        right: Box<LayoutNode>,
    },
    Leaf {
        pane_id: u64,
    },
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

pub struct PaneTree {
    pub panes: Vec<Pane>,
    pub root: LayoutNode,
    pub floats: Vec<(u64, Rect)>,
    pub next_id: u64,
}

impl PaneTree {
    pub fn new() -> Self { ... }
    pub fn add_pane(&mut self, title: &str) -> u64 { ... }
    pub fn split(&mut self, pane_id: u64, dir: SplitDirection) -> Option<u64> { ... }
    pub fn remove(&mut self, pane_id: u64) { ... }
    pub fn compute_rects(&self, width: f32, height: f32) -> (Vec<(Rect, &Pane)>, Vec<(&Pane, Rect)>) { ... }
    pub fn toggle_float(&mut self, pane_id: u64, default_rect: Option<Rect>) { ... }
    pub fn toggle_scratchpad(&mut self, pane_id: u64) { ... }
    pub fn hide(&mut self, pane_id: u64) { ... }
    pub fn show(&mut self, pane_id: u64) { ... }
}
```

Add `pub mod pane;` to `heca-core/src/lib.rs`.
</action>

#### Task 1.2: Implement PaneTree methods
<action>
Implement all methods on `PaneTree`:

- `new()` — empty tree with no panes
- `add_pane(title)` — creates a pane, adds to tree as root or replaces Empty, returns id
- `split(pane_id, dir)` — find leaf with that pane_id, replace with Split{dir, ratio:0.5, left: leaf, right: Empty}
- `remove(pane_id)` — replace leaf with Empty; if parent becomes unnecessary, collapse
- `compute_rects(w, h)` — traverse tree, subdivide rects by Split ratios; collect embedded rects + float panes
- `toggle_float(id, rect)` — if embedded, remove from tree and add to floats at given rect; if floating, re-embed into tree
- `toggle_scratchpad(id)` — toggle between scratchpad/hidden and floating visibility
- `hide(id)` — set disposition to Hidden
- `show(id)` — set disposition to Embedded (or previous state)

Use `Rect::new(x, y, w, h)` from `heca-core/src/types.rs`.
</action>

#### Task 1.3: Create placeholder panes and implement compute_rects
<action>
Implementation details for `compute_rects`:

```rust
pub fn compute_rects(&self, width: f32, height: f32) -> (Vec<(Rect, &Pane)>, Vec<(&Pane, Rect)>) {
    let mut embedded = Vec::new();
    self.collect_leaf_rects(&self.root, Rect::new(0.0, 0.0, width, height), &mut embedded);
    let floats: Vec<_> = self.floats.iter().filter_map(|(id, rect)| {
        self.panes.iter().find(|p| p.id == *id).map(|p| (p, *rect))
    }).collect();
    (embedded, floats)
}

fn collect_leaf_rects(&self, node: &LayoutNode, rect: Rect, out: &mut Vec<(Rect, &Pane)>) {
    match node {
        LayoutNode::Split { dir, ratio, left, right } => {
            let (left_rect, right_rect) = match dir {
                SplitDirection::Horizontal => {
                    let split_x = rect.x + rect.w * ratio;
                    (Rect::new(rect.x, rect.y, split_x - rect.x, rect.h),
                     Rect::new(split_x, rect.y, rect.x + rect.w - split_x, rect.h))
                }
                SplitDirection::Vertical => {
                    let split_y = rect.y + rect.h * ratio;
                    (Rect::new(rect.x, rect.y, rect.w, split_y - rect.y),
                     Rect::new(rect.x, split_y, rect.w, rect.y + rect.h - split_y))
                }
            };
            self.collect_leaf_rects(left, left_rect, out);
            self.collect_leaf_rects(right, right_rect, out);
        }
        LayoutNode::Leaf { pane_id } => {
            if let Some(pane) = self.panes.iter().find(|p| p.id == *pane_id) {
                if pane.disposition == Disposition::Embedded {
                    out.push((rect, pane));
                }
            }
        }
        LayoutNode::Empty => {}
    }
}
```
</action>

---

## Wave 2: Chrome — Tab Bar, Sidebars, Status Bar

### Plan 02: Window Chrome

**Objective:** Render the tab bar, status bar, left sidebar, and right sidebar around the pane content area. All chrome is drawn by the host compositor (not panes).

**Requirements:** COMP-05, COMP-06, COMP-07, COMP-11, COMP-12

**Tasks:**

#### Task 2.1: Tab bar data + rendering
<action>
Add tab management to `heca-core`:

```rust
pub struct Tab {
    pub id: u64,
    pub name: String,
    pub tree: PaneTree,
}

pub struct Workspace {
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
}
```

Update `heca/src/main.rs`:
- Before rendering panes, compute chrome layout:
  - Tab bar: 32px top strip across full width
  - Status bar: 24px bottom strip across full width
  - Left sidebar: 200px (collapsible to 32px) on the left
  - Right sidebar: 200px (collapsible to 32px) on the right
  - Remaining center: pane content area
- Draw tab bar: background from theme, tab labels as text, active tab highlighted with accent color
- Draw status bar: background, "N panes | Title | MODE" text
- Draw sidebars: background, "Sessions" header + empty tree placeholder in left, placeholder in right
- All chrome elements use `primitive_renderer.draw_rect` + `text_renderer.queue_text`
</action>

#### Task 2.2: Collapsible sidebar toggle
<action>
Implement sidebar state:

```rust
pub struct SidebarState {
    pub left_visible: bool,
    pub left_width: f32,
    pub right_visible: bool,
    pub right_width: f32,
}
```

When collapsed, sidebar width is 32px (icon strip). When expanded, 200px.
- Draw a small toggle icon/area at the edge of each sidebar
- Layout engine adjusts pane content area based on sidebar states

In the render loop:
```rust
let chrome = ChromeConfig {
    tab_bar_height: 32.0,
    status_bar_height: 24.0,
    left_sidebar_width: if sidebar.left_visible { 200.0 } else { 32.0 },
    right_sidebar_width: if sidebar.right_visible { 200.0 } else { 32.0 },
};
let pane_area = chrome.compute_content_rect(window_width, window_height);
```
</action>

#### Task 2.3: Pane borders with active/inactive styling
<action>
When rendering each embedded pane rect:
- Draw a filled rect for the pane background
- Draw a border around it
  - Active pane (the one with keyboard focus): use `theme.accent` color, `theme.border_width`
  - Inactive panes: use `theme.border` color, lighter opacity (e.g., 0.5 alpha)
- Draw the pane title at top-left of the pane rect + 4px padding
- Track `focused_pane: Option<u64>` in AppState, set on pane click or keyboard navigation
</action>

---

## Wave 3: Input Router

### Plan 03: Keyboard Input Router

**Objective:** Capture keyboard events, detect prefix key (Ctrl+B by default), route WM commands vs pane input. Implement HJKL navigation and split creation.

**Requirements:** INP-01, INP-02, INP-03, INP-04, INP-05, CONF-02

**Tasks:**

#### Task 3.1: Prefix key detection + mode
<action>
Add an input state machine to `heca-core`:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputMode {
    Normal,       // all keys go to focused pane
    Prefix,       // prefix was pressed, next key is a WM command
}

pub struct InputRouter {
    pub mode: InputMode,
    pub prefix_key: KeyChord,
    pub bindings: HashMap<KeyChord, WmAction>,
}

pub enum WmAction {
    FocusLeft, FocusRight, FocusUp, FocusDown,
    SplitHorizontal, SplitVertical,
    ResizeLeft, ResizeRight, ResizeUp, ResizeDown,
    Float, Scratchpad, Hide, Show,
    TabNext, TabPrev, TabClose,
    SidebarLeft, SidebarRight,
    ClosePane,
    CommandPalette,
}
```

In `heca/src/main.rs`, handle `WindowEvent::KeyboardInput { .. }`:
1. If mode = Normal and key = prefix (Ctrl+B): switch to Prefix mode, consume event
2. If mode = Prefix: look up key in bindings, execute action, switch to Normal
   - If key = prefix again: send Ctrl+B to focused pane
   - If key not bound: cancel prefix mode silently
3. If mode = Normal and key ≠ prefix: forward event data to the focused pane
</action>

#### Task 3.2: Keyboard navigation (HJKL) between panes
<action>
Implement focus navigation through the BSP tree:

- `focus_next(pane_id, dir)` — given the current pane ID and a direction (Left/Right/Up/Down), find the nearest pane in that direction
- Traverse the tree to find the parent split that matches the direction, then find the neighboring leaf
- Return the new pane ID

```rust
impl PaneTree {
    pub fn find_neighbor(&self, pane_id: u64, dir: SplitDirection) -> Option<u64> {
        // Walk up the tree to find a Split with the matching direction
        // Return the pane_id of the nearest leaf on the other side
    }
}
```

Implement in `InputRouter`:
- `WmAction::FocusLeft` → left neighbor (or horizontal split neighbor on left side)
- `WmAction::FocusRight` → right neighbor
- `WmAction::FocusUp` → up neighbor (or vertical split neighbor on top)
- `WmAction::FocusDown` → down neighbor

Default bindings after Ctrl+B:
- `h` → FocusLeft
- `j` → FocusDown
- `k` → FocusUp
- `l` → FocusRight
- `-` → SplitHorizontal
- `|` / `v` → SplitVertical
- `f` → Float
- `s` → Scratchpad
- `z` → Hide
- `[` → TabPrev
- `]` → TabNext
- `x` → ClosePane
- `Space` → SidebarLeft
</action>

#### Task 3.3: Split creation and pane operations via keyboard
<action>
Implement WM actions:

- `SplitHorizontal(pane_id)` — call `PaneTree::split(pane_id, Vertical)` to split current pane into top/bottom
- `SplitVertical(pane_id)` — call `PaneTree::split(pane_id, Horizontal)` to split current pane into left/right
- `FocusLeft/Right/Up/Down(pane_id)` — navigate to neighbor
- `ResizeLeft/Right/Up/Down` — adjust the ratio in the parent Split by ±0.05
- `FloatToggle(pane_id)` — call `toggle_float`
- `ScratchpadToggle(pane_id)` — call `toggle_scratchpad`
- `Hide(pane_id)` — call `hide`
- `Show(pane_id)` — call `show`
- `TabPrev/TabNext` — change active_tab index
- `SidebarLeft/SidebarRight` — toggle sidebar visibility
- `ClosePane(pane_id)` — call `remove` and focus the next available pane

Store the focused pane ID in AppState. After any operation that removes/focuses a pane, update focused_pane.
</action>

#### Task 3.4: Config keybindings
<action>
Extend `heca-config/src/theme.rs` (or create `heca-config/src/keybindings.rs`):

```rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Keybindings {
    pub prefix: String, // "Ctrl+B"
    pub bindings: Vec<KeyBinding>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: String,
    pub action: String,
}
```

Load from `config.toml`:
```toml
[keybindings]
prefix = "Ctrl+B"

[keybindings.bindings]
"h" = "focus.left"
"j" = "focus.down"
"v" = "split.vertical"
```

Merge with defaults. Add `pi-gsd-tools` won't work here — just hardcode defaults with optional override from config.
</action>

---

## Wave 4: Mouse + Layout Operations

### Plan 04: Mouse Input + Pane Operations

**Objective:** Mouse click to focus, drag borders to resize, drag floating panes by title bar, scroll forwarding. Implement pane move/swap and predefined layouts.

**Requirements:** INP-06, INP-07, INP-08, INP-09, INP-10, LAY-04, LAY-05, LAY-07, LAY-08, LAY-09, LAY-10

**Tasks:**

#### Task 4.1: Mouse hit-testing and click handling
<action>
In the main event loop, handle `WindowEvent::CursorMoved` and `WindowEvent::MouseInput`:

1. Compute current frame layout (pane rects, chrome rects)
2. **CursorMoved**: check cursor position against all rects
   - Over pane border area (within 4px of edge): set cursor to `RowResize`/`ColResize`
   - Over floating pane title bar: set cursor to `Move`
   - Over pane content: set cursor to `Default`
3. **MouseInput(Button::Left, Pressed)**: hit-test click position
   - Over pane content area → focus that pane
   - Over pane border edge → begin drag-resize
   - Over floating pane title → begin drag-move
   - Over sidebar toggle → collapse/expand
   - Over tab bar → switch tab
4. **MouseInput(Button::Left, Released)**: end any drag operation
5. **MouseWheel**: send scroll delta to focused pane

Use `winit::window::Window::set_cursor_icon()` to change cursor shapes.
</action>

#### Task 4.2: Border drag to resize splits
<action>
When a border drag starts:
1. Find which two panes the border separates (the two children of a Split node)
2. While dragging, update the Split's ratio based on mouse position
3. On release, finalize ratio (clamp to 0.15–0.85 minimum/maximum)

```rust
enum DragState {
    None,
    Resizing { pane_id: u64, start_ratio: f32, dir: SplitDirection },
    MovingFloat { pane_id: u64, start_mouse: (f32, f32), start_rect: Rect },
}
```

Store `drag_state: DragState` in AppState. On CursorMoved during drag, compute new ratio/position and set `needs_redraw`.
</action>

#### Task 4.3: Predefined layout templates (TOML)
<action>
Add layout template support:

```rust
/// A serializable layout template
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LayoutTemplate {
    Split { dir: String, ratio: f32, children: (Box<LayoutTemplate>, Box<LayoutTemplate>) },
    Pane { title: String, app: String },
    Empty,
}
```

Load from `config_dir() / "layouts" / "*.toml"`:
```toml
name = "dev-session"
root = { dir = "horizontal", ratio = 0.6, children = [
    { title = "editor", app = "nvim" },
    { dir = "vertical", ratio = 0.5, children = [
        { title = "terminal", app = "terminal" },
        { title = "files", app = "terminal" }
    ]}
]}
```

`PaneTree::apply_template(template)` creates the tree structure and placeholder panes.
</action>

#### Task 4.4: Move and swap panes
<action>
Implement:
- `move_pane(pane_id, target_pane_id)` — remove leaf from source, add as leaf near target (split target's pane)
- `swap_panes(a_id, b_id)` — exchange pane_ids in two leaves

Keyboard bindings:
- After Ctrl+B: `m` + `h/j/k/l` → move pane in that direction (swap with neighbor)
- After Ctrl+B: `Shift+M` + `h/j/k/l` → swap panes
</action>

---

## Verification

After all waves, verify:

1. `cargo test` passes
2. `cargo run -p heca` opens a window with tab bar, status bar, left/right sidebars, and pane content area
3. Ctrl+B → `v` creates a vertical split visible in the window
4. Ctrl+B → `h/j/k/l` navigates between panes
5. Ctrl+B → `h`/`l` with border drag resizes splits
6. Ctrl+B → `f` toggles a pane between embedded and floating
7. Clicking a pane focuses it (border changes to accent color)
8. Dragging a split border with the mouse resizes it
9. Dragging a floating pane title bar moves it
10. Tabs exist and Ctrl+B → `[`/`]` switches between them
11. Sidebars collapse/expand with Ctrl+B → `Space`
12. Config keybindings in `config.toml` override defaults
