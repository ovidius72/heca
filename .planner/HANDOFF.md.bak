# Handoff

Created at: 2026-07-14T14:00:00.000Z
Updated at: 2026-07-14T14:00:00.000Z
Reason: Phase **viewnode-choice is COMPLETE** (choice-1..8, all committed + pushed, PR open). Clean stopping point before the next phase.

## Current focus
- **Feature:** `ebb9ceb0` — 🧩 Pluggable Chrome Architecture
- **Phase just finished:** `1df9c36c` — **viewnode-choice** — ✅ DONE (8/8 tasks + 1 extra)
- **Suggested next phase:** `73c320aa` — **plugin-03: Built-in provider + WorkspacesContainer migration** (3 of its 6 tasks already done)

**Branch:** `feat/viewnode-choice-select`, 10 commits, pushed. **PR open against `main`.** Working tree clean; whole workspace green.

## What was being done
Closed **ViewNode coverage for the entire widget vocabulary** on the model the user locked: *an option is a node with a value and arbitrary content, and options are CHILDREN* — never a `props["options"]` list of strings.

Shipped, in order:
1. **`Select` composes `Choice` children** (the hard one). Its dropdown is an overlay, so taffy *measures* the rows in the trigger's flow and `Select` then **places** them, baking the offset into their bounds — the `ScrollRegion` trick (`shift_subtree`, now shared in `component.rs`). Invariant: **bounds === what is drawn === what is clickable**; picks go through `choice_at()`, never row arithmetic; off-window rows collapse to zero size.
2. **`Tabs` composes `Choice` children** — the sliding underline now tracks the selected child's **real bounds** (all the monospace-metric arithmetic is gone).
3. **realize arms** for `Choice`/`Select`/`Tabs` — and **value→intent**: the widgets emit an index, `realize` maps it back through the options' `value` props, so a `change` intent carries `{"value":"high"}`, never an opaque index that breaks on reorder.
4. **`ItemGroup`/`MarkerGroup`** arms + the **`toggle`** event (carries `args["expanded"]`).
5. **`PropValue::List` + `Grid`** — tracks as CSS-like strings (unknown → `Auto`, never a panic); **placement is a prop on the CHILD** (`area`, or `col`/`row`+spans).
6. **Named child slots** (a **`slot` prop on the child**) → `DockFrame` (header/body), `Item` (leading/trailing), `Toast`. `ViewNode.children` stays one flat vector.
7. **The coverage guard**: `every_widget_kind_realizes_to_a_live_widget_except_the_host_only_ones` walks `WidgetKind::ALL` and fails if a kind produces neither children nor paint.

Plus two user-requested extras that came out of testing the showcase:
- **Grid item alignment on both axes** (`Style::justify_items` / `justify_self` were missing) + the `areas`-template rule.
- **`Label` gains italic / underline / strikethrough**.

## How to resume
1. **Read AGENTS.md** — the ⭐ WIDGET ARCHITECTURE block and its **⛔ SETTLED** table, which this phase *extended*. Do not re-derive: realize coverage is **complete** (test-enforced); options are children; the `slot` prop; `ScrollBar` is host-only.
2. Merge the PR (or work on top of `feat/viewnode-choice-select`).
3. `planner-task-start <id>` **before editing** (the guard blocks edits otherwise), then delete this handoff.
4. Next phase: **plugin-03** (`73c320aa`) — its remaining tasks are *Implement `build_contribution` render seam (T3)*, *Bridge sidebar-nav selection into shared state*, and *Cutover sidebar to provider-mounted container (T4)*. It migrates the hardcoded sidebar façade (`sidebar/model.rs`) onto the first built-in provider (`WorkspacesContainerProvider`) mounted via `ChromeHost` + `ContainerContribution` — proving the pluggable chrome hosts a real container, not just a `TestProvider`.

## Files touched
`heca-grid-ui/src/`: `widgets/{select,tabs,choice,label,grid,item,dock_frame,badge,tag,badge_button,scroll_region}.rs`, `component.rs` (`with_translate`, `text_summary`, `shift_subtree`), `scene.rs` (`TextStyle`), `style.rs` + `builders.rs` (`align_self`, `justify_items`, `justify_self`), `lib.rs`, `tests/phase_a.rs`.
`heca/src/chrome/`: `view.rs` (`WidgetKind::Choice`, `PropValue::List`, `WidgetKind::ALL`), `realize.rs` (all the arms + the guard), `mod.rs`.
`heca-renderer/src/`: `text.rs` (italic through `queue_text_in_box`), `scene.rs` (bridge); `examples/showcase.rs`.
Docs: `docs/widgets.md` (heavily), `docs/plugin-authoring.md`, `AGENTS.md`.

## Blockers
None. `cargo test --workspace` green: **heca 354**, heca-grid-ui **142** + 71 + 1, heca-core 89. `cargo clippy --workspace --all-targets --all-features` 0 warnings (the transitive `block v0.1.6` note is pre-existing).

## Next steps
- Merge the PR, then start **plugin-03**.
- Two known gaps, recorded but **not** done (the user deferred them): **`Label` truncation / ellipsis** (a long label currently overflows its box — several widgets would want this; it is the load-bearing one), plus wrapping / multi-line, letter-spacing, case transforms. Also still open: a **typed builder SDK over `ViewNode`**.

## Recent decisions
- **`Select::option(Choice)` / `Tabs::tab(Choice)` are TYPED — `Parent` is deliberately NOT implemented** on them. They keep the option's state signals to drive selection in place; a `Box<dyn Component>` would erase them. That is why `realize` has a typed `realize_choice()`.
- **`PaintCx::with_translate`** — paint a subtree somewhere else. Used only so a `Select`'s trigger can echo the chosen option's content while that option is away in the open list (a component is laid out in exactly one place). **What it draws is NOT interactive** (no bounds ⇒ not hit-tested/focusable); never use it to *move* a widget — that is `shift_subtree` + `on_layout`, which keeps bounds honest.
- **`Component::text_summary()`** — the accessible name of composed content (first descendant text wins). `Label`/`Badge`/`Tag`/`BadgeButton` supply it. NB: on an `ItemGroup` the default picks the header's **chevron glyph**, not the label — it is only meaningful for content-only subtrees.
- **`PaintCx::text` takes a `TextStyle`** (`{bold, italic}`) instead of a bare `bold: bool`, so the next text attribute doesn't break the signature again. Italic is a **synthesized oblique** (glyph shear): Geist Mono has no italic face, and a real italic request would substitute a *proportional* fallback and break the monospace advances.
- **Underline/strikethrough are DECORATIONS the widget draws** (rects), not font attributes — which is why they cost no renderer change. They follow the **text run**, not the box.
- **A widget whose state is a live host signal is HOST-ONLY** (`ScrollBar`; also individual builders like `DockFrame::rail`). Plugins use `Scroll`.
- **Grid:** a grid item is pinned to the **top-left** of its cell by default (`Stretch` + an explicit size = nothing to stretch). `align` is the vertical knob, **`justify_items` the horizontal one** — `justify` is a trap (it is `justify-content`, which moves the whole track set). And the **`areas` template defines the structure**; `rows`/`columns` only *size* the tracks it implies — an extra line in the template silently creates an implicit row.

## Reminder
- Gates every task: `cargo clippy --workspace --all-targets --all-features` (0 warnings), `cargo test -p heca-grid-ui -p heca`, showcase builds. **Do NOT run `cargo fmt`.** Load `~/.agents/skills/rust/SKILL.md` and review before committing. **No commit until the user has tested.**
- **This project uses the PLANNER, not GSD** (user, explicit 2026-07-14). Open the task before editing, keep its status current, delete this handoff when work resumes.
- The showcase is the living reference: `cargo run -p heca-renderer --example showcase` (Select LEVEL dropdown, the composed Tabs strip, both Grid rows, the Label attribute row, the Choice row). The **declarative** path has no showcase surface — `realize` is app-side and `heca-renderer` does not depend on `heca` — so it is covered by the realize tests instead.
