# Workspaces

> Dynamic workspaces, workflow, addressing by index vs name.

## Dynamic Workspace System

- Each monitor has an independent vertical stack of workspaces
- Empty workspaces auto-disappear when you switch away from them
- Always one empty workspace at the bottom
- Opening a window on the empty workspace creates a fresh empty workspace below it
- Workspaces can be moved between monitors
- Workspaces remember their original monitor for reconnect; new windows reset this memory

## Addressing

- `focus-workspace 2` — refers to whatever workspace is second on the focused monitor (current position, not fixed ID)
- Named workspaces provide stable addressing: `focus-workspace "browser"`

## Example Workflow

From the developer: Browser on topmost workspace, then one workspace per project. On a single workspace: 1–2 frequently-switched windows, extra windows scrolled outside view. Move workspaces up/down as priorities change.

## Keybinds

- Mod+U/I or PageDown/PageUp — switch workspace down/up
- Mod+Ctrl+U/I — move focused column to workspace below/above
- Mod+Shift+U/I — move workspace up/down
- `move-workspace-to-monitor-*` — cross-monitor workspace movement
