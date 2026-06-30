# feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-03-slice-2-grid-ui-theme-compose-callsite-churn — Slice 2 — grid-ui Theme compose + callsite churn

**Status:** 📋 `planned`
**Created:** 2026-06-30T16:52:37.199Z
**Updated:** 2026-06-30T16:57:45.793Z

Slice 2 — grid-ui Theme compose + ~47 callsite churn + modal shadow rework

Refactor heca-grid-ui/src/theme.rs to the locked compose shape: struct Theme { colors: heca_theme::Theme, font_family: String, font_size: f32, focus_border_width: f32 }. Drop the ~16 duplicated color fields + radius (→ colors.border_radius) + shadow: Color (→ colors.shadow token). Drop the local control_radius() + CONTROL_RADIUS_FRAC; reuse heca_theme::Theme::control_radius(). Drop grid_tron()/grid_ares() constructors. Then update the ~47 widget callsites in 21 files: cx.theme().<color-token> → cx.theme().colors.<token>; cx.theme().radius → cx.theme().colors.border_radius; cx.theme().control_radius() → cx.theme().colors.control_radius(); GUI extras (font_size/font_family/focus_border_width) stay top-level. Rework the modal drop-shadow (modal.rs) to the config-driven token: scene::Shadow { color: theme.colors.shadow.color.with_alpha(theme.colors.shadow.alpha), radius: theme.colors.shadow.blur * SHADOW_BLUR_MULT, dx: 0.0, dy: SHADOW_DROP } (replaces the hardcoded SHADOW_BLUR). SHADOW_BLUR_MULT is a per-widget multiplier (design knob); SHADOW_DROP offset stays a widget constant (no config token; follow-up optional shadow.offset token). Atomic by nature: the struct-shape change forces all callsites at once or it won't compile.

## Goals
- grid-ui::Theme is the compose struct { colors, font_family, font_size, focus_border_width } (16 dup color fields + radius + shadow: Color removed)
- ~47 widget callsites updated: cx.theme().X -> cx.theme().colors.X; radius -> colors.border_radius; control_radius() -> colors.control_radius()
- modal drop-shadow reads the config token (color+alpha from colors.shadow, blur = colors.shadow.blur * SHADOW_BLUR_MULT)
- phase_a.rs test updated to a Theme::from_colors helper
- grid-ui compiles + tests green

## Dependencies
- Slice 1 (grid-ui primitive types Color/Intensity/GlowLevel from heca_theme)

## Risks
- a widget reads a theme field not on heca_theme::Theme (e.g. card_background_alpha) - verify all 47 callsite tokens exist on heca_theme::Theme (audit says they do)
- SHADOW_BLUR_MULT value choice changes modal visuals - pick ~3.75 to preserve ~30 from default blur 8, document the chosen value
- atomic slice: the struct-shape change forces all callsites at once or it won't compile - cannot split further

## Completion Criteria
- cargo check -p heca-grid-ui green (all 30+ widgets compile after compose + churn)
- cargo test -p heca-grid-ui --all-targets green (phase_a.rs updated to Theme::from_colors helper)
- cargo clippy -p heca-grid-ui --all-targets --all-features 0 warnings
- no Theme::grid_tron()/grid_ares() remaining (grep)
- no cx.theme().radius / cx.theme().shadow (as a field) remaining (grep)
- no local CONTROL_RADIUS_FRAC / control_radius() in grid-ui
- rust-skills review done

## Tasks

### 📋 slice-2-grid-ui-theme-compose-callsite-churn-task-001-refactor-theme-to-compose — Refactor grid-ui/src/theme.rs to compose struct { colors, font_family, font_size, focus_border_width }

Status: 📋 `planned`

Refactor heca-grid-ui/src/theme.rs: replace the duplicate `pub struct Theme { ...16 color fields + radius + shadow: Color + font_family + font_size + border_width + focus_border_width + glow_size + intensity + show_focus_border + icon_secondary_alpha + active_wash_alpha + card_background_alpha }` with the locked compose shape: `pub struct Theme { pub colors: heca_theme::Theme, pub font_family: String, pub font_size: f32, pub focus_border_width: f32 }`. The 16 color fields, border_width, glow_size, intensity, show_focus_border, icon_secondary_alpha, active_wash_alpha, card_background_alpha, control_radius(), effective_* — all now come from `colors` (the embedded heca_theme::Theme). `radius` → `colors.border_radius`. `shadow: Color` → `colors.shadow` (the heca_theme::Shadow token). This is task theming-task-13. Atomic: forces all callsite edits (task 3) or it won't compile.

### 📋 slice-2-grid-ui-theme-compose-callsite-churn-task-002-drop-local-control-radius — Drop local control_radius() + CONTROL_RADIUS_FRAC; reuse heca_theme's

Status: 📋 `planned`

In heca-grid-ui/src/theme.rs: drop the local `impl Theme { pub fn control_radius(&self) -> f32 { self.radius * CONTROL_RADIUS_FRAC } }` and the `CONTROL_RADIUS_FRAC` const. Reuse `heca_theme::Theme::control_radius()` (accessed as `theme.colors.control_radius()`). Verify heca_theme::Theme::control_radius() exists and has the same semantics (it does — derives from border_radius with the same frac). This is part of task theming-task-13.

### 📋 slice-2-grid-ui-theme-compose-callsite-churn-task-003-drop-grid-tron-ares-ctors — Drop grid_tron()/grid_ares() constructors

Status: 📋 `planned`

In heca-grid-ui/src/theme.rs: drop the `pub fn grid_tron() -> Self` and `pub fn grid_ares() -> Self` constructors (they build a grid-ui::Theme with hardcoded defaults — the composed Theme now gets its color tokens from heca_theme::load_theme at the app adapter boundary, not from a constructor). grid_ares() is only used in tests (task theming-task-16 handles the test replacement). This is part of task theming-task-13.

### 📋 slice-2-grid-ui-theme-compose-callsite-churn-task-004-update-callsites-colors-field — Update ~47 widget callsites: theme().X → theme().colors.X (radius → colors.border_radius)

Status: 📋 `planned`

Update the ~47 widget callsites across 21 files (heca-grid-ui/src/widgets/*.rs + component.rs + style.rs): `cx.theme().<color-token>` → `cx.theme().colors.<token>` for the 16 color fields + border_width + glow_size + intensity + show_focus_border + icon_secondary_alpha + active_wash_alpha + card_background_alpha; `cx.theme().radius` → `cx.theme().colors.border_radius` (~8 sites); `cx.theme().control_radius()` → `cx.theme().colors.control_radius()` (scroll_bar, marker_group). GUI extras `cx.theme().font_size` / `.font_family` / `.focus_border_width` stay top-level (no `.colors`). This is task theming-task-13a. Mechanical, one-time. grep: `rg -n "\.theme\(\)\.[a-z_]+" heca-grid-ui/src` to find all; the ones NOT to touch are font_family/font_size/focus_border_width (GUI extras).

### 📋 slice-2-grid-ui-theme-compose-callsite-churn-task-005-rework-modal-shadow-token — Rework modal drop-shadow to config-driven token (blur = colors.shadow.blur * SHADOW_BLUR_MULT)

Status: 📋 `planned`

Rework the modal drop-shadow in heca-grid-ui/src/widgets/modal.rs to the config-driven token (task theming-task-13b). Today: `const SHADOW_BLUR: f32 = 30.0;` (line 43) + `Shadow { color: shadow, radius: SHADOW_BLUR, dx: 0.0, dy: SHADOW_DROP }` (line 252) where `shadow` = `t.shadow` (was a Color field). After compose: `t.shadow` is gone; use the token `t.colors.shadow` (heca_theme::Shadow { color: Color, alpha, blur }). Replace the hardcoded SHADOW_BLUR with `t.colors.shadow.blur * SHADOW_BLUR_MULT` (new const multiplier, design knob — pick ~3.75 so default blur 8 → ~30 to preserve current visuals; or document the chosen value). The scene::Shadow color becomes `t.colors.shadow.color.with_alpha(t.colors.shadow.alpha)`. So: `const SHADOW_BLUR_MULT: f32 = 3.75;` (replaces SHADOW_BLUR); `cx.drop_shadow(r.panel, radius, Shadow { color: theme.colors.shadow.color.with_alpha(theme.colors.shadow.alpha), radius: theme.colors.shadow.blur * SHADOW_BLUR_MULT, dx: 0.0, dy: SHADOW_DROP })`. Keep SHADOW_DROP as a widget constant (no config token). Optional follow-up: add shadow.offset to heca_theme::Shadow.

### 📋 slice-2-grid-ui-theme-compose-callsite-churn-task-006-slice2-gate-check-test-clippy — Gate: cargo check/test/clippy -p heca-grid-ui (compose + churn) + rust-skills review

Status: 📋 `planned`

Gate for Slice 2: `cargo check -p heca-grid-ui` green (all 30+ widgets compile after the compose + callsite churn — the struct-shape change forces all callsites at once) + `cargo test -p heca-grid-ui --all-targets` green (the phase_a.rs test still uses Theme::grid_tron() — that's updated in Slice 4 task-16, but the test will break here if not updated; EITHER update phase_a.rs here to the GuiTheme helper OR mark this test #[ignore] temporarily with a note — prefer updating it here so the gate is green) + `cargo clippy -p heca-grid-ui --all-targets --all-features` 0 warnings. rust-skills review before complete (repo mandate). Note: the test helper (Theme::from_colors or GuiTheme{...}) should be added in this slice so tests pass.

### 📋 slice-2-grid-ui-theme-compose-callsite-churn-task-007-update-phase-a-test-helper — Update heca-grid-ui/tests/phase_a.rs: replace Theme::grid_tron()/grid_ares() with a Theme::from_colors helper

Status: 📋 `planned`

Task theming-task-16: update heca-grid-ui/tests/phase_a.rs which today builds `Theme::grid_tron()` (removed in Slice 2). Add a small test helper that builds the composed Theme from a loaded heca-theme: either `Theme::from_colors(heca_theme::Theme)` (a grid-ui constructor with GUI-default font/focus_border_width) OR inline `GuiTheme { colors: heca_theme::load_theme("grid_tron"), font_family: <default>, font_size: <default>, focus_border_width: <default> }`. Prefer a `Theme::from_colors(colors: heca_theme::Theme) -> Self` constructor on grid-ui::Theme (also useful for the app adapter in Slice 3) with sensible GUI defaults. Replace all `Theme::grid_tron()` / `Theme::grid_ares()` in the test with the helper. (grid_ares() is only in tests — drop it.)
