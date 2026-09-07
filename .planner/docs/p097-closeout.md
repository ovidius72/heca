# P097 closeout — what a resumer needs that the task records do not carry

P097 (F003, "One Tree — the input architecture") is **complete**: T494–T497 and T499–T502 done,
**T498 canceled** (duplicated `P094(F011)/T449`). The planner refuses a handoff on a finished phase,
so this is where the operational half lives. The *what and why* is in the task records —
`planner-task-show P097(F003)/T501 full=true` and `.../T502 full=true`. Read those first; this
file is only the things they do not hold.

## If you have never seen this project

1. **Read `AGENTS.md` in full before writing anything.** It is the entry point: the pre-flight
   questions, the widget-vs-component rule, the DOM-shaped event model, and RULE ZERO. Then
   `docs/widgets.md` for the library and `docs/surface-compositor.md` for layers and surfaces.
2. **The planner is the only source of truth for what to do next.** `BACKLOG.md` and any root
   `*-plan.md` are stale. Load it with `/planner load`, then `planner-task-recommend`. Never create
   a feature, phase or task without asking Antonio first — propose it.
3. **Build and run:** `cargo run -p heca`. The widget gallery is
   `cargo run -p heca-renderer --example showcase` and is the fastest way to see the library.
4. **Antonio drives every visual check.** Green tests are not verification for anything that lays
   out, paints, scrolls or routes input. Hand him the exact gesture to try.

## ▶ NEXT SESSION STARTS HERE — showing a surface is not yet mounting it

**The agreed API is built. The last piece is not.** Read this section, then the two below it.

### What a developer writes now (built, green, uncommitted)

```rust
let confirm = Dialog::new("Close pane?").body(..).action(..).handle();
Button::new("Delete").on_click(move || confirm.show())     // call it, from any closure

Dialog::new("..").open_when(is_editing)                    // follow YOUR signal
Dialog::new("..").default_open(false)                      // starting value only
Dialog::new("..").key("confirm.close")                     // show_layer confirm.close
```

Three ideas, one name each: **`default_open(bool)`** the starting value (the only form a
description can carry), **`open_when(Signal<bool>)`** your state followed for life, and
**`show()` / `close()` / `toggle()`** the verbs, on a copyable `SurfaceHandle` so they work inside
a closure — `&mut self` methods cannot.

`opened`, `open(bool)`, `open()` and `hide()` are **gone everywhere**, including the plugin-facing
SDK and the described prop name. `open()` meant "start open" on one type and "show it now" on
another; that is what put a test dialog on screen at boot.

### What is left, and it is the point of the whole thing

**`build_modal_root` (`heca/src/chrome/overlay.rs`) still hands the host a boxed dialog.** So a
developer who wants a dialog still assembles a `ModalSpec` — a title, a body converted to node
form, a list of actions, flags — hands it over, and writes a callback that works out which button
came back and digs the typed value out of a map. Every dialog in the app repeats that.

**Make showing mean mounting.** `show()` puts the surface in the layer stack; `close()` takes it
out. Then nobody hands specs to the host, `ModalSpec` stops being a developer's concern, and the
three doors above are the whole API.

Measured before starting: **three places construct a `Dialog`** — `build_modal_root`
(`heca/src/chrome/overlay.rs:495`), a layers test, and the showcase demo. Only the first is
production code that hands the widget onward, so the builder chain can return the handle.

⚠️ It touches the host's modal path, the layer registry and the result callback. Antonio must drive
it; nothing here can be proven by tests alone.

### The rules this API came from — do not re-litigate them

- **DOM-like.** A developer has a surface and shows it. Layers, stacks, mounting, `realize`,
  emitters and `ModalSpec` are the framework's business, exactly as paint order is not something
  you think about when you write `<dialog>`.
- **A plugin gets all three doors on the same terms** — its own signal, its own handle, its own
  name. Not a reduced version.
- **Mechanism on the generic surface, policy on the specific one.** `Overlay` carries out *how*
  (move focus, contain Tab, answer the dismiss key); `Dialog` decides *when* — its panel recipe,
  `dismissible`, which action is primary.
- **`key` is optional.** Anything that points at a control takes its declared key **or the words it
  reads by** — one lookup, `FocusManager::focus_named`.

### Also still open

`heca/src/chrome/described_confirm.rs` **should be deleted.** It was T502's acceptance fixture — a
confirm built the way a plugin must, to find out whether the path worked. It found what it was
built to find and its purpose is spent. It is a test artifact at a real surface's address, and it
registers a layer at boot for no reason.

Its guards for **library** behaviour have already been copied to where they belong
(`heca-grid-ui/src/widgets/button.rs` and `overlay/tests.rs`), so they are currently **duplicated**.
Deleting the file removes the duplicates; nothing else in it is worth keeping. Remove the module
line in `heca/src/chrome/mod.rs` and the `register` call in `heca/src/app/startup.rs` with it.

## Open, and the first one is a live bug

Three defects from Antonio's drive of the T502 surface, none fixed:

1. **`view intent 'close_pane' did not resolve to a known action`** — the described confirm's Delete
   button fires an action name that does not exist, so **Delete does nothing**. It needs the real
   by-id action; the target pane is not necessarily the focused one.
   `heca/src/chrome/described_confirm.rs`, in `register`.
2. **`a Button at 0/2/0 … declares no key, and it is one of 2 siblings of that kind`** — the
   identity reporter wants keys on the two sibling buttons, and it is right: two buttons of one kind
   side by side is a **collection**, which is the one case a `key` is for.
3. **`key 'y' … CopySelection overwritten by ShowLayer`** — the binding suggested for testing
   (`prefix+y`) clobbers copy-selection. Pick another; nothing in the code depends on it.

## How to see the T502 surface yourself

The described confirm is registered **hidden** as `heca.confirm` and opened by the ordinary
`show_layer` action — nothing about it is special-cased. Add this to `~/.config/heca/config.toml`
(pick a free key; `y` is taken by CopySelection — see defect 3):

```toml
[[keys.bind]]
keys = "prefix+u"
action = "show_layer"
args = { name = "heca.confirm" }
```

`[[keys.bind]]` is the parameterised form; the flat `action = "keys"` table has nowhere to put
`args`.

## Operational

- **Everything is COMMITTED and the tree is clean.** The surface API — the rename, the copyable
  handle,  — is . Green at that commit: 1492 tests, clippy at the one
  pre-existing warning, no file's rustfmt count raised.
- **`/Users/antonio/projects/heca`**, branch **`feat/hint-collapses-to-the-tree`**, cut from `main`
  at `069e3b3`, **not pushed**. Five commits: `4cb7e24` (T499), `1b8a5b1` (T500 fixes), `d013f5c`
  (T501 part), `89a3b3e` (T501 complete), `673356d` (T502).
- **Gates:** `./scripts/lint-changed.sh test`, then `./scripts/lint-changed.sh`. Never
  `--workspace`. Baseline **1483 tests**, clippy exit 0 with exactly **one** warning (pre-existing
  `toast/stack.rs` "very complex type") plus a third-party `block v0.1.6` note.
- ⭐ **The test gate now runs doctests.** It passed `--all-targets`, which **excludes** them, so no
  `///` example in the workspace had ever been run — found when a broken one passed the gate four
  times. It runs both now and takes the worse exit code; a broken example makes it exit 101.
- ⭐ **The rustfmt recipe in every earlier handoff was WRONG.** It said `--edition 2021`; the
  workspace is **2024**, so it errored and counted **zero blocks on both sides** — "no file's count
  raised" was never actually measured. Correct form, per file:
  `git show HEAD:<f> | rustfmt --edition 2024 --check | grep -c "^Diff in"` versus the working copy.
  Count violation **blocks**, not lines. Never `cargo fmt --all`, never raise a count.
- ⚠️ **Do not pipe a gate through `tail`** — the pipeline's exit code is `tail`'s.
- ⚠️ Flaky, not ours: `heca-core`'s
  `terminal_backend_command_spawn_runs_and_stays_open_for_policy` fails intermittently in a full
  run, passes alone.
- **A handoff cannot be written on a done phase**, and completing its last task clears the one it
  had. Write the closeout before completing the final task, or into a file like this one.

## Rejected — do not retry

**Collapsing only Tab onto `Overlay`** while leaving the other three focus operations on `Dialog`.
Tried; it broke `input_edit_reaches_focused_field_nav_moves_to_button`. Half a centralisation moves
the disagreement rather than removing it — all four (open-quiet, Tab, arrow motion, click-in-panel)
go together or none do.

## Pending verification

The `Dialog` focus centralisation — the `prefix+x` first-Tab fix — was verified headlessly and
through the *described* surface, never by driving a real dialog. Worth one pass over `prefix+x` with
Tab, Shift+Tab, arrows, and a rename dialog's text field.

## What bit hardest, and cost the most

- **Fixing at the call site when the fault is in the library.** Three times in one session. The tell:
  reaching for the numbers, the key or the flag at the place that consumes them.
- **A guard proves nothing until it is run against its own bug**, and **two guards here were
  vacuous** as first written: a Tab test passed with the traversal deleted, because the activate key
  reached the far button anyway. **When a sabotage does not turn a guard red, suspect the sabotage
  first** — one of mine did not compile and read as a pass.
- ⚠️ **Never `git checkout` a modified file to undo a test sabotage.** It discards every uncommitted
  change in that file, and did. Restore from a copy.
- ⚠️ **A scripted write can destroy a file.** Write to a temp file and move it; check the tail
  afterwards. A `python` script that asserts *after* mutating and then never writes will silently
  drop half an edit — that happened twice, leaving the showcase's two halves out of step.

## Runtime limitations

No agent can see the UI. Instrument — `#[cfg(debug_assertions)] eprintln!` with one greppable
prefix, hand Antonio the exact gesture, strip it after. An integration test does **not** enable
`cfg(test)` in the library, so such a probe silently never compiles. `AppState` needs a window, so
there is **no headless call** to any handler taking `&mut AppState` — `heca/tests/by_id_actions.rs`
is written as a source lint for exactly that reason.

## Files and symbols this phase left behind

`heca/src/chrome/described_confirm.rs` is the T502 surface — a confirm described end to end, and the
worked example of what a plugin writes. The rest, by crate:
`heca-grid-ui/src/{component,focus}.rs`; `heca-grid-ui/src/widgets/{button,dialog,overlay/mod}.rs`;
`heca-view/src/{lib,build}.rs`; `heca-view-realize/src/lib.rs`;
`heca/src/chrome/{overlay,mod}.rs`; `heca/src/chrome/layers/{mod,tests}.rs`.

Names worth knowing before you re-invent one:

- `DIALOG_PAD` / `DIALOG_GAP` / `DIALOG_BTN_GAP` — the dialog panel recipe, theme steps, public so a
  composed surface matches instead of guessing.
- `FocusManager::focus_named` — "put the keyboard on the control called X", by declared `key` **or**
  the words it reads by. The one place that rule lives.
- `Component::{advance_focus, focus_first_quiet, focus_at_trapped, set_default_focus}` — the
  surface's keyboard, implemented by `Overlay`, delegated to by `Dialog`.
- `Overlay::{on_dismiss, default_focus}`, `Dialog::default_action`.
- `open_view_layer` / `LayerRegistry::add_view` — the described-layer path, which now takes a name.

## Guards — do not "fix" these by relaxing them

Several tests here are deliberately strict and were each run red against their own bug:

- `an_overlay_with_no_dismissal_declared_does_not_swallow_the_key` — proves the dismissal fix did
  **not** take Escape away from `Dialog` / `ContextMenu` / `CommandPalette`.
- `the_first_tab_in_a_dialog_moves_off_the_default_button` — the `prefix+x` bug.
- `every_library_widget_is_describable_or_deliberately_not` — every library widget must have a kind,
  be a declaration, or carry a recorded reason. It fails when a widget is added with none.
- `every_widget_property_is_reachable_from_the_sdk` — also fails when a *kind* exists that no
  `check(..)` line covers, which is how two new kinds were caught unprompted.
- `every_universal_capability_is_reachable_from_the_sdk` — capabilities on `ComponentExt` are
  invisible to the per-kind check, which is how `tooltip` was unsayable for months.

## Where the work goes next

P097 is finished, so nothing here is the next task. The planner decides, but for orientation:

- **`P082(F003)`** — five open bugs (`T474`, `T475`, `T477`, `T478`, `T491`), plus the
  oversized-file splits `T466`/`T468`/`T470`/`T471`/`T472` and `T509`–`T514`, which were
  **deliberately deferred behind P097** because they carve up the same files. They are now unblocked.
- **`P094(F011)/T449`** — Terminal as a component; `T498` was canceled into it.
- **`P096(F003)/T504`, `T505`**; `P035(F003)/T506`; F009's `T381` and `P063`.
- The `app.actions.*` / `app.overlay.*` host-API namespaces that `docs/chrome-and-ui.md` §3.5 calls
  future phases — that is what T501's C10 decision points at, and what a plugin needs to open a
  dialog at all.

## How Antonio works

- **He drives every visual check and decides every commit. Never commit or push unasked.**
- **He decides what is filed in the planner.** Propose; do not create while passing through.
- **Short answers, plain words, no jargon.** Long replies are a failure.
- **"Fix all" means close the gap, not report it as still open.**
- **Do not tell him about an agent's own limits** — pacing the work is his call.
- **He asks "did you fix centralized?" and "have you written all the detail for another agent?"** —
  run both checks before he asks, not after.
- **A peer session may be working in the same folder.** Worth asking (one supplied real facts here),
  but verify what it says — one of its three claims was wrong. Identify it by its **folder**, never
  its name.

## Related records

- `.planner/docs/t497-open-issues.md` — this phase's defects, real causes, and disproved diagnoses.
- `.planner/docs/t497-header-buttons.md` — the pane header's action row and the API it added.
- `.planner/docs/p097-input-seams.md` — drag, pointer routing and the hint walk.
- `.planner/docs/session-2026-09-04-cli-and-resize-edges.md` — the resize edge argument and the CLI.
- `.planner/docs/oversized-files-2026-09-04.md` — the 65 files over 600 lines and who owns each.
- `.planner/docs/t468-oversized-files.md` — the split order and its traps.
