# Switch Events

> Complete reference for `switch-events {}`. Since 0.1.10.

```kdl
switch-events {
    lid-close { spawn "notify-send" "lid closed"; }
    lid-open { spawn "notify-send" "lid open"; }
    tablet-mode-on { spawn "bash" "-c" "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled true"; }
    tablet-mode-off { spawn "bash" "-c" "gsettings set org.gnome.desktop.a11y.applications screen-keyboard-enabled false"; }
}
```

Only `spawn` action supported. Switch events ALWAYS execute, even when session is locked. Lid close also automatically turns off internal monitor.
