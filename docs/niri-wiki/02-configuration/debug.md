# Debug Options

> Complete reference for the `debug {}` section. NOT covered by breaking change policy.

```kdl
debug {
    preview-render "screencast"
    disable-cursor-plane
    disable-direct-scanout
    render-drm-device "/dev/dri/renderD129"
    ignore-drm-device "/dev/dri/renderD128"
    force-pipewire-invalid-modifier
    disable-resize-throttling
    disable-transactions
    keep-laptop-panel-on-when-lid-is-closed
    disable-monitor-names
    strict-new-window-focus-policy
    honor-xdg-activation-with-invalid-serial
    skip-cursor-only-updates-during-vrr
    deactivate-unfocused-windows
    wait-for-frame-completion-before-queueing
    emulate-zero-presentation-time
    force-disable-connectors-on-resume
}
```

## Debug Keybinds

```kdl
binds {
    Mod+Shift+Ctrl+T { toggle-debug-tint; }     // green tint = direct scanout
    Mod+Shift+Ctrl+O { debug-toggle-opaque-regions; }  // blue = opaque
    Mod+Shift+Ctrl+D { debug-toggle-damage; }   // red = damaged
}
```
