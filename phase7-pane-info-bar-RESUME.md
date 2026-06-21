# Phase 7 — Pane Info Bar — RESUME / HANDOFF

> Detailed resume doc for the in-pane info bar work. Branch **`feature/phase-7`**, **PR #147 → main**.
> Read this + `pane-runtime-tasks.md` (Phase 7) + `pane-runtime-state-plan.md` (§0.3 + §4 Phase 7).
> Status as of 2026-06-21. **Slice 1, Slice 2 (action buttons), font decoupling, and straddle-widget removal
> = ALL DONE + committed on `feature/phase-7`** (`6a5d3d6` straddle removal, `344df73` font decouple,
> `b219a51` glyphs+parameterized actions, `be12e46` Slice 2 buttons). **Only remaining: live mouse
> verification** (button clicks fire their actions + don't leak to the terminal; hover highlight). Sections
> A/B/C below are historical design notes — all implemented; see `pane-runtime-tasks.md` Phase 7 for the
> as-built summary.

## What this feature is
A pane gets an **in-pane segmented info bar** = a `Tag` pill inside the pane top:
- **Left** = config segments (`location` / `app_name` / `git_branch` / `git_status`).
- **Right** = config action buttons (`split` / `move_left` / `move_right` / `close`) — **NOT built yet**.
Self-contained (theme `surface` header band, no border-matching). Replaced an earlier border-straddle
title (`Pane.title` + `PaneTitleStyle` Cut/Filled/Boxed) that fought the transparent pane over the terminal.

## DONE (Slice 1 — verified: clippy clean; tests config 219, heca 47, grid-ui 37+122, theme 17)
- **In-pane bar render:** `heca/src/app/terminal_render.rs` `paint_terminal_pane_shell` — builds the bar via
  `crate::chrome::build_pane_info_bar(...)`, computes layout, `translate_bounds` to the pane, draws a
  **header band** (`bar_theme.surface`, drawn *before* the frame so the rounded border traces over it),
  **vertically centers** the bar, **per-pane clip** so it can't spill into a neighbor.
- **Bar builder:** `heca/src/chrome/mod.rs` `build_pane_info_bar(programs, fallback_name, runtime, segments,
  theme, max_width, font)` — reuses `pane_info_view` projection; segments with no data are skipped; returns
  `None` if empty; **width-adaptive location truncation** (`truncate_path_left`, left-ellipsis keeps the tail).
- **Config:** `heca-config/src/appearance.rs` — `PaneSegment` + `PaneAction` enums; `pane_title_segments:
  Vec<PaneSegment>` (default `[Location, AppName]`) + `pane_title_actions: Vec<PaneAction>` (default all 4);
  `pane_info_bar_visible()` helper. **Removed** the old straddle config (`pane_title_style`/`pane_title_color`/
  `pane_title_background`, `pane_show_title`). `AppearanceConfig` is now `Clone` (not `Copy`) — fixed the two
  move sites (`main.rs:117`, `startup.rs:149` → `.clone()`).
- **Reserved space:** `pane_title_top_inset(state)` (terminal_render.rs) reserves a top strip for the bar so
  terminal content starts below; **gated on `pane_info_bar_shown(state)` = segments-only for now** (actions
  don't render yet — see Slice 2). Mirrored in `heca/src/app/terminal_host.rs` `content_rect_for_pane` /
  `inset_content_rect` so mouse→cell mapping matches the render rect.
- **Empty → no bar:** both lists `[]` ⇒ no band, no padding/margin.
- **Fonts:** UI = **Geist Mono** (embedded `GeistMono-*.ttf`); terminal = **Maple Mono Normal NF** (embedded
  `MapleMonoNormal-NF-*.ttf`). Removed all `JetBrainsMono Nerd Font`. Bar font = `chrome_gui_theme(state).
  font_size` (= sidebar size). **M7 sed-bug fixed:** terminal_font_family was wrongly clobbered to Geist in
  4 theme files — restored to Maple (latte/mocha config, frappe/mocha theme; grid_tron was already fine).
- **Configurable sidebar width:** `[appearance] sidebar_width` (clamped `MIN_SIDEBAR_WIDTH=160` ..
  `MAX_SIDEBAR_WIDTH=560`, default 300, wider than old 240). Applied at startup (`startup.rs`) + reload
  (`main.rs` — `set_left_size`/`set_right_size`; removed now-stale `#[expect(dead_code)]` on `set_right_size`).
- **Sidebar git branch:** `truncate_sidebar_git_branch` now **left-ellipsis** (keeps the tail, e.g.
  `…security-upgrade`), cap `SIDEBAR_GIT_BRANCH_MAX_CHARS = 22`; full branch on hover (tooltip). Fixes the
  sidebar card overflow.
- **example.config.toml:** documents `sidebar_width`, `pane_title_segments`, `pane_title_actions`.
- **Docs updated:** `PLAN.md` (status line + NfIcon backlog), `pane-runtime-state-plan.md` (§0.3 + §4 Phase 7),
  `pane-runtime-tasks.md` (Phase 7 board entry + activity log), `docs/widgets.md` (Pane/Tag).

## PENDING — pick up here

### A. Slice 2 — action buttons (the main remaining piece)
Right-aligned `IconButton` cluster in the bar, from `pane_title_actions`, each firing an existing WM action.
- **Hard part = interactivity.** The pane shell is painted **imperatively each frame with no event dispatch**;
  pane-area mouse events go to the **terminal**. The sidebar already has the pattern to copy: a **retained**
  widget tree + event dispatch (`chrome_dispatch_click` + hover dispatch in `heca/src/mouse.rs`). So: retain a
  per-pane header tree, dispatch `PointerMoved/Pressed` into it; if a button consumes it (`Handled::Yes`),
  do NOT forward to the terminal. Then `IconButton::on_click` fires + hover/press/focus work for free
  (reuse the showcase `IconButton`, do NOT hand-roll hitboxes).
- **Actions → existing `WmAction`s** via `ActionRegistry` (keyboard + click + RPC parity, per AGENTS rules).
  Map: `split`, `move_left`, `move_right`, `close`. **Need keybinds for tooltips** — likely `split=prefix+v`,
  `move_left=prefix+[`, `move_right=prefix+]`, **close = confirm which (prefix+x? prefix+w?)**. Tooltip text
  shows the keybind.
- After buttons render, **widen the gate**: `pane_info_bar_shown` → `pane_info_bar_visible()` (segments OR
  actions) in `terminal_render.rs`, and update the comment in `pane_info_bar_shown` (M6).
- When building buttons, subtract the button-cluster width from the bar's `max_width` so segments yield first.
- Files: `heca/src/chrome/mod.rs` (build bar w/ buttons), `heca/src/app/terminal_render.rs` (render + dispatch
  hookup), `heca/src/mouse.rs` (route pane-area events to the header), event/retain plumbing.
- Showcase + `docs/widgets.md` update.

### B. Decouple fonts from color themes → config.toml  (USER ASKED; NOT STARTED)
Today the **color theme preset bundles the font** (`Theme::load(settings.theme)`), and `[settings]` overrides
only the **terminal** font (`terminal_font_family`/`terminal_font_size` via loader `apply_terminal_overrides`,
`heca-config/src/loader.rs:149`). There is **no `[settings] font_family`/`font_size` for the UI font** → the UI
font is locked to the color theme. Fix (location confirmed = **`[settings]`**):
1. `heca-config/src/settings.rs`: add `font_family: Option<String>` + `font_size: Option<f32>` (mirror
   `terminal_font_family`/`terminal_font_size`; reuse the finite-positive validator for `font_size`).
2. `loader.rs` `apply_terminal_overrides` (rename `apply_overrides`): map settings → `theme.font_family` /
   `theme.font_size`.
3. **LANDMINE — `chrome_gui_theme` (`heca/src/chrome/mod.rs` ~L900):** it builds `GuiTheme::grid_tron()`
   (font_size **15**) and **never maps the config font size**. Meanwhile **`heca-config Theme.font_size`
   default = `32.0`** (theme.rs:123,164) — a DEAD value for the UI. So you **cannot** blindly do
   `gui_theme.font_size = state.theme.font_size` (UI would jump to 32). Plan: **normalize the config
   `Theme.font_size` default to the real UI size (~15)**, then set `gui_theme.font_size = state.theme.font_size`
   in `chrome_gui_theme`. The pane bar already reads `chrome_gui_theme(state).font_size`, so once mapped, both
   sidebar AND bar honor config `font_size`. Verify nothing else relied on `Theme.font_size == 32`.
4. example.config.toml `[settings]` block + a parse/override test.
- **Multiplier (confirmed):** `LayoutEngine.base_font` (host sets it to `theme.font_size`) → each widget =
  `base_font * style.font_scale * WidgetSize.font_scale()` (`heca-grid-ui/src/layout.rs build()`).
- (Optional, bigger) strip `font_family` out of the color preset `.toml`s so themes are colors-only; the
  Theme struct default + settings override then supply the font. Confirm serde defaults exist for the field.

### C. Remove the superseded straddle title widget (cleanup)
`heca-grid-ui/src/widgets/pane.rs`: `Pane.title()`, `PaneTitleStyle` (Cut/Filled/Boxed), `title_color`,
`title_background`, `paint_title`, `paint_title_backing`, `title_reserved_height`, the `TITLE_*` consts,
`truncate_to_width` (+ its tests), and `Glyph::secondary_char` (icon.rs) if unused elsewhere — all only used
by the **showcase** now (app uses the in-pane bar). Remove them + update `heca-renderer/examples/showcase.rs`
(replace the straddle demo with an in-pane-bar-style demo) + `docs/widgets.md`. This also dissolves review
**M1** (the `title_reserved_height` DRY/`DEFAULT_BASE_FONT` concern).

### D. Review items status (from the rust-skill review)
- **M1** DRY `title_reserved_height` — deferred; moot once C removes the straddle.
- **M2** `translate_bounds` dyn dispatch — leave (shallow tree, negligible).
- **M3** struct-update in `terminal_pane_gui_theme` — N/A (5 field overrides; `let mut` is idiomatic).
- **M4** per-frame allocs in `build_pane_info_bar` — leave (Tag API; ≤20 panes × ≤4 segments).
- **M5** `home_relative_path` calls `env::var_os("HOME")` per pane/frame — optional `OnceLock` cache (HOME is
  process-stable). Small follow-up if profiled.
- **M6** speculative comment in `pane_info_bar_shown` — left; resolves when Slice 2 widens the gate.
- **M7** font availability — RESOLVED: both faces embedded; terminal-font sed-bug fixed.

### E. Known minor visuals (tunables in `terminal_render.rs`)
- Header band uses uniform `border_radius` → bottom corners slightly rounded; square off if disliked.
- Bar centering uses an estimate (`BAR_TAG_VPAD`, `BAR_LINE_RATIO`, `BAR_VMARGIN`); tune if a hair off.
- Sidebar branch truncation is a fixed cap (22), not width-adaptive (threading width through 4
  `too_many_args` fns was judged not worth it). Make adaptive only if needed.

## Key files
- `heca/src/app/terminal_render.rs` — bar render, header band, reserve, `pane_title_top_inset`, `bar_font`,
  `translate_bounds`, `terminal_pane_gui_theme` (sets `background` = app bg).
- `heca/src/chrome/mod.rs` — `build_pane_info_bar`, `home_relative_path`, `truncate_path_left`,
  `truncate_sidebar_git_branch`, `chrome_gui_theme` (font_size mapping goes here for B).
- `heca-config/src/appearance.rs` — `PaneSegment`/`PaneAction`/`pane_title_*`/`sidebar_width` + clamps.
- `heca-config/src/settings.rs`, `loader.rs` — font override plumbing (for B).
- `heca/src/app/startup.rs`, `heca/src/main.rs` — startup + reload application of width/appearance.
- `heca/src/app/terminal_host.rs` — `content_rect_for_pane` mouse-mapping inset (must match render).
