# HANDOFF — focusable refactor · button styling · overlay policy · ViewNode architecture (2026-07-13)

> **Read first:** `AGENTS.md` (esp. the new **⭐ THE WIDGET ARCHITECTURE — `ViewNode`** block),
> `docs/widgets.md`, `pluggable-chrome-plugin-plan.md` §2.6.2/§2.7.2, `docs/plugin-authoring.md`.
> This handoff replaces the previous one. It is exhaustive on purpose.

---

## 0. Branch / PR state
- **Working branch: `feat/base-focusable`**, stacked on **`feat/widget-keymap-unify`** (**PR #235**, still OPEN → main). So this branch already contains #235's commits (widget-keys unification + disabled-button styling `e2ec620`) + a merge of `origin/main` (`d075ded`) + this session's work.
- Remote: `git@github.com:ovidius72/heca.git` (gh authed as `ovidius72`).
- **When #235 merges:** rebase `feat/base-focusable` onto `main` so its PR shows only this session's commits.
- Gates: `cargo build --workspace --all-targets` clean (only pre-existing transitive `block v0.1.6` note); `heca` 333 tests, `heca-grid-ui` 124+71 tests + doctests; `cargo clippy --workspace --all-targets --all-features` clean.

---

## 1. What was DONE this session (with files)

### 1a. Removed the legacy `Modal` widget (superseded by `Dialog`)
- **Why:** `Modal` (hand-drew its own buttons) and `Dialog` (real `Button` children) were two divergent widgets; the app uses only `Dialog` (`heca/src/chrome/overlay.rs` `build_modal_root` → `OverlayHost`). `Modal` was dead except the showcase + tests, and caused visible inconsistency (two different "Delete pane?" dialogs in the showcase).
- **Files:** deleted `heca-grid-ui/src/widgets/modal.rs`; removed exports (`widgets/mod.rs`, `lib.rs` both lists); fixed now-broken intra-doc links (`dialog.rs`, `command_palette.rs`, `context_menu.rs`); removed 6 Modal tests (`tests/phase_a.rs`); removed the showcase "Modal" demo (`heca-renderer/examples/showcase.rs`); removed the `### Modal` section + all widget-`Modal` refs in `docs/widgets.md` (kept app types `ModalSpec`/`ModalResult`/`ModalAction` + `Modal`-band).

### 1b. Centralized `focusable()` onto `Base`
- Added `pub focusable: bool` to `Base` (`heca-grid-ui/src/component.rs`, default `false`); changed the `Component::focusable()` **trait default** to `self.base().focusable && !self.base().disabled.get_untracked()`.
- **Per widget:** removed 13 boilerplate `focusable()` overrides and set the flag instead:
  - **Static (flag in constructor):** `button`, `checkbox`, `input`, `select`, `tabs`, `toggle`, `badge_button`, `scroll_region`.
  - **Callback-conditional (flag set inside `.on_click`/`.on_activate`):** `item`, `row`, `rail_cell`, `icon_button`, `toast`.
  - **Kept dynamic overrides:** `dialog`, `modal`(removed), `context_menu`, `command_palette`, `toast_stack` (`is_open()` / `!entries.empty()`).
- **NOTE — deviation from the old handoff §6.1 sketch:** it listed Toast/Modal/ContextMenu/CommandPalette/ToastStack as "drop to non-focusable" — but their code is dynamic/callback-based, so dropping would have *changed behavior*. Behavior was preserved exactly instead.
- Test: `focusable_is_driven_by_the_base_flag_and_disabled` (`tests/phase_a.rs`). Docs updated in `docs/widgets.md` (Base table, Component table, "Building a custom widget").

### 1c. `Button` styling — disabled + Secondary theme-consistency (`heca-grid-ui/src/widgets/button.rs`)
- **Disabled look** (committed in #235 as `e2ec620`, refined here): a disabled button drops accent/danger chrome to `muted`, label rendered `muted` at `DISABLED_CONTENT_ALPHA` (0.38), progress pinned to rest, weak scrim removed. Reads on every variant incl. transparent Ghost/Link.
- **Secondary theme-consistency (this session):** Secondary now fills with `theme.muted` at `SECONDARY_FILL_ALPHA` (36) + border from `theme.muted` (was `theme.surface` fill + `theme.border` border). Root cause: `surface`≈`background` on Tron (fill vanished) and `border`==`surface` on Mocha (border vanished); `muted` always contrasts the surface and never equals it. Fixes the cross-theme identity swing.
- Regression test `disabled_button_label_is_muted_and_faded_on_every_variant`; `docs/widgets.md` Button section updated.

### 1d. Overlay-capture **interaction policy** (`heca/src/app/interaction.rs`)
- **In `route_interaction`** (the policy router `dispatch_intent` consults), added at the top: `if crate::chrome::top_modal(state).is_some() { return RouteDecision::Block; }`.
- **This is a STATE condition (like `is_floating_domain`), NOT a new `ActionPolicy` variant** — `action_policy()` classifies actions on the *focus-domain* axis; overlay-open is orthogonal, so it lives in the router as `Block`. (Rationale is in the function doc comment; also see the memory `use-existing-policy-systems-never-hardcode`.)
- **Effect:** while a modal overlay (confirm `Dialog`, context menu / dropdown) is open, every WM action from keyboard/mouse/RPC is blocked — fixes `prefix+e`/`prefix+>` leaking behind a dialog. `SubmitOverlay`/`CloseOverlay` are intercepted **earlier** in `dispatch_intent`, so they still resolve; the overlay's own Esc/Space/Enter/nav go via the widget-keymap path (`events.rs`).

### 1e. Text-field uppercase fix (`heca/src/app/events.rs`)
- In the overlay key dispatch, for **printable** input the raw key is now taken from the actual `key_text` (case-preserved, shifted symbols intact) instead of the lowercased `combo_to_grid` key; the combo is used only for chord matching (which `Keymap::resolve` lowercases anyway). Space stays `GridKey::Space` (so it still activates a focused button); Ctrl/Meta combos + named keys keep the combo key. Fixes: Shift+letter → uppercase, `!@#` etc., in overlay `Input`s.

### 1f. Confirm dialog + random pane names (`heca/src/handlers.rs`, `actions.rs`, `main.rs`)
- **Confirm title:** `confirm_title` for `ClosePaneById` → `"Delete Pane?"` (unnamed) / `"Delete <custom_name>?"` (named) — uses `custom_name`, never a placeholder title.
- **Button verb:** `builtin_confirm_specs` `delete_pane` verb `"Close"` → `"Delete"` (`actions.rs`).
- **Random names removed:** `pane_name()` (`main.rs`) now returns `String::new()` (was the `PANE_NAMES` color list "Red"/"Green"/…). An unnamed pane shows no name (program/icon only). Fn kept (threaded as `pane_name_fn` pointer).

### 1g. `Glyph::CaretUp` (`heca-grid-ui/src/widgets/icon.rs`)
- Added variant + codepoint `0xe13c` (inferred from the alphabetical layout: down=`E136`, right=`E13A`) + entry in `ALL`. Renders in the showcase glyph grid **as an up-caret** (user confirmed the shape). **KEEP THIS — do not remove** (user directive). It was intended as the `Ctrl` (`⌃`) accelerator glyph but the user finds Phosphor's caret ugly → the accelerator will use **NerdFont** instead once embedded (see §3).

### 1h. AGENTS.md — the ViewNode architecture (the big directive)
- Added **⭐ THE WIDGET ARCHITECTURE — `ViewNode` + composition** block above "Creating new widgets". States the mandatory rule: widget *content* is composed from child `Component`s (the tree `realize` produces), realizable via `ViewNode`; **when you touch a widget you refactor it toward this** (no hand-drawn content; affordances are child slots). This is now a prime project rule.

---

## 2. DECISIONS locked (do not relitigate)
- **`Modal` widget is gone.** `Dialog` is the only confirm/overlay widget. `OverlayHost::open_modal` (`heca/src/chrome/overlay.rs`) builds a `Dialog` from `ModalSpec`.
- **`focusable` is a `Base` flag**, not a per-widget override (except genuinely dynamic overlays).
- **Overlay capture is a router `Block`** in `route_interaction`, **not** an `ActionPolicy` variant (state axis ≠ action axis). If per-action granularity is ever needed, add a *separate* `overlay_policy(action)` dimension, never a variant of the focus-domain enum.
- **Button accelerators / shortcuts:** decided model = per-button explicit `shortcut` (a prop/builder), triggered by **`Ctrl+<letter>`** (works in confirm AND form dialogs — no typing conflict), rendered **inside the button** as a **composed `Icon + Label`** (NOT hand-drawn `cx.icon`/`cx.text`, NOT adjacent/outside). The Button self-submits on `Ctrl+<c>`; a host helper (`FocusManager::deliver_accelerator`) routes the accelerator to the matching non-focused button. Letter is semantic (n=cancel/negative, y=confirm/positive), assigned by the caller/confirm-builder (a widget can't infer "Cancel"="No").
- **Glyph for the accelerator:** use **NerdFont**, not Phosphor `CaretUp` (kept in the enum but not used for this).
- **THE WIDGET ARCHITECTURE:** all widgets must move to the `ViewNode`/compositional model; refactor the widget you touch.

---

## 3. PLANNED / NOT DONE (the resume queue)

### 3a. [BIG, TOP] ViewNode → all widgets refactor (`plugin-ui` / `plugin-task-ui-9`)
The standing refactor AGENTS.md now mandates. Two parts:
1. **Complete `realize` coverage** (`heca/src/chrome/realize.rs`): currently missing `Select`, `Tabs`, `Grid`, `ItemGroup`, `DockFrame`, `MarkerGroup`, `ScrollBar`, `Toast` (need structured/list props — `plugin-task-ui-9`).
2. **Compose the leaf widgets** so their content is child `Component`s, not hand-drawn — the enabler for extension-by-slot. **`Button` is the first target** (it hand-draws its label text; it must become a container with a content slot [leading / label / trailing], mirroring `Item`'s slots). This unblocks the button-shortcut feature.
- **Do this in a FRESH session** (needs full context). Model + refs: `pluggable-chrome-plugin-plan.md` §2.6.2; `heca/src/chrome/{view.rs,realize.rs}`; `docs/widgets.md` "Declarative UI model".

### 3b. Button accelerator / shortcut (blocked on 3a Button compositional refactor + NerdFont)
- Add `Button::shortcut(char)` → composes an inner `Icon(NerdFont control glyph) + Label(letter)` trailing slot; renders inside; `Ctrl+<c>` self-submit; `FocusManager::deliver_accelerator`.
- `ModalAction.shortcut: Option<char>` prop set by the confirm builder (n=cancel, y=proceed).
- **NerdFont embedding first** (`gridui-03` `NfIcon`): vendor "Symbols Nerd Font Mono", confirm license, add a renderer font family/role, an `NfIcon` widget + curated `NfGlyph`. Then use its control/⌃ glyph.

### 3c. Smaller follow-ups
- **Dialog font-derived min-width** — `Style` has no `min_width`; needs a field + `to_taffy()` wiring, then set a `~22em` min on the Dialog panel so short confirms aren't cramped and dialogs share a baseline.
- **Confirm buttons — Ctrl+y/Ctrl+n universal accelerators** (agreed as the primary mechanism; part of 3b).
- **Context-menu nav (Ctrl-j/k/arrows)** — user reported it not navigating an open context menu. Code path traces correct (top_modal matches → widget-keymap delivers → `ContextMenu::event` returns `Handled::No` on raw arrows so the `Menu*` intent fires → `input_mode` reset to Normal at `input.rs:229`). **Needs a runtime repro** to find the real failure (likely the widget keymap defaults, or the `>` shifted-binding match).
- **AGENTS.md STOP list (line ~20)** still names `Modal` as an available widget — stale, remove it.

---

## 4. HOW TO RESUME
1. Read `AGENTS.md` ⭐ WIDGET ARCHITECTURE block + this file.
2. If PR #235 merged: `git fetch origin && git rebase origin/main` (drop the now-merged commits from `feat/base-focusable`).
3. Start the **ViewNode → all widgets** refactor (§3a) in a fresh session, beginning with **making `Button` compositional** (content slot), which then unblocks the accelerator feature (§3b).
4. Gates before any commit: `cargo clippy --workspace --all-targets --all-features` (0 warnings bar `block v0.1.6`), `cargo test -p heca-grid-ui -p heca`, don't run `cargo fmt`, load the rust SKILL and review.

## 5. File reference map
| Concern | File |
|---|---|
| ViewNode model / WidgetKind / PropValue / Intent | `heca/src/chrome/view.rs` |
| `realize(&ViewNode) -> Box<dyn Component>` (partial) | `heca/src/chrome/realize.rs` |
| Overlay build (Dialog from ModalSpec) | `heca/src/chrome/overlay.rs` (`build_modal_root`, `open_modal`, `open_dropdown`) |
| Overlay-capture policy | `heca/src/app/interaction.rs` (`route_interaction`) |
| Overlay key dispatch + uppercase fix | `heca/src/app/events.rs` |
| Confirm title/verb/names | `heca/src/handlers.rs` (`confirm_title`), `actions.rs`, `main.rs` (`pane_name`) |
| `Base.focusable` + `Component::focusable/shortcut` | `heca-grid-ui/src/component.rs` |
| Button (disabled + Secondary; the compositional refactor target) | `heca-grid-ui/src/widgets/button.rs` |
| `Glyph::CaretUp` | `heca-grid-ui/src/widgets/icon.rs` |
| Widget architecture rule | `AGENTS.md` (⭐ block) |
