# Theme Unification Plan

> Single source of truth for the theme refactor. Each phase ends with a review gate.
> Load `rust-skills` before writing code. Follow all rules in `AGENTS.md`.

---

## Handoff — 2026-06-21 (read before resuming; we'll start theming after pane-runtime Phase 9)

**Why this note:** clarifies the "default theme" confusion and records cross-cutting findings so the next session doesn't re-analyze from scratch.

**Two theme layers exist today — that's the confusion, not a bug:**
1. **App color palette** = `heca_config::theme::Theme` (`state.theme`, `[settings] theme`). **Currently defaults to `mocha`**; only `mocha` (dark) + `latte` (light) are bundled in `heca-config/src/themes/`. Drives terminal colors + is mapped onto the chrome.
2. **grid-ui widget base** = `heca_grid_ui::Theme::grid_tron()` (the `GuiTheme`). `chrome_gui_theme()` builds `grid_tron()` then **patches in** the palette's fg/surface/accent/border/radius/font_size. So the chrome you see = grid_tron *structure* + mocha *colors*.

So **"default = grid_tron" and "default = mocha" are both true at different layers** — not a conflict. `grid_tron` **is the intended config default** (Decisions Log below); the live `mocha` default is simply the symptom that **Phase 3 (consumer migration) is not done**. `heca-theme` is the **target** crate (Phases 1–2 ✅), today only wired into the `heca-renderer` showcase (`heca_theme_to_grid_ui` in `showcase.rs`) — it is NOT dead.

**Status recap:** Phase 1 ✅ (`heca-theme` crate: grid_tron/mocha/frappe + `load_theme` (light theme in code is still frappe; latte swap pending)). Phase 2 ✅ except 2.5/2.6 visual verify (showcase cycles themes live, works). **Phase 3 ⏳ is next** (migrate `heca-config` + `heca-grid-ui` to re-export `heca-theme`; wire `[settings].theme` + flip `default_theme()` to `grid_tron`; make `chrome_gui_theme()` a pass-through). Phase 4 deferred.

**Audit reconciliation (external agent, 2026-06-21) — corrections to apply during Phase 3:**
1. **Default theme — RESOLVED (2026-06-21):** default is **`grid_tron`**, overridable via `[settings].theme`; shipped alternatives are **mocha + latte**. Implementation: current code still defaults to `mocha` and `heca-config` only bundles mocha/latte (not grid_tron), so the default flips to grid_tron in **Phase 3** once the app loads from `heca-theme` (which bundles grid_tron) + `default_theme()` is changed. Decisions Log + 3A.8 below rewritten to match.
2. **3A.8 — RESOLVED:** the theme field already exists as `SettingsConfig.theme` (`[settings] theme`) and **stays there** (the `[appearance].theme` idea is dropped). No new field; only the default value changes in Phase 3. 3A.8 below rewritten.
3. **Light-theme glow assumption is stale:** the renderer now has explicit light-theme glow support (`heca-renderer/src/scene.rs` + `grid.wgsl`), so "Tron effects don't work on light backgrounds / latte must have glow off" is no longer a hard constraint — latte's glow/scanline values are a palette choice, not forced off. (Verify in 2.6.)
4. Phase 3 (consumer migration) is still **largely undone** and remains the active phase — `heca-config`/`heca-grid-ui` have no `heca-theme` dep yet; duplicate `color.rs`/`theme.rs` still exist in both. 3C hardcoded branches (`if theme.name == "Catppuccin Mocha"`, `chrome_colors()`, `Color::new(17,17,27,255)`) are all still present and valid targets.

**Requirement change (user, 2026-06-21): replace `frappe` with `latte` as the light theme.** Rationale: **Frappé is a *dark* Catppuccin flavor — Latte is the actual light/white one**; the plan mislabeled frappe as "light". Latte is also already the app's (`heca-config`) light theme, so this unifies both systems on one light palette. **This is a code task NOT yet done** — the references above were updated to `latte`, but `heca-theme` still ships `heca-theme/src/themes/frappe.toml` and the showcase still cycles `frappe`. Concrete steps:
- `heca-theme/src/themes/latte.toml` — author the **real Catppuccin Latte** palette (base it on `heca-config/src/themes/latte.toml` + add the grid-ui fields: `surface`, `muted`, `glow`, `danger`, `success`, `warning`, `glow_size`, `intensity`, `show_focus_border`, `icon_secondary_alpha`). Light glow is now allowed (reconciliation #3) — tune, don't force off.
- delete `heca-theme/src/themes/frappe.toml`; update `heca-theme/src/loader.rs` bundled list (`frappe`→`latte`).
- `heca-renderer/examples/showcase.rs` — `THEMES` cycle + switcher label `frappe`→`latte` ("Catppuccin Latte").
- then visually verify (2.6) the light theme renders correctly.
(The `[x]` on 1.6/2.x above now say "latte" but reflect the *target*, not current code — the swap is the task here.)

**Hard requirement (user, 2026-06-21): eliminate ALL hardcoded color/style values — everything must be theme-driven.** No literal colors/alphas/effect constants anywhere; every visual value reads from the `heca-theme::Theme` (adding a token if one is missing). This is the acceptance bar for Phases 3–4, covering at least:
- **App/chrome** (Phase 3C): the `if theme.name == "Catppuccin Mocha"` branch (`render.rs`), `chrome_colors()` + `Color::new(17,17,27,255)` (`chrome/mod.rs`), `RenderColors` arrays (`sidebar/render.rs`), `[0.118,0.118,0.180,0.7]` (`mouse/render.rs`), and `chrome_gui_theme()`'s manual patching → pass-through.
- **grid-ui widgets** (the "partly theme-driven" finding above): `IconButton` `HOVER_FILL_ALPHA`/`HOVER_BORDER_ALPHA` + the **white press flash** (`cx.flash` = `rgb(255,255,255)`), `Tag` `FILL_ALPHA`/`BORDER_ALPHA`/divider alphas, and any "muted danger"-type derived shade (currently a hand-`lerp`). Promote each to a `Theme` token.
- Grep gate before sign-off: no remaining `Color::new(`/`Color::rgb(`/`[0.` literal colors or `*_ALPHA` consts in render/widget paths that aren't theme-sourced.

**Two cross-cutting findings to fold into Phase 3/4:**
- **font landmine:** pane-runtime Phase 8 put a `state.theme.font_size` mapping inside `chrome_gui_theme()`, and the in-pane info bar reads `chrome_gui_theme().font_size`. Phase 3C.2 turns that fn into a pass-through — **fold the font handling into the unified `Theme`**, don't keep the patch. (Also note `[settings] font_family`/`font_size` now override the theme font via `loader::apply_overrides` — preserve that.)
- **widgets only PARTLY theme-driven:** grid-ui widgets read core colors from `cx.theme()` but **hardcode alphas/effects** that should be theme tokens — `IconButton` `HOVER_FILL_ALPHA` + the **white press flash** (`cx.flash` = `rgb(255,255,255)` in `component.rs`), `Tag` `FILL_ALPHA`/`BORDER_ALPHA`, and the pane-bar close-red is a hand-`lerp` (`danger.lerp(surface,0.25)`) for lack of a "muted danger" token. Promote these into `heca-theme::Theme` tokens during widget tokenization so a theme switch fully restyles them (the white flash especially looks wrong off-theme).

---

## Architecture

```
heca-theme (new crate, standalone)
├── Theme struct (unified — all fields from config + grid-ui)
├── Color type (with serde + hex parsing)
├── Intensity, GlowLevel enums
├── load_theme(name) → user config dir → bundled → grid_tron fallback
└── bundled themes/
    ├── grid_tron.toml  (dark, cyan Tron — DEFAULT)
    ├── mocha.toml      (dark, Catppuccin Mocha)
    └── frappe.toml    (light — code today; TARGET: replace with latte.toml)

heca-theme  ◄──  heca-config   (replaces its Theme/Color)
heca-theme  ◄──  heca-grid-ui  (replaces its Theme/Color/Intensity/GlowLevel)
heca-theme  ◄──  heca          (direct, for theme loading)
```

---

## Phase 1 — Create `heca-theme` crate

> Standalone crate: Theme struct, Color, bundled .toml themes, loader.

- [x] 1.1 Create `heca-theme/Cargo.toml` — deps: `serde` (derive), `toml`, `dirs`
- [x] 1.2 Create `heca-theme/src/color.rs` — merge grid-ui `Color` + config `Color` into one type (serde hex, `with_alpha`, `lerp`, `to_f32x4`, `FromStr`, `Display`)
- [x] 1.3 Create `heca-theme/src/theme.rs` — unified `Theme` struct with ALL fields:
  - From current config: `name`, `background`, `foreground`, `border`, `accent`, `font_family`, `font_size`, `terminal_*`, `border_radius`, `border_width`, `pane_padding`, `shadow`, `float_*`, `drag_*`, `drop_*`, `sidebar_*_font_size`
  - From current grid-ui: `surface`, `muted`, `glow`, `danger`, `success`, `warning`, `glow_size` (GlowLevel), `intensity` (Intensity), `show_focus_border`, `icon_secondary_alpha`
  - `Shadow` struct (color String, alpha f32, blur f32)
  - `Intensity` enum (Off/Low/Medium/Heavy) with `glow_scale()`, `scanline_opacity()`, `next()`
  - `GlowLevel` enum (None/Thin/Medium/Large) with `radius_scale()`, `parse()`, `ALL`, `label()`
  - `Theme::control_radius()` helper (radius * 0.5)
- [x] 1.4 Create `heca-theme/src/themes/grid_tron.toml` — current `grid_tron()` hardcoded values as TOML
- [x] 1.5 Create `heca-theme/src/themes/mocha.toml` — copy from `heca-config/src/themes/mocha.toml`, add missing grid-ui fields (`surface`, `muted`, `glow`, `danger`, `success`, `warning`, `glow_size`, `intensity`, `show_focus_border`, `icon_secondary_alpha`)
- [x] 1.6 Create `heca-theme/src/themes/frappe.toml` — Catppuccin Frappe palette (TARGET: replace with latte.toml), light theme defaults (`glow_size = "none"`, `intensity = "off"`)
- [x] 1.7 Create `heca-theme/src/loader.rs` — `load_theme(name)`:
  1. Try `~/.config/heca/themes/{name}.toml`
  2. Try bundled `{name}.toml` (via `include_str!`)
  3. Fallback to bundled `grid_tron.toml` (never fails)
- [x] 1.8 Create `heca-theme/src/lib.rs` — re-export all public types
- [x] 1.9 Add `heca-theme` to workspace `Cargo.toml` members
- [x] 1.10 Run `cargo check -p heca-theme` and `cargo test -p heca-theme`
- [x] 1.11 Run `cargo clippy -p heca-theme --all-targets --all-features` — fix all warnings
- [x] Load `/Users/antonio/.agents/skills/rust/SKILL.md` and review against its rules before marking complete

**Review gate:** Come to user for approval before proceeding to Phase 2.

---

## Phase 2 — Showcase live theme switching

> Validate the theme system works end-to-end by cycling themes in the showcase.

- [x] 2.1 Add `heca-theme` dependency to `heca-renderer/Cargo.toml`
- [x] 2.2 Update showcase: replace `Theme::grid_tron()` with `heca_theme::load_theme("grid_tron")`
- [x] 2.3 Add theme cycling to showcase — button in the button row cycles `grid_tron → mocha → frappe`, rebuilds the showcase tree on change, and updates the window title with the active theme
- [x] 2.4 Add a visible theme switcher button showing current theme name (`⇄ THEME: Grid Tron` / `⇄ THEME: Catppuccin Mocha` / `⇄ THEME: Catppuccin Frappe`)
- [ ] 2.5 Verify all widgets react to theme change (colors, radius, border, glow, fonts)
- [ ] 2.6 Verify the light theme renders correctly (frappe today; latte after the swap); light glow now supported
- [x] 2.7 Run `cargo clippy --all-targets --all-features` — fix all warnings
- [x] Load `rust-skills` and review before marking complete

**Review gate:** Come to user for approval before proceeding to Phase 3.

---

## Phase 3 — Migrate consumers to `heca-theme`

> Replace duplicate Theme/Color definitions in heca-config and heca-grid-ui.

### 3A — Migrate heca-config

- [ ] 3A.1 Add `heca-theme` dependency to `heca-config/Cargo.toml`
- [ ] 3A.2 Remove `heca-config/src/color.rs` — re-export `heca_theme::Color` from `heca-config::color`
- [ ] 3A.3 Remove `heca-config/src/defaults.rs` — move serde default helpers into `heca-config/src/theme.rs` or inline them (they use `Color` which now comes from `heca-theme`)
- [ ] 3A.4 Update `heca-config/src/theme.rs`:
  - Remove `Theme` struct definition — re-export `heca_theme::Theme`
  - Remove `Shadow` struct — re-export `heca_theme::Shadow`
  - Remove `catppuccin_mocha()` and `catppuccin_latte()` constructors — use `heca_theme::load_theme()`
  - Keep `Theme::load(name)` as thin wrapper around `heca_theme::load_theme(name)`
  - Keep `Theme::terminal_cell_size()` method (app-specific, not in heca-theme)
- [ ] 3A.5 Update `heca-config/src/loader.rs`:
  - `load_theme(name)` calls `heca_theme::load_theme(name)`
  - `apply_terminal_overrides()` stays here (app-specific terminal overrides)
- [ ] 3A.6 Update `heca-config/src/appearance.rs` — replace `use crate::color::Color` with `use heca_theme::Color`
- [ ] 3A.7 Update all other `heca-config` files that import `Color` or `Theme`
- [ ] 3A.8 Theme selection **already exists** as `SettingsConfig.theme` (`[settings] theme`) — do **NOT** add an `AppearanceConfig.theme` field, and do **not** move it. Keep it in `[settings]`. The only change: once the app loads themes from `heca-theme` (3A.4/3A.5 + 3C.7), flip `default_theme()` from `"mocha"` to `"grid_tron"` so the default is grid_tron unless the user sets `[settings].theme`.
- [ ] 3A.9 Update `heca-config/src/lib.rs` exports
- [ ] 3A.10 Run `cargo check -p heca-config` and `cargo test -p heca-config`
- [ ] 3A.11 Run `cargo clippy -p heca-config --all-targets --all-features`

### 3B — Migrate heca-grid-ui

- [ ] 3B.1 Add `heca-theme` dependency to `heca-grid-ui/Cargo.toml`
- [ ] 3B.2 Remove `heca-grid-ui/src/color.rs` — re-export `heca_theme::Color` from `heca-grid-ui::color`
- [ ] 3B.3 Update `heca-grid-ui/src/theme.rs`:
  - Remove `Theme` struct — re-export `heca_theme::Theme`
  - Remove `Intensity` enum — re-export `heca_theme::Intensity`
  - Remove `GlowLevel` enum — re-export `heca_theme::GlowLevel`
  - Remove `grid_tron()` and `grid_ares()` constructors
  - Keep `CONTROL_RADIUS_FRAC` constant if `control_radius()` is used (move to `heca-theme` or keep as re-export)
- [ ] 3B.4 Update `heca-grid-ui/src/lib.rs` — re-export `Theme`, `Intensity`, `GlowLevel` from `heca-theme` instead of local `theme` module
- [ ] 3B.5 Update `heca-grid-ui/src/prelude` — same re-exports
- [ ] 3B.6 Update `heca-grid-ui/src/component.rs` — `use crate::theme::Theme` should still work via re-export
- [ ] 3B.7 Verify all 30+ widgets still compile (they call `cx.theme()` — should work if re-export is correct)
- [ ] 3B.8 Update `heca-grid-ui/tests/phase_a.rs` — replace `Theme::grid_tron()` with `heca_theme::load_theme("grid_tron")`
- [ ] 3B.9 Run `cargo check -p heca-grid-ui` and `cargo test -p heca-grid-ui`
- [ ] 3B.10 Run `cargo clippy -p heca-grid-ui --all-targets --all-features`

### 3C — Migrate heca (main app)

- [ ] 3C.1 Add `heca-theme` dependency to `heca/Cargo.toml` (if not transitive enough)
- [ ] 3C.2 Fix `heca/src/chrome/mod.rs`:
  - `chrome_gui_theme()` → direct pass-through or clone of `heca_theme::Theme` (no more manual 5-field patching)
  - `chrome_colors()` → replace hardcoded `Color::new(17, 17, 27, 255)` with `theme.background`
  - Remove `use heca_grid_ui::theme::Theme as GuiTheme` — use `heca_theme::Theme` directly
- [ ] 3C.3 Fix `heca/src/app/render.rs`:
  - Replace `if theme.name == "Catppuccin Mocha" { [0.067, ...] }` with `theme.background.to_f32x4()`
  - All `primitive_renderer.draw_rect()` calls that use hardcoded colors → read from theme
- [ ] 3C.4 Fix `heca/src/app/terminal_render.rs` — update `terminal_pane_gui_theme()` to use `heca_theme::Theme`
- [ ] 3C.5 Fix `heca/src/sidebar/render.rs` — `RenderColors` should derive from theme, not hardcoded arrays
- [ ] 3C.6 Fix `heca/src/mouse/render.rs` — replace hardcoded `[0.118, 0.118, 0.180, 0.7]` with theme colors
- [ ] 3C.7 Wire `[settings].theme` (the existing `SettingsConfig.theme`) into config loading — it drives which theme `heca_theme::load_theme` resolves; change `default_theme()` to `"grid_tron"` so that's the default unless the user overrides. (NOT `[appearance].theme` — decided 2026-06-21.)
- [ ] 3C.8 Run `cargo check -p heca` and `cargo test -p heca`
- [ ] 3C.9 Run `cargo clippy --workspace --all-targets --all-features` — fix ALL warnings
- [ ] Load `rust-skills` and review before marking complete

**Review gate:** Come to user for approval before proceeding to Phase 4.

---

## Phase 4 — Hand-drawn → grid-ui widget migration (deferred)

> Migrate remaining `primitive_renderer.draw_*()` calls to proper grid-ui widgets.
> This is a larger effort — plan separately after Phase 3 is validated.

Areas to migrate:
- [ ] 4.1 Tab bar background + text (render.rs:222)
- [ ] 4.2 Status bar (render.rs:726-848)
- [ ] 4.3 Collapsed sidebar rail (render.rs:807-848)
- [ ] 4.4 Sidebar tree (sidebar/render.rs — 349 lines, biggest chunk)
- [ ] 4.5 Pane select/swap overlays (mouse/render.rs)
- [ ] 4.6 Decide on `grid_ares()` — keep as 4th bundled theme or drop

---

## Decisions Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Default theme | `grid_tron` — overridable via `[settings].theme`. Bundled target (grid_tron default; mocha + latte) — code today ships **frappe** not latte, swap pending. (Current code still defaults to `mocha`; flips to `grid_tron` in Phase 3 once the app loads from `heca-theme`, which bundles grid_tron — `heca-config` does not.) | User decision (2026-06-21) — Tron identity is the default; mocha/latte are the shipped alternatives |
| Theme crate | Separate `heca-theme` | Keeps grid-ui lean, avoids pulling in config/filesystem deps |
| Light theme | Latte (target; code today ships frappe) — glow now OPTIONAL (renderer supports light-theme glow; see Audit reconciliation #3) | ~~Tron effects don't work on light backgrounds~~ stale |
| `grid_ares()` | Deferred to Phase 4 | Only used in tests; decide after migration |
| Config field | `[settings].theme` (already exists) — **stays in `[settings]`** (decided 2026-06-21; the earlier `[appearance].theme` idea is dropped) | One theme key; settings already owns it |
| Fallback chain | user dir → bundled → grid_tron | Never fails, always has a valid theme |
