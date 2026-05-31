# Tabs

> Column display mode for windows. Since 25.02.

## Behavior

Switch a column between normal (vertical stack) and tabbed (side indicator):

```
Mod+W → toggle-column-tabbed-display
```

- All tabs in a column share the same window size
- Gives more vertical space compared to stacking
- Tabbed columns CAN go fullscreen with multiple windows
- All operations work the same: focus, consume, expel, close

## Tab Indicator

Side indicator (default: right of column) shows tabs. Configurable:

```kdl
layout {
    tab-indicator {
        width 4
        position "right"   // left, right, top, bottom
        length total-proportion=0.5
        place-within-column   // inside column (affects sizing)
    }
}
```

## Default Column Display

```kdl
layout {
    default-column-display "tabbed"  // all new columns tabbed by default
}
```
