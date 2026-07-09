# HANDOFF — context-menu overlay migration + follow-ups

> **How to resume:** a fresh session starts in the MAIN checkout `/Users/antonio/projects/myvim`.
> This work is in the sibling worktree `/Users/antonio/projects/myvim-gridui-styling`.
> **Branch:** `feat/context-menu-migration` (off `origin/main`).
> **Read first:** `AGENTS.md` (esp. ⛔ STOP + "Creating new widgets"), `docs/widgets.md`,
> `docs/surface-compositor.md`, `BACKLOG.md` § `context-menu` / `menu-nav` / `topbar-menu`.

---

## 0. THE CARDINAL PRINCIPLES (violated repeatedly this session — read every time)
1. **Never hardcode** style / colors / sizes / keys / labels / shortcuts. Every value comes from the
   `Theme`, a widget **variant**, or the action/keymap registries.
2. **Always check for an existing reusable pattern FIRST.** Before writing UI, grep the widgets +
   run the showcase. There is almost always something to reuse.
3. **Extend an existing widget/primitive, don't write a new one** (and don't hand-draw in a
   consumer's `paint`). If a primitive lacks a variant you need, add the **variant to the shared
   primitive** so every consumer benefits — never a one-off in the caller.
4. **The caller picks a variant; the widget owns its styling.** No size/layout/position/style math
   at call sites or in `paint`.
5. **Docs in BOTH places** on any widget/UI-model change: complete **rustdoc** (mandatory) AND
   `docs/widgets.md` (supplementary human catalog), each with internal-code AND plugin (`ViewNode`)
   examples. Plus the **showcase** demo.
6. **When the user asks/interjects, STOP writing** and address it. No commit/push/PR/merge without
   explicit OK. Don't code while the user is discussing.

---

## 1. Git state
- **Merged to `main` already:** PR #225 (grid-ui styling foundation + action-interaction + overlays +
  ViewNode + focus-ring) and PR #226 (realize scalar-fill, modal form marshalling, context-menu
  foundation `open_dropdown`/`ContextMenu::on_dismiss`, ViewNode docs + AGENTS doc rule).
- **This branch `feat/context-menu-migration`, committed:**
  - `7d031c5` **context-menu-4** — the migration (see §2). **User GUI-verified it: OK.**
  - `c07e1e7` docs(backlog) reframe of context-menu-3 (superseded by §4 decision — see note there).
- **Uncommitted on this branch (green: heca 318, clippy clean) — ready to commit:**
  - **`OpenContextMenu` action** (context-menu-3): `input.rs` (variant + `action_from_name` + priority),
    `app/interaction.rs` (`action_policy`, grouped TiledOnly with `ZoomColumn`), `handlers.rs`
    (`handle_open_context_menu`), `app/registry.rs` (register), `rpc.rs` (`open-context-menu`),
    `actions.rs` (descriptor), `mouse.rs` (`open_focused_context_menu`), `keybindings.default.toml`
    (`open_context_menu = "prefix+."` — **to change to `prefix+>`, see §3.3**).
  - **`open_dropdown` item cleanup** (`chrome/overlay.rs`): menu items now show **only the
    single-letter quick-pick** (removed the `λ` global-binding label — see §4 decision).

---

## 2. WHAT WAS DONE (context-menu arc)
### context-menu-4 — migration onto OverlayHost (DONE, committed `7d031c5`, GUI-verified)
Both builders (`open_context_menu` / `open_sidebar_context_menu` in `mouse.rs`) now build a
`DropdownSpec` and open via **`OverlayHost::open_dropdown`** (`heca/src/chrome/overlay.rs`) — a
host-owned **Overlay-band, `modal=true` layer**. This reuses the **proven** confirm-modal path:
`top_modal` input routing (`events.rs`) + `layout_layers`/`paint_layers` (`chrome/mod.rs`). The
chosen action dispatches through the **central confirm gate** (`dispatch_action`), icons come from
the **action registry** (`ActionCatalog::icon`).
**Removed the entire bespoke path** (dead once the menu is a layer): `AppState.context_menu` +
`context_menu_action` sink (+ `startup.rs` init); the 3 `events.rs` input branches +
`context_menu_open`/`settle_context_menu` (the adjacent `top_modal` branches replace them; the
open-gate now checks `top_modal`); `chrome/mod.rs` `layout_context_menu`/`paint_context_menu` + 2
hint-collection blocks; the 2 `render.rs` calls.

### context-menu-3 — `OpenContextMenu` action (DONE, uncommitted, green)
Reachable mouse (right-click, pre-existing) / keyboard (`prefix+.`) / RPC (`open-context-menu`).
Handler reuses `mouse::open_focused_context_menu` → `open_context_menu(focused_pane, mouse.pos)`.
**Anchor is the last cursor pos — a STOPGAP; see §3.2.**

### Earlier this session (merged via #225/#226)
Dialog trapped-focus fix; `realize` scalar-fill + `glyph_from_name`; modal form marshalling
(`FormBindings`, `"name"` prop → `ModalResult::Action{data}`); full `ViewNode` docs + AGENTS
doc-in-both-places rule; `ContextMenu::on_dismiss`; `DropdownSpec`/`open_dropdown`.

---

## 3. WHAT REMAINS (do fresh, properly — each references the widget to REUSE)

### 3.1 Bordered keycap — via a REUSABLE widget, NOT hardcoded  ⚠️ (the mistake to avoid)
The menu keycap must look **bordered** (outline, like the showcase `CMD P` / `λ f` style — user's
Image #4), not the current filled chip (Image #3).
- **DO NOT** hand-draw it in `ContextMenu::paint` (I did that and reverted it —
  `heca-grid-ui/src/widgets/context_menu.rs` ~L417-433 currently hand-draws the cap with `cx.rect` +
  `cx.text`; that itself is a pre-existing hardcode to replace).
- **REUSE the shared keycap primitive:** `paint_keycap` / `keycap_size` (exported from grid-ui;
  defined in `heca-grid-ui/src/widgets/key_hint.rs`) — the same one `KeyHint` uses.
- **Add a `bordered` variant to that shared primitive** (or a param), then make `ContextMenu` (and
  anyone drawing a keycap) call it. One change → consistent everywhere. Update the `KeyHint` showcase
  demo + `docs/widgets.md` + rustdoc.

### 3.2 Keyboard-open anchor → CENTER OF THE FOCUSED WIDGET (decision, user)
Mouse open = at the cursor (already). **Keyboard open should anchor at the center of the focused
widget** (focused pane; sidebar item in sidebar-nav; the active **layer/`current_index`** surface
once the compositor lands). Today `mouse::open_focused_context_menu` uses `state.mouse.pos` (stopgap).
- Needs the focused pane's **screen rect** — reuse the render-side geometry: `content_rect` /
  `stable_tiled_content_rect` (`heca/src/app/render.rs`), not a new computation.

### 3.3 Default binding → `prefix+>` (if free)
Change `open_context_menu = "prefix+."` → `"prefix+>"` in `keybindings.default.toml` (verify it's
unbound first) and the descriptor `default_binding` in `actions.rs` (currently `"."`).

### 3.4 Configurable `menu-nav` (shared) — arrows + **Ctrl+j/k**
Today nav is **arrows-only, hardcoded** in two places: `ContextMenu::event` (`context_menu.rs`
~L431-448, no Ctrl+j/k) and the separate `SidebarNav` keymap. **Make ONE configurable list/menu-nav
binding set** (Up/Down + Ctrl+j/k, Enter, Esc) **shared** by menu + sidebar (+ future top-bar menu),
and migrate the sidebar keys onto it. Tracked as the `menu-nav` requirement in `BACKLOG.md`. Reuse
the keymap/`ActionRegistry` config path — never hardcode the keys in the widget.

### 3.5 context-menu-5 — plugin `Contribution::ContextMenu`
New `Contribution` variant (`heca/src/chrome/contribution.rs`) so a plugin registers entries for a
named context (`pane`, `sidebar_item`, …); host **merges** built-in + contributed entries when
opening. Study the existing `Contribution`/`Provider` system first (host.rs / contribution.rs).

---

## 4. DECISIONS (with WHY — do not relitigate)
- **Context menu = data spec (`DropdownSpec`) → `open_dropdown` → host-owned Overlay-band modal
  layer**, reusing `top_modal` + `paint_layers` (proven by the confirm modal). It is
  **plugin-declarable** (entries carry intents, not closures). This is "Option B's real value"
  (data-driven, plugin-declarable) while reusing the `ContextMenu` widget for the visuals
  (focused left-border + glow + icon are already theme-driven in it).
- **Menu items show ONLY the single-letter quick-pick** (a bordered keycap via §3.1). **No global
  `λ x` binding label** — it's redundant (a keyboard user triggers the action directly without the
  menu) AND the leader doesn't fire while the menu is open (the original "misleading" complaint).
  The quick-pick letter is the one accelerator that actually works in-menu. (User confirmed: "only
  the quick pick with the border. that's it.")
- **Icons + labels + shortcuts come from the registries**, never hardcoded: `ActionCatalog::icon`,
  `ActionShortcuts` + `shortcut::format_shortcut` (renders the leader as `λ`; config token stays
  `prefix+…`).
- **Destructive menu actions still confirm** via the central gate (`dispatch_action`).
- **Opening the menu from keyboard = a real, standard action** (like the Menu key / Shift+F10) — kept
  (`OpenContextMenu`); it's what justifies the in-menu quick-pick + nav. The earlier "more-button"
  reframe in `BACKLOG.md` (commit `c07e1e7`) is **superseded** by this keyboard-action decision —
  reconcile the backlog next session.

---

## 5. WIDGET / PATTERN REFERENCE (reuse these; extend, don't reinvent)
- **Keycap:** `paint_keycap` / `keycap_size` (`key_hint.rs`) — used by `KeyHint`. Add a bordered
  variant here for §3.1.
- **Context menu / dropdown:** `ContextMenu` widget + `DropdownSpec` / `DropdownItem` /
  `open_dropdown` (`heca/src/chrome/overlay.rs`).
- **Overlay panel with REAL buttons (dispatch + keypress + KeyHint for free):** `Dialog`
  (`heca-grid-ui/src/widgets/dialog.rs`) — the pattern for any overlay that hosts action buttons.
- **Buttons:** `Button` — already dispatches its action, is focus/Enter-activatable, and
  KeyHint-reachable (`prefix+/`). Never reinvent action-dispatch or key handling for clickable UI.
- **Shortcut text:** `shortcut::format_shortcut(keys, with_prefix)` + `ActionShortcuts` (on
  `AppState`, resolves the live binding). Never hardcode `prefix+x` strings.
- **Declarative UI model:** `ViewNode` (`heca/src/chrome/view.rs`) + `realize` (`realize.rs`) —
  extend the vocabulary host-side (a `WidgetKind` variant + a `realize` arm + showcase + docs).
- **Layering:** the surface compositor / `LayerRegistry` + `top_modal` + `paint_layers`
  (`docs/surface-compositor.md`, `chrome/layers.rs`) — mount overlays as layers, never bespoke.

---

## 6. IMMEDIATE NEXT STEP
Commit the clean uncommitted `#3` + quick-pick changes (§1) if the user OKs, then start §3.1
(bordered keycap via the shared `paint_keycap` primitive) — extending the primitive, not
hardcoding in `ContextMenu::paint`.
