# T497 — what is broken right now, and what is known about each (2026-09-03)

Written at the end of a bad session. Antonio drove every round; most of what follows are **his
observations**, with my diagnosis marked as such. Treat an undiagnosed item as undiagnosed.

## Standing decisions from today (do not re-open)

- **`ButtonGroup` reads the layout. It never measures.** An earlier version cached each button's
  width in both forms and laid them out standalone; that is the same throwaway measuring pass this
  task deleted from the header, moved into the library. Antonio: *"you don't have to measure
  nothing. it must adapt to the available space, like a css flex property."* It is gone; do not
  bring it back in any form.
- **Fix at library/widget/component level, never at the call site.** Said many times today. Two of
  my regressions were exactly this: a danger colour picked in the pane header, and a size rule
  written into a surface.
- **The picker's compact-target rule stays** — a small target gets a 0.85× cap so the letter does
  not cover what it labels. Antonio: *"Leave the picker size as it is now."*
- **Never mention context budget.** In memory; broken again today.

## OPEN — with a diagnosis

### 1. A destructive button still shows a red border, and Antonio wants the border optional for all buttons
Antonio: *"Descructive should be red and border should be optional for all buttons. check how it is
now."* and, after a fix attempt, *"Red border is still there."*

What was done: `Button` gained a `dangerous` flag set by the `Destructive` variant. `Component::
set_variant` lets a container hand its chrome down while a dangerous button keeps reading in the
theme's danger. `ButtonGroup::child` now routes children through `set_variant` (commit `7a4a853`)
— **before that commit children skipped the rule entirely**, which is why the border was still
there when he last looked. **It has not been driven since `7a4a853`. Ask him to look again first.**

The larger request is separate and not started: **a border should be an optional property on every
button**, not something a variant decides. That is a library change and needs his agreement on the
shape before anything is written.

### 2. The red button flashes at the top-left corner while resizing
**My diagnosis, untested:** `Button::set_icon_only(false)` **pushes a new `Label` child**, and that
runs from `ButtonGroup::apply()` inside `on_layout`. A child created during the layout walk has no
bounds yet, and the frame that paints next draws it at the window's origin. This is the *third* time
this shape has bitten today (the ⋮ constructor, the ⋮ rebuild, and now the label), so it is worth
solving as one rule rather than a third patch: **nothing may be added to a tree while it is being
laid out.**

Note the group only re-adds a label when the words come back, which is exactly what the new
grow-back behaviour (commit `e1e1da2`) made possible — so this regression arrived with that.

### 3. The gaps inside the button row are uneven — the ⋮ sits too close to its left neighbour
Antonio, with a screenshot. **My diagnosis, untested:** the ⋮ is `Button::empty().icon(..)` and has
never had a label, while its siblings are `Button::new(label).icon(..)` reduced to icon-only. Both
end up `[Icon]`, but they reach it by different routes and `remeasure` may leave different padding.
Compare the two constructions before changing anything.

### 4. The picker letter is above the ⋮ and below every other button
Came back after being fixed. **My diagnosis:** the letter's placement depends on whether the target
is judged *compact* (`is_compact`, in units of the picker font). If the ⋮ is wider than its siblings
— issue 3 — it stops being compact and gets the large-target placement. **Issues 3 and 4 are very
likely one bug.** Fix the width and re-drive the letter.

## OPEN — pre-existing, confirmed by Antonio on a clean tree

He stashed everything and tested `0695caf`. Both reproduce there, so **neither is from this
session's work**, and both still need fixing.

### 5. Splitting a column into three panes pushes the last pane off the screen
The column's arithmetic is **provably correct** — I measured it with no pinned panes, one pinned and
two pinned, and the heights sum to exactly the available height every time, with `pane_y_in_column`
agreeing (three gaps plus three panes lands one gap short of the bottom).

So the fault is **not** in `compute_pane_sizes`. It is a disagreement between the `working_height`
the layout is handed and the space actually drawn. Making the fill pass scale unconditionally
*masks* it — the panes get renormalised to whatever height they were told — and that masking is what
causes issue 6, so the two are coupled and cannot both be fixed at that layer.

**Where to look next:** trace `working_area.size.h` from where it is set to where a pane rect is
drawn, printing at each step. Do not touch `compute_pane_sizes` again without that trace.

### 6. Terminal text flickers when dragging a divider between two panes in one column
The upper pane flickers, the lower does not. A terminal re-rasterises whenever its box changes, so
any per-frame jitter in pane heights shows as shimmer. Coupled to issue 5 as above.

## OPEN — known and recorded, lower priority

- **`Display::Auto` is not settled.** Taking the words off makes the row narrower, so the row then
  fits, which is the condition for putting them back; at some widths it alternates. `IconOnly` is
  the default and is stable; the pane header uses it. Written in the catalog entry.
- **Resize-direction actions** — Antonio asked for `resize_top` / `resize_bottom` / `resize_left` /
  `resize_right`, or a modifier on `j`/`k` in resize mode, so he can choose which edge moves. Not
  started. Needs the full AGENTS "Adding New Actions" checklist, not a keybinding.
- **The exposé columns looked wrong** in one screenshot, taken while the app was pegged at 100% CPU.
  Not re-checked since that was fixed. Confirm before investigating.

## Two guards I do not trust

- `constructing_a_group_does_not_request_a_frame` (heca-grid-ui/tests/button_group.rs) **passed with
  the bug restored**. The real evidence for that fix is the measurement: 33.7% idle before, 0.0%
  after, against a 0.0% baseline. Rebuild the guard or delete it.
- Every other guard added today was verified by sabotage — restore the bug, watch it go red, restore
  the fix. Do that for anything you add.

## How the CPU bug was found, because the method is the point

The app sat at 100% CPU. I instrumented the frame loop and printed every condition it uses to decide
whether to redraw: all false, yet it ran. So something was calling `request_frame()` directly. I put
a counter on `request_frame` and dumped a backtrace at call 400 — it named `ButtonGroup::new`.

`set_hidden` asks for the layout that must follow a change, and asking for a layout asks for a frame.
A widget being *constructed* has no layout to redo — and the pane headers are rebuilt every frame to
work out their key, so every frame's construction requested the next one.

**Measure, name the caller, then change one thing.** Three of today's four worst mistakes came from
reasoning about the layout instead of printing what it actually did.
