# Input architecture — moved

This content now lives in **[`docs/surface-compositor.md` § 0](surface-compositor.md)**, in full.

It was folded in because four documents described overlapping ground — this one,
`surface-compositor.md`, `hint-architecture.md` and `overlay-design.md` — and they were free to
drift. They had already: `covers_content` was honoured by the action router and ignored by the hint
walk, which blanked every `prefix+/` letter in the app for as long as a toast stack was mounted. Two
descriptions of one mechanism is the same defect as two implementations of it.

`surface-compositor.md` is what AGENTS.md already flags as **required reading before adding any
layer, surface, overlay, modal, exposé, or a button on a new surface** — so it is where someone about
to make this mistake is already looking.

What moved there:

| was here | now |
|---|---|
| the one-tree model and the four passes | § 0.8 |
| why a second dispatcher is wrong | § 0.9 |
| why a host-maintained surface list is wrong | § 0.9 |
| what one tree buys | § 0.9 |
| plugin overlays — plumbing vs capability | § 0.10 |

Nothing was summarised away; the sections are transferred whole.

This stub stays because planner entries point at this path. **Do not add new content here** — it goes
in `surface-compositor.md`.
