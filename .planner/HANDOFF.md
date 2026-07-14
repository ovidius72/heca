# Handoff

Created at: 2026-07-14T14:00:00.000Z
Updated at: 2026-07-14T14:20:00.000Z
Reason: Phase **viewnode-choice is COMPLETE** (choice-1..8, committed + pushed, **PR #239** open). Clean stopping point.

## Current focus
- **Feature:** `ebb9ceb0` — **F003** 🧩 Pluggable Chrome Architecture
- **Phase just finished:** `1df9c36c` — **viewnode-choice** — ✅ DONE (8/8 tasks + 1 extra)
- **Next:** pick between the two candidate moves below (they differ in kind).

**Branch:** `feat/viewnode-choice-select`, pushed. **PR #239 open against `main`.** Working tree clean; workspace green.

## What was being done
Closed **ViewNode coverage for the entire widget vocabulary** on the model the user locked: *an option is a node with a value and arbitrary content, and options are CHILDREN* — never a `props["options"]` list of strings.

1. **`Select` composes `Choice` children** (the hard one). Its dropdown is an overlay, so taffy *measures* the rows in the trigger's flow and `Select` then **places** them, baking the offset into their bounds — the `ScrollRegion` trick (`shift_subtree`, now shared in `component.rs`). Invariant: **bounds === what is drawn === what is clickable**; picks go through `choice_at()`, never row arithmetic.
2. **`Tabs` composes `Choice` children** — the sliding underline tracks the selected child's **real bounds** (all monospace-metric arithmetic gone).
3. **realize arms** for `Choice`/`Select`/`Tabs` + **value→intent**: the widgets emit an index; `realize` maps it back through the options' `value` props, so a `change` intent carries `{"value":"high"}`, never an index that breaks on reorder.
4. **`ItemGroup`/`MarkerGroup`** arms + the **`toggle`** event (carries `args["expanded"]`).
5. **`PropValue::List` + `Grid`** — tracks as CSS-like strings (unknown → `Auto`, never a panic); **placement is a prop on the CHILD**.
6. **Named child slots** (a **`slot` prop on the child**) → `DockFrame` (header/body), `Item` (leading/trailing), `Toast`. `ViewNode.children` stays one flat vector.
7. **The coverage guard**: `every_widget_kind_realizes_to_a_live_widget_except_the_host_only_ones` walks `WidgetKind::ALL` and fails if a kind produces neither children nor paint.

Plus two user-requested extras from testing the showcase: **Grid item alignment on both axes** (`justify_items`/`justify_self` were missing) + the `areas`-template rule; and **`Label` italic / underline / strikethrough**.

## How to resume
1. **Read AGENTS.md** — the ⭐ WIDGET ARCHITECTURE block and its **⛔ SETTLED** table, which this phase *extended*. Do not re-derive: realize coverage is **complete** (test-enforced); options are children; the `slot` prop; `ScrollBar` is host-only.
2. Merge PR #239 (or work on top of `feat/viewnode-choice-select`).
3. `planner-task-start <id>` **before editing** (the guard blocks edits otherwise), then delete this handoff.
4. Pick one of the two candidate next moves below.

## The two candidate next moves

### A) Finish **F003/P012 — context-menu** (2 loose ends on a feature that already works by mouse)

**F003/P012/T008** (`bc9ca992`) — *"keyboard navigation of an open context menu does nothing"*. **PROBABLY ALREADY FIXED — VERIFY BEFORE DEBUGGING.** The note was rescued **verbatim** from the old root `HANDOFF.md` on 2026-07-13 and filed without re-testing, but the work that would have fixed it **predates the rescue**:
- `769d7cd` feat(menu-nav): one configurable key set for overlay list/menu navigation — **2026-07-11**
- `e5753c6` refactor(widget-keys-config): unify to `WidgetIntent` + host-owned `Keymap` + `[keys.widgets]` — **2026-07-12**

All four suspects the note named check out in code today: `[keys.widgets]` binds `menu_up = ["ArrowUp","Ctrl+k"]` / `menu_down = ["ArrowDown","Ctrl+j"]` (`keybindings.default.toml:224-225`); `build_widget_keymap` consumes exactly those names (`heca/src/app/registry.rs:145-146`); the overlay branch dispatches through it (`heca/src/app/events.rs:156`); `ContextMenu::event` handles `MenuUp`/`MenuDown`/`Activate`/`Dismiss` (`heca-grid-ui/src/widgets/context_menu.rs:469-484`).

**Do NOT close it on that reading** — the note's whole point is that the trace looked correct *last time too*, and a fix attempted from the code alone already failed once. **One runtime check settles it:** open a context menu (right-click, or `prefix+>`), press `Ctrl+j` / `Ctrl+k` / arrows. Highlight moves ⇒ mark T008 done (already fixed by the menu-nav / widget-keys work). It doesn't ⇒ instrument the four trace points above and find which one doesn't fire.

**F003/P012/T005** (`3724e701`) — plugin `Contribution::ContextMenu`. Design **locked** (C1/C2/C3, 2026-07-09); the foundation landed in context-menu-6/7: `ContextMenuRegistry`, `ContextPath` (dotted: `pane`, `sidebar.pane`, `sidebar.column`, `sidebar.workspace`), unified `open_context_menu_for`, `resolve_active_context`. What remains is the plugin-facing contribution: a plugin declares `context_path` + `weight: Vec<i64>` (Dewey — it inserts `[1,1,1]` between `[1,1]` and `[1,2]`) + `build(target)`. It does **NOT** pass mode/origin (host-internal); the host includes its items when that context is active (keyboard) or clicked (mouse). Native closure now; WASM marshalling deferred to plugin-08.

**Suggested order: T008 before T005.** T008 is a user-visible bug (or a stale ticket) on a shipped feature and is cheap to settle; closing it first also makes T005's plugin items keyboard-testable the moment they exist.

### B) Start **F003/P004 — plugin-03: Built-in provider + WorkspacesContainer migration** (`73c320aa`)

Remaining: **T006** *Implement `build_contribution` render seam* (`12dcda61` — the phase text calls it "T3"; the planner number is 6), then **T003** *Bridge sidebar-nav selection into shared state* (`eb07808e`), then **T004** *Cutover sidebar to provider-mounted container* (`d96d1e07`).

It migrates the hardcoded sidebar façade (`heca/src/sidebar/model.rs` → `chrome/mod.rs::build_workspaces_container`), which **bypasses `ChromeHost` entirely**, onto the first built-in provider (`WorkspacesContainerProvider` — skeleton already registered in `app/startup.rs`) mounted via `ChromeHost` + `ContainerContribution`. That is what proves the pluggable chrome hosts a **real** container, not just the `TestProvider` stand-in whose render seam is literally `unimplemented!()`.

**T1 already established the key fact:** the expanded sidebar's workspace tree is *already* a grid-ui `Component` (a `Flex`), so the render seam (`WidgetModel = Box<dyn Component>`) is a **wiring extraction, not a rewrite**. The open detail for T006: `ChromeCtx` exposes only read-only selectors today, but the builder needs `SidebarTree`, `ProgramsConfig`, the theme, the intent emitter, and the per-frame `signals`/`drag`/`hints` registries — resolve with interior mutability on `ChromeCtx` or an extended `build` signature. Estimate ~7–12h for the phase.

## Files touched (this session)
`heca-grid-ui/src/`: `widgets/{select,tabs,choice,label,grid,item,dock_frame,badge,tag,badge_button,scroll_region}.rs`, `component.rs` (`with_translate`, `text_summary`, `shift_subtree`), `scene.rs` (`TextStyle`), `style.rs` + `builders.rs` (`align_self`, `justify_items`, `justify_self`), `lib.rs`, `tests/phase_a.rs`.
`heca/src/chrome/`: `view.rs` (`WidgetKind::Choice`, `PropValue::List`, `WidgetKind::ALL`), `realize.rs` (all arms + the guard), `mod.rs`.
`heca-renderer/src/`: `text.rs` (italic via `queue_text_in_box`), `scene.rs` (bridge); `examples/showcase.rs`.
Docs: `docs/widgets.md` (heavily), `docs/plugin-authoring.md`, `AGENTS.md`.

## Blockers
None. `cargo test --workspace` green: **heca 354**, heca-grid-ui **142** + 71 + 1, heca-core 89. `cargo clippy --workspace --all-targets --all-features` 0 warnings (the transitive `block v0.1.6` note is pre-existing).

## Next steps
- Merge PR #239.
- Then **A** (settle T008, then T005) or **B** (plugin-03 T006).
- Deferred, recorded, **not** done: **`Label` truncation / ellipsis** (a long label overflows its box — the load-bearing one; several widgets want it), plus wrapping / multi-line, letter-spacing, case transforms. Also open: a **typed builder SDK over `ViewNode`**.

## Recent decisions
- **`Select::option(Choice)` / `Tabs::tab(Choice)` are TYPED — `Parent` is deliberately NOT implemented.** They keep the option's state signals to drive selection in place; a `Box<dyn Component>` would erase them. Hence `realize`'s typed `realize_choice()`.
- **`PaintCx::with_translate`** — paint a subtree somewhere else. Used only so a `Select`'s trigger can echo the chosen option's content while that option is away in the open list (a component is laid out in exactly one place). **What it draws is NOT interactive** (no bounds ⇒ not hit-tested/focusable); never use it to *move* a widget — that is `shift_subtree` + `on_layout`, which keeps bounds honest.
- **`Component::text_summary()`** — accessible name of composed content (first descendant text wins). `Label`/`Badge`/`Tag`/`BadgeButton` supply it. NB: on an `ItemGroup` the default picks the header's **chevron glyph**, not the label — it is only meaningful for content-only subtrees.
- **`PaintCx::text` takes a `TextStyle`** (`{bold, italic}`) instead of a bare `bold: bool`, so the next text attribute won't break the signature again. Italic is a **synthesized oblique** (glyph shear): Geist Mono has no italic face, and a real italic request would substitute a *proportional* fallback and break the monospace advances.
- **Underline/strikethrough are DECORATIONS the widget draws** (rects), not font attributes — which is why they cost no renderer change. They follow the **text run**, not the box.
- **A widget whose state is a live host signal is HOST-ONLY** (`ScrollBar`; also individual builders like `DockFrame::rail`). Plugins use `Scroll`.
- **Grid:** an item is pinned to the **top-left** of its cell by default (`Stretch` + an explicit size = nothing to stretch). `align` is the vertical knob, **`justify_items` the horizontal one** — `justify` is a trap (it is `justify-content`, which moves the whole track set). And the **`areas` template defines the structure**; `rows`/`columns` only *size* the tracks it implies — an extra line in the template silently creates an implicit row.

## Reminder
- Gates every task: `cargo clippy --workspace --all-targets --all-features` (0 warnings), `cargo test -p heca-grid-ui -p heca`, showcase builds. **Do NOT run `cargo fmt`.** Load `~/.agents/skills/rust/SKILL.md` and review before committing. **No commit until the user has tested.**
- **This project uses the PLANNER, not GSD** (user, explicit 2026-07-14). Open the task before editing, keep its status current, delete this handoff when work resumes.
- Showcase is the living reference: `cargo run -p heca-renderer --example showcase` (Select LEVEL dropdown, composed Tabs strip, both Grid rows, the Label attribute row, the Choice row). The **declarative** path has no showcase surface — `realize` is app-side and `heca-renderer` does not depend on `heca` — so it is covered by the realize tests.
