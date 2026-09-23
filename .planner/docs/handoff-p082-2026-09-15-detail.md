# P082 — session detail, 2026-09-14/15

Extended detail for the phase handoff. The capsule is the index; this is the record.

## What landed, in order

See also `.planner/docs/t474-library-capabilities-2026-09-15.md` (the four library capabilities) and
T474's own notes.

1. **T474 C1–C8 — the authoring API, checklist COMPLETE (8/8).**
   - `a73034e` Length speaks every spelling; six sizing builders take `impl Into<Length>`.
   - `69e8e05` `IntoComponent`; all sixteen `*_boxed` twins deleted.
   - `350e0d8` `Pane::border_style`; `PaneFrame` deleted as a third copy of `FrameStyle`.
   - `dbbab0d` spacing's native half.
   - **Uncommitted from here on.** Spacing finished: `.gap`/`.padding`/`.padding_x/y`/four per-side
     paddings/`.margin`/`.margin_x/y` all take a number, a `Spacing` step or a string via
     `style::Space`, one parser shared with serde. THE WIRE MOVED: `"gap": "sm"` lands, because the
     described vocabulary IS the layout field list — the fields hold a `Space`. Retired names
     (`gap_spacing`, `pad_spacing_x/y`) still READ, mapped in `retired_name` in
     heca-view-realize; nothing writes them. The SDK has ONE `Style::accent`-style setter per
     property; `Button::tone`/`IconButton::tone` per-kind copies deleted and excused in NOT_IN_SDK.
   - ⚠️ The authored step SURVIVES layout. The old code copied resolved pixels back over the
     authored field every pass, so a font/zoom/theme change had nothing left to re-resolve. Pixels
     are made only where the font is known: `Space::resolve`, the four `pad_*` cascade accessors,
     `Layout::to_taffy(font)`. The whole-pixel rounding moved there too.
   - C7 sweep: real code only, one commit per crate, showcase included, test files skipped.
     Measured 302 in non-test files (52 showcase), 153 in tests, 455 across 58 files.
   - C8 docs: docs/widgets.md "Writing a space" + "Spaces on the wire"; docs/plugins.md §6.

2. **`.accent(Color)` — override the accent for a widget AND its subtree.**
   Theme → nearest ancestor that set one → the widget's own. `PaintCx::accent()` reads it;
   `paint_child` applies it to the subtree so nothing opts in. `Base::style.visual.accent`.
   WHY: a pane re-tinted its contents by being painted under a COPY OF THE THEME with the accent
   swapped — one paint call per pane, which is what stopped a container painting its own children.
   **This removes trap 4 of the column parent move.** The pane publishes its hue in
   `chrome/pane/shell.rs::focus_state_to`. Deleted with it: the theme swap, `pane_gui_theme`'s
   border_color parameter, `TerminalPaneShell::border_color` and `is_active`, and three host-side
   reads of a pane's frame colour in render.rs.
   Named `accent` (not `tone`) on Antonio's call, so `tone` means only what `hint_tone` means.
   ⚠️ `Badge::accent("3")` is a VARIANT CONSTRUCTOR; `.accent(colour)` is the hue override. Both
   documented.

3. **The picker stopped counting children.** It decided "this node is a mere wrapper" from
   `children.len() == 1`, so a container's letter appeared and vanished with how many things were
   inside it — the workspaces dock wore one with two workspaces and none with one. `Base::transparent`,
   set by `wrap_transparently` (all five wrappers already call it), is the node answering for itself.
   VERIFIED ON SCREEN at one, two and three workspaces.

4. **The dock's own letter is scoped to the dock pick** (`DOCK_PICK_SCOPE`, `chrome/scene.rs`).
   It comes from the host's focus wrapper, which clicks to `FocusDock` — nobody declared it.
   `prefix+Shift+e` already letters every dock, so `prefix+/` was spending one per placement.
   ⚠️ Scoping it BROKE the dock pick and an existing test caught it. The real fix: a scope governs
   COLLECTION; naming a target outright is not a collection. `is_addressable` splits the two, and
   `offer.rs` uses it throughout.

5. **An exit animation now ends.** `Component::close` leaves a surface `visible` while its exit
   plays and nothing ever took it down, so any animated surface stayed "visible" forever after its
   first close — the exposé kept being offered letters by `prefix+/` long after being shut.
   `Presence::take_finished_exit()` reports the one frame an exit ends; `Base::tick_presence` acts
   on it and the default `Component::tick` calls it. Toast and Overlay now call it too.
   ⚠️ Asked of the presence, never inferred: "not open and not leaving" is also true of a widget
   that was never up, and reading it that way HID THE ENTIRE TREE (8 tests red).
   VERIFIED ON SCREEN via `HECA_LOG_PRESENCE=1` (temporary trace, since removed) — `EXIT SETTLED`
   fires — and by `HECA_LOG_HINTS=1` showing no `expose.pane.*` after closing the exposé.

6. **The exposé card declares its own pick.** `KeyHint::new(card)` deleted; the `Row` says
   `.on_hint(..).hint_placement(TopLeft)` itself. The wrapper was the shape from before `on_hint`
   moved onto every widget (T432). It cost a letter of its own: the wrapper declares, the card
   inside is actionable, so a picker saw two targets where the author wrote one — TWO KEYCAPS ON
   EVERY CARD under the exposé's own `s` picker. `PaneCard::build` returns `Row` now, so the
   compiler rejects putting the wrapper back.

7. **One walk, not two.** `KeyHintGroup` had a private copy of the collector's walk. The two had
   already drifted once about what a target IS (fixed by sharing the predicate); the walk stayed
   duplicated, so every rule learned afterwards reached `prefix+/` and not a surface picker.
   `collect_hints_scoped(root, scope)` is the one walk; the copy is deleted.
   This is what gives a surface picker the zero-size rule (`unseen` = clipped OR squeezed to
   nothing) — the narrow-window fix, previously live only for `prefix+/`.

8. **Docs.** `docs/hint-architecture.md` § 5a "Which one do I reach for" — `on_hint`,
   `hintable(false)`, `KeyHint` (and when NOT), `KeyHintGroup`, plus a symptom→cause table.
   `docs/widgets.md` warns beside `hintable` and points at § 5a from both the `KeyHint` and
   `KeyHintGroup` entries.

## Verification

1825 tests pass. Clippy clean at the one pre-existing `heca-grid-ui/src/widgets/toast/stack.rs`
type_complexity warning. No file left worse formatted than HEAD (measured as newly-unformatted
lines, not hunk counts — hunk counts move when well-formatted code lands between two bad regions).

Guards added this session, each run red against its own bug by sabotage. **Three proved nothing on
the first attempt and were rewritten; one did not exist until a sabotage passed.** Specifically:
- `spacing_is_written_under_one_property_name` — the first version could not fail, because the
  retired wire name still works, so nothing downstream can tell you the builder picked wrong. It
  pins the emitted property NAME.
- `a_pane_publishes_its_frame_colour_to_everything_it_holds` — did not exist until sabotage E
  printed nothing.
- `a_mounted_dock_is_not_a_target_of_the_ordinary_picker` — same.
- `a_card_is_one_pick_target_not_two` — PASSES BUT IS NOT PROVEN. Three sabotages failed to turn it
  red: two hit branches the fixture does not run (`folder: None`), one was swallowed by the
  zero-size rule. The real protection is the return-type change. **Treat it as unproven.**

## Verified on screen by Antonio

- Panes unchanged after the accent move.
- The dock's letter consistent at one/two/three workspaces and absent from `prefix+/`.
- The exposé not lettered once closed.
- NOT yet checked: one letter per card under the exposé's own picker after the wrapper removal.
  **The exact sequence to hand Antonio: `prefix+Tab` to open the exposé, then `s`.** Both are
  defaults in `keybindings.default.toml` — `prefix+Tab` is `toggle_layer` with
  `args = { name = "heca.expose" }` (~line 288); `s` is `pick` in the `[[keys.surface]]` block
  named `heca.expose` (~line 890), beside `delete_pane = "x"`, `delete_column = "r"`,
  `delete_workspace = "d"`. Both are user-rebindable. Expect ONE keycap per card, top-left.
  Before the fix there were two: one from a `KeyHint` wrapper and one from the `Row` inside it.
  `prefix+/` was never affected — its walk collapses a wrapper into what it wraps; only the
  exposé's own picker showed the double, because `KeyHintGroup` kept a private copy of that walk.

## Operating notes that cost time this session

- `HECA_LOG_HINTS=1` prints every target with identity, surface, rect and `declared`. **Use it.**
  Every wrong answer this session came from reading code; every right one from running it.
- **There is no log for a surface picker.** `s` prints nothing — the log lives in the host's
  `prefix+/` path only. That is a gap.
- `planner-task-update` with a `checklist` array CLEARS every tick. Re-tick and read
  `.planner/phases/<id>.json` back.
- A large `description` payload can fail; put detail in `.planner/docs/` and point at it.
- A peer session committed this session's uncommitted work with `git add -A` under an unrelated
  message. If another session is live in this folder, a clean `git status` is not a safe signal.

## Carried over — filed NOWHERE, and true as of 2026-09-15

These were in the previous handoff for one reason: no task holds them, so a handoff that drops them
loses them. Verified still true.

- **Two defects in `heca/src/mouse/hit_test.rs`**, both older than this work and both unfixed:
  - **line 29** — a hardcoded `40.0` dead zone on the left when the sidebar is hidden. It is a rail
    width, for a rail that was deleted.
  - **line 37** — `if pos.0 > win_h / 2.0`: an **x** position compared against half the window
    **HEIGHT**. Almost certainly meant `win_w`.
- **`macos_option_as_alt`-style setting** — still wanted, never started. It appears nowhere in the
  repo (checked: no Rust, no TOML).
- **`.planner/docs/p094-pane-hosts-any-app.md` is CURRENT, not an archive.** Its samples already use
  the new spellings. Read it before writing any component there.
- **`prefix+Ctrl+c` is `move_pane_to_column_pick`; `prefix+Shift+g` is
  `move_column_to_workspace_pick`.** Different actions. They have been conflated before.
- **The undeclared letters are BY DESIGN.** Of 31 letters in a running window, 14 are widgets nobody
  declared — pane-header buttons, fold toggles, docks. That is the rule working (being pickable is
  not opt-in) and Antonio confirmed it. **Do not "fix" it.** The arithmetic worth knowing: four panes
  already spend 8 of the 52, so the known >52 limit arrives sooner than a pane count suggests.
- **`descriptionRef` refuses every path spelling** and has never held a value in this planner. Do not
  retry it; put long text in `.planner/docs/` and point at it in prose.
