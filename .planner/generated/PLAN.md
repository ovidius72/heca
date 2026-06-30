# heca — theming-03 (grid-ui → heca-theme compose) — Project Plan

> *Goal not defined yet.*

**Last updated:** 2026-06-30T16:58:03.761Z
**Version:** 1
**Project ID:** `34823f1f-59ec-4388-9b13-7aa477160a5d`

---

## Workflow Rules

---
## Features

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose — theming-03 — Migrate heca-grid-ui to heca-theme (compose)

Migrate heca-grid-ui to compose heca-theme (NOT re-export). Locked design (grill-me 2026-06-30): grid-ui::Theme = { colors: heca_theme::Theme, font_family, font_size, focus_border_width }. Re-export Color/Intensity/GlowLevel. radius → colors.border_radius; shadow → colors.shadow config-driven token (Shadow.color: String→Color, blur = theme.colors.shadow.blur * SHADOW_BLUR_MULT replacing hardcoded SHADOW_BLUR). App adapter (app_theme_to_gui_theme/chrome_gui_theme) STAYS, simplified to embed theme + GUI extras. Contains/generalizes the compositor-04c adapter, does not remove it. Branch: feat/theming-03-grid-ui-port (rebased on origin/main). Plan source: theming-plan.md 3B + BACKLOG.md theming-03.

Status: 📋 `planned`

**Phases:**
- 📋 **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-01-slice-0-heca-theme-prereq** Slice 0 — heca-theme prereq (0/5 tasks)
- 📋 **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-02-slice-1-grid-ui-primitive-re-export** Slice 1 — grid-ui primitive re-export (0/5 tasks)
- 📋 **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-03-slice-2-grid-ui-theme-compose-callsite-churn** Slice 2 — grid-ui Theme compose + callsite churn (0/7 tasks)
- 📋 **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-04-slice-3-app-adapter-simplification** Slice 3 — app adapter simplification (0/4 tasks)
- 📋 **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-05-slice-4-verify-rust-skills-review** Slice 4 — verify + rust-skills review (0/4 tasks)

**Accepted decisions:**
- **Approach = compose (embed heca_theme::Theme)**
  - Decision: Compose, not re-export: grid-ui::Theme = { colors: heca_theme::Theme, font_family, font_size, focus_border_width }
  - Rationale: grid-ui::Theme is a GUI-adapter (carries font_family/font_size/focus_border_width post-compositor-04c); re-exporting heca_theme::Theme would drop those and break 40+ widget callsites + the compositor-04c adapter.
  - Implementation: Refactor heca-grid-ui/src/theme.rs to embed heca_theme::Theme in a `colors` field; keep GUI-only extras as direct fields. App adapter embeds theme.clone().
  - Accepted at: 2026-06-30T00:00:00Z
- **Access = explicit `colors` field, not Deref**
  - Decision: Explicit named field `colors` (cx.theme().colors.<token>); NOT impl Deref<Target=heca_theme::Theme>
  - Rationale: rust-skills cautions Deref-polymorphism for non-smart-pointer; Deref would be border-line here and risk a commit-time review fail. Explicit field keeps the API clear (color tokens vs GUI extras).
  - Implementation: ~47 callsites in 21 files: insert `.colors`. GUI extras (font_size/font_family/focus_border_width) stay top-level.
  - Accepted at: 2026-06-30T00:00:00Z
- **radius -> colors.border_radius (rename)**
  - Decision: Rename grid-ui Theme.radius -> colors.border_radius (pure rename of heca_theme::Theme.border_radius, same f32); drop local control_radius()/CONTROL_RADIUS_FRAC, reuse heca_theme's
  - Rationale: grid-ui radius is already an alias of heca_theme.border_radius (adapter copies radius: theme.border_radius); renaming dedups it and exposes it via colors.
  - Implementation: ~8 callsites theme.radius -> theme.colors.border_radius; control_radius() calls -> theme.colors.control_radius().
  - Accepted at: 2026-06-30T00:00:00Z
- **shadow = config-driven token; Shadow.color: String->Color; blur x multiplier**
  - Decision: shadow = config-driven token via colors.shadow (heca_theme::Shadow); prereq Shadow.color: String->Color; modal blur = theme.colors.shadow.blur * SHADOW_BLUR_MULT (replaces hardcoded SHADOW_BLUR); offset SHADOW_DROP stays widget constant
  - Rationale: Respects the repo rule (no hardcoded values that override config; widget-multiplier pattern like badge radius*2.0). heca_theme::Shadow.color was the only color field still a String (inconsistent); aligning to Color is a clean fix. SHADOW_BLUR=30 hardcode overrode config blur=8.
  - Implementation: heca-theme: Shadow.color: String->Color + Color::with_alpha_f32. Modal: scene::Shadow { color: colors.shadow.color.with_alpha(colors.shadow.alpha), radius: colors.shadow.blur * SHADOW_BLUR_MULT, dx, dy }. Drop grid-ui Theme.shadow: Color field.
  - Accepted at: 2026-06-30T00:00:00Z
- **App adapter stays, simplified (contains compositor-04c)**
  - Decision: app_theme_to_gui_theme/chrome_gui_theme STAY (simplified to embed colors + GUI extras); drop app_color_to_gui/shadow_to_gui/glow_level_to_gui/intensity_to_gui
  - Rationale: compositor-04c established the adapter (fills grid-ui GUI theme from heca_theme::Theme + FontConfig + appearance). theming-03 contains/generalizes it, does not remove it. Removes the per-field color-copying boilerplate now that Color/enums are shared.
  - Implementation: Adapter becomes GuiTheme { colors: theme.clone(), font_family, font_size, focus_border_width }; chrome_gui_theme keeps the appearance.effective_focus_border_width override.
  - Accepted at: 2026-06-30T00:00:00Z

---
## Requirements

_No requirements defined yet._

---
## Phases

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-01-slice-0-heca-theme-prereq — Slice 0 — heca-theme prereq

Slice 0 — heca-theme prereq: Shadow.color: String→Color + Color::with_alpha_f32

Status: 📋 `planned`

**Completion criteria:**
- cargo test -p heca-theme green
- cargo clippy -p heca-theme --all-targets --all-features 0 warnings
- cargo check --workspace green (the 2 fallout-fix sites compile)
- no test assumed shadow.color was String
- rust-skills review done

**Tasks:** 0/5

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-02-slice-1-grid-ui-primitive-re-export — Slice 1 — grid-ui primitive re-export

Slice 1 — grid-ui primitive re-export: Color/Intensity/GlowLevel from heca-theme

Status: 📋 `planned`
Dependencies: Slice 0 (needs with_alpha_f32 on heca_theme::Color)

**Completion criteria:**
- cargo check -p heca-grid-ui green
- cargo test -p heca-grid-ui --all-targets green
- cargo clippy -p heca-grid-ui --all-targets --all-features 0 warnings
- all 30+ widgets compile (Theme still old duplicate struct; only primitive types swapped)
- Intensity/GlowLevel diff documented (identical) or reconciled
- rust-skills review done

**Tasks:** 0/5

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-03-slice-2-grid-ui-theme-compose-callsite-churn — Slice 2 — grid-ui Theme compose + callsite churn

Slice 2 — grid-ui Theme compose + ~47 callsite churn + modal shadow rework

Status: 📋 `planned`
Dependencies: Slice 1 (grid-ui primitive types Color/Intensity/GlowLevel from heca_theme)

**Completion criteria:**
- cargo check -p heca-grid-ui green (all 30+ widgets compile after compose + churn)
- cargo test -p heca-grid-ui --all-targets green (phase_a.rs updated to Theme::from_colors helper)
- cargo clippy -p heca-grid-ui --all-targets --all-features 0 warnings
- no Theme::grid_tron()/grid_ares() remaining (grep)
- no cx.theme().radius / cx.theme().shadow (as a field) remaining (grep)
- no local CONTROL_RADIUS_FRAC / control_radius() in grid-ui
- rust-skills review done

**Tasks:** 0/7

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-04-slice-3-app-adapter-simplification — Slice 3 — app adapter simplification

Slice 3 — app adapter simplification (chrome_gui_theme embeds colors + GUI extras)

Status: 📋 `planned`
Dependencies: Slice 2 (grid-ui Theme composed; grid-ui::Color == heca_theme::Color; enums re-exported)

**Completion criteria:**
- cargo check -p heca green
- cargo test -p heca --all-targets green (~266 tests)
- cargo clippy -p heca --all-targets --all-features 0 warnings
- cargo check --workspace green
- no app_color_to_gui/shadow_to_gui/glow_level_to_gui/intensity_to_gui remaining (grep)
- no Color::new(17,17,27,255) remaining in chrome/mod.rs (grep)
- chrome_gui_theme focus_border_width appearance override preserved
- rust-skills review done

**Tasks:** 0/4

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-05-slice-4-verify-rust-skills-review — Slice 4 — verify + rust-skills review

Slice 4 — verify: showcase theme cycling + workspace tests + clippy + rust-skills review

Status: 📋 `planned`
Dependencies: Slice 0, Slice 1, Slice 2, Slice 3

**Completion criteria:**
- cargo run -p heca-renderer --example showcase - all widgets react to theme cycling; latte ok
- cargo test --workspace --all-targets - only the 2 known pre-existing failures (white-press-flash toast; flaky PTY exit-captures)
- cargo clippy --workspace --all-targets --all-features 0 warnings (block v0.1.6 future-incompat acceptable)
- rust-skills review checklist complete; clippy --fix applied; all findings fixed

**Tasks:** 0/4
