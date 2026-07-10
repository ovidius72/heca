# HANDOFF — rename dialog + pane naming + keycap/dialog + context-menu follow-ups

> **Branch:** `feat/planner-backlog-sync`. **All work below is UNCOMMITTED** (working tree).
> **State:** builds clean, `cargo clippy --workspace --all-targets --all-features` = 0 warnings,
> **all tests green** (heca 331, heca-grid-ui 127, heca-core 88, heca-config 78, …).
> **Read first:** `AGENTS.md` (⛔ STOP rules), `docs/widgets.md`, `docs/surface-compositor.md`,
> `HANDOFF-context-menu.md` (earlier arc), `.planner/HANDOFF.md`, `BACKLOG.md`.
>
> **Cardinal rules (violated repeatedly — obey):** never hardcode style/colors/sizes/keys — all from
> `Theme`/variant/registry/config; extend the shared primitive, don't hand-draw in a consumer;
> config → `config.default.toml` + `keybindings.default.toml` (single source) + README; docs in BOTH
> rustdoc AND `docs/widgets.md`; **don't touch the pane info-bar segment logic beyond what's below**.

---

## 0. WHAT WAS DONE THIS SESSION (all uncommitted, all green)

### A. context-menu-3 §3.1 — bordered keycap (shared primitive, reusable)
- `heca-grid-ui/src/widgets/key_hint.rs`: added **`KeycapVariant { Filled, Bordered }`** + a `variant`
  param on `paint_keycap`. **Filled** = old glowing chip. **Bordered** = **no fill (transparent
  interior) + full-accent border (`cx.border(keycap_c)`, full alpha) + accent glyph, no glow** —
  matches the showcase KeyHint. Exported via `widgets/mod.rs` + `lib.rs`.
- `heca-grid-ui/src/widgets/context_menu.rs`: quick-pick keycap now calls `paint_keycap(…,
  KeycapVariant::Bordered)` (was hand-drawn); uses shared `keycap_size`; **`KEYCAP_FONT_SCALE=0.72`**
  so the letter is small. Removed the local `KEYCAP_PAD_*`/`keycap_size` dupes.
- Callers passing `KeycapVariant::Filled`: `chrome/mod.rs` (2 link/hint sites), `KeyHint::paint`.
- Docs: `docs/widgets.md` (Standalone keycap section + ContextMenu note), showcase comment, rustdoc.

### B. context-menu-7 review fixes
- **Stale `pending_context` bug fixed:** it's set by the SidebarNav prefix arm but was only cleared
  by `handle_open_context_menu`. Now cleared at **every prefix-exit** in `app/input.rs`
  (`handle_prefix_mode`: double-prefix, mode-trigger, **after** dispatch, invalid-key) and the prefix
  **timeout** in `app/lifecycle.rs`. Added `needs_redraw` to the sidebar prefix arm.
- Refactored `resolve_active_context` → pure `resolve_context_for(input_mode, sidebar_item,
  focused_pane)` in `chrome/context_menu.rs` + unit tests (context_menu tests 5→9).

### C. Rename: menu entries + by-id actions + sidebar keybindings
- **New `WmAction`s** `RenamePaneById { pane_id }`, `RenameWorkspaceByIdx { ws_idx }` — full wiring:
  `input.rs` (variant + `action_from_name` `rename_pane_by_id`/`rename_workspace_by_idx` + priority),
  `app/interaction.rs` (`action_policy`: PaneById→FocusedPaneLocal, WorkspaceByIdx→WorkspaceLevel),
  `handlers.rs` (`handle_rename_pane_by_id`/`handle_rename_workspace_by_idx`), `app/registry.rs`,
  `rpc.rs` (`rename-pane-id`/`rename-workspace-idx`). (No descriptor/binding — menu/RPC-only, like
  `ClosePaneById`.)
- **Menu entries** (`chrome/context_menu.rs`): pane → *Rename* (`RenamePane`) + *Rename workspace*
  (`RenameWorkspace`); `sidebar.pane` → *Rename pane* (`RenamePaneById`); `sidebar.workspace` →
  *Rename workspace* (`RenameWorkspaceByIdx`). Extracted `sidebar_column_items`/`sidebar_workspace_items`.
  `ContextTarget::pane_id()` / `ws_idx(state)` accessors.
- **Sidebar-mode keybindings:** `prefix+$` / `prefix+Shift+w` now work in `SidebarNav` — the unit
  handlers `handle_rename_pane`/`handle_rename_workspace` consume `pending_context` to target the
  sidebar-cursor item (fall back to focused/active otherwise). `keybindings.default.toml` comments updated.

### D. Rename → modal dialog (replaces the bottom-bar input)
- `handlers.rs`: `apply_rename(state, target, name)`, `enter_pane_rename`/`enter_workspace_rename`,
  `open_rename_dialog(state, target, current_name)` — opens `OverlayHost::open_modal` with an `Input`
  `ViewNode` body (`.text(prefill).prop("name", …)`) + **OK/Cancel**. `handle_rename_column` too.
- **Removed** `InputMode::Rename` variant (`app_state.rs`), `handle_rename_input` + its call
  (`app/input.rs`), the status-bar `"RENAME"` arm + its test (`app/render.rs`). `RenameTarget` is now
  `Copy`.
- **Dialog field-first key routing** (`heca-grid-ui/src/widgets/dialog.rs`): Dialog owns only **Esc**
  + **Tab/Shift+Tab**; **every other key goes to the focused widget first** (so an `Input` keeps ALL
  native editing — typing, Ctrl+h delete, Ctrl/Cmd+A select-all, caret); only keys the field ignores
  fall back to nav (Enter→**primary** action via `activate_primary`, arrows, Ctrl+h/j/k/l). Forwards
  `ModifiersChanged` to the panel. Tests: typing+Enter-submits-primary, field-first Ctrl+h/j, nav.
- **Disabled-OK when empty** (`chrome/overlay.rs`): `ModalAction::disabled_when_empty("field")`;
  `FormBindings.text_signal(name)` exposes the `Input`'s live `Signal<String>` (`chrome/realize.rs`);
  `build_modal_root` binds the button's `base.disabled` via **`create_effect`**. Rename OK uses it.
  Completion also guards blank (belt-and-suspenders). **⚠️ Verify reactive re-grey in-app** — this is
  the first `create_effect` in the app; initial state is correct, live re-run untested.

### E. Sidebar highlight persists under an overlay
- `app_state.rs`: `AppState::sidebar_nav_active()` = `SidebarNav` OR `overlay_origin_mode ==
  Some(SidebarNav)`. Used in `sync_chrome_state` (nav-cursor projection, `chrome/mod.rs`) + collapsed
  rail (`app/render.rs`). So a sidebar-opened menu keeps the target row highlighted.

### F. Info-bar `AppName` shows the app/program name (NOT the rename)
- `chrome/mod.rs`: `PaneInfoView` gained **`app_name`** (always the program name); the `AppName`
  segment renders `view.app_name` (was `view.title`). `title` (custom-wins) stays for the sidebar.
  **This is the fix for the "ciao shows in the info bar" complaint.**

### G. Prefill fixes (rename dialog)
- `handlers.rs`: **pane** prefills `custom_name` only (blank if it only tracks its process — the
  process name is NOT the pane's name). **workspace/column** prefill the **displayed label**
  (`ws.name` else `Workspace N`; `col.name` else `Column N`) — because `AppState` only stores
  `ws.name`/`col.name` (None until renamed); the default label is computed, not stored.

### H. Sidebar-rename mode restore
- `handlers.rs`: `open_rename_dialog` captures `overlay_origin_mode = Some(SidebarNav)` when opened in
  SidebarNav (mouse-menu path); `handle_rename_pane`/`handle_rename_workspace` set it from
  `pending_context.origin` (keyboard `prefix+$` path, where the mode is already `Normal`). So the
  rename dialog restores `SidebarNav` on close (via `overlay::resolve`).

### I. `pane_renamed_add_process_name` config (⚠️ rendering NOT wired — see §1.2)
- `heca-config/src/settings.rs` (field + `default_…` + `Default` impl), `config.default.toml`,
  `README.md`, `AppState.pane_renamed_add_process_name` (startup + `main.rs` reload). `pane_info_view`
  computes `process_hint` (Some when custom + flag). **The info-bar rendering was REVERTED** (it
  caused an empty header — see §2 lessons). The plumbing (`PaneHeaderContent.add_process_name` →
  `build_pane_info_bar` / `pane_header_key`) remains but is **unused/dead** and must be moved to the
  sidebar (§1.2) or removed.

### J. Backlog
- Added requirements: **`widget-keys-config`** (make in-widget keys config-driven — Dialog/Input/menu
  nav), **`available-actions`** (context→available-actions query for a future command palette). See
  `BACKLOG.md`.

---

## 1. WHAT REMAINS (do next, in order)

### 1.1 NEW — `pane_name` title-segment (user request, latest)
Add a new `[appearance.pane] title_segments` value **`pane_name`** that shows the **pane's name
(original OR renamed = `view.title`, custom-wins)**. **NOT in the default `title_segments`** (opt-in).
Contrast with `AppName` which now shows the program name (§0.F).
- `heca-config/src/appearance.rs`: add `PaneName` to the `PaneSegment` enum (doc-comment).
- `chrome/mod.rs` `build_pane_info_bar` segment match: `PaneSegment::PaneName => (view.icon,
  view.title.clone())` (or a name-glyph). `view.title` already exists.
- Update README (Pane Info Bar supported segments), `config.default.toml`, showcase if relevant.

### 1.2 Sidebar `(process)` suffix — via `WorkspacesContainerState` signal (user endorsed AppState+signal)
The `(nvim)` suffix belongs in the **sidebar pane card**, not the info bar. Thread the flag WITHOUT
editing every `column_view`/`pane_card` signature: carry it on **`WorkspacesContainerState`** (which
`pane_card` already receives as `ws_state`).
- `chrome/state.rs`: add `pane_renamed_add_process_name` (+ `pane_show_cwd`, §1.3) as a `Signal<bool>`
  or `Cell<bool>` field + setter (mirror `active_pane`/`set_active_pane`).
- `chrome/mod.rs` `sync_chrome_state`: set them from `state.pane_renamed_add_process_name` /
  `state.pane_show_cwd`.
- `chrome/mod.rs` `pane_card` (~L1371): pass the flag to `pane_info_view` (currently `false`); when
  `info.process_hint` is `Some`, append a **small muted `(process)` `Label`** after the title labels
  in the card row (`Label::new(format!("({p})")).font_scale(0.8).color(theme.colors.muted)`). NB the
  title is signal-driven; the hint appears on rebuild (rename changes the tree). Re-add the removed
  `PROCESS_HINT_FONT_SCALE` const (or use 0.8).
- Then **remove the dead info-bar plumbing** from §0.I (`PaneHeaderContent.add_process_name`, the
  `build_pane_info_bar`/`pane_header_key` `add_process_name` params + `view.process_hint` in the key).

### 1.3 NEW config + row — `pane_show_cwd` (user request, earlier)
A new **row in the sidebar pane card, between the name row and the git-status row**, showing the
pane's **cwd** with a folder icon. Guided by new config **`[settings] pane_show_cwd: bool`**.
- Config: `settings.rs` + `config.default.toml` + README + `AppState.pane_show_cwd` (startup +
  reload) — mirror `pane_renamed_add_process_name` exactly.
- `pane_card`: `runtime` (from `runtime_snapshot`) has `.cwd`. When `pane_show_cwd` (from the store
  signal) and cwd present, insert a row `[Icon(Folder) + Label(home_relative_path(cwd)).font_scale]`
  between the name row and the git row. Reuse `home_relative_path` (`chrome/mod.rs`).

### 1.4 Column rename is unreachable — add it
`RenameColumn` exists but has **no keybinding** (`prefix+Shift+c` reused for move-pane-to-column) and
**no menu entry**. Add *Rename column* to the `sidebar.column` menu (`sidebar_column_items` in
`chrome/context_menu.rs`). Needs a target: add a **`RenameColumnByIdx { ws_idx, col_idx }`** action
(mirror `RenameWorkspaceByIdx` — full wiring) → the menu entry dispatches it; handler → a shared
`enter_column_rename(state, ws_idx, col_idx)` that opens the rename dialog.

### 1.5b Declare icons on EVERY menu/button action (centralized) — user request
The menu entries ALREADY resolve icon+tooltip centrally (id = action name → `ActionCatalog::icon` +
`ActionShortcuts`; `ActionDescriptor` declares `label`+`description`). The gap: **`ActionDescriptor
.icon` is `None`** for the actions, and the central `Glyph` enum (`heca-grid-ui/src/widgets/icon.rs`,
`enum Glyph`) lacks the needed glyphs. **Never hardcode a glyph in a menu entry — it flows from the
descriptor via the action name.**

**STEP 1 — add these Phosphor glyphs to the central `Glyph` enum + the icon-font codepoint mapping
(one place) + document ALL available glyphs in `docs/widgets.md`** (the user wants a full glyph list
there). Glyphs to add (get each Phosphor codepoint):
`Pencil`, `Backspace`, `Trash`, `XCircle`, `XSquare` (already exists), `PlusCircle`,
`FolderSimpleMinus`, `FolderSimplePlus`, `PlusSquare`, `StackPlus`, `StackMinus`, `ColumnsPlusLeft`,
`ColumnsPlusRight`, `SquareHalf`, `SquareSplitHorizontal`, `SquareHalfBottom`.

**STEP 2 — set `icon: Some(Glyph::…)` on the descriptors (`actions.rs` `ActionRegistry::ALL`), using
the user's chosen mappings:**
- `rename_pane`/`rename_workspace`/`rename_column` (~L561/586/569) → `Pencil`
- `close` (delete/close pane, L461) → `Backspace` (user: backspace for delete_pane); consider `Trash`/
  `XCircle` variants where appropriate
- `create_workspace` (New workspace, L578) → `StackPlus`
- `delete_workspace` (L594) → `StackMinus`
- `add_pane_to_column` (New pane) → `FolderSimplePlus` — **user: also update the pane-header
  `title_actions` add-pane button (`pane_action_spec`/`PaneAction`) to this glyph**
- remove-pane action → `FolderSimpleMinus`
- `add_column_to_workspace` / `split_horizontal` (New column, `prefix+Enter`) → `ColumnsPlusLeft`
- (later use) → `ColumnsPlusRight`
- `split_vertical` (`prefix+v`) → `SquareHalfBottom` (also `SquareHalf` / `SquareSplitHorizontal`
  available for split variants)
- also fill: `zoom_column`, `float`, `open_link`, `delete_column` with fitting glyphs.

**STEP 3** — every menu/button referencing those actions then shows the icon for free (via the action
name). Verify in the pane menu + all three sidebar menus + the pane-header buttons.

### 1.5 BUG — "wrong pane renamed" from the sidebar (needs repro)
User reports renaming from the sidebar renames the wrong pane. **Not diagnosed.** Hypothesis: in
`SidebarNav` with the cursor on a **non-pane** row (column/workspace), `pending_context.target
.pane_id()` is `None` and `handle_rename_pane` falls back to `focused_pane_id` (≠ cursor). If
confirmed, don't fall back to focus in sidebar mode. **Get exact repro steps first** (keyboard
`prefix+$` vs right-click menu; what the cursor was on).

### 1.6 In-app verifications (rendering not test-covered)
- Rename dialog: typing + Ctrl+h/Cmd+A + Enter=OK/Esc=Cancel; **OK greys reactively** when blank (§0.D).
- Keycap: no-fill + full-accent border + small letter in the context menu.
- Info-bar `AppName` shows the program name after a rename; sidebar shows the renamed name.

---

## 2. LESSONS FROM THIS SESSION (don't repeat)
- **`if let (Some(x), …) = (opt.take(), cond)` runs `take()` even when the tuple pattern fails** — it
  emptied the info-bar Tag when `process_hint` was `None` → **empty pane header**. Guard the `.take()`
  behind the real condition. (This is why the info-bar process-hint was reverted.)
- The pane info-bar renders `[appearance.pane] title_segments`; **it must always show them** — don't
  put conditional/feature logic that can drop the whole Tag.
- The user wants the `(process)` suffix in the **sidebar**, the app name in the **info bar** — two
  different places, two different `PaneInfoView` fields (`title` vs `app_name`).

## 3. RECONCILE WITH EXISTING HANDOFFS / PLANNER
- `HANDOFF-context-menu.md`: its §3.1 (bordered keycap), §3.3 (`prefix+>`), context-menu-3/4/6/7 are
  **DONE** (this session + prior). Its §3.2 (keyboard anchor = center), §3.4 (`menu-nav`), §3.5
  (context-menu-5 plugin) remain. That handoff predates the worktree move — **this branch is
  `feat/planner-backlog-sync` in the main checkout `/Users/antonio/projects/heca`.**
- `.planner/HANDOFF.md`: still lists context-menu-3 in-progress — it's effectively done; the rename
  dialog + pane-naming work here is NOT yet reflected in the planner. Update the planner phase/tasks
  and delete stale handoffs when this lands.
- **BACKLOG:** a `pane-naming` entry was added (see below) covering §1.1–1.5.

## 4. COMMIT PLAN (nothing committed; run the rust skill review first, per AGENTS)
Suggested slices: (1) keycap+dialog+overlay grid-ui changes; (2) rename dialog + by-id actions +
sidebar keybindings + mode-restore; (3) cm-7 review fixes; (4) pane naming (AppName/prefill/config)
+ docs. Each after `~/.agents/skills/rust/SKILL.md` review + clippy.
