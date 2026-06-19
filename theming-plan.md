# Theme Unification Plan

> Single source of truth for the theme refactor. Each phase ends with a review gate.
> Load `rust-skills` before writing code. Follow all rules in `AGENTS.md`.

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
    └── frappe.toml     (light, Catppuccin Frappe)

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
- [x] 1.6 Create `heca-theme/src/themes/frappe.toml` — Catppuccin Frappe palette, light theme defaults (`glow_size = "none"`, `intensity = "off"`)
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
- [x] 2.3 Add theme cycling to showcase — on key press `T`, cycle through `["grid_tron", "mocha", "frappe"]`, store current theme name, repaint with new theme next frame
- [x] 2.4 Add a small label in the showcase showing current theme name
- [ ] 2.5 Verify all widgets react to theme change (colors, radius, border, glow, fonts)
- [ ] 2.6 Verify light theme (frappe) renders correctly — glow/scanlines off, readable text
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
- [ ] 3A.8 Add `theme` field to `AppearanceConfig`:
  ```rust
  #[serde(default = "default_theme_name")]
  pub theme: String,  // default: "grid_tron"
  ```
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
- [ ] 3C.7 Wire `[appearance].theme` into config loading — `AppearanceConfig.theme` drives which theme is loaded
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
| Default theme | `grid_tron` | User preference — Tron identity is the default |
| Theme crate | Separate `heca-theme` | Keeps grid-ui lean, avoids pulling in config/filesystem deps |
| Light theme | Frappe with glow/scanlines off | Tron effects don't work on light backgrounds |
| `grid_ares()` | Deferred to Phase 4 | Only used in tests; decide after migration |
| Config field | `[appearance].theme` | Consistent with existing appearance config |
| Fallback chain | user dir → bundled → grid_tron | Never fails, always has a valid theme |
