# Fullscreen and Maximize

> Sizing modes comparison.

## Comparison

| Mode | Command | Effect | Gaps | Borders | Window-aware |
|------|---------|--------|------|---------|-------------|
| Maximized column | Mod+F | Full output width | Yes | Yes | No |
| Maximized-to-edges | Mod+M | Screen edges (no gaps) | No | No | Yes |
| Fullscreen | Mod+Shift+F | Entire screen, black backdrop | No | No | Yes |
| Windowed fullscreen | Mod+Ctrl+Shift+F | Fake fullscreen for screencasts | N/A | N/A | Yes (app thinks it's FS) |

## Key Details

- All modes are normal participants of the scrolling layout — you can scroll away from a fullscreen window
- Fullscreen/maximized windows render a solid black backdrop behind them
- Fullscreening a floating window moves it into the scrolling layout (and back when un-fullscreened)
- Maximized-to-edges windows can tab together (since 25.11)
- Windowed fullscreen useful for browser-based presentations in screencasts
