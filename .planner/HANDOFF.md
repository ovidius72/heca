# Handoff

Created at: 2026-07-13T21:00:00.000Z
Updated at: 2026-07-13T21:00:00.000Z
Reason: End of session. `choice-1` is code-complete and green but **uncommitted, awaiting the user's runtime test**. `choice-2` is the biggest task in the phase and should start in a fresh session.

## Current focus
- **Feature:** `ebb9ceb0-ea12-4374-af6e-12aa4256bcc3` — 🧩 Pluggable Chrome Architecture
- **Phase:** `1df9c36c-5c29-4288-9fe1-7a6e9c72179b` — **viewnode-choice**: Choice primitive + compose the remaining widgets (full ViewNode coverage) — 8 tasks (choice-1..8), created this session with the user
- **Task:** `d19bdbe7-a355-4c64-923e-d3f48af6db19` — **choice-1** (`Choice` widget) — **in-progress, code complete, awaiting user test → commit**
- **Next task:** `6ec99b20-bdf1-49d3-abc6-f5ba07ae39bf` — **choice-2** (refactor `Select` onto `Choice` children) — the fiddly one

**Branch:** `feat/viewnode-button-composition` (off `feat/base-focusable` = `origin/main` + the `36936d1 alined planner` commit that main lacks).
**PR #237 → main is OPEN** and carries the two committed pieces of this session: `07d4c1a` (Button/Item composition) + `cbada04` (docs).

## What was being done
Two things landed and one is in flight.

**1. `viewnode-task-2` + `viewnode-task-3` — DONE, in PR #237.**
`Button` and `Item` now **compose their content from child components** instead of hand-drawing it (AGENTS.md ⭐ WIDGET ARCHITECTURE). `Button` holds arbitrary children (any tree, any depth); the convenience forms are **sugar that builds those same children**. Three foundations made it work, and they are the load-bearing pieces every later task depends on:
- **Content color is INHERITED, not assigned** — `PaintCx::with_content_color(color, |cx| …)`; unstyled `Label`/`Icon` fall back to it (then the theme foreground). A control cannot set its children's colors (they are `impl Component`; the `Theme` only exists inside `paint`), so it publishes one state-derived color per frame and children pull it → composed content animates with hover / fades when disabled with **zero** per-child wiring. An explicit `.color()` or an intrinsic one (`Badge::danger`) still wins.
- **The size variant CASCADES down the tree** (like the base font already did) — `Style::size_explicit` + `Style::set_size()`; `layout.rs::build(c, inherited_size)`. Before this, a `Small` button's `Label` stayed `Normal`. `IconButton`'s manual copy-into-child hack was deleted.
- **A control is ONE Tab stop** — `Base.focus_barrier`; `focus.rs::for_each_focusable` no longer descends past it (a focusable child would otherwise be Tab-reachable but click-dead).
Also: `realize_button` now **attaches `node.children`** (it silently DROPPED them before, so a declarative `Button(Icon+Label)` rendered bare); `Label.bold` became a `Signal`; AGENTS.md gained the rule that widget docs must be exhaustive with examples for **both** audiences.

**2. `choice-1` — code complete, UNCOMMITTED.**
New `Choice` widget: a selectable container carrying a **value** and composing **arbitrary content**. It is the option primitive `Select`/`Tabs` (later `ContextMenu`/`CommandPalette`) will mount.

## Files touched (uncommitted — `choice-1`)
- `heca-grid-ui/src/widgets/choice.rs` — **NEW**. `Choice::new(value)` / `Choice::labeled(value, label)` (sugar → one `Label` child); `impl Parent + LayoutExt`; `.selected(bool)` / `.state()` / `.hovered()` / `.value()` / `.on_activate(f)`. Chrome only (selected pill / hover tint / press flash / focus ring — the **same interaction tokens as `Item`**); publishes its state color via `with_content_color` (selected → accent, else foreground); `focus_barrier = true`; hugs its content (`Auto` + `padding = 8.0 * pad_scale`, `gap = font * 0.5`); `tick` recurses. Plus **`pub fn choice_at(&[Box<dyn Component>], Point) -> Option<usize>`** — resolves a pick from the children's **real bounds** (exists so choice-2/3 cannot fall back to row arithmetic).
- `heca-grid-ui/src/widgets/mod.rs`, `heca-grid-ui/src/lib.rs` — exports (`Choice`, `choice_at`) + prelude.
- `heca-grid-ui/tests/phase_a.rs` — +4 tests (value independent of content; state color inherited + explicit child color wins; one Tab stop with an interactive child inside; `choice_at` picks from real bounds).
- `heca-renderer/examples/showcase.rs` — new "Choice — a value + composed content" row.
- `docs/widgets.md` — full `Choice` entry **+ the `Choice` vs `Item` comparison table and rule of thumb** (the guidance the user asked for) + TOC.

**DECISION RECORDED: the `Choice` value is a plain `String`.** `heca-grid-ui` never depends on `heca` (AGENTS.md ⛔ SETTLED), so it cannot store `PropValue`; `realize` converts at the boundary (choice-4). A grid-ui-local `ChoiceValue{Text,Int}` enum was rejected — nothing needs numeric fidelity and it would duplicate `PropValue` in the library.

## How to resume
1. **Read AGENTS.md** — especially the ⭐ WIDGET ARCHITECTURE block and its **⛔ SETTLED** table. Do **not** re-propose `Button::new(ViewNode)` or moving `ViewNode` into `heca-grid-ui`; both are impossible and have been re-litigated too many times.
2. **Ask the user whether they tested the showcase** (`cargo run -p heca-renderer --example showcase` → the "Choice" row: hover them; the selected MEDIUM option's icon *and* label must tint together). AGENTS.md gate: **no commit until the user has tested.**
3. If approved: commit `choice-1` (all files above), push to `feat/viewnode-button-composition` (updates PR #237, or open a separate PR if the user prefers), then `planner-task-complete d19bdbe7-a355-4c64-923e-d3f48af6db19`.
4. Start **choice-2**: `planner-task-start 6ec99b20-bdf1-49d3-abc6-f5ba07ae39bf`. Its task description has the full brief.

## Blockers
- **`choice-1` is blocked on the user's runtime test** (AGENTS.md: no commit until the user has tested).
- Nothing else. All gates are green.

## Next steps
- **choice-2 (the hard one):** refactor `Select` (`heca-grid-ui/src/widgets/select.rs:77`) to compose `Choice` children. Today it stores `options: Vec<String>`, paints its dropdown rows itself and **hit-tests them by arithmetic**. The rows must become children whose **bounds are real** — but the panel is an **overlay** (`cx.with_overlay`), so the children must still be laid out at the panel's position, since hit-testing, the KeyHint picker and `FocusManager` all read bounds. Documented fallback: lay them out in the trigger's flow and **bake the panel offset into their bounds**, exactly as `ScrollRegion` bakes `-scroll_offset` — the invariant is **bounds === what is drawn**. Preserve: `overlay_active()`, flip/cap against the viewport, internal scroll + scrollbar, wheel, the `MenuUp`/`MenuDown`/`Activate`/`Dismiss` intents, and the `select-change` payload. `Select::new([..])` must stay as sugar so every existing call site compiles.
- Then choice-3 (`Tabs`), choice-4 (realize `Choice`/`Select`/`Tabs` + **value→intent args**), choice-5 (`ItemGroup`/`MarkerGroup`), choice-6 (`PropValue::List` + `Grid`), choice-7 (named `slot` prop → `DockFrame`/`Item`/`Toast`), choice-8 (close the vocabulary: `ScrollBar` host-only, docs sweep, coverage-guard test).

## Recent decisions
- **The option model (user, 2026-07-13):** an option is **a node with a value and arbitrary content**, and options are **children** — NOT a `props["options"]` list of strings. Hence the `Choice` primitive. Named `Choice` because a widget named `Option` would shadow `std::Option` at every call site.
- **`Choice` and `Item` COEXIST** (user): `Item` = a *row* (leading/label/trailing, fixed height, `ActiveMarker`); `Choice` = *any content + a value*. Documented with a comparison table + rule of thumb; choice-8 cross-links it.
- **`ScrollBar` is HOST-ONLY, not plugin-authorable**: its state is live host signals (`content_extent`/`viewport_extent`/`offset`) which static serializable data cannot drive. General principle to apply to any future widget of that shape: *a widget whose state is a live host signal is host-only.* Plugins use `Scroll`.
- **Grid tracks are CSS-like strings** (`"1fr"`, `"22px"`, `"auto"`/`"min"`/`"max"`) parsed by `realize`; unknown tokens degrade to `Auto` (realize stays total for untrusted input). No new schema invented.
- **Named slots via a `slot` prop on the child node** — no change to the `ViewNode` shape.
- **`viewnode-task-1` was CANCELED as superseded** by this phase: it assumed the missing kinds only needed structured props, but `Select`/`Tabs` hand-draw their content and need a *widget refactor*.

## Reminder
- Gates every task: `cargo clippy --workspace --all-targets --all-features` (0 warnings; the transitive `block v0.1.6` future-incompat note is pre-existing and expected), `cargo test -p heca-grid-ui -p heca`, showcase builds. **Do NOT run `cargo fmt`.** Load `~/.agents/skills/rust/SKILL.md` and review before committing.
- `heca-core::terminal_backend_nvim_tui_produces_non_default_background_cells` flakes under workspace-parallel load; it passes in isolation and is unrelated to this work.
- **Planner discipline (user, explicit):** open the task before editing, keep its status current, and delete this handoff once work is resumed.
