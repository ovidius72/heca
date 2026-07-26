# Handoff

**Created:** 2026-07-26
**Reason:** ~20% context left. Everything committed and pushed; PR opened.

## Where the work is

Branch `feat/p017-t1-style-split`. **915 tests green, clippy 0.** Every code change was verified by
the user in the GPU showcase — no visual difference at any step.

## Code shipped — F003/P017 T1, T2, T3 all done

The declarative model could not express things the widgets could do. `Input::placeholder` and
`ScrollRegion`'s second axis existed for months and no description could reach them. That class of
hole is now closed and cannot reopen.

**T1** — `Style` split into two peers, `layout` and `visual`. The user rejected my first shape
(layout flat, visual nested) because it needed one `#[serde(skip)]` line whose deletion would
silently expose colours. Two peers need no attribute at all. Also removed `Style::accent` and
`Style::fg` — declared with hardcoded colour literals, never read anywhere.

**T2** — `realize` holds no layout property list. It merges by name against `Layout`'s own fields,
so a new field is settable with no mapper change. `Length` reads as an author writes it: a bare
number is px, `"auto"`, `"50%"`. The merge lands ON TOP of the constructed widget, so
`ScrollRegion`'s constructor settings survive. A bad value costs only itself.

**T3** — new crate `heca-grid-ui-macros`. `#[props]` generates a widget's property surface from its
own builders. `realize` now names **zero** widget properties.
- **Order is not a concept.** Properties apply after children are attached, which dissolves the only
  real dependency (`Select`/`Tabs` clamping `selected`). `#[prop]` **rejects any argument** at
  compile time, so a sequencing hint cannot be reintroduced widget by widget.
- **Nothing is classified by silence.** A builder must be `#[prop]` or `#[host_only("reason")]`;
  neither fails the build. 168 builders classified: 93 settable, 75 not, each with its reason.
- A **drift guard** reads the widget sources and fails when a widget has no surface. It found nine
  widgets my own annotation pass had missed, plus a stale entry in its own exceptions list.

## Rules the user changed — do not restore the old ones

**Appearance is a property (2026-07-26).** The theme gives the default; code may override it with a
string. This reversed a SETTLED rule that appeared in three places, all now updated: `AGENTS.md`
line 721 and its settled table, `docs/widget-architecture.md` §5, and the chrome plan's rule C.
Eight builders are marked `host_only("colour — reachable once F003/P017/T7 makes appearance
overridable")` and flip to `#[prop]` in that task.

**Plugins may write functions.** You send a reference, not a function — which is what `Intent`
already is. A TypeScript plugin kit stores the closure and puts a reference on the wire. Nothing
about the boundary changes.

**The closed widget list does not require a fixed enum.** It means plugins cannot invent widgets. A
host-filled registry satisfies it in one step instead of three edits in two crates. I told the user
this broke a rule; I was wrong, and the correction is recorded.

## Documentation: 9 files → 1

`docs/chrome-and-ui.md` (1738 lines) is the single design record. Part I the UI model, Part II every
architecture section transferred **verbatim** by script, Part III the planner mapping plus a table
of what in Part II is now out of date.

Deleted, all content transferred first: `pluggable-chrome-plugin-plan.md`, `grid-ui-chrome-plan.md`,
`docs/plugin-authoring.md`, `docs/declarative-ui.md`, `docs/chrome-architecture.md`,
`docs/overlay-design.md` (every phase shipped), `docs/surface-compositor.md`,
`docs/sidebar-provider-modes.md`, `docs/the-grid-ui.md`, `PLAN.md`, `agents-communication-plan.md`.

## Seven phases added — all were designed somewhere and tracked nowhere

- **F003/P018** plugin-10 — bind and list contributed actions. `ActionMeta.default_binding` and
  `describe_all()` both exist; nothing reads either, so a registered action can never fire.
- **F003/P019** plugin-11 — layer content from a `ViewNode` + paint/input through the layer stack.
  `layers.rs` says in its own comments that this lands later.
- **F003/P020** plugin-12 — region display modes, the render-per-mode contract and the collapsed
  rail. The "drop it" work is already done; the future design is preserved in full.
- **F004/P009** gridui-08 — drag a container between regions. `DragSurfaceId` has one variant,
  `DockFrame` has no drag handle.
- **F004/P010** gridui-09 — one border-width control; three paths disagree today.
- Detail appended to **F006/P001** (render.rs split), **F006/P003** (niri parity) and **F006/P004**
  (damage-region) — rescued from PLAN.md.

Checked and found already built, so no phase was created: the shared chrome state layer
(`heca/src/chrome/state.rs`, 1158 lines).

## Still overlapping, not touched

- `docs/widget-architecture.md` (176 lines) says the same as Part I. Its contradicting styling row
  is fixed; the duplication is not.
- `docs/widgets.md` — the catalog is unique and must stay; its ViewNode section duplicates Part I.
- `AGENTS.md` duplicates deliberately and argues why in its own text.
- `BACKLOG.md` (2664 lines) competes with the planner for the same job. That is a decision about
  whether it should exist, not a merge.

## Next in F003/P017

T4 is largely absorbed by T3. **T6 and T7 both need a user decision before any code**: T6 the `Row`
name collision (the library's clickable `Row` and the model's flex `Row` are different things), T7
the colour work whose decision is already recorded in the task.

## How this user works — read before starting

- **Plain English. No jargon, and no commentary about your own wording.** They raised this three
  times in one session. Memory `plain-english-preference` lists the banned words.
- They will not accept a summary where the detail was transferred. Move documents **verbatim**.
- They count files. Do not solve a duplication problem by adding a document.
- Rendering, layout and interaction changes are verified by them in the GPU app. Green tests are
  never verification for anything visual.
