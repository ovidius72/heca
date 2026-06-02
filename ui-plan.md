# heca-ui: Planned UI Component Library

This document outlines the planned UI components for the new `heca-ui` crate, along with an analysis of the current theme variables and proposals for a more generic, reusable theme system.

---

## Proposed UI Components

The following components are candidates for extraction into `heca-ui`. Each component should be UI‑agnostic (no hardcoded layout logic specific to heca’s sidebar, tab bar, etc.) and should receive styling information (colors, fonts, sizes) from a theme.

### 1. Primitive Shapes
- **Rect** – filled rectangle
- **Border** – rectangle outline (stroke)
- **RoundedRect** – filled rectangle with optional border and radius
- **Line** – optional, for separators
These map directly to the functions in `heca-renderer/src/primitive.rs` (`draw_rect`, `draw_border`, `draw_rounded_rect`).

### 2. Label
- Displays a string with a given font size, font family, and color.
- Wraps the GPU text rendering logic from `TextRenderer` (via `TextLabel`/`TextCommand`).
- Should support horizontal/vertical alignment, wrapping, and ellipsis (future).

### 3. Button
- A clickable label with optional icon, background, border, and text.
- States: normal, hovered, pressed, disabled.
- Receives colors for each state from the theme.

### 4. Toggle / Checkbox
- Binary state widget (on/off) with a label.
- Could be built from primitive shapes + label.

### 5. TreeView
- Displays hierarchical data (e.g., workspaces → columns → panes).
- Supports expand/collapse, keyboard navigation (arrow keys), and selection.
- Item renderer is a closure or trait that draws the label for a given node.
- The current `SidebarTree` model can be reused as the data model; the view logic (rendering, hit‑testing, navigation) becomes generic.

### 6. TabBar
- Horizontal strip of tabs.
- Active tab highlighted with accent color; inactive tabs use border/foreground.
- Each tab has a label (and optionally an icon).
- Clicking a tab selects it.

### 7. StatusBar
- Typically located at the bottom of the window.
- Displays dynamic text (mode, pane count, etc.) split into left/right/center sections.
- Background and foreground colors from theme.

### 8. DragGhost
- A label that follows the cursor during drag‑and‑drop operations.
- Used to show what is being dragged (e.g., pane name).
- Background and foreground colors supplied by the theme.

### 9. InputField (future)
- For rename mode, command palette, etc.
- Editable text with cursor, selection, and placeholder.

### 10. Modal / Popup (future)
- Window‑like UI that overlays the content (e.g., settings, confirmation).

---

## Theme Variable Analysis

The current `heca-config::theme::Theme` struct contains many specific values that are tightly coupled to the existing UI (sidebar, drag‑and‑drop, floats). To make a generic UI library, we should identify which variables are truly generic (fonts, base colors, sizing) and which are component‑specific and should be moved into component configuration or derived from a base scale.

### Current Theme Fields (from `heca-config/src/theme.rs`)

| Field | Type | Current Use | Suggested Treatment |
|-------|------|-------------|---------------------|
| `name` | String | Theme identifier (e.g., "Catppuccin Mocha") | Keep – generic metadata. |
| `background` | Color | Main window background | **Generic** – base background color. |
| `foreground` | Color | Default text color | **Generic** – base foreground color. |
| `border` | Color | Pane borders, tab/border elements | **Generic** – base border color. |
| `accent` | Color | Active tab, sidebar active item, resize highlights | **Generic** – accent color for interactive/highlighted elements. |
| `font_family` | String | Font used for all text (tabs, status, sidebar, etc.) | **Generic** – base font family. |
| `font_size` | f32 | Base pixel size for text (used for tab text, status text, etc.) | **Generic** – base font size; UI components can scale this (e.g., label = base_size * 0.8). |
| `border_radius` | f32 | Rounded corners for pane borders, popups, etc. | **Generic** – base radius; components can use a fraction. |
| `border_width` | f32 | Width of pane borders, tab separators, etc. | **Generic** – base width. |
| `shadow` | Struct (color, alpha, blur) | Drop‑shadow for floating panes | **Generic** – base shadow parameters. |
| `float_background` | Color | Background of floating panes | **Component‑specific** (floating pane UI). Could be derived from `background` with an alpha overlay, or kept as a float‑pane override. |
| `float_accent` | Color | Accent inside floating panes (e.g., title bar) | **Component‑specific**. |
| `float_focus` | Color | Focused state inside floating panes | **Component‑specific**. |
| `sidebar_drag_ghost_bg` | Color | Background of drag ghost label in sidebar | **Component‑specific** (sidebar drag). |
| `sidebar_drag_ghost_fg` | Color | Foreground of drag ghost label | **Component‑specific**. |
| `sidebar_drag_source_bg` | Color | Background of dragged item in sidebar tree | **Component‑specific**. |
| `sidebar_drag_source_border` | Color | Border of dragged item source | **Component‑specific**. |
| `sidebar_label_font_size` | f32 | Font size for sidebar tree labels | **Derivable** – should be a scale of base `font_size` (e.g., 0.44 * base_size). |
| `sidebar_button_font_size` | f32 | Font size for sidebar buttons (+w, +c, +p) | **Derivable** – scale of base `font_size`. |

### Proposed Generic Theme Structure

We propose a new `heca-ui::theme::Theme` (or we can adapt the existing one) that contains only the truly generic, foundation‑level values. Component‑specific tweaks can be provided via:

1. **Component configuration structs** (e.g., `ButtonStyle`, `TreeViewStyle`) that accept colors, fonts, sizes, and radii—either derived from the base theme or overridden.
2. **Semantic color roles** (e.g., `text_primary`, `text_secondary`, `background`, `surface`, `primary`, `secondary`, `border`, `hover`, `focus`, `disabled`) that map to base colors with optional adjustments.
3. **Typography scale** – a set of font sizes relative to the base (caption, body, heading, label, button, etc.).
4. **Shape scale** – border radius and width scales.

#### Example Generic Theme Fields

```rust
pub struct Theme {
    pub name: String,
    pub background: Color,      // -base
    pub foreground: Color,      // text on background
    pub surface: Color,         // elevated surfaces (menus, popups, floating panes)
    pub primary: Color,         // accent for interactive elements
    pub border: Color,          // separator / outline color
    pub font_family: String,
    pub font_size: f32,         // base size (e.g., 16 dp)
    pub font_scale: FontScale,  // e.g., { caption: 0.75, body: 1.0, label: 0.875, button: 0.875, heading: 1.5 }
    pub radius: f32,            // base radius
    pub radius_scale: RadiusScale, // e.g., { none: 0.0, sm: 0.5, md: 1.0, lg: 1.5, full: 9999.0 }
    pub border_width: f32,
    pub border_width_scale: BorderWidthScale,
    pub shadow: Shadow,         // base shadow
}
```

Component styles would then be built from the theme:

```rust
pub struct ButtonStyle {
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub border_width: f32,
    pub radius: f32,
    pub font_size: f32,
    // ... etc.
}

impl ButtonStyle {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            bg: theme.surface, // or theme.background depending on elevation
            fg: theme.foreground,
            border: theme.primary,
            border_width: theme.border_width * theme.border_width_scale.md,
            radius: theme.radius * theme.radius_scale.md,
            font_size: theme.font_size * theme.font_scale.button,
        }
    }
}
```

### Actions for the `heca-ui` Crate

1. **Create the crate**: `cargo new heca-ui` inside the worktree (or we can add it as a workspace member later).
2. **Define the generic `Theme`** (or import and adapt the existing one after stripping component‑specific fields).
3. **Implement primitive shapes** (`Rect`, `Border`, `RoundedRect`) that expose a `draw` method taking a `&mut PrimitiveRenderer` (or we can re‑expose the primitive functions directly).
4. **Implement `Label`** that uses `heca_renderer::text::TextRenderer` internally (or we could copy the text rendering logic into `heca-ui` to avoid renderer dependency—prefer depending on `heca-renderer` for now).
5. **Implement `Button`**, `TreeView`, `TabBar`, `StatusBar`, `DragGhost` as reusable widgets that accept a theme (or style struct) and a renderer context.
6. **Provide example usage** in a README or test.

### Next Steps

- Verify that the current `heca-renderer` and `heca-core` crates are usable as dependencies from `heca-ui`.
- Begin implementing the primitive shape wrappers.
- Define the trait/generic interfaces for rendering (e.g., a `RenderContext` that provides access to `PrimitiveRenderer` and `TextRenderer`).

---

## References

- Existing UI code in `heca/src/sidebar.rs`, `heca/src/main.rs` (tab/status bar rendering), `heca-renderer/src/primitive.rs`, `heca-renderer/src/text.rs`.
- Current theme definition in `heca-config/src/theme.rs`.
