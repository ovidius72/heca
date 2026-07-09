# Proposal — gridui-styling-foundation (remaining)

**Branch:** `feat/gridui-styling-foundation` (worktree `/Users/antonio/projects/myvim-gridui-styling`, off `origin/main` @ b849b42 incl. PR #224).
**Status:** PROPOSAL for review — nothing coded yet (design-first, per the heca UI contract).

Closes the last two library gaps that force *callers* to hand-calculate style, breaking
"widget owns styling; caller only picks a token/variant":

1. **Interaction-alpha tokens** — every widget bakes its own hover/active/border/fill alphas.
2. **`WidgetSize` header-size coverage** — pane-header buttons compute px sizes instead of `.size(WidgetSize)`.

---

## Gap A — interaction-alpha tokens

### Evidence
Widgets define private `const *_ALPHA` and inline literals for interaction states. Same
semantic role, scattered values (and some exact duplicates):

| Semantic role      | Widgets (value)                                                         |
|--------------------|-------------------------------------------------------------------------|
| `rest_border`      | button, input, toggle, checkbox, select — **all `150.0`** (exact dup)   |
| `keycap`           | context_menu `200`, key_hint `200` (exact dup)                          |
| `hover_fill`       | icon_button `28`, rail_cell `18`, item `16`, row `16`, toast `22`       |
| `active_fill`      | icon_button `64`, rail_cell `34`, item `30`, row `30`                   |
| `hover_border`     | icon_button `190`                                                       |
| `active_border`    | icon_button `215`, rail_cell `190`, row `180`                           |
| `tonal_fill`       | badge `38`, badge_button `38`, alert `22`, tag `22`, toast `16`         |
| `tonal_border`     | tag `130`, badge_button outline `150`/hover `235`                       |
| `selection`        | input `70`, select hilite `48`                                          |
| `scrim`            | modal `150`, command_palette `140`                                      |
| `panel_border`     | command_palette / context_menu `200`, rows `150`, row-fill `30`         |
| `nav_outline`      | dock_frame `235`, row `220`                                             |
| `nav_wash`         | dock_frame `30`                                                         |
| `unlit`/`disabled` | gauge `40`, spinner min `0.18`                                          |

### Home for the tokens
`heca_theme::Theme` **already** carries semantic alpha tokens:
`icon_secondary_alpha`, `active_wash_alpha`, `card_background_alpha`
(`heca-theme/src/theme.rs:236/242/247`). New interaction alphas belong here, next to
them — one theme-owned source, config/theme overridable, zero new struct plumbing.

### Proposed token set (grouped, `f32` 0–255 to match the existing ones)
```
interaction:
  hover_fill_alpha        active_fill_alpha
  hover_border_alpha      active_border_alpha
  rest_border_alpha       tonal_fill_alpha     tonal_border_alpha
  selection_alpha         scrim_alpha
  keycap_alpha            panel_border_alpha
  nav_outline_alpha       nav_wash_alpha
  disabled_alpha
```

### DECISION 1 — how to reconcile the differing values
The same role has different numbers per widget today, so migration must choose:

- **(1a) Canonical — one value per role.** Simplest theme; but merging e.g. hover_fill
  16↔28 shifts some widgets' look. Risk of subtle visual regressions across many widgets.
- **(1b) No-regression default + unify exact dups (RECOMMENDED).** Introduce the token
  group; unify only the already-identical roles (`rest_border` 150 ×5, `keycap` 200 ×2);
  for the rest, set each token's default to the **current** value so there is *zero* visual
  change, while making it theme-overridable. Full value harmonization becomes an optional
  follow-up once the plumbing exists.

I recommend **1b**: future-proof plumbing now, no surprise visual regressions, harmonize later on purpose.

### Scope note
This is a broad, mechanical migration (~20 widget files). It touches many files but each
edit is small (replace a `const`/literal with `cx.theme().colors.<token>`). Showcase +
`docs/widgets.md` unaffected in behavior; no showcase demo change needed unless a value moves.

---

## Gap B — `WidgetSize` header-size coverage

### Evidence
Pane-header action buttons hand-compute size in the caller
(`heca/src/chrome/mod.rs`):
- `header_icon_size(font) = (font*1.25).max(15.0)` — line 352
- `header_button_cell(font) = header_icon_size + 6.0` — line 358
- `header_buttons_width(...)` layout math — line 363
- Consumed at 721–726: `Icon::new(glyph).size(header_icon_size(font))` + `.cell(cell)`.

`IconButton` already accepts `.size(WidgetSize)` (via `LayoutExt::size` → `remeasure`
scales padding + cascades to the icon). But `WidgetSize` is a **shrink** vocabulary —
`Small 0.8 / Normal 0.9 / Large 1.0` (`style.rs:59`), all ≤ 1.0× the inherited font. The
header wants icons **larger** than body font (1.25×, min 15px). WidgetSize can't express
"grow", so the caller falls back to px math + `.cell()`.

### DECISION 2 — how the library should express header sizing
- **(2a) Extend WidgetSize upward.** Add a variant > 1.0 (e.g. `WidgetSize::Header ≈ 1.25×`
  with a matching `pad_scale`). Header passes `.size(WidgetSize::Header)`, `IconButton`
  derives its square cell from size+padding automatically → drop `header_icon_size` /
  `header_button_cell` / `.cell` from the caller. Con: "Header" is a *role* smuggled into a
  *size* enum; and WidgetSize was explicitly a compaction scale.
- **(2b) Emphasis multiplier, separate from WidgetSize (RECOMMENDED).** Keep WidgetSize a
  compaction scale. Add a small font-relative emphasis concept the header opts into — a
  theme-derived token (like `Spacing`) e.g. `.emphasis(TextScale::Lg)` or an IconButton
  `.header()` preset — so the widget owns the 1.25×/min-15/cell math and the caller stops
  computing px. `header_buttons_width` then asks the widget for its measured width instead
  of re-deriving it.
- **(2c) Leave px math in the app.** Rejected — it's exactly the contract violation we're closing.

I lean **2b** but this one is genuinely open — the cleanest vocabulary depends on whether
"emphasis > base font" should exist anywhere else (top-bar, toasts) or is header-only.

### Scope note
Smaller + localized (IconButton + `chrome/mod.rs` header cluster). Needs a showcase update
if a new sizing API is added (widget-change rule). GUI check owed (header icons unchanged size).

---

## Suggested execution order (after decisions)
1. Gap A first (foundation, isolated to theme + widget paint), one commit family.
2. Gap B second (depends on nothing in A), separate commit.
3. Update `docs/widgets.md` + showcase in the same change where API changes.
4. `cargo build -p heca` + `cargo clippy -p heca -p heca-grid-ui -p heca-config --all-targets`;
   no `cargo fmt`. GUI verify. **No commit until user review.**

## DECISIONS (2026-07-04)
- **A** → token group with **no-regression defaults + unify exact duplicates**.
- **B** → **extend `WidgetSize` upward** (add `WidgetSize::Header ≈ 1.25×`).
- **Harmonization** → do it **in this branch**.

Reconciled: unify exact dups (no regression) **and** harmonize *within a family* the
near-identical values (e.g. row hover 16/16/18 → 16), keeping distinct families (control vs
row vs tonal-fill) separate so no cross-widget regression. Tokens live in a new
`heca_theme::InteractionAlphas` sub-struct (`#[serde(default)]` → TOML presets inherit).

### Token taxonomy (u8 0–255; default = value shown)
Controls: `control_hover_fill 28`, `control_hover_border 190`, `control_active_fill 64`,
`control_active_border 215`, `control_rest_border 150` (unifies button/input/toggle/checkbox/select).
Rows/cells: `row_hover_fill 16` (row/item/rail 16/16/18→16), `row_active_fill 30` (30/30/34→30),
`row_active_border 185` (180/190→185), `row_active_tint 90`, `row_hover_tint 40`.
Nav cursor: `nav_outline 225` (row/dock 220/235→225), `nav_wash 30`.
Tonal fills: `badge_fill 38` (badge/badge_button), `tag_fill 22` (tag/alert), `tag_border 130`,
`toast_tint 16`, `outline_rest 150`, `outline_hover 235`.
Text: `selection 70`, `hilite 48`.
Overlays: `scrim 150` (modal/palette 150/140→150), `keycap 200` (context_menu/key_hint),
`panel_border 200`, `panel_row_border 150`, `panel_row_fill 30`.
Scrollbar: `thumb_rest 90`, `thumb_hover 200`. Dim: `unlit 40`.

**Harmonized (intended small visual shift):** row hover/active/border across row+item+rail;
nav_outline row↔dock; scrim modal↔palette. Everything else keeps its current value.

### Intentionally left widget-local (documented, not silently skipped)
Not palette roles — bespoke single-use or animation-internal values, so not tokenized:
- `spinner.rs` `MIN_ALPHA 0.18` — trailing-dot fade floor (animation curve, like `HOVER_DURATION`).
- `modal.rs` ModalButton hover literals (26/180/200/235) — the fu-13 confirm-dialog button
  treatment; tokenizing risks shifting the just-landed dialog look.
- `toast.rs` action-ghost-button hover literals (40/22/220/150) — self-contained ghost button.
- `row.rs` attention-pulse alphas (`(40.0*attn)`, `(235.0*attn)`) — computed from the pulse curve.

## Gap A — STATUS: DONE (compiles clean, no warnings)
`InteractionAlphas` added to `heca_theme::Theme`; all named `const *_ALPHA` + the overlay
scrim/panel/keycap literals across ~22 widgets migrated to `cx.theme().colors.interaction.*`.
Extra structural tokens surfaced during migration and added: `toggle_on_fill`, `tooltip_border`,
`menu_shortcut`, `menu_shortcut_dim`; marker-group grip folded into `thumb_rest/thumb_hover`
(hover 170→200, intended). `cargo check -p heca-grid-ui` clean.

## Gap B — NEXT (separate commit): `WidgetSize::Header`
Add a `>1.0` variant, make `IconButton` derive its square cell from `WidgetSize` (drop the
caller's `header_icon_size`/`header_button_cell`/`.cell`), expose an intrinsic-cell helper so
`header_buttons_width` stops hand-computing px. Update showcase + `docs/widgets.md` (widget-change rule).

## Open questions for the user
- **Decision 1:** 1a canonical vs **1b no-regression+unify-dups** (recommended)?
- **Decision 2:** 2a WidgetSize::Header vs **2b separate emphasis token** (recommended) vs other?
- Should full alpha value *harmonization* (making the scattered values consistent on
  purpose) be in-scope now, or a tracked follow-up?
