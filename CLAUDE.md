# CLAUDE.md

*Agent profile for this project.*

## ⭐ THE SYNTAX — every widget and component is written like this

Declarative and nested, in one expression: the shape of Flutter and SwiftUI, with DOM-shaped events.

```rust
Component::new()
    .prop_a(..)                                    // its own typed properties, as builders
    .prop_b(..)
    .child(
        Grid::new()
            .template_row("auto 1fr")              // the arrangement is a track template
            .child([header, body]),                // children in order, composed to any depth
    )
    .on_click(|ev| ev.dispatch(Intent::new("an_action")))   // behaviour lives on the widget
```

**Never:** find a child by its position (`children[0]`), write a style, a bound or a position by
hand, rebuild a widget to change one of its properties, or put a number in the code. If what you
need cannot be written in this shape, the library is missing a builder — propose it (⭐⭐ RULE
ZERO); never work around it. Real examples: [`docs/layout.md` § The one rule](docs/layout.md),
[`docs/widgets.md` → Grid](docs/widgets.md#grid).

## ⭐ THE EVENTS — one way to listen, everywhere

A state change is heard with one line, the same in heca's own components, in an extension and in a
plugin (WASM included):

```rust
cx.on("pane.cwd.changed", |ev| …)        // a component or provider, in a window
rooms.on("session.changed", |rooms, cx, ev| …)   // an extension's service, on the server
// a plugin: the same on("name", …) through the event bridge (F003/T79)
```

`"*"` listens for every event. Names are dotted (`pane.status.changed`, `session.changed`). Where
changes are rare and a listener wants them together, one event carries a `kind` saying what happened
(`session.changed`: `created` · `renamed` · `terminated` · `restarted` · `deleted`) and the data a
listener needs. Each event is raised from ONE place, and the scripts' stream (`heca rpc --events`)
carries the same event. **Never** a signal, a polling loop, a dedicated trait method or a
per-consumer callback in its place; a change nobody can listen to is not done. (Project decision
f639e662; the contract is P054(F006)'s "Pane runtime state", section 0.1.)

## Read these before writing any code

1. **[`AGENTS.md`](AGENTS.md)** — in full. It is the operating guide for every agent on this
   project, not just Claude: the pre-flight, the widget/component rule, the DOM-shaped event
   system, and Rule Zero. § 0 is the part that gets ignored and the part that gets work rejected.
2. **[`docs/layout.md`](docs/layout.md)** — before arranging anything with more than one part.
   Which container to reach for, how to say a size, a space, a track and an alignment, and the
   traps with the defect each one cost.
3. **[`docs/widgets.md`](docs/widgets.md)** — the catalog. Search it before building any UI; the
   thing you are about to write usually exists.

The planner is the single source of truth for what is being worked on. Load it with
`/planner load` when you need it.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

**graphify-out/ is NOT versioned** — it is a generated artifact (derived from the code's AST) and is git-ignored. On a fresh clone it does not exist, so generate it once with `graphify update .` before the query commands below will work.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
