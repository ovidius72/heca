# P097 — the input and drag seams, as they now stand

Companion to the P097 handoff, which has no room for this. **Read it before touching drag, pointer
routing or the hint walk** — this is live code, not history.

## How a drag works, end to end

A row declares `.draggable_as("pane")` and `.accepts(["pane"])` (or `.accepts_beside(["column"])`).
The framework runs the whole gesture: threshold, `DragStart` / `Drag` / `DragEnter` / `DragOver` /
`Drop` / `DragEnd`.

**What is carried is the widget's own identity** — `drag::resolve::drag_identity`: the declared `key`
when there is one (unscoped, so a right-click and a drag point at the same thing via `nav::key_at`),
the derived identity when there is not (`nav::identity_of`, scoped by keyed ancestors).

⚠️ **A widget never has to be named.** `key` is optional; one that declares none still has an
identity derived from its **content**, never its position. **Never gate a capability on `Base::key`
being present** — `.draggable()` did, and silently did nothing on every widget nobody had reason to
name. Antonio: *"this makes developers know about an internal API not common"*. Guard:
`an_unnamed_widget_is_still_draggable`.

**Resolution** is `drag::resolve_at_for(root, pos, kind)`. It skips a target that refuses the kind
**and the whole subtree of whatever is being dragged**, and returns `DropHit { key, path, bounds,
side }`. ⚠️ **`path` is the node the walk found — never search for it again by name.** Two seatings of
one container give their rows the same name, so a name search lights whichever comes first, which is
why the left sidebar lit up while you dragged in the right.

**A drop reaches the app** through `heca_grid_ui::drag::install_drop_sink`, installed once in
`heca/src/app/startup.rs` beside the menu sink. It pushes `drag::Dropped { source, target, side,
action, modifiers }` onto `AppState::pending_drops`; `chrome::drain_pending_drops`
(`heca/src/chrome/dispatch.rs`) drains it each frame from `app/render.rs`. It resolves both **names**
through `RetainedChrome::drag_items` — a registry keyed by NAME, never parsed — then calls
`chrome::pane_drop_row` and `mouse::surface_left::accept_drop` or `mouse::release::column_drop`.
**Both take an already-resolved target.**

**What a drop DOES** is `drag::DropAction` {Move, Swap}, answered once by `DropAction::held()` and
carried on `Dropped`. Which modifier means swap is a host setting (`drag::set_swap_rule`, wired from
`settings.swap_modifier`, default Shift). **Never re-derive it from a modifier** — it was written
twice and the two disagreed.

**Where a drag is painted:** `component::paint_drag_feedback`, called from `paint_child` beside the
hint letter — one place every widget passes through, so nothing opts in. The target draws the
insertion line or the swap outline from `PointerState::drag_side`; the source draws a picture of
itself under the pointer from `drag_pos`, and fades where it still sits.

**"Is a drag in flight"** is `heca_grid_ui::dragging(root)`, asked app-side through
`chrome::drag_in_flight`.

**The showcase is migrated** (`heca-renderer/examples/showcase.rs`) — a live example of the
authoring API.

## How pointer input reaches a tree

The event loop builds **one `Event::Raw` per device event** and hands that same event to each tree:

- `chrome::dispatch_surface_pointer` — surfaces above the page, asked first, positionally and once.
- `chrome::deliver` — the window root. The one door.
- `chrome::deliver_to_panes` — every pane's own tree (its frame and its header slot).
- `chrome::deliver_to_pane_viewports` — the scrollback badge and scrollbar.
- `chrome::pane_viewport_at` — a geometry question, deliberately not bolted onto a move.
- `chrome::cancel_every_tree` — the pointer left the window; tell all of them.

There is **no function per event kind** anywhere, and none hardcodes a button.

## What guards all of this

- **`heca/tests/surface_drift.rs`** — fails when a surface is registered instead of placed, and its
  second test fails when the known-offender list goes stale (`chrome/palette.rs`,
  `chrome/overlay.rs`, `chrome/expose/mod.rs` still register for context/modality).
- **`heca/tests/pointer_funnel.rs`** — 9 lints on the event loop, every one verified by sabotaging
  the code and watching it go red: every pointer branch reaches the funnel; the chrome tree gets the
  release and the wheel; the button branch feeds both halves; the mouse layer sends a release for
  every press; the key funnel delivers releases; the divider resize ends before anything can swallow
  the release; the move that drives a drag is not withheld; the tree learns the modifiers before the
  app reacts; and **the window tree has one door and not a function per kind** (covering all three
  tree families).
- **`heca/src/chrome/layers/tests.rs`** (29 tests) — including
  `a_key_dispatched_at_the_window_root_reaches_a_placed_surface`,
  `a_menu_seated_as_a_surface_keeps_its_own_height`,
  `a_layer_seated_as_a_surface_still_fills_the_window`,
  `a_surface_that_draws_nothing_does_not_take_the_pointer_from_the_chrome`.
- **`heca-grid-ui/src/drag/resolve.rs`** — `an_unnamed_widget_is_still_draggable`,
  `an_unclaimed_drop_is_handed_to_the_host`,
  `a_target_that_refuses_the_kind_is_skipped_for_the_one_that_takes_it`,
  `the_thing_being_dragged_is_not_a_target_for_itself`,
  `a_sibling_target_is_two_halves_and_a_container_is_three_bands`, `a_drag_in_flight_clears_hover`,
  `what_a_drop_does_is_decided_in_one_place`,
  `a_pointer_event_built_without_modifiers_still_knows_what_is_held`,
  `the_host_says_which_modifier_means_swap`.
- **`heca/src/app/conflicts.rs`** — `one_modifier_cannot_both_start_a_drag_and_change_what_it_means`,
  `when_the_modifiers_collide_the_gesture_survives_and_swap_gives_way`.
- **`heca-grid-ui/src/layout.rs`** — `a_placement_that_leaves_an_axis_auto_keeps_the_widgets_own_size`.

## Earlier commits this work rests on (previous session, all driven)

`c811494` T495 — modal as a node; menu sizing; the surface pointer pass-through.
`e441d9c` a row is dragged by the name it already declares; widgets draw their own drag feedback.
`966cc98` an unclaimed drop is handed to the host, once.
`5d69f4a` the row owns the gesture, in every sidebar.
`2019b22` mark the node the walk found; `accepts_beside`; not a target for itself; the source fades.

⚠️ Two defects from T495 worth not reintroducing, both the same shape — a widget's BOX taken for
something it is not: a `ContextMenu` stretched down the whole window by its seat (a placement's
`Auto` axis is not an answer; the widget's own size stands), and an empty invisible notification
surface swallowing **every press in the app** (a seated surface passes the pointer through where it
covers nothing — `Base::surface`, the browser's `pointer-events: none`).
