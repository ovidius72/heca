# T469 — the palette composed: implementation plan (2026-11-23)

Branch `feat/t469-palette-from-main`, worktree `/Users/antonio/projects/heca-feat-469`. Old branch
`feat/t469-palette-composes` (commit ae6003a9) is reference only.

## Done (C1–C4 of the checklist)
- `Input::placeholder(impl IntoTextValue)` — follows a value; no positional label lookup.
- `Keycap` widget (+ described kind, SDK, docs, tests).
- `.open(signal | "action")` on Overlay, Dialog, KeyHintGroup (`builders::Open`); `open_when` and
  `opens_on` gone (described `opens_on` still read, with a notice). A closed surface answers only its
  own opening action (`DeclaredAction::while_closed`).

## Decided by Antonio
- Selected row: an existing Row state (option 1 — `selected_background` fill + the nav outline), no
  new Row variant. "proviamo prima 1, poi valutiamo".
- Shortcut column right-aligned in each row, a row of `Keycap`s (Bordered) per chord.
- Palette lists are an open door (C5): `ext.palette_list(..)`, heca's own lists through it.

## C6 — the library palette composed (keep the public API so the app keeps working)
Keep: `Command` (all fields), modes/sigils, `SearchModel` ranking + history, `source`, `query`,
`query_in`, `match_description_in`, `on_mode_enter`, `label_signals`, `panel_size`, `on_dismiss`,
`query_writer`, `open_signal`, `query_signal`. Add `.open(signal | "action")`; `placeholder` takes
`IntoTextValue`.

Replace the hand geometry/paint/hit-testing (`geometry.rs`, `paint.rs`, the PointerMove/PointerDown
arms of `component.rs`, `remeasure`/`on_layout` writing sizes and shifting subtrees):

```
base.children = [ Overlay::new().blocking(true).open(own signal).on_outside_click(dismiss)
    .panel( Surface::column().elevation(Panel) …
        Grid::new().template_row("auto 1fr").child([
            query Input (placeholder follows the mode),
            ScrollRegion::new().child( Flex::column().child(results.map(row)) ),
        ]) ) ]
row(i) = Row::new().active(live: selected == i).reveal_when(keyboard moved)
            .on_click(choose i).on(PointerEnter, select i)
         .child( Grid::new().template_column("auto 1fr auto").child([
            Icon (tone by state), text column (title Label w/ marks, description Label whose wrap
            follows "selected", detail), chords: Flex::column of Flex::row(keys.map(Keycap::new
            .variant(Bordered))) right-aligned ]) )
```

- `results: Signal<Vec<usize>>` (command indices in rank order) drives the rows (BoundChildren); a
  row is kept while its command stays in the results.
- `selected` becomes a `Signal<Option<usize>>` (a command index); rows read it live.
- Library addition needed: `Row::active(impl IntoBoolValue)` (live), consistent with the live text /
  visibility builders.
- Panel width / max rows from `panel_metrics(panel_size)` as the panel's own size.
- Filter stays a pure function over items (`results()`), testable without the widget.

## Then
- C5: palette lists open door in the app (replace `MODES`, fold in `PaletteRows`).
- C7: same picture in app / showcase / plugin; tests (box test, nothing outside, rows never empty);
  docs; lint macOS + Windows; Antonio's eye check.
- Report: `Dialog` reaches its Overlay as `children[0]` — fix with the palette (same shape).
