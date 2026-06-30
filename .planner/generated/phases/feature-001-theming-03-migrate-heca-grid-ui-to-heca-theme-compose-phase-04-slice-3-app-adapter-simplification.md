# feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-04-slice-3-app-adapter-simplification — Slice 3 — app adapter simplification

**Status:** 📋 `planned`
**Created:** 2026-06-30T16:52:44.774Z
**Updated:** 2026-06-30T16:57:56.324Z

Slice 3 — app adapter simplification (chrome_gui_theme embeds colors + GUI extras)

App-side wiring (theming-plan.md 3C.2 + BACKLOG theming-04 task-18). Keep chrome_gui_theme()/app_theme_to_gui_theme() as the GUI-adapter; simplify to GuiTheme { colors: theme.clone(), font_family, font_size, focus_border_width } (no per-field color patching). Drop app_color_to_gui (same Color type now after Slice 1), shadow_to_gui (shadow is the token now, accessed via colors.shadow), glow_level_to_gui/intensity_to_gui (enums re-exported). Replace chrome_colors() hardcoded Color::new(17,17,27,255) with theme.background. Keep GuiTheme as the grid-ui adapter type (do NOT replace it with a bare heca_theme::Theme — grid-ui widgets read GUI-only fields font_size/focus_border_width that the color theme does not own). This contains/generalizes the compositor-04c adapter; it does NOT remove it. Scope: ONLY the chrome adapter simplification + the chrome_colors literal — the other 3C hardcoded-color cleanups (render.rs pane-select overlay, sidebar/render.rs RenderColors, mouse/render.rs hover colors, terminal_render.rs) are theming-04 and NOT in this slice unless trivially adjacent.

## Goals
- app adapter simplified to embed colors + fill GUI extras (no per-field color patching)
- dead conversion helpers removed (app_color_to_gui, shadow_to_gui, glow_level_to_gui, intensity_to_gui)
- chrome_colors() hardcoded Color::new(17,17,27,255) replaced with theme.background
- app + workspace build green

## Dependencies
- Slice 2 (grid-ui Theme composed; grid-ui::Color == heca_theme::Color; enums re-exported)

## Risks
- chrome_gui_theme override of focus_border_width from appearance.effective_focus_border_width must be preserved (compositor-04c behavior)
- alpha_u8 may be used by other code - grep before dropping (only drop if dead)
- the other 3C hardcoded colors are out of scope here - do not creep the slice

## Completion Criteria
- cargo check -p heca green
- cargo test -p heca --all-targets green (~266 tests)
- cargo clippy -p heca --all-targets --all-features 0 warnings
- cargo check --workspace green
- no app_color_to_gui/shadow_to_gui/glow_level_to_gui/intensity_to_gui remaining (grep)
- no Color::new(17,17,27,255) remaining in chrome/mod.rs (grep)
- chrome_gui_theme focus_border_width appearance override preserved
- rust-skills review done

## Tasks

### 📋 slice-3-app-adapter-simplification-task-001-simplify-app-adapter — Simplify app_theme_to_gui_theme/chrome_gui_theme to embed colors + GUI extras

Status: 📋 `planned`

Simplify heca/src/chrome/mod.rs `app_theme_to_gui_theme` + `chrome_gui_theme` to the compose shape (task theming-task-18). Today app_theme_to_gui_theme does per-field color copying via app_color_to_gui (heca_config::Color → grid-ui::Color) + shadow_to_gui + glow_level_to_gui + intensity_to_gui. After Slice 1 (grid-ui::Color == heca_theme::Color) + Slice 2 (GuiTheme = { colors, font_family, font_size, focus_border_width }): the adapter becomes `GuiTheme { colors: theme.clone(), font_family: font_config.family.ui_normal().to_string(), font_size: font_config.size.ui, focus_border_width: 1.5 }` (the 1.5 is then overridden by chrome_gui_theme from appearance.effective_focus_border_width — keep that override). No per-field color patching, no app_color_to_gui (same Color type), no shadow_to_gui (shadow is the token now), no glow_level_to_gui/intensity_to_gui (enums re-exported).

### 📋 slice-3-app-adapter-simplification-task-002-drop-dead-conversion-helpers — Drop app_color_to_gui/shadow_to_gui/glow_level_to_gui/intensity_to_gui (dead after compose)

Status: 📋 `planned`

Drop the now-dead conversion helpers in heca/src/chrome/mod.rs: `app_color_to_gui` (heca_config::Color → grid-ui::Color — same type now, identity), `shadow_to_gui` (shadow is the token now, accessed via colors.shadow), `glow_level_to_gui` (grid-ui re-exports heca_theme::GlowLevel — same type), `intensity_to_gui` (same). `alpha_u8` may still be used elsewhere — keep if so, drop if only used by shadow_to_gui. Verify with `rg -n "app_color_to_gui|shadow_to_gui|glow_level_to_gui|intensity_to_gui|alpha_u8" heca/src`. Remove dead ones (no `#[allow(dead_code)]` — repo rule: remove dead code).

### 📋 slice-3-app-adapter-simplification-task-003-chrome-colors-theme-background — Replace chrome_colors() hardcoded Color::new(17,17,27,255) with theme.background

Status: 📋 `planned`

Replace the hardcoded `Color::new(17, 17, 27, 255)` in `chrome_colors()` (heca/src/chrome/mod.rs:3089) with `theme.background` (the loaded theme's background token). This is the literal-color cleanup in theming-task-18. Verify with `rg -n "Color::new(17" heca/src/chrome/mod.rs` → should be gone after. (The other 3C literal cleanups — render.rs pane-select [1.0,0.9,0.3,0.9], mouse/render.rs [0.118,...], sidebar RenderColors — are theming-04 and NOT in this slice.)

### 📋 slice-3-app-adapter-simplification-task-004-slice3-gate-check-test-clippy — Gate: cargo check/test/clippy -p heca + cargo check workspace + rust-skills review

Status: 📋 `planned`

Gate for Slice 3: `cargo check -p heca` green + `cargo test -p heca --all-targets` green (heca has ~266 tests) + `cargo clippy -p heca --all-targets --all-features` 0 warnings. Then `cargo check --workspace` green (full build after the adapter change). rust-skills review before complete (repo mandate).
