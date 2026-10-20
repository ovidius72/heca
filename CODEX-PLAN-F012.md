# F012 plan — who does what

Two agents working side by side on F012 (open heca becomes a server + clients, so heca-pro can build on it). This splits the remaining work so we don't touch the same files.

## Where things are now

- **Branch:** `feat/server-client-split` (in `~/projects/heca`), based on current `main`.
- **Last commit:** `69389672` — T525 **C9** (shown workspace is a `WorkspaceId`, not an index) and **C12** (each column's focused pane moved out of shared `Column` into the window's `ScrollView`, keyed `ColumnId → PaneId`). Both done, all heca-core + heca tests pass, clippy clean.
- **T468** (the file-split census) is already up as **PR #292** to `main`.
- `Column` is now pure content — no `active_pane_idx`. Focus lives in `WindowView`/`ScrollView` and is read at render time.

## The model (already decided — do not re-open)

- Server holds shared content (workspaces, columns, panes, terminals). A window is a client with its own **view**. Decisions 463ccd07 (content shared, view per window), 2c249a29 (view lives in the window; server keeps only the last view), d108b88c (server/client, one process for now).
- **The pattern we are following:** per-window state goes in `WindowView`/`ScrollView`, keyed **by id, not index**, and resolved against the content at read time (see `shown_idx` in `handle.rs` and `active_pane_row` in `scrolling/mod.rs`). The server never reads a window's view; it reads what it needs from the `Asker` (`focused_pane`, `column`, `area`).

## T525 — what is left (the AppState server/client split)

Three checklist items remain. **None is visible until there are two windows (P099)** — so they land behind unit tests only; break-check every test (put the bug back, see red).

- **C10 — the rest of the per-window state still in shared content.** Move these out of the shared `Workspace`/content into the window, same id-keyed pattern as C12:
  - floating-pane geometry (position/size) and a floating pane's `is_active` — today in `Workspace.floating_panes`, rescaled per-window by `Workspace::update_working_area`;
  - `focus_domain` (tiled vs floating) — per window;
  - `interactive_move_offset` (the drag offset) — per-window visual state.
  (Column widths, pane heights and move animations are already view-derived or in `ScrollView.motion` — C11 — so leave those.)
- **C5 — terminal rows through the interface.** The window asks the server for a terminal's snapshot + changed rows; pixel cell size is the window's, rows × cols come from the window (`Asker::terminal_cells`). `heca-core/src/backend/snapshot.rs` already has `TerminalSnapshot`/`TerminalDamage`.
- **C7 — docs.** A server/client architecture section (`docs/`), both audiences.

## The rest of F012 (phases, in order)

P099 connection over a local socket (detach/attach, several windows, `heca rpc`) → P100 sessions (palette `#` mode, CLI, plain `heca` attaches to last) → P105 save/restore sessions as JSON. Independent: P101 per-project settings (done), P102 agent contract/status/sounds, P103 widgets (lines, tree layout, multi-line input, message list).

## Suggested split (so we don't collide — different phases)

- **Other agent → phase P103** (`gridui-06`-style widgets heca-pro needs: lines/arrows, tree layout, multi-line input, message list). It lives entirely in **`heca-grid-ui`** and does **not** depend on the server work (per the feature's phase notes), so it never touches the files below. This is a whole phase of its own — read P103's tasks in the planner and follow the normal widget rules (docs/widgets.md, showcase, both-audiences docs).
- **Me → finish phase P104** (T525: **C10** floating + focus-domain, **C5** terminal through the interface, **C7** docs), then start **P099**. These live in `heca-core/src/layout/*`, `heca-core/src/backend/`, and `heca/src/server/` — all outside `heca-grid-ui`.
- If you'd rather take a different phase, say so in this file before starting.

## Rules (both of us)

- AGENTS.md § 0 in full: pre-flight per type, widget-vs-component, DOM-shaped events, Rule Zero. Nothing hardcoded; fix library-side, never at the call site.
- One file one job; aim < 400 lines, hard limit 800.
- Build/lint scoped: `./scripts/lint-changed.sh` / `./scripts/lint-changed.sh test`, or `cargo … -p <crate>`. Full workspace only before a PR, prefixed `HECA_FULL_WORKSPACE=1`.
- Verify by running, not grepping. Commit only on Antonio's word; PRs only with his OK. Antonio tests by eye from `~/projects/heca-check`.
- Pre-existing: 2 `surface_drift` tests fail on `main` (T526 palette-merge drift) — not ours.
