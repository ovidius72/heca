# Phase 1: The Shell — Context

**Gathered:** 2026-05-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Open a native OS window and render styled text, rectangles, and borders using `wgpu` and `cosmic-text`. Establish the config and theme system. Prove the GPU compositor concept by rendering a mock tiling layout.

</domain>

<decisions>
## Implementation Decisions

### Visual Target
- **D-01:** Phase 1 renders a **mock tiling layout** — multiple colored rectangles arranged in splits, each with a text label (e.g., "Pane 1", "Pane 2"). This validates the compositor architecture immediately rather than just proving text rendering.

### Config Organization
- **D-02:** Config uses a **two-file system**:
  - `config.toml` — keybindings, general settings, reference to active theme
  - `themes/<name>.toml` — separate theme files for colors, fonts, border radius, shadows

### Default Theme
- **D-03:** Default colorscheme is **Catppuccin Mocha**. Provide at least one light variant (Catppuccin Latte) as a second bundled theme.

### Frame Loop
- **D-04:** **Event-driven redraw** — only redraw when window events, input events, or internal state changes occur. No continuous 60fps loop. Damage tracking for region-level optimization is a v4 concern; v1 simply avoids redraw when nothing changed.

### the agent's Discretion
- Exact `wgpu` surface configuration (present mode, format)
- Font fallback chain (system monospace → bundled JetBrains Mono Nerd Font)
- `config.toml` exact section names and nesting depth
- Mock layout arrangement (specific split ratios, number of mock panes)

</decisions>

<specifics>
## Specific Ideas

- Mock layout should look like a real tiling WM: panes with borders, titles, and distinct background colors per pane
- Text should render crisply at various sizes (pane titles, status text)
- Window should be resizable and layout should reflow on resize
</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Requirements
- `.planning/REQUIREMENTS.md` — COMP-01 through COMP-04, COMP-10, CONF-01, CONF-03, CONF-04
- `.planning/RESEARCH.md` — Stack choices (wgpu, cosmic-text, winit)
- `.planning/ROADMAP.md` — Phase 1 success criteria

### External Docs
- No external specs — requirements fully captured in decisions above

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — greenfield project

### Established Patterns
- None — greenfield project

### Integration Points
- None — first phase

</code_context>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-the-shell*
*Context gathered: 2026-05-29*
