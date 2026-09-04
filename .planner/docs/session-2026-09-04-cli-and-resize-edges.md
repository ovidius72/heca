# Resize edges, and heca's command line (2026-09-04)

Antonio asked for both during T497's session. **Neither is T497's work** and neither has a task of its
own — he has not decided whether they should. This is the record so a resumer is not left guessing.

## Resize edges — `resize` gained an `edge`, and lost its `axis`

**The want** (2026-09-03): *"resize_top / resize_bottom / resize_left / resize_right, or a modifier
on j/k in resize mode"* — so he can choose **which edge moves**.

**What was built:** an optional `edge` argument on the existing `resize` action, values
`auto | top | bottom | left | right`. **Not four new actions** — each would restate what `resize`
already does and then drift. One action, one code path, and a binding says which edge it wants.

⚠️ **A boundary move is a TRANSFER.** Both sides change by equal and opposite amounts and the active
thing's *far* edge stays exactly where it was — the rule `Column::resize_pane_height` already stated
for panes. My first column version resized only the neighbour, which shoves the active column
sideways unchanged; Antonio caught it: *"shift+h resize the columns before the current one not the
one i'm focused into."* The clamp is worked out **once**, from whichever side runs out first
(`ScrollingSpace::achievable_width_delta`), so the two can never disagree.

Which edge means what lives in the layout, not the handler: `Column::move_active_pane_top_boundary`
and `ScrollingSpace::move_active_column_left_boundary`. The handler only names an edge.

**`axis` was DELETED from `resize`.** It never carried information — the target decides it, a column
being resized across and a pane down — and its two invalid pairs (`column`+`y`, `pane`+`x`) silently
did nothing, which is worse than an error. Antonio: *"no one is using this app. still in development
so make the change."* Now `resize pane -40 top`, `resize column 50 left`.

**Defaults** (`keybindings.default.toml`, resize mode): `h`/`l` and `j`/`k` move the near edge,
`Shift+` the far one, and `Ctrl+hjkl` (plus `Ctrl+arrows`) **moves the focus without leaving the
mode**. That last cost **zero code** — a sticky mode already stays after any action and `focus_*`
already exist, so it is four config entries. Guards:
`a_pane_can_be_resized_from_either_of_its_edges` and
`moving_a_columns_left_edge_trades_width_with_the_column_before_it` (heca-core), plus the RPC test.

## The command line — `heca/src/app/cli.rs`

```
heca -h  --help                    heca -a  --list-actions [--json]
heca -k  --keys-show [--json]      heca -d  --describe-action <name> [--json]
```

All answer **before the window opens**, so a script can ask without a display.

**The shape, which is the point.** One `COMMANDS` table. Each entry carries the flag, its one-letter
form, a description, **the arguments it takes as data**, whether `--json` means anything to it, and
the function that runs it. Everything else is derived:

- `--help` is rendered from the table, so it cannot list a command that does nothing nor miss one
  that works — a test asserts both directions, and that every argument a command declares is
  described in the help;
- arguments are **checked before the command runs**, so a command cannot read one it never declared;
- an unknown flag is answered with the nearest real one, rather than silently opening the window;
- a leading `-` is not automatically a flag, so `heca -d -40` reads `-40` as a value.

⚠️ **The argument declaration is `ArgDescriptor` — the action system's own type — checked by the same
`check_args`.** A CLI flag and a WM action are different things, but "what arguments does this take"
is one idea, and heca had solved it once already. So a bad flag reads exactly like a bad keybinding:
`missing required argument 'name'`, `argument 'edge' expected one of auto, top, bottom, left, right`.

Short forms are **one letter**, never two: `-la` reads as the combined `-l -a` everywhere else on the
machine. A test asserts no two commands claim the same letter — a collision would make one silently
unreachable.

`heca/src/app/keys_show.rs` keeps only how its listing is built; its flag dispatch moved into the
table, so there is one place that knows what heca answers.

## Stale documentation corrected — and the lesson

`[[keys.bind]]` has carried `args` for global (non-mode) keybindings **all along**. Two comments in
`keybindings.default.toml` and a README section headed *"Planned parameterized keybindings contract"*
all said it did not exist. The default file **uses it with args on line 199**.

⚠️ **I repeated one of those comments back to Antonio as a fact about the system before checking.**
Verify a limit before quoting it — a stale doc is a confident-sounding lie.

## What has no task

Neither of the above is filed. Antonio decides what gets a task (AGENTS § 0); this doc exists so the
work is findable either way.
