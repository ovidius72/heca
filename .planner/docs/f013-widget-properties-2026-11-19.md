# heca-grid-ui — which widget properties can change on a live widget (2026-11-19)

Read from the struct of each of the 56 widgets in heca-grid-ui/src/widgets. "Live" = a Signal: can be
changed after the widget is built and the widget follows. "Fixed" = set by the builder or the
constructor; after that nothing can change it except rebuilding the widget.

## A. Base — every widget has these

| Field | Today | Notes |
|---|---|---|
| visible, disabled, focused, focus_visible | live (Signal) | |
| style.layout (width, height, min/max, padding, margin, gap, direction, justify, align, grow, shrink, flex, grid_cell, order, hidden…) | plain public field | changeable by hand, but nothing follows it (hidden has `set_hidden`) |
| style.visual (background, border, radius, glow, font_size, font_scale…) | plain public field | same |
| children | plain public field | `.child(signal.map(..))` is live; anything else must call `mark_needs_layout` by hand |

So Flex, Surface, Card, Tile, Panel, KeyHint, Tooltip and TreeLayout have NO properties of their own:
everything they are (direction, gap, padding, background, border…) is in Base.style — plain fields.

## B. Widget properties fixed at construction (to make live)

| Widget | Live today | Fixed today |
|---|---|---|
| Alert | title | body, variant |
| Badge / BadgeButton | label | variant |
| Button | — | **label** (constructor), variant, glyph, active, dangerous, show_glow, show_border |
| ButtonGroup | — | variant, display, entries (the buttons) |
| CardGrid | row_cursors, reveal | rows |
| Checkbox | checked | label, label_side |
| Choice | selected | value |
| ChromeRegion | mode | orientation, expanded_px, rail_px |
| CommandPalette | query, open, title… | modes, placeholder, panel_size, describe_scopes, named_prefill |
| ContextMenu | open, anchor | menu, centered (name is its id) |
| Dialog | — | dismissible, danger |
| DockFrame | expanded, active, nav, rail_mode… | frameless, areas |
| FocusScope | — | radius, color |
| Grid | — | **columns, rows, areas** (the template) |
| Icon | glyph | size, color, secondary, glow |
| IconButton | — | glyph/name, cell, active, show_glow |
| Image | source, fit, color… | root, alt |
| Input | text | placeholder, placeholder_on_focus, multiline, max_lines |
| Item | label, label_bold, active | marker, muted, leading_border, trailing_border |
| KeyHintGroup | open | letters |
| Label | text, bold, italic, underline, strikethrough, wrap, max_lines, marks | align, color, truncate, preserve_whitespace, mark_color, muted |
| LandingSlot | — | label, filled, edge, reach, line |
| Line | — | start_x, start_y, end_x, end_y, arrows |
| MarkerGroup | active, nav, hovered | marker, gutter |
| MessageList | — | visible_lines |
| NfIcon | glyph | size, color |
| Overlay | — | blocking, frosted, position, panel_size, default_focus |
| Pane | — | frame, border_width, border_color, frost |
| ProgressBar, Gauge, StatusDot, Panel(title), Tag(label) | their value/label | Tag: color |
| RailCell | active | cell |
| Row | active, nav, previous, reveal, attention | marker, highlight, previous_tint, attention_color |
| ScrollRegion | scroll offsets | axes, follow_bottom, reveal_align, overscroll, scrollbars, eased |
| Separator | — | orientation, length |
| Splitter | — | orientation, grab, line |
| Toast | title, max_lines, countdown_paused | severity, icon, show_icon, dismissible, progress, countdown_secs |
| ToastStack | items, hovered, max_lines | position, gap, margin |
| Select, Tabs, ItemGroup, ScrollBar, Visibility, Keyed, Spinner | — | nothing fixed of their own |

## C. Not properties — internal state, no change needed

Animation progress (progress, phase, shown, eased_seeded), what was last measured or applied
(seen_text, seen_label, seen_glyph, seen, applied, shown, words, wanted_with_words, overflow_width),
cursors and scroll positions kept by the widget (cursor, anchor, selected, scroll, highlight,
thumb_grab…), callbacks (on_click, on_change, on_dismiss…), caches (viewport, panel, natural_h,
handle, requested), flashes. Some of these are wrongly in "fixed" by name in the raw count (e.g.
ButtonGroup shown/words, KeyHintGroup seen, Visibility seen, ScrollRegion eased_seeded) — they are
state.

## Counts

56 widgets; 77 properties live today; about 110 widget properties fixed at construction (B), plus
all of Base.style for every widget (A); about 120 fields are internal state (C).
22 widgets have no live property of their own: Button, ButtonGroup, Card, Dialog, Flex, FocusScope,
Grid, IconButton, KeyHint, Keyed, LandingSlot, Line, MessageList, Overlay, Pane, Separator,
Spinner, Splitter, Surface, Tile, Tooltip, TreeLayout.
