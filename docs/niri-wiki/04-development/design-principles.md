# Design Principles

> niri's core design philosophy as stated by the author.

## General

1. **Opening a new window should not affect the sizes of any existing windows.**
   - Scrollable tiling alternative: open temporary windows alongside existing ones without reflow.
2. **The focused window should not move around on its own.**
   - Windows opening/closing/resizing to the left of the focused window should not visually move it.
3. **Actions should apply immediately.**
   - Resizing, consuming, focus changes take effect instantly. Input goes to the final target during animations.
4. **When disabled, eye-candy features should not affect performance.**
   - Disabled animations are zero-duration, not skipped conditionally (avoids edge cases).
5. **Eye-candy features should not cause unreasonable excessive rendering.**
   - clip-to-geometry allows direct scanout when possible. Animations CAN use offscreen (they're short). Rounded corners should not.

## Invisible State

Be mindful of state not apparent from looking at the screen:
- Workspaces remember their original monitor for reconnect → new windows reset this memory
- View position preserved across temporary changes (dialogs, fullscreen toggles) → only one position remembered per workspace, forgotten on focus change

## Window Layout

1. If a window/popup is larger than the screen, align top-left (content is most important there).
2. Fixed pixel sizes affect the window itself; proportions affect the tile including borders.
3. Fullscreen windows are normal participants of the scrolling layout (unique to scrollable tiling).

## Default Config Philosophy

- Not a "rice config" — familiar, helpful, not jarring
- Thoroughly commented with wiki links
- Default CSD (prefer-no-csd commented out) — familiar for new users
- Focus ring behind windows by default (to work with arbitrary CSD shapes)
- Default binds from PaperWM experience, QWERTY layout
- General pattern: if a key switches somewhere, Ctrl+key moves there; Shift+key does alternative action
