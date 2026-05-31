# Named Workspaces

> Complete reference for workspace declarations. Since 0.1.6.

```kdl
workspace "browser"
workspace "chat" { open-on-output "HDMI-A-1"; }
```

Named workspaces are persistent (don't auto-delete when empty). Support per-workspace layout overrides since 25.11:

```kdl
workspace "aesthetic" {
    // Overrides apply when this workspace is active
    layout {
        gaps 32
        struts { left 64; right 64; bottom 64; top 64; }
        border { on; width 4; }
    }
}
```

Unset a flag with `false`:
```kdl
layout { always-center-single-column; }
workspace "uncentered" { layout { always-center-single-column false; } }
```

## Dynamic Name Changes

Since 25.01. `set-workspace-name "name"` / `unset-workspace-name` actions.

## Removing Named Workspaces

Delete from config → workspace becomes unnamed → if empty, auto-removed.
