# WS-A Resume — grid-ui chrome integration + appearance

**Branch:** `grid-ui-chrome-integration` · **HEAD:** `36876fe` (F5a) · working tree clean, build green.
**Plan:** `grid-ui-integration-and-appearance-plan.md` (reorganized foundation-first).
**Memory (read these first):** `chrome-integration-realignment`, `heca-transparency-architecture`, `heca-sidebar-design-spec`, `build-future-proof-no-half-measures`, `grid-ui-no-cargo-fmt`, `fix-warnings-even-preexisting`.

## Workflow rules (do not violate)
- One Sonnet subagent per small task; **review every result** (read the diff + re-run build/clippy/tests yourself); ask it to fix.
- **Never `cargo fmt`**. Fix ALL warnings (even pre-existing). Local commits per task; one PR later.
- `transparency` on/off needs an app **restart** (winit creation-time); the *amount* + theme live-reload via `prefix+Shift+r`.

## DONE (committed)
- **F1** appearance config — `[appearance]` amounts: `transparency`/`blur`/`vibrancy` in `heca-config/src/appearance.rs` (helpers `opacity()`, `is_transparent()`, `chrome_opacity()`, `blur_radius()`, `os_vibrancy()`).
- **F2a** app renders through `Compositor` (offscreen → blit). **F2b** window/surface transparency. **Premultiplied alpha** unified across primitive+text renderers (THE wash-out fix). **F2c** macOS vibrancy as a **sibling NSVisualEffectView behind** winit's content view (objc2; reparenting panics winit). Frosted chrome via `chrome_opacity()` (clear fully transparent + chrome panels at `opacity()*0.7`). **Transparency+vibrancy WORKS** (`vibrancy="sidebar"` frosts; `"none"`=raw desktop).
- **F5a** `crate::chrome::build_sidebar_shell` — a GRIDCN-nav placeholder shell. **⚠️ SUPERSEDED** (wrong content — see below). Not wired.

## NEXT — F5: the real sidebar (see `heca-sidebar-design-spec` memory)
Build the LEFT sidebar like the **showcase explorer** (`heca-renderer/examples/showcase.rs` ~L589-712): a **floating, bordered, sticky-left** frosted panel hosting the workspace tree:
- **Workspace** → `DockFrame` (titled, collapsible, count `Badge`) like EXPLORER.
- **Column** → `ItemGroup` (like src/tests).
- **Pane** → rich `Row` card (leading `Icon` + name). Icon = `Glyph::Terminal` now but **dynamic-ready** (function of the app running in the pane). Future: multi-segment `Tag` for cwd/git-branch/process (seam only for now).
- Drop the F5a GRIDCN-nav placeholder content; keep/replace the frosted bordered shell.

### F5 wiring approach (from the reverted A3b — reuse it)
1. `chrome.rs`: refactor `build_chrome_scene` → a full-window `chrome_scene(w,h,tab_h,status_h,left_w,status,side_bg,fg,theme,sidebar: Option<...>)`: column[ tab band (transparent) · row[ sidebar(left_w) · content spacer ] · status row ]. Bridge `GuiTheme` (surface=side_bg@chrome_opacity, accent/foreground/border from app theme). Build sidebar from `state.sidebar_tree` (SidebarTree) when `left_w >= SIDEBAR_EXPANDED_THRESHOLD`.
2. `render.rs::render_frame`: guard the hand-drawn LEFT sidebar to **collapsed-only** (`if !left_expanded { bg+divider+render_sidebar_collapsed(...) }`); delete the `render_sidebar_expanded` call. If that orphans `render_sidebar_expanded` + helpers as dead code, remove them deliberately (compiler-guarded) — they were ~18 expanded-only fns last time.
3. Right "Details" sidebar untouched. Input/interaction for the new sidebar = later slice (showcase Row/FocusManager hit feature).

## Also pending: F3 (in-app blur, portable — `blur` is wired but inert), F4 (shared chrome state).

## Sidebar grill answers (2026-06-14, ongoing)
- **Q1 framing:** Sidebar is **fixed + bordered** (NOT floating — small padding/inset around it gives a float-like look). One outer bordered panel hosting workspace **Docks** separated by **dividers** (NO per-dock internal border). Follow the **showcase** style (Option 1: one panel, workspaces as DockFrames inside → column ItemGroups → pane cards).
- Workspace **count badge = total panes** it contains.
- **Keybindings must keep working** — there are many (sidebar mode + global mode). The showcase **`p`** keybinding is the model for invoking **hints** (KeyHint overlay) to peek/jump panes/ws/cols (replacing the current one).
- **Q2 action controls:** Keep +w/+c/+p/– actions but as grid-ui **`Button` (WidgetSize::Small)** with a proper **icon or letter** label (like now), each with a **`Tooltip`**, placement ~as today (per header/row). **Preserve keybindings** for all.
- **Q3 collapsed rail:** Collapse → narrow **icon rail showing ws + cols + panes** (all peekable, not just panes). Operations are NOT just jump — there is **select / swap / move(take)**. Grounding (verified in code):
  - `InputMode` (`heca/src/app_state.rs`): `PaneSelect { candidates }`, `PaneSwap { candidates, .. }`, `PaneTake { candidates, .. }` — each carries `Vec<(char, PaneId)>`. `InputMode::candidates()` exposes them.
  - `heca/src/app/selection.rs::collect_all_pane_candidates(session) -> Vec<(char,PaneId)>` builds the letter→pane map (PANE_CANDIDATE_LIMIT).
  - `heca/src/sidebar/render.rs::render_sidebar_collapsed` already renders ws activity bars + `render_collapsed_columns` + candidate letters; `input.rs` also has directional SwapLeft/Right etc.
  - **grid-ui mapping:** `RailCell` per pane + the **`KeyHint`** overlay (signal `Signal<Option<String>>`) for the pick letters during select/swap/take — app feeds candidates from `collect_all_pane_candidates`. See memory `grid-ui-chrome-followups` (enumerate-rail design settled).

## Sidebar grill — OPEN questions for next session
- Header/toolbar at top of the sidebar panel? (title? the +w new-workspace button location?)
- Active/selected highlight: active pane + active workspace/column styling.
- Exact divider style between workspace docks; padding/inset values.
- How `Tooltip` + the small action `Button`s sit in dock/group headers vs rows.
- Wiring the existing sidebar-mode + global keybindings to the new grid-ui sidebar (input slice — reuse showcase `FocusManager::dispatch` + KeyHint).
