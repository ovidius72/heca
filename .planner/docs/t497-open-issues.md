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

## CLOSED 2026-09-03 (second session) — do not reopen

### 1. The destructive button's red border — CLOSED BY DECISION
Antonio, after seeing why it happens: *"I decided to not go ahead with this fix. Let's leave as the
UI style dictate. Danger has border and it is ok. Remove attempts to fix it."*

**Why it happened, for the record:** a group tells its children to look like the row, and a button
obeys only if it has not already named a look of its own. Closing a pane is declared destructive by
the action, and `Destructive` is a whole appearance — hue *and* frame — so that button named one and
ignored the row. Its neighbours named nothing and took the row's quiet look, which is frameless at
rest. Nothing to do with the theme or the header.

**What was removed:** the clause in `Button::set_variant` that let a container take a *dangerous*
button's variant away. A named variant is now kept whole by anything that hands a variant down —
not just by a `ButtonGroup` — so the decision holds centrally rather than at one call site. The
`dangerous` flag stays: it decides the hue of the held-on frame.

### 2. The red button flashing at the top-left corner while resizing — FIXED
**Cause, as diagnosed and now confirmed to be one rule rather than three patches:** a widget's
layout node is written by the engine as it walks, so a widget the walk has never reached has none —
and its default bounds are at the window's origin with no size. Painting it drew it there for the
frame before the next layout arrived. A widget added *while* its tree is being laid out is exactly
that case, which is what putting a button's words back does.

**Fix:** the paint pass skips a widget that has never been laid out, in the one place every widget's
paint passes through (`paint_child`, beside the hint letter and the tooltip). No widget opts in, and
the same rule covers all three instances that were patched separately. Guard:
`a_widget_that_has_never_been_laid_out_paints_nothing` (`heca-grid-ui/tests/phase_a.rs`), verified by
sabotage. Two older tests painted trees they had never laid out and asserted their text appeared;
both now compute layout first, which is what every other paint test already did.

Still to be driven by Antonio.

### 3. The ⋮ was a sliver next to full-width buttons — FIXED
Antonio, with two screenshots and *"buttons should have the same width, regardless the icons
inside"* and *"it's even hard to click"*.

❌ **One written diagnosis was disproved first:** *"the ⋮ and its siblings reach icon-only by
different routes and keep different padding."* The layout re-measures every child after resolving
the font, so both go through the same step with the same numbers — and an `Icon` already lays out as
a square of the font size, so the glyph inside never decides a button's width.

✅ **The cause:** the ⋮ was the only child not told to refuse shrinking. Every button an author adds
is; the group's own affordance was not. Being the only thing in the row that could give, it was the
thing the layout took the room from — so the group never saw an overflow, never moved a button into
the menu, and the ⋮ absorbed the whole shortfall.

**Why the first harness missed it:** a group alone in its parent never reproduces it. The squeeze
needs something beside the group competing for the same strip — which the pane header has, a title.
**Measured** with the header's real shape (title + group, space-between) across 40–200px: the ⋮ came
out **4 to 8 pixels wide beside a 24-pixel sibling**, at every width from 149 up.

**Fix:** the rule is stated once (`never_gives_way`) and used at both places a button enters the
group, so a future child cannot miss it. After it, no width in that sweep squeezes anything. Guard:
`the_overflow_trigger_is_never_squeezed_by_a_title_beside_it`, verified by sabotage — it reports
`the row is [25.0, 4.0]` with the fix removed.

## OPEN — with a diagnosis

### 4. The picker letter is above the ⋮ and below every other button
The letter's placement depends on whether the target is judged *compact* — a box small enough that a
letter drawn over it would cover it. A ⋮ four pixels wide is a different kind of target from a
24-pixel one, so issue 3 is the likely cause of this too. **Re-drive it before investigating**: it
may already be gone.

## OPEN — pre-existing, confirmed by Antonio on a clean tree

He stashed everything and tested `0695caf`. Both reproduce there, so **neither is from this
session's work**, and both still need fixing.

### 5. Splitting a column into three panes pushes the last pane off the screen — FIXED (2026-09-04)
**Nothing was ever pushed off the screen.** The heights always summed to exactly the column, which is
why the earlier look concluded the arithmetic was correct and stopped there. The new pane was being
**starved**: measured at `[507.3, 259.7, 0.998]` — a pane one pixel tall, present in the column and
invisible.

**Cause:** a height a pane was dragged to is stored as a preference, and two panes dragged to fill
the column hold the whole of it between them. A third arriving had nothing left, and the pass that
scales the column to fit shaved barely one per cent off the other two.

**Fix — and where it is NOT:** making room happens when a pane is *added*
(`Column::make_room_for_one_more`), which scales the preferences down just enough to leave every pane
its floor. Putting the same reservation inside `compute_pane_sizes` is the obvious move and is
**wrong**: that runs on every drag too, and a boundary must move space between its own two panes and
nothing else. Trying it there turned `a_resize_leaves_every_other_pane_where_it_was` red immediately —
that guard is doing its job, do not weaken it.

Result: `[508, 160, 100]`, the pinned pane giving way by the amount the newcomer needs. Guard:
`a_pane_added_to_a_column_that_was_dragged_full_still_gets_room`, verified by sabotage. ⚠️ Its
assertion is that **every pane is big enough to be a pane** — asserting the heights sum to the column
proves nothing here, since they did so throughout the bug.

Not yet driven by Antonio.

### 6. Terminal text flickers when dragging a divider between two panes in one column
The upper pane flickers, the lower does not.

❌ **The recorded diagnosis is wrong, and so is the claim that this is coupled to issue 5.**
*"Any per-frame jitter in pane heights shows as shimmer"* assumes the heights jitter. **They do not.**
Measured 2026-09-04 by dragging the first divider of a three-pane column one pixel at a time for
forty steps: the dragged pane grows by exactly 1px per step, its neighbour shrinks by exactly 1px,
and **the third pane does not move at all** — no oscillation anywhere, in any pane, at any step.

And issue 5 was fixed without touching the fill pass the coupling claim rested on, so the two are
not two faces of one bug.

**So the shimmer is in the render path, not the layout.** A terminal re-rasterises when its box
changes, which during a drag is every frame *by design* — the question is why that reads as a flash
in the upper pane and not the lower. Look at the retained terminal layer: a resize forces a full
repaint and reallocates the layer's texture, so a frame that blits the new texture before the
re-render has filled it would show exactly one blank/stale frame per step. Start at
`ensure_size` / `retained_damage_to_apply` in `heca/src/app/terminal_render.rs`, and instrument the
order rather than reasoning about it.

⚠️ **Not confirmed to still reproduce** since the split fix. Have Antonio drag a divider before
investigating.

## OPEN — known and recorded, lower priority

- **`Display::Auto` is not settled.** Taking the words off makes the row narrower, so the row then
  fits, which is the condition for putting them back; at some widths it alternates. `IconOnly` is
  the default and is stable; the pane header uses it. Written in the catalog entry.
- **Resize-direction actions** — Antonio asked for `resize_top` / `resize_bottom` / `resize_left` /
  `resize_right`, or a modifier on `j`/`k` in resize mode, so he can choose which edge moves. Not
  started. Needs the full AGENTS "Adding New Actions" checklist, not a keybinding.
- **The exposé columns looked wrong** in one screenshot, taken while the app was pegged at 100% CPU.
  Not re-checked since that was fixed. Confirm before investigating.

## Guards — all verified

- `constructing_a_group_does_not_request_a_frame` (heca-grid-ui/tests/button_group.rs) was recorded
  as **vacuous** — "passed with the bug restored". **That was wrong.** Re-sabotaged 2026-09-03
  (second session) by hiding the ⋮ with `set_hidden` at construction, which is the original defect,
  and the guard **failed**. It is real; keep it. The earlier attempt must have edited a path that
  does not run, or read a stale build. Corroborating measurement stands either way: 33.7% idle
  before the fix, 0.0% after, against a 0.0% baseline.
- Every guard added is verified by sabotage — restore the bug, watch it go red, restore the fix. Do
  that for anything you add.

## How the CPU bug was found, because the method is the point

The app sat at 100% CPU. I instrumented the frame loop and printed every condition it uses to decide
whether to redraw: all false, yet it ran. So something was calling `request_frame()` directly. I put
a counter on `request_frame` and dumped a backtrace at call 400 — it named `ButtonGroup::new`.

`set_hidden` asks for the layout that must follow a change, and asking for a layout asks for a frame.
A widget being *constructed* has no layout to redo — and the pane headers are rebuilt every frame to
work out their key, so every frame's construction requested the next one.

**Measure, name the caller, then change one thing.** Three of today's four worst mistakes came from
reasoning about the layout instead of printing what it actually did.

---

# Three more, found while driving (2026-09-04) — all FIXED

Not in the original six. Each was found because Antonio drove something and saw it, and each turned
out to be a rule in the wrong place rather than a local bug.

## A pane redefined what "a border" means for everything inside it

**Symptom** (Antonio): a header button showed a border on hover in the ACTIVE pane and none in any
other pane.

**Cause:** the pane's tree was painted with a theme whose `colors.border`, `border_width` and
`border_radius` had been overwritten with the pane's own FRAME values — and those are the tokens
*every control inside the pane* reads for its own chrome. The frame is a strong accent on the active
pane and nearly the background on the rest, so every control inherited that as its border.

**Fix:** stop overwriting them. Nothing is lost — the shell already states its frame as its own style
(`chrome::pane::shell`, `.border(colour, width).radius(radius)`), which is where a widget's own look
belongs. The accent override stays: an active pane really does mean to re-tint what it holds.

⚠️ **Why it went unnoticed:** the function took `&AppState`, so it needed a window and could not be
tested. Splitting the rule from the state (`pane_gui_theme`) is what made a guard possible, and the
guard fails if the frame colour is written back into `border`.

## A tooltip needed a mouse nudge to appear

**Symptom** (Antonio): rest the pointer on a button — nothing. Move it one pixel — the tooltip
appears instantly.

**Cause, in two halves, and the second is the one that matters.** A resting pointer produces no
events, so nothing draws the frame the bubble would appear in. The library has always answered this
(`Component::next_redraw`, folded down a whole tree, guarded by
`a_pending_tooltip_asks_the_host_to_wake_for_it`) — **but only the showcase ever asked.** The app
never did.

⚠️ **The first fix was wrong and I reported it as done.** I made the loop *sleep* until the tooltip
was due and never made *arriving* draw anything: at the deadline every reason-to-draw is false — the
widget no longer reports a pending wake, because it is due NOW — so the loop woke and went straight
back to sleep. **Scheduling a wake and acting on it are two things.**

**Fix:** `chrome::next_redraw_across_trees` asks every tree the app draws (beside `cancel_every_tree`,
so a fourth tree family is added in one place), and `AppState::widget_frame_due` remembers the
deadline so that *reaching* it is itself a reason to draw — the same shape the app already uses for
the visual bell and toast auto-dismiss. Guarded by a source lint in `heca/tests/pointer_funnel.rs`
that checks BOTH halves; removing either turns it red.

## The picker decided a button's letter by what it was placed inside

**Symptom** (Antonio): the ⋮ had no `prefix+/` letter, while its siblings did.

**Cause:** a declared hint silenced mere *actionability* anywhere beneath it. So a button's letter
depended on its ancestors — every pane-header button had to repeat its own click as a hint to win a
letter back, and the ⋮, which the widget builds for itself, had no author to do that for it.

Antonio: *"Users/Developers MUST do not thing whare a widget is… We want dom like structure. Do you
say a button where it is when you put a button in the dom????"*

**Fix — count things, not layers.** A **wrapper** (a node holding exactly one other node) and the
node it holds are ONE thing and share one letter. A node with **more than one child** is a real
container, and what is inside it are separate things, each keeping its own. A pane holds a bar and
its content, so the pane keeps its letter and every button in its bar keeps one too.

⚠️ **Among layers of one thing, a declaration outranks mere actionability.** My first version let the
innermost layer always win, which instantly reversed what a pick means in the sidebar: a pane row
declares *"peek without leaving"* and something inside it says *"go there and leave"*, and the inner
one won. Saying what a pick does IS saying it is not the click, so that is the layer the letter runs.

**What it removed:** four hand-written hints in the pane header, and the `Hint::click()` capability I
had added an hour earlier to work around the old rule.
