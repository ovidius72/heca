# Handoff

**Created:** 2026-07-26
**Reason:** About a third of context left. Everything is committed and pushed. Nothing in progress.

## Where the work is

Branch `feat/p017-t1-style-split`, PR #247, 14 commits. **937 tests, clippy clean.** Every change to
drawing, layout or interaction was driven in the running app by the user.

## What shipped today

The day started on F003/P017/T4 (a description could not set an input's placeholder or a scroll
area's axes). That turned out to need no code — the generated property surface had already made both
reachable — so the task became proof and documentation. Checking it in the app is what found the real
problem, and the rest of the day is F004/P011.

**A widget can no longer drop its children's events.** `Component::event` is gone. A widget declares
`on_event_capture` (before its children) and `on_event` (after they declined), and `dispatch` walks
the children — the only place in the library that does. Six widgets keep their own walk and say why
on the method: `Select`, `Dialog`, `Overlay`, `CommandPalette`, `ContextMenu`, `ItemGroup` and
`DockFrame`. Their code is unchanged; `heca-grid-ui/tests/pointer_delivery.rs` mounts a probe inside
each and fails if any kind of event goes missing.

**The app no longer picks which events to pass on.** Four places did, each choosing differently. All
four — the dialog layer, the sidebar tree, the pane headers, the terminal viewport widgets — now get
move, press, release and wheel. `heca/tests/pointer_funnel.rs` reads the event loop and fails when
one stops. Both checks were confirmed by putting the original bug back and watching them fail.

**Two rules came out of it, both in the library so every future widget gets them:**
`clips_children` (content scrolled out of sight stops being clickable) and `wants_visible` (a widget
says it is the current one, and every scroll area it sits inside brings it into view — that is how
the keyboard sidebar follows its cursor).

**The sidebar has a real scroll area.** It had none. Its position lives in
`SharedChromeState.workspaces.scroll` so it survives tree rebuilds, and `chrome_dispatch_press` no
longer throws the tree away on every click.

**Scrolling can be watched:** `on_scroll_start` / `on_scroll` / `on_scroll_end`, carrying the
position and the event that caused it. No event means the app's own `scroll_to` moved it.

**`Layout` gained per-side padding** (`padding_left/right/top/bottom`), which the scroll area uses to
reserve the scrollbar's space in layout instead of drawing over content.

## Decisions to keep

- **Plugins watching scrolling waits for the plugin runtime.** No plugin is running, so there is
  nothing to send an event to. Recorded in F004/P011/T4 (deferred) with what the API should be: the
  author attaches a function to the scroll area, and that is all — no id, no name, no arguments. The
  user rejected an earlier design that made the author name their own scroll area, and was right.
- **`ScrollBar` stays.** The terminal viewport cannot be a `ScrollRegion`: its content is a cell grid
  drawn by the renderer, not child widgets. `ScrollRegion` scrolls children it owns; `ScrollBar` is a
  control over content the widget tree does not own.
- **`SCROLLBAR_W` is the number to change** if the bar looks too thick (now 5). The grab area is
  separate and much wider, so a thinner bar is just as easy to hit.

## Next, in rough order

1. **An action does not describe the arguments it takes.** `ActionMeta` has a label, an icon, a
   policy and a confirm prompt, and no list of arguments — so a wrong or misspelled one is ignored in
   silence. Same shape as the two defects fixed today. Worth doing before anything else adds
   argument-carrying events.
2. **F003/P017** still has T5 (add `Separator` to the vocabulary), T6 (the `Row` name collision) and
   T7 (colour becomes overridable). T6 and T7 each need a decision from the user first.
3. The right sidebar's scroll position is not kept across rebuilds — no container state owns one yet.

## How this user works — read before starting

- **Plain words, short sentences, no idioms.** Do not write a term and then explain it; use the
  ordinary words the first time. "What gets sent between the two programs", never "the wire format".
  The user stopped the session over this more than once today. See the `plain-english-preference`
  memory, which now lists the exact failures.
- **Do not explain machinery they will never write.** Before describing how something works
  underneath, check whether they would ever type it. If not, say what they type and stop.
- **Only an explicit instruction to start means start.** A question, or agreement on a design, is not
  approval to write code. Ask "shall I start?" and wait.
- **Fixes belong in the widget, not at the call site.** Their standing rule: a fix must hold for every
  future use, not just the one place the problem appeared.
- **Never propose deferring work** to a later phase to avoid effort. Deferred work is forgotten work.
  Deferring because something it depends on does not exist yet is different, and fine — say which.
- **They verify rendering, layout and interaction themselves in the running app.** Green tests are
  never verification for anything visual.
