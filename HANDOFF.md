# HANDOFF — widget-keys-config + WidgetIntent/Keymap unification (2026-07-12)

> **Read first:** `AGENTS.md` (⛔ STOP rules), `docs/widgets.md`, `docs/surface-compositor.md`,
> `docs/sidebar-provider-modes.md`. This handoff supersedes the previous one for the
> `widget-keys-config` arc. It is intentionally exhaustive: an agent resuming should need nothing else.

---

## 0. Cardinal rules (obey)
- **Never hardcode** style/colors/sizes/keys — all from `Theme`, a widget **variant**, or the
  action/keymap registries. No magic numbers.
- **Reuse/extend existing widgets**; never hand-draw in a consumer's `paint`.
- Every setting → `config.default.toml`; every keybound action → `keybindings.default.toml`.
- Docs in **both** rustdoc AND `docs/widgets.md` for any widget/UI-model change.
- **Don't run `cargo fmt`**; verify with `cargo clippy --workspace --all-targets --all-features`.
- **Communication: plain simple English, no jargon.**
- **Centralize, don't re-declare per widget/host.** (The recurring theme of this arc — see §2.)

---

## 1. Branch / PR state
- **PR #234** (`feat/widget-keys-config` → `main`, OPEN): the first pass — per-widget configurable
  keys using three separate event vocabularies (`MenuNav`/`DialogNav`/`InputEdit`) + three app maps.
  **Superseded in design** by the unification below, but still the open PR.
- **Branch `feat/widget-keymap-unify`** (off `feat/widget-keys-config`, NOT yet PR'd): the unification
  — one `WidgetIntent` + one host-owned `Keymap` + `[keys.widgets]`. Commits:
  - `e5753c6` refactor: unify to WidgetIntent + host-owned Keymap + [keys.widgets]
  - `5127604` fix: resolve overlay keys via macOS-robust event_combo (Ctrl+letter)
  - `83f8361` fix: case-insensitive combo_to_grid restores named keys (Tab/arrows/Enter)
- **Decision pending (user):** fold the unification into PR #234 (retarget), or open a 2nd stacked PR.
- Remote: `git@github.com:ovidius72/heca.git` (gh authed as `ovidius72`).
- Gates on `feat/widget-keymap-unify`: workspace clippy 0; all tests green (grid-ui 128+69, heca
  333+89, heca-config 78); showcase builds.

---

## 2. The problem & the decisions (WHY) — read before touching anything

### 2.1 The problem
Several `heca-grid-ui` widgets **hardcoded their internal keys** in `event()` (Dialog focus-nav,
Input editing, Select/Tabs/ContextMenu/CommandPalette list-nav). This violates the project rule that
**keys pass through config**. Requested by the user 2026-07-09.

### 2.2 The model we chose (approved by the user) — host-driven semantic events
Widgets are **headless and config-agnostic** (same boundary as "no GPU/winit"). They must NOT read
`config.toml`. So:
- **Widgets speak semantic intents**, never keys. The key→intent mapping is the **host's** job
  (keys are configurable; the library is headless; mouse=keyboard=RPC parity).
- **One shared vocabulary** across all widgets (the user rejected per-widget vocabularies as
  redundant — same instinct as "don't re-declare"): `WidgetIntent` (see §3.1).
- **One host-owned `Keymap`**, built once from config + rebuilt on reload, consulted once at
  dispatch. **NOT a global** — each host/window owns its `Keymap` (the user asked about detached
  panes / multi-window: a `thread_local`/`static` would break because grid-ui is `Rc`-single-threaded
  and each UI thread is its own world; a per-host `Keymap` is safe).

### 2.3 Naming/axis decisions (user, 2026-07-12) — IMPORTANT, do not relitigate
- **`item_previous` / `item_next` = HORIZONTAL** (left/right). Keys: `Ctrl+h` / `Ctrl+l` (+ ←/→).
  Used by **Tabs** and a **Dialog's button row**.
- **`menu_up` / `menu_down` = VERTICAL** (up/down). Keys: `Ctrl+k` / `Ctrl+j` (+ ↑/↓).
  Used by **menus, Select lists, command palette**.
- **`activate` (Enter) / `dismiss` (Esc)** shared. **`edit_*`** for Input.
- **Tab / Shift+Tab are NOT configurable.** They are the universal focus-traversal primitive
  handled by the `FocusManager` (main tree) and trapped inside a modal `Dialog`. Always on.
- **Config table is `[keys.widgets]`** (a dedicated table, not flat `[keys]`).
- **Buttons activate on Space/Enter** (classic; `Button::event` already does this — button.rs:414).
- **`Ctrl+h` overload** (delete in an Input, previous in Tabs/dialog) disambiguates **by focus**: the
  keymap resolves `Ctrl+h` to `[EditDeleteBack, ItemPrevious]`; delivered to the focused widget,
  which consumes the one it understands. Edit intents are bound **first** (field-first).

### 2.4 The next decision (user, 2026-07-12) — NOT YET DONE (see §6.1)
**Centralize `focusable()`.** 18 widgets re-implement `Component::focusable()`. The user wants
focusability to be a **property on `Base`**, defaulted per widget in its constructor, with `disabled`
handled centrally — not a re-implemented method. Same "centralize, don't re-declare" principle as the
keymap. **This is the top next task.** Full design in §6.1.

---

## 3. Architecture as built (the unification)

### 3.1 `heca_grid_ui::WidgetIntent` + `Event::Widget` — `heca-grid-ui/src/component.rs`
Replaces the three enums (`MenuNav`/`DialogNav`/`InputEdit`). Delivered as `Event::Widget(WidgetIntent)`.
```rust
pub enum WidgetIntent {
    ItemPrevious, ItemNext,          // horizontal (Tabs, dialog button row)
    MenuUp, MenuDown,                // vertical (menus, Select list, palette)
    Activate, Dismiss,               // shared
    EditDeleteBack, EditDeleteToLineStart, EditSelectAll, // Input only
}
```

### 3.2 `heca_grid_ui::Keymap` — `heca-grid-ui/src/keymap.rs` (NEW FILE)
Host-owned `key chord → Vec<WidgetIntent>`. Key methods:
- `Keymap::new()` (empty), `Keymap::with_defaults()` (built-in vim-friendly defaults for config-less
  hosts like the showcase), `bind(key, mods, intent)`, `resolve(key, mods) -> &[WidgetIntent]`.
- **`dispatch(key, mods, deliver)`** — the one resolution point. **Field-first**:
  ```rust
  // 1. raw Event::Key first (focused widget's own typing/caret/quick-pick wins)
  if deliver(&Event::Key{key,pressed:true}) == Yes { return Yes; }
  // 2. then each resolved Event::Widget(intent), in order (edit-first), stop at first consumed
  for intent in self.resolve(key, mods) { if deliver(&Event::Widget(*intent)) == Yes { return Yes } }
  ```
  `deliver` is host-specific (a focused component, or an overlay's root that forwards field-first).
- `KeyChord { key: GridKey, mods: Modifiers }` — letters normalised to lowercase.
- `GridKey` and `Modifiers` now derive `Hash` (for the map). Tests in the file cover axis resolution,
  the Ctrl+h overload ordering, case normalisation, and field-first dispatch.

### 3.3 Widget consumption (which intents each widget handles)
- **ContextMenu / CommandPalette** (`context_menu.rs`, `command_palette.rs`): vertical →
  `MenuUp`/`MenuDown`/`Activate`/`Dismiss`; raw `Char` = quick-pick letter / palette typing.
- **Select** (`select.rs`): CLOSED trigger opens on raw `Enter`/`Space`/`↓`; OPEN list (overlay) →
  `MenuUp`/`MenuDown`/`Activate`/`Dismiss`.
- **Tabs** (`tabs.rs`): horizontal → `ItemPrevious`/`ItemNext`. No raw key handling (pointer + Widget).
- **Dialog** (`dialog.rs`): `ItemPrevious`/`ItemNext` = focus_prev/next, `Activate` = submit primary,
  `Dismiss` = fire_dismiss. `Edit*` (and any `Menu*`) are **forwarded field-first** to the focused
  child. Raw `Event::Key` field-first; **Tab/Shift+Tab handled internally** (classic, uses `self.mods`
  from `ModifiersChanged`). See dialog.rs `event()`.
- **Input** (`input.rs`): `EditDeleteBack`/`EditDeleteToLineStart`/`EditSelectAll`. Plain keys
  (typing, Backspace/Delete, arrows, Home/End, word/line motion) stay in `handle_key` (built-in).
  A modified char is ignored so the host can resolve it into an Edit intent.

### 3.4 App wiring (heca)
- **`AppState.widget_keymap: heca_grid_ui::Keymap`** (`heca/src/app_state.rs`) — replaces the three
  old maps. Built by `build_widget_keymap` at startup (`heca/src/app/startup.rs`) and rebuilt on
  reload (`heca/src/main.rs` `reload_config`).
- **`build_widget_keymap(config)`** (`heca/src/app/registry.rs`) — reads `[keys.widgets]`
  (`config.keys.widgets`, falling back to `KeysConfig::default().widgets`), binds `edit_*` FIRST
  (field-first ordering), converts each config string via `combo_to_grid`.
- **`combo_to_grid(&KeyCombo) -> Option<(GridKey, Modifiers)>`** (`registry.rs`) — converts a parsed
  `KeyCombo` to a grid chord. **Matches the key name case-INSENSITIVELY** (critical — see §5).
- **events.rs overlay branch** (`heca/src/app/events.rs`, in the keyboard handler, gated on
  `!picker_seq && crate::chrome::top_modal(state).is_some()`):
  ```rust
  if let Some((key, mods)) = crate::app::registry::combo_to_grid(&event_combo) {
      let keymap = state.widget_keymap.clone();
      keymap.dispatch(key, mods, |ev|
          state.layers.top_modal_root_mut().map(|root| root.event(ev)).unwrap_or(Handled::No));
  }
  ```
  Uses `event_combo` (from `build_event_combo`, which has the macOS physical-key fallback), NOT the
  raw logical key. Modifiers are also broadcast to the overlay as `Event::ModifiersChanged` (so the
  Dialog can tell Tab from Shift+Tab).
- **heca-config**: `KeysConfig.widgets: KeybindingMap` field (`heca-config/src/keys.rs`) parses the
  `[keys.widgets]` table (a named field so serde's `#[serde(flatten)] bindings` doesn't swallow it).
- **`keybindings.default.toml`**: the `[keys.widgets]` table (placed BEFORE `[[keys.mode]]`; TOML does
  not allow re-opening `[keys]` after a subtable). Defaults: `item_previous=[ArrowLeft,Ctrl+h]`,
  `item_next=[ArrowRight,Ctrl+l]`, `menu_up=[ArrowUp,Ctrl+k]`, `menu_down=[ArrowDown,Ctrl+j]`,
  `activate=Enter`, `dismiss=Escape`, `edit_delete_back=Ctrl+h`, `edit_delete_to_line_start=Ctrl+u`,
  `edit_select_all=[Ctrl+a,Super+a]`.

### 3.5 Showcase wiring (`heca-renderer/examples/showcase.rs`)
- `GpuState.keymap: Keymap` = `Keymap::with_defaults()`.
- `route_overlay_key(gk)` → `keymap.dispatch(gk, mods, |ev| focus.offer_to_overlay(&mut ui, ev))`.
- `route_focused_key(gk)` → `keymap.dispatch(gk, mods, |ev| focus.deliver_event(&mut ui, ev))`.
- `grid_mods()` builds `Modifiers` from `self.ctrl/shift/meta`.
- Every widget demo has a visible `caption("Name")` title (from PR #234).

---

## 4. What is DONE (with code refs)
- ✅ `WidgetIntent` + `Event::Widget` (component.rs); `MenuNav`/`DialogNav`/`InputEdit` deleted.
- ✅ `Keymap` (keymap.rs) + `FocusManager::deliver_event` (focus.rs, for field-first non-key delivery).
- ✅ All 6 widgets migrated (context_menu/command_palette/select/tabs/dialog/input). Tests migrated
  (`heca-grid-ui/tests/phase_a.rs`, dialog.rs `#[cfg(test)]`).
- ✅ App: `widget_keymap`, `build_widget_keymap`, `combo_to_grid`, events.rs dispatch, startup/reload.
- ✅ heca-config `[keys.widgets]`; `keybindings.default.toml`.
- ✅ Showcase migrated; `route_*` now call `Keymap::dispatch`.
- ✅ Docs: `docs/widgets.md` (Input/Select/Tabs/Dialog/ContextMenu/CommandPalette), `README.md`
  (`[keys.widgets]` section), rustdoc.
- ✅ **Bug fixes this session:** macOS `Ctrl+letter` (`5127604` — use `event_combo` not logical key);
  named-key regression (`83f8361` — `combo_to_grid` case-insensitive).

---

## 5. The two bugs found & fixed this session (so you understand the traps)
1. **macOS `Ctrl+letter` → control char.** `winit`'s `logical_key` for `Ctrl+j` is a control char, not
   `Char('j')`. FIX: resolve from `event_combo` (which uses `normalize_key_text`'s physical-key
   fallback), via `combo_to_grid`. (`5127604`)
2. **Named keys capitalised.** `normalize_key_text` names a `NamedKey` via `format!("{:?}", n)` →
   `"ArrowDown"`, `"Tab"`, `"Enter"`, `"Escape"` (capitalised). `combo_to_grid` matched lowercase →
   returned `None` for every named key → the overlay dispatch was **skipped entirely**, breaking
   Tab/arrows/Enter/Escape AND Space/Enter button activation in overlays. FIX: lowercase the key name
   in `combo_to_grid` before matching. (`83f8361`)

**Trap for the next agent:** any conversion from the app's `KeyCombo` to a grid chord must be
**case-insensitive** on the key name — config strings are lowercased by `KeyCombo::parse`, but live
events name `NamedKey`s capitalised via `{:?}`.

---

## 6. WHAT TO DO NEXT (ranked)

### 6.1 [TOP] Centralize `focusable()` — a `Base` property, not 18 method overrides
**Decision (user 2026-07-12):** `Button::focusable()` returning `!disabled` is boilerplate repeated by
18 widgets. Make focusability a **declared property**, defaulted per widget, with `disabled` central.

**Design (locked):**
- Add `pub focusable: bool` to `Base` (`heca-grid-ui/src/component.rs`), default `false`.
- Change the `Component::focusable()` **trait default** to the single definition:
  `self.base().focusable && !self.base().disabled.get_untracked()`.
- Each widget **sets the property**, removes its method override:
  - always-focusable (`Input`, `Select`, `Tabs`, `Checkbox`, `Toggle`, `ScrollRegion`, `RailCell`) →
    `base.focusable = true` in the constructor.
  - conditional → set the flag when the callback is wired: `Button` in constructor (disabled now
    central), `IconButton`/`Item`/`Row`/`BadgeButton` inside `.on_click()`/`.on_activate()`.
  - non-focusable (`Toast`, `ToastStack`, `Modal`, `ContextMenu`, `CommandPalette`) → drop the
    override (default false).
  - **genuinely dynamic** (`Dialog` = focusable only while open) → keep a small override; that is the
    rare exception the escape hatch exists for.
- Find them: `grep -rn "fn focusable" heca-grid-ui/src/widgets/` (18 files).
- **Verify:** every currently-focusable widget stays focusable (run the showcase; Tab must still
  cycle Button/Input/Tabs/Select/Checkbox/Toggle). Grid-ui tests + clippy green.

### 6.2 [BUG] Dialog Tab reaches Input↔Cancel but **never OK** (needs the running app)
The rename dialog (app): Tab traverses Input and Cancel but skips the OK/primary button. Focus
enumeration IS centralized (`Base` focus signals + `FocusManager::advance`/`for_each_focusable`), so
this is NOT decentralization — a specific focusable isn't enumerated/reached. Leads:
- The action buttons are built in `heca/src/chrome/overlay.rs` `build_modal_root` (~line 338–363):
  `Button::new(action.label).on_click(...)` then wrapped by `super::action_tooltip(button, ...)` and
  added via `dialog.action(...)`. **Check the `Tooltip` wrapper** — does `for_each_focusable` recurse
  into a `Tooltip`'s child? And **check `spec.actions` order** for the rename modal.
- `Button::focusable()` is `!disabled` (button.rs:227) — so OK *should* be focusable. Instrument the
  dialog's embedded `FocusManager` to log how many focusables it yields and their identity.
- Likely interacts with §6.1 (do the focusable refactor first, then this may be clearer).

### 6.3 [VERIFY] Confirm the key fixes in-app (GPU app — user/next agent must run)
After `83f8361`, verify in `cargo run -p heca-renderer --example showcase` AND the heca app:
- Context menu: ↑/↓ and `Ctrl+k`/`Ctrl+j` navigate; Enter runs; Esc closes.
- Dialog: Tab/Shift+Tab traverse; `Ctrl+h`/`Ctrl+l` traverse; Enter submits / Space activates focused
  button; Esc cancels.
- Select: ↑/↓ and `Ctrl+k`/`Ctrl+j`; Input: `Ctrl+h`/`Ctrl+u`/`Ctrl+a`.

### 6.4 [DECISION] PR strategy
Fold `feat/widget-keymap-unify` into PR #234 (retarget/rebase), or open a 2nd stacked PR. User's call.

---

## 7. File reference map
| Concern | File |
|---|---|
| `WidgetIntent` + `Event::Widget`, `GridKey`/`Modifiers` (Hash) | `heca-grid-ui/src/component.rs` |
| `Keymap`/`KeyChord`/`dispatch`/`with_defaults` | `heca-grid-ui/src/keymap.rs` |
| `FocusManager` (advance, for_each_focusable, deliver_event, offer_to_overlay) | `heca-grid-ui/src/focus.rs` |
| `Base` (focus signals; add `focusable` in §6.1) | `heca-grid-ui/src/component.rs` |
| widgets consuming intents | `heca-grid-ui/src/widgets/{context_menu,command_palette,select,tabs,dialog,input}.rs` |
| widget tests | `heca-grid-ui/tests/phase_a.rs`, `.../widgets/dialog.rs` `#[cfg(test)]` |
| `AppState.widget_keymap` | `heca/src/app_state.rs` |
| `build_widget_keymap`, `combo_to_grid`, registry tests | `heca/src/app/registry.rs` |
| overlay key dispatch | `heca/src/app/events.rs` (keyboard handler, `top_modal` branch) |
| `build_event_combo`/`normalize_key_text` (KEY NAMING — capitalised NamedKey) | `heca/src/app/keyboard.rs` |
| app context menu / dialog build (modal layers) | `heca/src/chrome/overlay.rs`, `heca/src/chrome/context_menu.rs` |
| top_modal / layer stack | `heca/src/chrome/overlay.rs` (`top_modal`), `heca/src/chrome/layers.rs` (`top_modal_root_mut`) |
| config table field | `heca-config/src/keys.rs` (`KeysConfig.widgets`) |
| default bindings | `keybindings.default.toml` (`[keys.widgets]`) |
| showcase host | `heca-renderer/examples/showcase.rs` (`route_overlay_key`/`route_focused_key`/`keymap`) |

---

## 8. Gotchas
- **`cargo fmt` is banned** here; hand-format, verify with clippy.
- **KeyCombo→grid conversion must be case-insensitive** (§5).
- **`[keys.widgets]` must sit before `[[keys.mode]]`** in the TOML (no re-opening `[keys]`).
- **Keymap is host-owned, never a global** (detached-pane safety; grid-ui is `Rc`-single-threaded).
- **Widget unit tests drive semantic events** (`Event::Widget(..)`) directly — they never touch the
  `Keymap`, which is only exercised at the thin dispatch boundary + the `keymap.rs`/registry tests.
- **In-app visual pass is the user's** (GPU app). This session's fixes are verified by build/tests
  only; the interactive behavior (§6.3) still needs a human at the keyboard.
