# Config Includes

> Multi-file configuration via `include`. Since 25.11.

```kdl
// config.kdl
include "colors.kdl"
include optional=true "local-overrides.kdl"   // since 26.04
```

## Rules

- Includes work only at the top level (not inside sections)
- Relative paths: relative to current file
- Absolute paths supported
- `~` expands to home (since 26.04)
- Included files are watched for changes (live reload)
- Includes are positional — override settings set before them

## Merging

- Most sections merge (only change written properties)
- `window-rule`, `output`, `workspace` — inserted as-is (no merge)
- `binds` — override previously-defined conflicting keys
- Flags can be disabled with `false`
- Some sections NOT merged: `struts`, `preset-column-widths`, animation subsections, pointing device sections in `input`
