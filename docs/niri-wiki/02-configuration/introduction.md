# Configuration Introduction

> Config file syntax, loading paths, live-reload, KDL format.

## Config Location

Loaded from (in order):
1. `--config` CLI flag or `$NIRI_CONFIG` env var
2. `~/.config/niri/config.kdl` or `$XDG_CONFIG_HOME/niri/config.kdl`
3. `/etc/niri/config.kdl`
4. If none found: creates `~/.config/niri/config.kdl` from embedded defaults

## Live Reload

Edit and save → changes apply immediately. Includes key bindings, output settings, window rules, everything.

## KDL Syntax

- Sections: `key { property value; }` or `key "string" { ... }`
- Comments: `//` for line, `/-` prefix to comment out entire section
- Flags: writing the key enables it, commenting it disables it
- Strings in double quotes: `"value"`
- Numbers: `16`, `0.5`, `2.0`
- Colors: `"#rgb"`, `"#rrggbb"`, `"#rrggbbaa"`, CSS named colors, CSS functional notation

## Rule: Sections Cannot Repeat

```kdl
// Valid: each section appears once
input {
    keyboard { ... }
    touchpad { ... }
}

// Invalid: input appears twice
input { keyboard { ... } }
input { touchpad { ... } }
```

Exception: output/workspace sections by name, window-rule/layer-rule blocks.

## Validation

```bash
niri validate    # Parse config and report errors
```

## Breaking Change Policy

Existing config files must parse correctly across releases. No removal of parsing support for previously valid constructs. Exceptions for bug fixes (e.g., silently accepting broken syntax).
