# Handoff — fix `bracket_frame` (brackets render as a full border)

> Written 2026-06-23. A **pre-existing regression on `main`** (came in with the
> `bracket_frame` rewrite merged from origin/main — NOT the context-menu work).
> Affects every bracketed widget. Fix on its **own branch off `main`**.

## Symptom (visually confirmed)
- The `Surface` demo's **"Bracketed"** variant renders an identical **full rounded
  border** to "Bordered" — no corner-bracket reticle.
- The showcase **DASHBOARD `Pane`** (a `Pane::new()` at `heca-renderer/examples/showcase.rs:635`)
  shows a full/hairline frame instead of brackets.
- I.e. anything drawn via `PaintCx::bracket_frame` looks like a plain border.

## Root cause — `heca-grid-ui/src/component.rs` `bracket_frame` (lines 787–844)
The renderer's bracket primitive (`heca-renderer/src/scene.rs:180 draw_brackets`) is
**correct but only draws SQUARE 90° corner arms** (verified). To get *rounded*
brackets, `bracket_frame` was rewritten to fake them:
1. draw a **full rounded accent border** (`self.rect(b, …, Border{width: bracket_width}, radius)`, lines 818–824), then
2. overlay "cover" rects (fill/background at `BRACKET_STRAIGHT_DIM`) on each edge's
   **straight midsection** to dim it, leaving bright rounded corners + short arms
   (lines 826–843).

**Step 2 has no visible effect**, so only step 1 (the full border) shows.

## Key diagnostic — it's NOT the alpha
`BRACKET_STRAIGHT_DIM = 178` (~0.70) — a substantial cover. So a 70%-opaque cover
that produced *no* visible dimming means the cover rects **don't overlap the drawn
border line** → the bug is **geometry / z-order alignment**, not opacity.

Constants (component.rs:425–438): `BRACKET_ARM_LEN=12.0`, `BRACKET_WIDTH_MUL=2.0`,
`BRACKET_STRAIGHT_DIM=178`, `BRACKET_HAIRLINE_WIDTH=1.0`, `BRACKET_HAIRLINE_ALPHA=80`.
So `bracket_width = border_width * 2`; cover thickness `t = bracket_width + 1`; the
cover top/bottom/left/right rects sit at `y`, `y+h-t`, `x`, `x+w-t`.

## Prime suspect
How the renderer draws a `Border` relative to the rect edge — **inside / centered /
outside**. The cover rects start at the edge (`y`, `x`, …) and extend *inward* by `t`.
If the border is **centered on or outside** the perimeter, its outer half sits at
`< y` (above the top edge), which the inward cover never reaches → border stays
bright. Check `heca-renderer/src/primitive.rs` / `scene.rs` border stroke placement,
then either (a) align the cover rects to the actual border band, or (b) make the
border draw fully inside the rect so the inward cover lands on it.

## Fix options
1. **Align the cover band** to where the border actually renders (likely shift/grow
   the cover to straddle the border, matching the stroke placement). Lowest-risk if
   the border is centered/outside.
2. **Revert `bracket_frame`** to the prior bracket-primitive approach (square corners
   via `draw_brackets`) if rounded brackets aren't worth the overlay hack. Simplest,
   loses the rounded look.
3. Draw rounded brackets directly in the renderer (proper SDF/arc corners) — biggest,
   cleanest long-term.

## How to verify
`cargo run --example showcase -p heca-renderer`, then look at:
- the **Surface "None / Bordered / Bracketed"** row — Bracketed must show corner
  arms + rounded corners, NOT a full border;
- the **DASHBOARD `Pane`** — bracket frame visible.
Iterate visually (this needs eyeballing — that's why it wasn't fixed inline at the
end of a near-full context window).

## Branch / PR context
- Open work lives on `feature/context-menu` (PR for the ContextMenu widget + λ
  prefix display). This bracket bug is **separate** — branch off `main`.
- After fixing, remove this file.
