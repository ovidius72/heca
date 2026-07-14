# Handoff

Created at: 2026-07-14T14:00:00.000Z
Updated at: 2026-07-14T14:30:00.000Z
Reason: Phase **viewnode-choice COMPLETE** (choice-1..8, **PR #239** open). **F003/P012/T008 closed as stale.** Clean stopping point; next task chosen but not started.

## Current focus
- **Feature:** `ebb9ceb0` — **F003** 🧩 Pluggable Chrome Architecture
- **NEXT TASK (decided with the user): `F003/P012/T005` — `3724e701` — plugin `Contribution::ContextMenu`.** Not started; full context below.

**Branch:** `feat/viewnode-choice-select`, pushed. **PR #239 open against `main`.** Workspace green; tree clean.

## What was done this session
**1. Phase `viewnode-choice` (1df9c36c) — DONE, 8/8 + 1 extra.** Closed **ViewNode coverage for the whole widget vocabulary** on the locked model: *an option is a node with a value and arbitrary content, and options are CHILDREN* — never a `props["options"]` list of strings.
- **`Select` composes `Choice` children** (the hard one): the dropdown is an overlay, so taffy *measures* the rows in the trigger's flow and `Select` **places** them, baking the offset into their bounds (`shift_subtree`, now shared in `component.rs` — the `ScrollRegion` trick). Invariant: **bounds === what is drawn === what is clickable**; picks via `choice_at()`, never row arithmetic.
- **`Tabs` composes `Choice` children** — the underline tracks the selected child's **real bounds**.
- **realize arms** for `Choice`/`Select`/`Tabs`/`ItemGroup`/`MarkerGroup`/`Grid`/`DockFrame`/`Item` slots/`Toast`; **value→intent** (the `change` intent carries `{"value":"high"}`, not an index); the **`toggle`** event; **`PropValue::List`**; **named child slots** (a `slot` prop on the *child*).
- **The coverage guard**: `every_widget_kind_realizes_to_a_live_widget_except_the_host_only_ones` walks `WidgetKind::ALL` and fails if a kind produces neither children nor paint.
- Extras from the user's showcase testing: **Grid item alignment on both axes** (`justify_items`/`justify_self` were missing) + the `areas`-template rule; **`Label` italic / underline / strikethrough**.

**2. `F003/P012/T008` (`bc9ca992`) — CLOSED AS STALE, no code change.** "Keyboard nav of an open context menu does nothing" was rescued **verbatim** from the old root `HANDOFF.md` on 2026-07-13 **without re-testing** — but the work that fixes it *predates the rescue*: `769d7cd` (menu-nav, 07-11) and `e5753c6` (widget-keys-config → `WidgetIntent` + host-owned `Keymap` + `[keys.widgets]`, 07-12). Audited all four suspects (bindings at `keybindings.default.toml:224-227`; `build_widget_keymap` at `app/registry.rs:145-146`; overlay dispatch at `app/events.rs:156`; `ContextMenu::event` at `context_menu.rs:469-484`) — all correct. User confirmed it navigates in the running app.
> **Lesson worth keeping:** a bug rescued from a deleted handoff is *a claim about a moment, not a fact about HEAD*. Re-verify before working it.

## How to resume
1. **Read AGENTS.md** — the ⭐ WIDGET ARCHITECTURE block + its **⛔ SETTLED** table (this phase extended it). Do not re-derive: realize coverage is **complete** (test-enforced); options are children; the `slot` prop; `ScrollBar` is host-only.
2. Merge PR #239 (or work on top of `feat/viewnode-choice-select`).
3. `planner-task-start 3724e701-8519-4dfc-bed2-7aa978b96979` **before editing** (the guard blocks edits otherwise), then delete this handoff.

## NEXT TASK — F003/P012/T005: plugin `Contribution::ContextMenu` (`3724e701`)

The **last** task in the context-menu phase (6 of 8 done, T008 just closed).

**Design is LOCKED** (C1/C2/C3, 2026-07-09) and the foundation already landed in context-menu-6/7:
- `heca/src/chrome/context_menu.rs` — `ContextPath` (dotted: `pane`, `sidebar.pane`, `sidebar.column`, `sidebar.workspace`), `ContextTarget`, `ContextMenuProvider { weight: Vec<i64>, build }`, `ContextMenuRegistry::with_builtins()` (4 built-in providers) + `register` + `items_for`, and the unified `open_context_menu_for(state, path, target, anchor, source, origin)`.
- `resolve_active_context(state)` (keyboard) + `PendingContext` + `overlay_origin_mode` (mode-restore: open a menu from the sidebar → close → you are back in `SidebarNav`).

**What is left:** the plugin-facing contribution. A plugin declares **`context_path` + `weight: Vec<i64>` + `build(target)`**; it does **NOT** pass mode/origin (host-internal — the plugin declares *where*, the host decides *when*). The host merges its items with the built-ins whenever that context is active (keyboard `resolve_active_context`) or clicked (mouse hit-test).
- **C2 — weight is a Dewey / fractional index** (`Vec<i64>`): built-ins have stable weights, and a plugin inserts `[1,1,1]` between `[1,1]` and `[1,2]`. Merge by weight, with separators + a tie-break.
- **C3 — native + WASM designed now** (it simplifies the contract): `Contribution::ContextMenu { context_path: String, weight: Vec<i64>, build }`. Native closure for now; **WASM marshalling is deferred to plugin-08**.
- Suggested proof: register a demo plugin menu on `sidebar.workspace` and confirm it appears on right-click *and* via `prefix+>` in `SidebarNav`, ordered by its weight.

## After that
**F003/P004 — plugin-03: Built-in provider + WorkspacesContainer migration** (`73c320aa`). Remaining: **T006** *Implement `build_contribution` render seam* (`12dcda61` — the phase text calls it "T3"; the planner number is 6), then **T003** *Bridge sidebar-nav selection into shared state* (`eb07808e`), then **T004** *Cutover sidebar to provider-mounted container* (`d96d1e07`).

It migrates the hardcoded sidebar façade (`heca/src/sidebar/model.rs` → `chrome/mod.rs::build_workspaces_container`), which **bypasses `ChromeHost` entirely**, onto the first built-in provider (`WorkspacesContainerProvider` — skeleton already registered in `app/startup.rs`) mounted via `ChromeHost` + `ContainerContribution`. That proves the pluggable chrome hosts a **real** container, not just the `TestProvider` whose render seam is literally `unimplemented!()`.
**T1's key finding:** the expanded sidebar's workspace tree is *already* a grid-ui `Component` (a `Flex`), so the render seam (`WidgetModel = Box<dyn Component>`) is a **wiring extraction, not a rewrite**. Open detail for T006: `ChromeCtx` exposes only read-only selectors today, but the builder needs `SidebarTree`, `ProgramsConfig`, the theme, the intent emitter, and the per-frame `signals`/`drag`/`hints` registries — resolve with interior mutability on `ChromeCtx` or an extended `build` signature. Estimate ~7–12h.

## Files touched (this session)
`heca-grid-ui/src/`: `widgets/{select,tabs,choice,label,grid,item,dock_frame,badge,tag,badge_button,scroll_region}.rs`, `component.rs` (`with_translate`, `text_summary`, `shift_subtree`), `scene.rs` (`TextStyle`), `style.rs` + `builders.rs` (`align_self`, `justify_items`, `justify_self`), `lib.rs`, `tests/phase_a.rs`.
`heca/src/chrome/`: `view.rs` (`WidgetKind::Choice`, `PropValue::List`, `WidgetKind::ALL`), `realize.rs` (all arms + the guard), `mod.rs`.
`heca-renderer/src/`: `text.rs`, `scene.rs`; `examples/showcase.rs`. Docs: `docs/widgets.md`, `docs/plugin-authoring.md`, `AGENTS.md`.

## Blockers
None. `cargo test --workspace` green: **heca 354**, heca-grid-ui **142** + 71 + 1, heca-core 89. clippy 0 warnings (the transitive `block v0.1.6` note is pre-existing).

## Next steps
1. Merge PR #239.
2. **F003/P012/T005** (above) — closes the context-menu phase.
3. Then **F003/P004** (plugin-03).
- Deferred, recorded, **not** done: **`Label` truncation / ellipsis** (a long label overflows its box — the load-bearing one), wrapping / multi-line, letter-spacing, case transforms; and a **typed builder SDK over `ViewNode`**.

## Recent decisions
- **`Select::option(Choice)` / `Tabs::tab(Choice)` are TYPED — `Parent` deliberately NOT implemented.** They keep the option's state signals to flip selection in place; a `Box<dyn Component>` would erase them. Hence `realize`'s typed `realize_choice()`.
- **`PaintCx::with_translate`** — paint a subtree somewhere else. Used only so a `Select`'s trigger can echo the chosen option's content while that option is away in the open list. **What it draws is NOT interactive** (no bounds ⇒ not hit-tested/focusable); never use it to *move* a widget — that is `shift_subtree` + `on_layout`, which keeps bounds honest.
- **`Component::text_summary()`** — accessible name of composed content (first descendant text wins); `Label`/`Badge`/`Tag`/`BadgeButton` supply it. NB: on an `ItemGroup` the default picks the header's **chevron glyph**, not the label.
- **`PaintCx::text` takes a `TextStyle`** (`{bold, italic}`), not a bare `bold: bool`. Italic is a **synthesized oblique** (glyph shear): Geist Mono has no italic face, and a real italic request would substitute a *proportional* fallback and break the monospace advances.
- **Underline/strikethrough are DECORATIONS the widget draws** (rects) — which is why they cost no renderer change. They follow the **text run**, not the box.
- **A widget whose state is a live host signal is HOST-ONLY** (`ScrollBar`; also individual builders like `DockFrame::rail`). Plugins use `Scroll`.
- **Grid:** an item is pinned to the **top-left** of its cell by default (`Stretch` + an explicit size = nothing to stretch). `align` is the vertical knob, **`justify_items` the horizontal one** — `justify` is a trap (it is `justify-content`, which moves the whole track set). The **`areas` template defines the structure**; `rows`/`columns` only *size* the tracks it implies — an extra line silently creates an implicit row.

## Reminder
- Gates every task: `cargo clippy --workspace --all-targets --all-features` (0 warnings), `cargo test -p heca-grid-ui -p heca`, showcase builds. **Do NOT run `cargo fmt`.** Load `~/.agents/skills/rust/SKILL.md` and review before committing. **No commit until the user has tested.**
- **This project uses the PLANNER, not GSD** (user, explicit 2026-07-14). Open the task before editing, keep its status current, delete this handoff when work resumes.
- Showcase: `cargo run -p heca-renderer --example showcase` (Select LEVEL dropdown, composed Tabs strip, both Grid rows, the Label attribute row, the Choice row). The **declarative** path has no showcase surface — `realize` is app-side and `heca-renderer` doesn't depend on `heca` — so it is covered by the realize tests.
