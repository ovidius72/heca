# T474 — the four library capabilities that landed 2026-09-14/15

═══════════ WHAT LANDED 2026-09-14/15, BEYOND THE CHECKLIST ═══════════

Four library capabilities, each found by Antonio driving the app, each fixing a rule that was in the
wrong place rather than a symptom. All verified on screen except where noted. Nothing committed.

1. **A space is one property** (checklist C5/C6, above). `.gap` / `.padding` / `.margin` and their
   per-axis and per-side forms take a number, a `Spacing` step, or a string, through `style::Space`
   with one parser shared with serde. The described side gets it free — the vocabulary IS the layout
   field list, so `"gap": "sm"` needed no name table. Retired names (`gap_spacing`,
   `pad_spacing_x/y`) are still READ, mapped in one place; nothing writes them.
   ⚠️ The authored step now SURVIVES layout. The old code copied the resolved pixels back over the
   authored field every pass, so a font, zoom or theme change had nothing left to re-resolve.

2. **`.accent(Color)` — override the accent for a widget and everything inside it.** CSS's
   inherited custom property, for a hue. Theme first, then an ancestor's override, then the widget's
   own; read through `PaintCx::accent()`, applied to the subtree by `paint_child`.
   WHY IT EXISTS: a pane re-tinted its contents by being painted under a COPY OF THE THEME with the
   accent swapped. That forces one paint call per pane and is therefore exactly what stopped a
   container from painting its own children — the blocker on the column parent move below. The pane
   publishes its frame colour now; the theme swap, the dead parameter it needed, a dead struct field
   and three host-side reads of a pane's frame colour are all deleted.
   `Button::tone` and `IconButton::tone` were private fields meaning the same thing and are now the
   same property. `tone` as a word is left meaning only what `hint_tone` means: a named meaning the
   theme colours. Verified on screen: panes unchanged.

3. **A container's letter no longer depends on how many children it holds.** The picker decided
   "this node is a mere wrapper" by COUNTING CHILDREN — exactly one meant wrapper — so the
   workspaces dock wore a letter with two workspaces and none with one. A parent counting its
   children to work out what it is is the pattern this file forbids everywhere else.
   `Base::transparent`, set by `wrap_transparently` (which all five wrappers already call), is the
   node answering for itself. Verified on screen across one, two and three workspaces.
   With it: **the dock's own letter is scoped to the dock pick** (`DOCK_PICK_SCOPE`), so `prefix+/`
   stops spending a letter per placement on a keyboard destination `prefix+Shift+e` already owns.
   The dock's letter comes from the host's focus wrapper in `chrome/scene.rs`, which clicks to
   `FocusDock` — nobody declared it; it is lettered because it is actionable.
   ⚠️ Scoping it broke the dock pick, and an existing test caught it. The cause was worth fixing:
   a scope answers WHICH COLLECTED PICKER may letter a target, but NAMING a target outright is not
   a collection. They were one question; `is_addressable` splits them, so a target can leave
   `prefix+/` without vanishing from the pick that owns it.

4. **An exit animation now ends.** `Component::close` leaves a surface `visible` while its exit
   plays, and nothing ever took it down: a surface with an exit animation stayed `visible` forever
   after its first close. The exposé's cards were therefore offered letters by `prefix+/` long after
   it was shut — a full-window target over everything you could actually see.
   `Presence::take_finished_exit` reports the one frame an exit ends; `Base::tick_presence` acts on
   it, called from the default `Component::tick` so every widget gets the rule.
   ⚠️ Asked of the presence, never inferred: "not open and not leaving" is also true of a widget
   that was never up, and reading it that way hid the entire tree on the first tick.
   ✅ **VERIFIED ON SCREEN 2026-09-15**, twice, by Antonio: `HECA_LOG_PRESENCE=1` ended with
   `[presence] EXIT SETTLED key=None — visible is now false`, and `HECA_LOG_HINTS=1` after
   opening and closing the exposé listed 13 targets with **no `expose.pane.*` at all** — where
   before the fix the same sequence listed `expose.pane.2` at full window size. The trace was
   temporary and has been removed. (An earlier version of this line said NOT YET VERIFIED; it
   was written before he ran it.)
   ⚠️ A SECOND copy of this bug is still left: `LayerRegistry::hide`
   leaves the layer RECORD's `visible` true while leaving, and nothing settles it either, so
   `is_active` reports a closed animated layer as up. The hint walk reads the widget, so the letters
   are fixed — but `content_covered` and `visible_host_layer_names` read the record. Two
   visibilities for one surface is the underlying smell.

OPEN, reported by Antonio 2026-09-15 and not yet traced: `prefix+Shift+e` shows ONE letter where the
workspaces dock is mounted in BOTH sidebars, and `dock_candidates` builds one candidate per mounted
container — so two were expected. Also: the dock pick's keycap is tinted `warning`, which is the
same tone a workspace's own peek letter uses, so two different acts read identically.

VERIFICATION STATE: 1824 tests pass; clippy clean at the one pre-existing toast/stack.rs warning; no
file left worse formatted than it was. Sixteen new guards, each run red against its own bug by
sabotage — three of them proved nothing on the first attempt and were rewritten, and one did not
exist until a sabotage passed.
