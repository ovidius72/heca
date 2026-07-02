# Handoff — Sidebar mode regressions + Grid-UI pruning + start of Pluggable Chrome

**Date:** 2026-07-02. **Purpose:** resume this discussion with clean context. Read this fully before acting.

## Where we are (shipped this arc)
- **Font zoom** (whole-app + per-pane) — MERGED (PR #213/#214 area).
- **Terminal inline images + animation** (GIF/APNG/AnimRgba8) + per-image damage — MERGED (PR #214). `terminal-task-07` manual-validation matrix CLOSED (PR #216).
- **Grid-UI pruning** — PR **#217** (docs, open at time of writing): added a verified `PRUNING DECISIONS (2026-07-02)` block to the Grid UI Widget Library section; removed Metric Row; fixed the stale `terminal-05` dependency line. **Note:** the user was (rightly) annoyed I committed/PR'd mid-discussion — do NOT commit/PR while a discussion is still open; keep changes local until explicitly told "we're done." (See memory `dont-code-during-discussion` and the new `update-backlog-before-pr`.)

## Decision taken (this session)
- **Next big work = Pluggable Chrome / Plugin arc** (`plugin-01 → plugin-02 → plugin-03 …`), NOT the App/Chrome grab-bag. Reason: it's the last architectural foundation and it GATES AI-agent integration (`agents-01`) and unblocks sidebar reshape.
- App/Chrome (`app-*`) is largely independent but **soft-collides on the sidebar** with `plugin-03` (which migrates `heca/src/sidebar/` into a `WorkspacesContainerProvider`). So sidebar fixes should be done INSIDE `plugin-03`, not in the App phase, to avoid double rework.
- Parallel agent delegation was explored but **NOT started** (user chose to decide the agent later). Only `app-01` (niri audit, doc) and `app-08` (vibrancy warning) are collision-free enough to parallelize; everything sidebar/registry/input-heavy collides with the plugin arc.

## Sidebar mode — 3 regressions (analysis done, fixes DEFERRED to when we touch those tasks)
Verified against code (file:line). **Do not fix now** — these are captured so the owning tasks carry them.

### #3 — Entering sidebar mode force-expands (should stay collapsed)
- **Root cause (certain):** `handle_sidebar_focus` (`heca/src/handlers.rs:1309`) does `set_left_mode(RegionMode::Expanded)` + `set_left_size(DEFAULT_SIDEBAR_WIDTH)` on every entry, then `input_mode = SidebarNav`.
- **Fix:** enter `SidebarNav` WITHOUT changing `left_mode`/`left_size` — keep whatever it was (collapsed stays collapsed).
- **Owner:** NO existing task → **new task needed** (small, standalone; but deferred per user).

### #2 — Collapsed rail: wrong/incoherent style, no KeyHint, doesn't show pane name
- **Root cause (certain):** the collapsed rail is still the LEGACY hand-drawn path, not grid-ui widgets. `render_sidebar_collapsed` (`heca/src/sidebar/render.rs:30`), labels via `collapsed_pane_label` (`render.rs:353`) which shows only the FIRST CHAR of `pane.name` (or a candidate letter). Not theme-coherent with the expanded side (which uses `DockFrame`/`Card`/`MarkerGroup`). No `KeyHint` overlay.
- **Owner:** `app-task-21` ("migrate collapsed rail to `RailCell`/`ChromeRegion` widgets") — but its scope is TOO NARROW. Must be EXTENDED to include: (a) `KeyHint` in collapsed, (b) show the pane name (tooltip/label, updating on rename), (c) render the sidebar-nav CURSOR/selection in collapsed mode (the collapsed analog of `plugin-task-10a`).

### #1 — Keybindings "don't work" in sidebar mode (regression)
- **Most likely (high confidence): keys FIRE but produce no VISIBLE effect.** This is `plugin-task-10a`.
  - Routing is correct: `InputMode::SidebarNav` → `handle_sidebar_nav_mode` (`heca/src/app/input.rs:603`) resolves via the `"sidebar"` mode keymap (j/k/arrows → `sidebar_up/down`); `handle_sidebar_up/down` move the cursor (`cursor_up`/`cursor_up_collapsed`). So keys resolve + fire.
  - The expanded sidebar only highlights `active_pane` (the "selected look", `chrome/mod.rs:1260`), NOT the sidebar-nav cursor (`sidebar_tree.current_item()`). So `prefix+e` → `j/k` moves the model but nothing changes on screen.
- **Caveat:** a real key-RESOLUTION regression can't be 100% excluded without a runtime check. To distinguish: enter sidebar mode, press j/k, log whether `sidebar_tree.cursor` changes. If it changes → pure rendering (10a). If not → keymap.
- **Owner:** `plugin-task-10a` (in `plugin-03`) already exists for this.

### Interconnection
- #3 and #2 are coupled: if you fix #3 (stay collapsed), the collapsed rail MUST show the nav cursor, or you navigate blind → so #2's scope must include collapsed-cursor rendering.
- **Decision:** do #1 and #2 INSIDE `plugin-03` (sidebar becomes `WorkspacesContainerProvider` anyway — avoid double rework). #3 is isolable but is ALSO deferred (do it when we touch the sidebar in `plugin-03`, or as a quick standalone if convenient). `plugin-task-10a` covers #1; `app-task-21` must be extended for #2 (or folded into `plugin-03`).

## What the user asked for next (this is the resume plan)
1. **Update the backlog THOROUGHLY** — remove wrong things, detail the sidebar decisions above minutely so other agents know EXACTLY the decisions. (Partially started; finish: add the #3 task, extend `app-task-21` scope, add a "Sidebar mode decisions" block. If not done yet in the backlog, DO IT — see next section.)
2. **Then DEFER** all sidebar work until we actually touch those tasks.
3. **Start the Pluggable Chrome plugin plan NOW** to get ahead — begin `plugin-01` (formal contracts, mostly writing `pluggable-chrome-plugin-plan.md` §3.1/§3.4/§2.7) then `plugin-02` (ChromeHost + region hosts). Source of truth: `pluggable-chrome-plugin-plan.md`. Backlog: section "Pluggable Chrome / Plugin", tasks `plugin-task-01…`.
4. This handoff exists so a CLEAN session resumes without re-deriving.

## Backlog edits still TODO (if not completed before context ran out)
- Add a new task for **#3** (forced-expand fix): "Enter `SidebarNav` without forcing `Expanded`/`DEFAULT_SIDEBAR_WIDTH`; keep current collapsed/expanded state. File: `heca/src/handlers.rs` `handle_sidebar_focus`." Place under `app-06` or a new sidebar-fixes note.
- EXTEND `app-task-21` scope: + KeyHint in collapsed, + pane name (tooltip, updates on rename), + render sidebar-nav cursor/selection in collapsed mode. Note it overlaps `plugin-03` (do it there).
- Add a "SIDEBAR MODE — regressions & decisions (2026-07-02)" block (the content of the "Sidebar mode" section above) near `app-06` / `plugin-03` so both owners see it.

## Grid-UI pruning (already in PR #217 — for reference)
CUT (already built/covered): scrollbar token, Pane header + CornerBrackets, HUD Frame, Accordion, Search Input widget (Input + CommandPalette filter already do search). CUT (no consumer): ScrollRegion horizontal scroll (ViewOffset handles horizontal), DrawCommand::Custom. DEFER: bloom, visual-regression tests, nested-region hit-testing. KEEP: status-bar REBUILD (badges/tags/plugin → bottom-bar region), tab-bar for stacked/tabbed layout, multi-select Select (not done), Item DnD reorder, sidebar scroll wiring, pick-a-scrollable-region (SMALL — KeyHint + 6 pick modes already exist), NF icons, showcase audit, gridui-07 debt. REMOVED: Metric Row (stat-card look already in showcase via `card()` helper). Note: `gauge.rs` likely unused (candidate removal); stacked/tabbed pane layout is a heca-core LAYOUT feature (track under App), consumes `tabs.rs`.

## Open PRs
- #217 — Grid-UI pruning (docs). Decide: merge, or hold until sidebar decisions are added too (user may want one consolidated backlog PR).
- Any sidebar-decisions backlog edits from the resume: consider adding to #217's branch (`docs/gridui-pruning`) rather than a new PR, so backlog docs land together. **But only commit/PR when the user says the discussion is done.**
