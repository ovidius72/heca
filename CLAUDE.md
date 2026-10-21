# CLAUDE.md

*Agent profile for this project.*

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

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
