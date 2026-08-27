# Input architecture — one tree

## The model

There is **one retained tree** per window. Everything visible is a node in it:

- the chrome (sidebar, bars) is a subtree
- an overlay — a toast stack, a menu, a dialog, a plugin panel — is a positioned node
- a modal is a node that **swallows** any event it does not itself handle
- a pane's header is a child of that pane
- the terminal viewport is a **leaf node whose paint is special** — it still draws through the
  GPU terminal path, but for layout, hit-testing and event delivery it is an ordinary leaf

Stacking order is **tree position**, not a separate z-band system: a node painted later, and
hit-tested earlier, is simply further down the child list. The layer stack already models
stacking this way; input is the part that never followed.

Given that tree, the host does exactly four things per frame, each **one walk** over the whole
tree:

| pass | what it does |
|---|---|
| layout | size and place every node |
| paint | draw every node (the terminal leaf swaps in its GPU path here) |
| input | deliver each device event: capture down to the target, bubble back up |
| hints | collect the `prefix+/` letter targets |

`heca-grid-ui` already provides the input walk: `dispatch(node, ev)` in
`heca-grid-ui/src/component.rs` is a capture → target → bubble traversal, DOM-shaped and
complete. The host's job is to own **one** tree and call it **once** per event — not to call it
once per root and arbitrate between roots itself.

## Why this shape, and not the alternatives

Two designs get proposed repeatedly. Both are wrong for the same underlying reason, and it is
worth stating so they are not proposed a third time.

### Not: a second dispatcher for "overlay" events

The tempting local fix, when an overlay does not receive input, is to add a dispatch function
for that kind of overlay next to the one for modals. This does not scale: the next ambient
surface needs a third, then a fourth. Each is a hand-written enumeration of which roots exist
and in what order, and **each new surface must be added to every one of them** — the hint
walk, the visibility check, the letter assignment. A miss is not a compile error; it is a
surface that is painted but dead to the mouse, with nothing reporting the gap. One input, one
delivery path. A second path over the same input cannot stay consistent with the first.

### Not: a host-maintained list of surfaces

A cleaner-looking version collapses those functions into one struct — `{ root, rect, z, modal }`
— and one dispatch that loops a `Vec<Surface>`. The duplication goes away, but a **list the
host maintains** remains: to make a widget receive input a developer must know it needs to
become a `Surface` and must enrol it in that list. That enrolment step is a registry, and a
registry is the signature of a missing API — the thing that should have been automatic is
instead a rule every caller has to remember. The framework already walks children
automatically. The correct move is to put the widget **in the tree**, where the existing walk
finds it, not into a parallel list the walk does not know about.

### What "one tree" buys

Adding a surface becomes **adding a child**. No new dispatch function, no new match arm, no
new hint-walk case. The behaviour a widget needs — pointer input, keyboard, hint letters,
layout, paint — all come from the same traversal that every other node already gets. The
showcase and the app converge, because neither has any bespoke wiring left in which to differ:
a widget works the same whether it is mounted in a demo, in the chrome, or in a plugin's
overlay.

## Plugin overlays

Once input is a tree walk, a **described** overlay contributed by a plugin is just another
node. It receives pointer input, keyboard, hint letters, layout and paint from the same walk,
with nothing the host has to be taught about it. That is the plumbing half, and it follows
directly from the model above.

The **capability** half is separate and larger: a plugin can only build an overlay out of
widgets that have a declarative form. Several widgets do not have one yet — including
`CardGrid`, which the exposé's cursor is built on — and some (`ChromeRegion`, `Pane`,
`FocusScope`) are host-only by design and never will. Until an overlay of the app's own is
built purely through the described path, "a plugin can add an overlay" describes the intended
architecture, not a demonstrated fact. The described-widget coverage, and one real overlay
rebuilt through it, are the proof.
