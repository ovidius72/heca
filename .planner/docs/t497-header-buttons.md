# T497 — what the header's action buttons became (2026-09-03)

Continuation of T497. The pane header being a child of its pane exposed a defect Antonio drove:
in a narrow pane every action button was squashed to a seven-pixel sliver while the title beside
them ellipsed correctly. Fixing it properly meant three library capabilities and one layout-engine
bug; the header is the first caller of all of them.

## Library — new

**`ButtonGroup`** (`heca-grid-ui/src/widgets/button_group.rs`) — a row of related actions that fits
the space it is given. Children are typed to `Button`, so their text cannot be omitted (every
`Button` constructor takes it) — the same reason `Select` types its options to `Choice`, and it is
what makes a collapsed row readable with nothing extra written. As the room runs out it gives up the
words first (the buttons become their icons, keeping the words for hover and the menu), then moves
what still does not fit into a menu behind a trailing `⋮`.

**It reads the layout; it never measures.** The group takes the room that is left, lays its buttons
out at their own size aligned to the end of it, and counts what the layout placed *outside* the box —
dropping that many from the trailing end. An earlier version cached each button's width in both
forms and laid them out standalone to get them; that was the same throwaway measuring pass this task
deleted from the header, moved into the library, and it is gone.

Three things keep it stable, each guarding a loop that showed as a visibly rearranging toolbar:
it **grows** into the room (a group that hugs its content is as wide as whatever it decided to show,
so asking it how much room there is returns the answer it just produced); the buttons are **built in
the mode they will be shown in** (otherwise the first decision is made from an arrangement that was
never going to be drawn); and the `⋮` is **built at construction**, then shown or hidden — created
during the decision it had no bounds yet and the next frame drew it at the window's origin.

`Display` — `IconOnly` (default), `Full`, `Auto`. ⚠️ **`Auto` is not settled**: taking the words off
makes the row narrower, so it then fits, which is the condition for putting them back; at some
widths it still alternates. Recorded in the catalog entry.

**A collapsed row runs the button's own click.** `Button::on_click` became a shared handle so a menu
row holds and runs the same closure. It first wrote down which button was chosen for the group to
run on its next event — but the menu is in a layer, so nothing reaches the group when a row is
clicked, and the action sat there for seconds.

## Library — changed

- **Tooltips are a property of every widget** (`Base::tooltip`, `ComponentExt::tooltip`), timed off
  the hover clock the pointer router now keeps (`PointerState::hovered_for`) and drawn in
  `paint_child` beside the hint letter and the drag feedback. The wrapper survives only for a region
  that is not a widget you can put a builder on, implemented in terms of the property. Needed because
  a widget held by a typed container cannot be wrapped without ceasing to be what the container
  accepts.
- **Icon buttons keep their size** — they never said they could not shrink, so they gave away
  everything. The library's own rule already anticipated it: *a widget that must keep its size still
  says so.*
- **Buttons gained** a held-on state (`active`, the same tokens `IconButton` uses), a `tone` (danger
  hue without the destructive variant's border), and `icon_only`. ⚠️ `icon_only` **removes** the
  label rather than hiding it: `display: none` left the button's box answered differently by the two
  passes the layout makes — it reported its full height and was then placed as though it had almost
  none, so it hung out of its row and took the picker's letters with it. Reproduced with a plain
  `Flex` holding one such button, which is how it was found.
- **The picker's letters have their own size and colour** (`Theme::hint_font_size` from
  `[appearance] hint_font_size`, and `Theme::hint_color`). They read the *widget's* font before, so
  an emphasized header button wore a letter a quarter larger than the pane's own; and the colour came
  from the ambient theme, which a pane replaces to mark itself active — so one picker showed two
  colours. Both set at `chrome_gui_theme` and carried unchanged into a pane's derived theme.
  The compact-target rule (a small target gets a 0.85× cap) is **deliberate and stays** — Antonio,
  2026-09-03.
- `Base::root_font` — the tree's base font, written by the layout pass, for chrome a widget floats
  beside itself.

## App

- The header is `Surface(padding) > Flex(gap, centre, space-between) > [Tag, ButtonGroup]`, the
  shape Antonio specified. The hand-subtracted 6px margin is gone: the strip holds its own padding.
- The throwaway pass that measured the button cluster to guess the title's width budget is deleted,
  along with the guess itself (character counts and two font multiples, whose stated backstop was the
  per-pane clip that this task removed).
- **Which actions are destructive is the action's own declaration** — `ActionCatalog::destructive`,
  beside `icon` and `label`. The header had `matches!(action, PaneAction::Close)` written into it: a
  styling rule keyed to a name, in a file that should know nothing about which acts cannot be undone.
  It reads the confirm spec, which is how this project already declares destructiveness.
- `action_tooltip` sets the property and returns the widget, so a header button stays a `Button` and
  the group will take it. `sidebar_toggle_button` returns its widget too; the `Tooltip` wrapper is
  gone from the chrome entirely.

## heca-core — a bug found while driving this

Splitting into three panes pushed the last off screen, **only in a column that had been resized**,
and the effective minimum differed between that column and a fresh one. A pane you have resized
carries a fixed height; the floor given to the others was worked out against the whole column but
applied against the room left after the fixed ones had taken theirs. The heights summed past the
column. Two changes: the auto panes' floor is bounded by the room they actually have, and the final
proportional pass scales in **both** directions so "the panes exactly fill their column" holds by
construction. The first is also what fixed the second symptom — resizing a middle pane moved the
bottom edge and then, past a point, the top one too, because the over-sum made the scale pull the
pinned panes off the sizes the drag had just set.

## Verification

Antonio drove every round. Guards added for: the tooltip property and its wake; a button's held-on
state; the group's collapse, its end-alignment, its single-pass settling, and that it adds nothing to
the tree after the first layout; an icon-only button's placement and that it still knows its words;
the destructive declaration; and three column-height invariants. Each guard was run against its own
bug — several passed on first write and were rebuilt until they went red.

## Open

- `Display::Auto` alternates at some widths (above).
- The top bar flashes slightly during a resize; cause not identified.
- Resize-direction actions (`resize_top` / `resize_bottom` / `resize_left` / `resize_right`, or a
  modifier on `j`/`k` in resize mode) — requested 2026-09-03, not started. Needs the full
  "Adding New Actions" checklist, not a keybinding.
