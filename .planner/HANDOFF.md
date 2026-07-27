# Handoff

**Created:** 2026-07-27
**Reason:** About a third of context left. Everything is committed, nothing is pushed.

## Where the work is

Branch `feat/action-arg-declaration`, **13 commits**, not pushed, no PR. Working tree clean.
**Whole workspace green across 20 suites, clippy 0 warnings** at every commit.

The branch carries **five tasks across two phases**. It was never split — the user was asked and
said to leave it as one.

## What shipped, by task

### F003/P010/T006 — an action declares the arguments it takes ✅
`dd350c5` `3963446` `331d40c`

`ActionMeta` had a label, an icon, a policy and a confirm prompt, and no list of arguments. The only
record of an argument's name was the string literal inside `build_action`'s match arm. Three
silences came out of that: a misspelled **required** argument made a binding do nothing forever
(one stderr line in debug, none in release); a misspelled **optional** one silently used the
default, in every build; and **eight actions answered their bare name with every index set to 0** —
binding `delete_workspace` to a key deleted workspace 0.

Now each action declares its arguments as data. A vocabulary argument points at
`EnumArg::VALUES` beside its own `FromStr` rather than restating the list. `check_args` compares a
call against the declaration at both doors that carry arguments — a `config.toml` binding at load,
an `Intent` at dispatch — and names what is wrong, offering the nearest declared name on a typo.
**23 argument-taking actions had no metadata at all** and now have descriptors; five more
(`move_column_up/down`, `move_column_to_workspace`, `resize_column_by`, `resize_pane_height_by`)
were reachable only from RPC or the mouse and are nameable now.

Two hand-written tables died: `builtin_policy`'s representative-variant map (it named six actions
and refused the rest, which is *why* the other 23 could never be catalogued), and six entries of the
descriptor test's UNBOUND list — an action that requires an argument cannot carry a bare keybinding,
so it is exempt by rule now.

### F003/P017/T005 — Separator in the vocabulary ✅
`67fb118` `bf8a5c9`

Two defects in the widget itself, both fixed **in the widget**, not the call site:
- `length` and `orientation` depended on each other. Properties arrive sorted by name, so `length`
  landed before `orientation` and went onto the axis the rule runs *across*. The widget recomputes
  both axes from the pair now.
- A rule in a **centring** container did not appear at all (1px × 0). The docs told the caller to
  pass a `length` to work around it. It asks to be stretched itself now.

The second was found by the user looking at the showcase, not by a test.

### F003/P017/T006 — the vocabulary's Row becomes the interactive widget ✅
`a413730`

`WidgetKind::Row` → **`HStack`**, `Column` → **`VStack`**, and `Row` now maps to `heca-grid-ui`'s
real clickable, selectable widget. Wire spelling is `"h_stack"` / `"v_stack"` — the model's own rule
(enums travel as their name in snake_case), not a hand-written exception. Press is wired to a click,
to Enter/Space **and** to a KeyHint target. With no press intent the row stays inert.

### F003/P017/T007 — appearance is an ordinary property ✅
`eb9e021` `aec4d8a`

The whole of `Visual` (fill, border, glow, radius, font size, font scale). `merge_layout_props`
became `merge_style_half<T>`, generic over the half — that was the entire realize-side change, which
is the argument for having kept the `Layout`/`Visual` split. Eight per-widget colour builders that
were `host_only("reachable once F003/P017/T7…")` are props now; the macro learned `Color`.

**First caller:** the destructive confirm prompt renders its message in the theme's `danger`
colour, written as the token name. It exists because the capability was otherwise unverifiable —
nothing in the app overrode anything, so "look at the modal" meant "confirm it is identical".
Verified by the user with `ctrl+b` then `x`.

### F003/P017/T008 — docs match the code 🔶 REOPENED, see below
`a3485a6` `5768cd3` `e5a3b56` `a5ac706`

**`Panel` is a real widget now** (`heca-grid-ui/src/widgets/panel.rs`) — the user chose that over
rewriting the examples onto `Card`. Heading, a rule under it, then body; the heading and rule hide
together when untitled. `WidgetKind::Panel` stops aliasing `Surface`.

Doc errors fixed: `Panel::new().title(..)` against a titleless widget; `Label::variant(Variant::Heading)`
(`Label` has **no** `variant` builder, ever); `Variant::Danger` on a Button (it is `Destructive`);
`Panel::new("id")` taking an argument it has not; `kind:"Panel"`/`kind:"Row"` in a serialized-form
comment; `Theme::grid_ares()` (never existed — bundled themes are grid_tron/mocha/latte via
`load_theme`); AGENTS.md's catalog missing eight widgets.

**Part II of `docs/chrome-and-ui.md` was edited, and its header no longer claims to be unedited.**
The user pushed on this: the verbatim rule protects design *reasoning*, not identifiers that no
longer compile. Prose is untouched; §21 records what was wrong.

## ⛔ TWO THINGS OPEN

### 1. F003/P017/T008 is IN-PROGRESS on purpose — one visual check
The user saw the Panel and said *"not sure the title position is what i expected but for now is
ok."* Rather than leave that, they were given four concrete options and chose **a rule under the
heading**, adding *"like in the pane header we have in the app."*

The pane header turns out to be a **marker bar plus a button cluster in a centred row** — there is
no titled band there to copy. So `a5ac706` builds the shape they picked (heading → rule → body),
not that construction, and they were told so. **Ask them to look before closing the task.** If it is
still wrong, the likely axes are the header's height, where the rule sits, or the title's weight.

### 2. F003/P017/T009 is the last item in F003/P017 — and it is a DECISION, not code
Whether the pure-data model (`ViewNode`, `WidgetKind`, `PropValue`, `Intent`) moves out of the app
into a small crate of its own. **Two independent reasons now, and the second is new:**
- a plugin author gets no editor help, because the model is inside the app crate;
- **the showcase cannot render a described tree at all.** It lives in `heca-renderer`, *below*
  `heca`, so it cannot call `realize`. That is why neither `Separator` nor `Row` could be shown to
  the user declaratively, and it is a verification gap on **every** future vocabulary task.

The settled rule forbids pushing `ViewNode` **down** into `heca-grid-ui` (inverts the graph; it
cannot carry closures/signals). A separate crate **alongside** inverts nothing and is a different
question — the task says so explicitly. `realize` itself stays app-side; it needs
`InteractionIntent`, the hint registry and theme wiring.

The user has been shown the trade and has **not** chosen. Do not start it.

## Decisions taken today, so they are not re-litigated

- **Row naming:** the KIND gave up the name (`HStack`/`VStack`), not the widget. `Column` followed
  for symmetry.
- **Appearance scope:** the whole of `Visual`, not colour alone.
- **Token resolution:** resolved **eagerly** in `realize`, which takes a `&Theme`. I first told the
  user the opposite (store unresolved, resolve at paint) and corrected it: `reload_config` already
  drops `chrome_tree` and every pane header, so a rebuild makes a token follow a theme change for
  free. Storing unresolved would have meant a source type through all 14 paint sites.
- **Panel:** a real widget with a real title, over rewriting the examples onto `Card`.
- **Part II code examples:** corrected in place; prose stays verbatim.
- **Horizontal scroll (F003/P011/T012):** `prefix+Shift+Home/End/PageUp/PageDown` — the same four
  keys as vertical, Shift for the other axis. Widens the 2026-07-17 decision, which named only
  PageUp/PageDown. That task stays **blocked** on a chrome ScrollRegion target; re-verify, since the
  2026-07-26 sidebar scroll work may already have discharged it.
- **F003/P010/T005 (register(ActionSpec) migration): CANCELED** by the user. Measuring killed it —
  148 names vs 132 handler-bearing variants, so they do not pair 1:1; the rewrite would have added
  code and needed 148 hand-paired literals. The safety it existed for is now two guard tests
  (`every_catalogued_action_has_a_handler`, `every_wm_action_variant_is_reachable_by_name`).
  **Do not re-plan it.**

## How this user works — read before starting

- **Full planner ids, always: `F003/P017/T006`.** Not `T6`, not a slug. A bare `T6` exists under
  many phases and is unsearchable. Zero-pad to three digits, as the web UI does.
- **Never decide whether a task lives or dies.** Bring the analysis and a recommendation; they
  decide. I set F003/P010/T005 back to `planned` on my own and was pulled up for it.
- **A question in progress is not permission to act.** *"wait before acting. we are discussing
  damn."* Answer, stop, wait.
- **Do not leave a task `planned` when its value has gone** — that is delay in a different costume.
  Close it (`canceled`, not `done`, when the work was not carried out) once they say so.
- **"Update the docs" means fix everything that does not match the code.** Do not report a mismatch
  and ask; fix it. *"Come on."*
- **Plain words, short sentences.** No jargon-then-gloss.
- **They verify rendering and interaction themselves.** Green tests are never verification for
  anything visual — and if a capability cannot be seen in the app, say so plainly instead of
  offering a look at something that should be identical.
- **Fixes belong in the widget, not the call site.** Twice today the right fix was in the widget.
- **`git checkout <file>` to undo an experiment wipes uncommitted work.** It cost me the Separator
  change once and a test once. Copy to the scratchpad first.
