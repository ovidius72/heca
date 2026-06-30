# feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose — theming-03 — Migrate heca-grid-ui to heca-theme (compose)

Status: 📋 `planned`

Migrate heca-grid-ui to compose heca-theme (NOT re-export). Locked design (grill-me 2026-06-30): grid-ui::Theme = { colors: heca_theme::Theme, font_family, font_size, focus_border_width }. Re-export Color/Intensity/GlowLevel. radius → colors.border_radius; shadow → colors.shadow config-driven token (Shadow.color: String→Color, blur = theme.colors.shadow.blur * SHADOW_BLUR_MULT replacing hardcoded SHADOW_BLUR). App adapter (app_theme_to_gui_theme/chrome_gui_theme) STAYS, simplified to embed theme + GUI extras. Contains/generalizes the compositor-04c adapter, does not remove it. Branch: feat/theming-03-grid-ui-port (rebased on origin/main). Plan source: theming-plan.md 3B + BACKLOG.md theming-03.

## Accepted Decisions
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

## Phases

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-01-slice-0-heca-theme-prereq — Slice 0 — heca-theme prereq

Status: 📋 `planned`

Slice 0 — heca-theme prereq: Shadow.color: String→Color + Color::with_alpha_f32

**Tasks:** 0/5 done

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-02-slice-1-grid-ui-primitive-re-export — Slice 1 — grid-ui primitive re-export

Status: 📋 `planned`

Slice 1 — grid-ui primitive re-export: Color/Intensity/GlowLevel from heca-theme

**Tasks:** 0/5 done

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-03-slice-2-grid-ui-theme-compose-callsite-churn — Slice 2 — grid-ui Theme compose + callsite churn

Status: 📋 `planned`

Slice 2 — grid-ui Theme compose + ~47 callsite churn + modal shadow rework

**Tasks:** 0/7 done

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-04-slice-3-app-adapter-simplification — Slice 3 — app adapter simplification

Status: 📋 `planned`

Slice 3 — app adapter simplification (chrome_gui_theme embeds colors + GUI extras)

**Tasks:** 0/4 done

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-05-slice-4-verify-rust-skills-review — Slice 4 — verify + rust-skills review

Status: 📋 `planned`

Slice 4 — verify: showcase theme cycling + workspace tests + clippy + rust-skills review

**Tasks:** 0/4 done
