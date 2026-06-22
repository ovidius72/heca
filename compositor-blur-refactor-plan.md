# Compositor Blur Refactor — z-Layer Background Frost

> Plan for replacing the tiled-tint + OS-vibrancy blur approach with a **heca-owned
> z-layer background**: a blurred translucent gradient behind panes, giving a
> real, cross-platform, tunable frosted-glass effect for both tiled and floating
> panes.
>
- **Status:** in progress — Phases 0–4 complete (PR #165 = Phase 1, PR #167 = Phase 2, PR #169 = Phase 3, PR #170 = docs check-off, Phase 4 docs = this PR); Phase 5 (interactive visual tuning with the user) next.
- **Branch (to create):** `feature/compositor-blur-refactor` (off latest `origin/main`)
- **Depends on:** `feature/terminal-blur` (PR #125) findings — the architectural
  conclusion that heca (an app, not a compositor) cannot blur the real desktop
  cross-platform, and must blur content it renders itself.
- **Supersedes:** the frosted-tint approach in `feature/terminal-blur`
  (`terminal_frost_color` / `terminal_frost_opacity()` / the Pass A tint).

---

## 0. Reconciliation with the theme-unification merge (PR #160)

> Added 2026-06-21 after PR #160 (`feat(theme): replace frappe with latte and
> polish theme surfaces`) landed on `main`. The merge migrated `heca-config` to
> re-export `heca-theme` types, deleted `defaults.rs`, replaced `frappe` with a
> bundled `latte`, added chrome background theme tokens
> (`left_sidebar_background` / `right_sidebar_background` /
> `top_bottom_pane_background`), and introduced a `content_canvas_fill()`
> stopgap in the render pipeline. Several tasks below were written against the
> pre-merge layout and are updated here so the plan is executable verbatim off
> the current `main`. Inline task edits are marked **(post-PR-#160)**.

The merge introduced these changes that affect this plan (detailed inline in
the tasks, summarized here):

1. **`Theme` struct relocated.** `pub struct Theme` now lives in
   `heca-theme/src/theme.rs:156`; `heca-config/src/theme.rs` is a re-export shim
   (`heca_theme::{Color, GlowLevel, Intensity, Shadow, Theme}`).
   `heca-config/src/defaults.rs` is **deleted**. → Task 2.3 rewritten.
2. **Theme definitions are TOML.** Bundled themes are
   `heca-theme/src/themes/{grid_tron,mocha,latte}.toml` (note: `latte` replaced
   `frappe`; the active set is `grid_tron` / `mocha` / `latte`; `frappe` exists
   only in historical notes). → Task 2.3 + Phase 4 TOML cleanup updated.
3. **`content_canvas_fill()` stopgap.** `heca/src/app/render.rs` now paints a
   theme-background fill over pane-less content area (helpers `FillRect` /
   `subtract_fill_rect` / `uncovered_fill_rects` + tests) in the exact slot z=0
   will occupy. `theming-plan.md` flags it as a stopgap to fold into z=0. This
   plan did not reference it. → New Task 3.4b added to remove it alongside z=0.
4. **Clippy baseline is now 0**, not 9 (the pre-merge "9 pre-existing lints"
   from PR #125 are gone as of PR #160). → Phase 0.2 + 6.2 updated.
5. **`cargo test -p heca-core` has a failing bash-integration test** on the
   current baseline (`terminal_backend_bash_integration_reports_success_error_and_cwd`).
   It is almost certainly env-dependent (shell-integration exit reporting), not
   caused by the theme merge (the only `heca-core` change was adding
   `cursor_color` to `TerminalSnapshot`). → Phase 0.2 now requires verifying it
   is pre-existing before starting.
6. **`latte` ships a transparent terminal background**
   (`terminal_background = "#e6e9ef00"`, alpha 0). This bypasses the plan's
   `terminal_transparency` → `surface_alpha` translucency channel and is the
   prime suspect for the deferred "no blur/transparency" regression. → Phase 3
   critical review + Phase 5.1 now require reconciling which channel owns pane
   translucency.

New theme tokens added by the merge (`left_sidebar_background`,
`right_sidebar_background`, `top_bottom_pane_background` on `Theme`) do **not**
conflict with this plan; `background_gradient_top/bottom` are additional fields
on the same struct.

---

## 0.1 Decisions from the 2026-06-21 grill-me (locked)

> The grill-me session before implementation resolved the open design questions
> and locked the model below. These supersede any conflicting earlier wording in
> §2/§3/risk-register; the inline edits there are kept consistent with these.

**The user's architectural model (governs the whole plan):**

1. **Terminal owns its translucency/blur** via config.toml knobs (`terminal_transparency`,
   `terminal_floating_blur`). Already implemented.
2. **Pane = border + padding only, no background fill.** Verified: `paint_terminal_pane_shell`
   sets `.border()` + `.radius()` only (no `.background(...)`), and the `Pane` widget
   paints a transparent fill + border when no background is set — so principle 2 is
   **already satisfied** at the widget level. No change needed there.
3. **App/window owns transparency + blur** (already implemented), and to give the
   blur something to blur, the background is a **gradient** (not a solid color) —
   this is the z=0 layer heca renders itself.

**Decided (with the option letter from the grill):**

| # | Question | Decision |
|---|---|---|
| Q1 | Which channel owns pane translucency? | **Option A** — theme `terminal_background` is **opaque**; `terminal_transparency` → `surface_alpha` is the **only** translucency channel. **Action:** fix `heca-theme/src/themes/latte.toml` `terminal_background = "#e6e9ef00"` → `"#e6e9ef"` (the alpha-0 value was the prime suspect for the deferred "no blur/transparency" regression). |
| Q2 | Where do the z=0 gradient colors come from? | **Option C** — explicit `background_gradient_top` / `background_gradient_bottom` `Option<Color>` fields on `Theme` (set per-theme in the TOMLs), with a **derived fallback** when unset: top = `theme.background`, bottom = a shifted variant. Zero-config default + full per-theme/user control. |
| Q2-extra | CRT scanline placement (`intensity` theme value)? | **z=0.5** — scanlines render **over the blurred gradient, under panes** (they frost with the background, not across pane content). Add a scanline overlay pass at z=0.5 to the pipeline order. |
| Q3 | Tiled terminal frost ownership | **Option A** — **drop** `terminal_blur` + `terminal_frost_color` for tiled panes; the tiled terminal's frost **is** the z=0 gradient blurred by `background_blur` showing through the translucent surface. One blur source (app z=0). Keep `terminal_transparency` (translucency) and `terminal_floating_blur` (floating real blur). **Action:** update docs + `example.config.toml` removing `terminal_blur` references. |
| Q4 | z=0 default opacity | **Option A** — opaque by default (`background_transparency = 0`). Clean cross-platform frost out of the box (no sharp desktop on Linux/Windows); user opts into translucency. macOS-vs-others rationale: macOS has native window vibrancy (OS blurs the desktop behind a translucent window); Linux/Windows have no reliable cross-platform OS-vibrancy, so a translucent window there shows a **sharp** desktop — hence z=0 must own the frost and default opaque. |
| Q5 | Start execution | Merge state already resolved — **#163 and #164 were merged** (not awaiting approval, as an earlier stale note claimed). Branch off `origin/main` (`351d857`), run Phase 0, begin Phase 1. |
| Q6 | macOS vibrancy theme-matching | **DEFERRED pending z=0 visual evaluation** — see §3 vibrancy note. |

### Q6 / macOS vibrancy — try WITHOUT it first

The user's directive: **implement our own heca-owned blur/transparency (z=0)
that looks like the macOS vibrancy frosted-glass effect, WITHOUT using OS
vibrancy. Try z=0 + our opacity + our blur, and see how it comes.**

- **Why this is the right first step:** vibrancy is an OS-foreign layer whose color
  heca does not control — it follows the macOS system Light/Dark appearance and is
  tinted by the desktop wallpaper, so it **cannot match the heca theme** (e.g. light
  `latte` theme + macOS Dark mode → dark vibrancy behind a light theme = clash), and
  it does not reload on `prefix+Shift+r`. Since heca defines its own theme regardless
  of the system theme, vibrancy-follows-OS is a real mismatch, not a cosmetic one.
- **The plan's z=0 model already makes vibrancy optional** (§3 lists it as an
  optional macOS bonus). The grill-me confirmed: **try z=0 alone first.** If the
  heca-owned gradient+blur frost looks good cross-platform, vibrancy may be
  unnecessary.
- **Parked work (only revisit if z=0 frost is insufficient on macOS):** adding a
  light/dark detection on `Theme` (explicit `appearance: Option<Light|Dark>` field
  + luminance-of-`theme.background` fallback) and calling
  `NSVisualEffectView::setAppearance(vibrancyLight/vibrancyDark)` so vibrancy
  tracks the heca theme — **plus live-reload** by retaining the effect-view handle
  and updating `setAppearance`/`setMaterial` on `prefix+Shift+r` (currently
  vibrancy is apply-once with the handle dropped; the reload path explicitly does
  not re-apply to avoid stacking effect views). This is **not** part of Phases 1–5.
  If needed, it becomes a follow-up task after Phase 5 user sign-off.
- **Do not wire vibrancy in Phases 1–5.** The success criterion "cross-platform:
  tiled + floating frost render identically via wgpu (no OS-vibrancy dependency)"
  is the goal; vibrancy stays `Vibrancy::None` by default throughout this work.

---

## 1. Goal & motivation

Give heca panes a **real frosted-glass blur** that:

1. **Works cross-platform** (Linux / macOS / Windows) — no OS-vibrancy dependency.
2. Is **heca-tunable** (`terminal_blur`-style knobs control the wgpu blur radius,
   not the OS).
3. Is **per-pane** (each pane opaque or translucent independently).
4. **Solves the floating-pane text collision** — a floating terminal's text must
   not collide with the tiled content behind it (no sharp un-blurred leak).

### Why the current approach fails

- **Tiled panes** currently get a frosted *tint* (`terminal_frost_color` rect over
  macOS vibrancy). heca cannot blur the desktop (the OS composites it *behind* the
  window, outside heca's render target), so the tint is indistinguishable from the
  OS vibrancy and is macOS-only.
- **Floating panes** get a real blur of tiled content, but the backdrop stamps it
  at `floating_frost_opacity = √(blur/100) ≈ 0.95`, leaving **5% sharp un-blurred
  content** leaking through → the "text collides / doesn't look good" defect.

### The z-layer model (the fix)

heca renders its **own background layer at z=0** (a 2-color gradient, blurred once
and cached), then composites pane content at z=1 (tiled) / z=2 (floating) over it:

```
z=0   background layer (heca-owned): 2-color vertical gradient
        → blurred by heca (wgpu Kawase), ONCE, cached (static — recompute on resize/param change)
        → composited at alpha = background_alpha() (1 − background_transparency/100)
z=1   tiled panes: translucent terminal surface (terminal_transparency) reveals z=0 frost
z=1.5 blurred-z=1-content (dynamic, per-frame) — only when floating panes exist
z=2   floating panes: translucent surface reveals z=1.5 frost (blurred tiled content)
        → backdrop stamped at 100% (no raw leak → no collision)
```

This is exactly niri/hyprland's model, but heca owns the "wallpaper" (the gradient)
instead of the OS, so it is **cross-platform and tunable**.

---

## 2. Locked decisions (defaults baked into the plan)

| Decision | Choice | Rationale |
|---|---|---|
| Knob naming | **New clean knobs** (`background_blur`, `background_gradient_top/bottom`, `background_transparency`); **drop** `terminal_blur`/`terminal_frost_color` for tiled | Two names for one thing is confusing; manual config migration is acceptable |
| Gradient | **Vertical only** (top→bottom), 2 colors | Simplest shader; sufficient for v1 |
| z=0 default opacity | **Opaque** (`background_transparency = 0`) | Guarantees cross-platform clean frost (no sharp desktop on Linux/Win); user opts into translucency |
| z=0 blur | **Static + cached** (recompute only when dirty: resize / gradient / blur change) | Gradient is static → blur once, cheap |
| Floating backdrop | **100% opacity** when blur active (drop `floating_frost_opacity` coupling) | Kills the 5% sharp-text leak → no collision; frost visibility = `terminal_floating_transparency` |
| Float blur reuse | Reuse the shared `state.blur` for the (rare) dirty-frame z=0 blur; copy `view_b` → cache before the floating blur runs | Minimal new GPU resources (only z0 render target + cache texture) |
| Translucency channel (Q1, grill-me 2026-06-21) | **Option A** — opaque theme `terminal_background`; `terminal_transparency`/`surface_alpha` is the only translucency channel | Theme bg alpha was baking a second translucency channel (latte `#e6e9ef00`); fix to opaque `#e6e9ef` |
| Gradient color source (Q2) | **Option C** — explicit `background_gradient_top/bottom` theme fields with derived fallback (top=bg, bottom=shifted) | Zero-config default + full per-theme/user control |
| CRT scanlines (Q2-extra) | **z=0.5** — over the blurred gradient, under panes | Scanlines frost with the background, not across pane content |
| Tiled frost ownership (Q3) | **Option A** — drop `terminal_blur`/`terminal_frost_color` for tiled; tiled frost = z=0 (`background_blur`) | One blur source; kills the "two names for one thing" confusion |
| macOS vibrancy (Q6) | **Try z=0 WITHOUT vibrancy first**; vibrancy deferred pending z=0 visual evaluation; do not wire in Phases 1–5 | heca owns its theme; vibrancy follows the OS and cannot match the theme |

---

## 3. Scope & out-of-scope

**In scope:**
- z=0 gradient + static cached blur (heca-renderer).
- Config knobs + resolvers (heca-config).
- Pipeline integration: z=0 base blit, remove tiled tint, floating backdrop-100% fix (heca).
- Docs (keybindings.toml, README, AGENTS, ARCHITECTURE).
- Visual tuning to a user-approved look.

**Out of scope (explicitly deferred):**
- Horizontal/diagonal/radial gradients, multi-stop gradients (v1 = 2-color vertical).
- Background **image** support (wallpaper). The design leaves room for it later
  (z=0 source is content-agnostic), but v1 is gradient-only.
- OS blur (vibrancy/Acrylic/compositor) — **deferred pending z=0 visual
  evaluation** (grill-me Q6). The directive is to **try z=0 (heca-owned blur +
  transparency) WITHOUT macOS vibrancy first** and see how it looks. If z=0 alone
  gives an acceptable cross-platform frosted-glass look, vibrancy may be
  unnecessary. The vibrancy-theme-matching work (light/dark detection on `Theme` +
  `NSVisualEffectView::setAppearance` + live-reload via a retained effect-view
  handle) is **parked** — revisit only if z=0 frost is insufficient on macOS, as a
  follow-up after Phase 5. Do not wire vibrancy in Phases 1–5; it stays
  `Vibrancy::None` by default throughout this work.
- Per-pane *individual* blur radius (one z=0 blur for all tiled panes; floating has
  its own dynamic blur).

---

## 4. Task-relation convention

- **Relations** list task ids a task depends on (must be done first).
- **Check** is the concrete, runnable verification for the task.
- **A task is `checked` only at the end of a review of that task.** The review
  runs the task's Check (command / visual confirmation), inspects the result,
  and only then marks the `[ ]` → `[x]`. A box ticked without a completed review
  is invalid. Reviews are per-task; the **Phase exit** is a separate, combined
  tests + review pass before starting the next phase.
- **Phase exit** = tests + review together before starting the next phase.
- Per AGENTS.md: no commit until the user has tested; rust-skill review at phase end
  (not per task); `cargo clippy --workspace --all-targets --all-features` clean.
- **Review = review the work just done against its Check** (does the build pass /
  does the test pass / does the user confirm the visual), then tick the box.

---

## Phase 0 — Setup & baseline

**Goal:** clean branch off latest `origin/main` with a green build, before any change.

### Tasks

- [x] **0.1 Sync with main** — `git fetch origin && git checkout -b feature/compositor-blur-refactor origin/main`.
  - Relations: none.
  - Check: `git log -1` shows the latest `origin/main` HEAD; branch is clean.

- [x] **0.2 Baseline green** — confirm the starting point builds, tests pass, clippy is at the known pre-existing baseline (9 lints — 8 selection `dead_code` + 2 `too_many_arguments`; see PR #125 notes).
  - Relations: 0.1.
  - Check (post-PR-#160):
    - [ ] `cargo build --workspace --all-targets` → green.
    - [ ] **Baseline test verification (post-PR-#160):** `cargo test -p heca-core`
      currently fails on `terminal_backend_bash_integration_reports_success_error_and_cwd`
      (`heca-core/src/backend/terminal.rs:801`, "bash shell integration should
      report `true` as Success with exit code 0"). Before proceeding, confirm
      this failure is **pre-existing/env-dependent** on plain `origin/main` and
      **not** caused by this plan's changes (the theme merge only added
      `cursor_color` to `TerminalSnapshot`, which is unrelated to shell-integration
      exit reporting). Record the verdict:
      - [ ] If pre-existing/env: note it as an excluded baseline failure (do not
        block the plan on it); record the exact failing test name.
      - [ ] If introduced: fix it before Phase 1 (do not start on a red baseline
        you don't own).
    - [ ] `cargo test --workspace` → green **excluding** any pre-existing/env
      failure recorded above (state the exclusion explicitly in the Phase 0
      exit notes).
    - [ ] `cargo clippy --workspace --all-targets --all-features` → **0 warnings**
      (the pre-merge "9 pre-existing lints" baseline is gone as of PR #160;
      record the actual count, expected 0). If any warning appears, it is **new**
      and must be fixed before Phase 1.

### Phase 0 exit
- [x] Tests + clippy baseline recorded. No review needed (no code changed). Proceed to Phase 1.

**Baseline recorded (2026-06-22):** clippy = 0 warnings; `cargo test --workspace` green except the pre-existing env-dependent `terminal_backend_bash_integration_reports_success_error_and_cwd` (verified failing on plain `origin/main` — excluded baseline failure, not introduced by this plan).

---

## Phase 1 — GPU plumbing: gradient + background layer (heca-renderer, headless)

**Goal:** two new, isolated, unit-testable GPU primitives in `heca-renderer` — a
gradient fill pipeline and a `BackgroundLayer` (gradient → static cached blur →
blit source). **No app wiring yet.** This phase stays within `heca-renderer` so it
is headless and testable in isolation.

### Tasks

- [x] **1.1 `heca-renderer/src/gradient.rs` — gradient pipeline** — a `GradientRenderer` that fills a render-target texture view with a 2-color **vertical** gradient (top→bottom) via a fullscreen-triangle pipeline. Inline wgsl (vertex: fullscreen triangle; fragment: `mix(top, bottom, uv.y)`). Colors are linear f32x4 (match `primitive_renderer` convention). No app types.
  - API:
    ```rust,ignore
    impl GradientRenderer {
        pub fn new(device: &Device, format: wgpu::TextureFormat) -> Self;
        pub fn render(&self, device: &Device, queue: &Queue, encoder: &mut CommandEncoder,
                      target: &TextureView, top: [f32;4], bottom: [f32;4]);
    }
    ```
  - Relations: none (leaf primitive).
  - Check:
    - [ ] `cargo build -p heca-renderer` green.
    - [ ] Smoke unit test (existing renderer test harness / headless device if available): construct `GradientRenderer::new` without panic. If no headless device, a compile-only smoke is acceptable and GPU behavior is verified visually in Phase 5.

- [x] **1.2 `heca-renderer/src/background.rs` — `BackgroundLayer`** — a struct owning the z=0 layer state and a **static cached blur**:
  ```rust,ignore
  pub struct BackgroundLayer {
      z0_tex: Texture, z0_view: TextureView,      // gradient render target
      cache_tex: Texture, cache_view: TextureView, // blurred result (cached)
      dirty: bool,
      // cached params (to detect change → set dirty)
      top: [f32;4], bottom: [f32;4], blur_radius: f32,
      size: (u32, u32), format: wgpu::TextureFormat,
      gradient: GradientRenderer,
  }
  impl BackgroundLayer {
      pub fn new(device, format, w, h) -> Self;
      pub fn resize(&mut self, device, w, h);               // sets dirty
      pub fn set_params(&mut self, top, bottom, blur_radius); // sets dirty if changed
      pub fn render(&mut self, device, queue, encoder, blur: &Blur) -> &TextureView;
      // if dirty: gradient.render → z0; blurred = blur.process(z0); copy blurred → cache; dirty=false
      // if clean: return cache_view (no work)
  }
  ```
  - `cache_tex` needs `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_DST`; the copy is `encoder.copy_texture_to_texture(blurred_view → cache_tex)`. Reuses the shared `Blur` (passed in) so no dedicated ping-pong pair is allocated. **Critical:** the caller must finish using / copy the returned blurred view before the next `blur.process` call in the same frame (documented in the method).
  - Relations: 1.1 (uses `GradientRenderer`); reuses existing `Blur`.
  - Check:
    - [ ] `cargo build -p heca-renderer` green.
    - [ ] Unit test: `render()` twice with no `set_params`/`resize` between → asserts the blur path runs only once (dirty flag). If no headless device, assert via a pure helper `fn params_changed(&self, ...) -> bool` factored out to be unit-testable without a device.

- [x] **1.3 Export modules in `heca-renderer/src/lib.rs`** — `pub mod gradient; pub mod background;` + re-export `BackgroundLayer` and `GradientRenderer` from the crate root (or a renderer prelude) so the app can import them.
  - Relations: 1.1, 1.2.
  - Check:
    - [ ] `cargo build -p heca-renderer` green.
    - [ ] `cargo test -p heca-renderer` green.

### Phase 1 exit — tests + review
- [x] `cargo build -p heca-renderer` + `cargo test -p heca-renderer` green.
- [x] **Review together:** confirm both modules are headless (no `wgpu::Surface`, no app types), reuse `Blur`, and the dirty-cache logic is correct. No app coupling. Proceed to Phase 2.

**Done (2026-06-22):** 15 unit + 3 integration tests green; clippy 0 warnings. User-reviewed (PR #165). `GradientRenderer::render` drops the unused `_device` param; `gradient_params_layout_matches_wgsl_uniform` pins the `#[repr(C)]` uniform layout.

---

## Phase 2 — Config: new z=0 knobs (heca-config, additive only)

**Goal:** add the new config surface **additively** (no removals yet — keeps the
workspace building). Removal of the old tint path happens in Phase 3 alongside
the render change, so there is no broken intermediate state.

### Tasks

- [x] **2.1 Add `Appearance` fields (`heca-config/src/appearance.rs`)** — add
  ```rust,ignore
  pub background_gradient_top: Option<Color>,
  pub background_gradient_bottom: Option<Color>,
  pub background_blur: u8,            // 0..=100
  pub background_transparency: u8,    // 0..=100 (0 = opaque z0)
  ```
  with serde defaults (`#[serde(default)]`). Mirror the existing `terminal_blur`/`terminal_transparency` style + clamping.
  - Relations: none.
  - Check: `cargo build -p heca-config` green.

- [x] **2.2 Resolvers (`heca-config/src/appearance.rs`)** —
  - `effective_background_gradient_top(&self, theme) -> Color`: config → `theme.background_gradient_top` → `theme.background` (fallback chain).
  - `effective_background_gradient_bottom(&self, theme) -> Color`: config → `theme.background_gradient_bottom` → a derived shifted variant → `theme.background`.
  - `background_blur_radius() -> f32`: `terminal_blur_radius()`-style mapping (`background_blur` 0..100 → 0..`MAX_BLUR_PX` logical px).
  - `background_alpha() -> f32`: `(1.0 - background_transparency as f32 / 100.0).clamp(0.0,1.0)`.
  - Relations: 2.1.
  - Check: unit tests in `appearance.rs` — defaults, override, clamping, fallback chain; `cargo test -p heca-config` green.

- [x] **2.3 Theme defaults (relocated to `heca-theme` post-PR-#160)** — the `Theme` struct and bundled theme definitions moved out of `heca-config` in PR #160. `heca-config/src/defaults.rs` is **deleted** and `heca-config/src/theme.rs` is now a re-export shim, so this task targets `heca-theme` instead:
  - **Struct fields** — add to `pub struct Theme` in `heca-theme/src/theme.rs` (currently at line ~156), mirroring the existing `#[serde(default)]` style used by the merge-added chrome background tokens:
    ```rust,ignore
    #[serde(default)]
    pub background_gradient_top: Option<Color>,
    #[serde(default)]
    pub background_gradient_bottom: Option<Color>,
    ```
    Place them near `left_sidebar_background` / `right_sidebar_background` / `top_bottom_pane_background`. Keep `Option<Color>` so the resolver fallback chain (2.2) can fall back to `theme.background` when unset.
  - **Bundled theme TOMLs** — add a tasteful top→bottom pair to **all three** active themes: `heca-theme/src/themes/grid_tron.toml`, `heca-theme/src/themes/mocha.toml`, `heca-theme/src/themes/latte.toml` (e.g. mocha: a slightly lighter top + the base bg bottom). Do **not** add a `frappe.toml` — it was replaced by `latte`; `frappe` exists only in historical notes now.
  - **`Appearance` defaults** (these stay in `heca-config`) — in `heca-config/src/appearance.rs`, set the new fields' serde defaults to `background_blur = 0`, `background_transparency = 0` (opaque z=0 by default, per the locked decision in §2). Do **not** recreate `defaults.rs`.
  - Relations: 2.1.
  - Check:
    - [ ] `cargo build -p heca-theme` green; `cargo build -p heca-config` green.
    - [ ] Unit test in `heca-theme` asserting each bundled theme has gradient colors (or that the resolver falls back to `theme.background` when unset).
    - [ ] `cargo test -p heca-theme` green; `cargo test -p heca-config` green.

- [x] **2.4 Workspace still green (additive only)** — confirm no removals were made; the old tint fields still exist (removed in Phase 3).
  - Relations: 2.1–2.3.
  - Check:
    - [ ] `cargo build --workspace --all-targets` green.
    - [ ] `cargo test --workspace` green.

### Phase 2 exit — tests + review
- [x] `cargo test -p heca-config` green; `cargo build --workspace` green.
- [x] **Review together:** resolver fallback chains, default values (opaque z0), knob names. Proceed to Phase 3.

**Done (2026-06-22):** heca-theme 20 tests, heca-config 50 tests green; clippy 0 warnings. User-reviewed. Bonus (user directive): `Theme::grid_tron()` now parses `include_str!("themes/grid_tron.toml")` — single source of truth, all hardcoded `Color::rgb` removed from the Rust constructor. `bundled_themes()` made `pub(crate)` so the gradient-contract test iterates the loader's bundled map (no hardcoded theme-name list). Resolver test split into 3 focused tests.

---

## Phase 3 — Pipeline integration + remove tint (heca app)

**Goal:** wire z=0 into the render pipeline, remove the tiled tint path, and apply
the floating backdrop-100% fix. This is the riskiest phase (core pipeline reorder);
build incrementally and review the diff carefully.

### Tasks

- [x] **3.1 Own `BackgroundLayer` in `AppState` (`heca/src/app_state.rs`)** — add a `pub background: heca_renderer::background::BackgroundLayer` field; construct it in the renderer-init path (where `blur` / `compositor` are created), sized to the physical framebuffer; add an accessor if needed.
  - **Done (PR #169):** `pub background: BackgroundLayer` field added to `AppState`; constructed in `heca/src/app/startup.rs` alongside `blur`/`compositor`/`backdrop`, sized to `physical.width`/`physical.height`. Imports added to both `app_state.rs` and `startup.rs`.
  - Relations: Phase 1 (BackgroundLayer exists).
  - Check: `cargo build -p heca` green. ✅

- [x] **3.2 Resize hook (`heca/src/app/events.rs`)** — in `WindowEvent::Resized`, alongside `state.blur.resize(...)` and `state.compositor.resize(...)`, call `state.background.resize(&device, w, h)` (sets dirty → z=0 reblurs next frame).
  - **Done (PR #169):** `state.background.resize(&state.device, phys.width, phys.height)` inserted in `WindowEvent::Resized`, right after `state.blur.resize(...)`.
  - Relations: 3.1.
  - Check: `cargo build -p heca` green; behavior verified in Phase 5 (resize test). ✅ (build; visual deferred to Phase 5)

- [x] **3.3 z=0 base blit (`heca/src/app/render.rs`)** — after the scene clear pass and **before** the pane stencil/content passes:
  1. `state.background.set_params(top, bottom, blur_radius)` from `appearance.effective_background_gradient_*` + `background_blur_radius() * scale_factor`.
  2. `let bg_view = state.background.render(&device, &state.queue, &mut encoder, &state.blur);`
  3. Blit `bg_view` into `scene_view` as a fullscreen textured quad at alpha = `appearance.background_alpha()`. Reuse the `Backdrop::draw` (full-screen dst) or the composite blit with an alpha uniform — pick whichever needs the least new pipeline; if a new alpha-blit pipeline is needed, add it to `backdrop.rs` or `composite.rs`.
  - **Done (PR #169):** reused `Backdrop::draw` (full-screen dst, `src_uv = [0,0,1,1]`, `opacity = background_alpha()`, `stencil = None`) — **no new pipeline**. Reads `effective_background_gradient_top/bottom(theme)` + `background_blur_radius() * scale`. Drawn pre-stencil (before the tiled stencil-write). An `// Order invariant:` comment documents the shared-`state.blur` snapshot contract (z=0 must render before any other blur user this frame, since `BackgroundLayer` snapshots into its own cache inside `render()`).
  - Relations: 3.1, 3.2.
  - Check: `cargo build -p heca` green; **visual** in Phase 5 (gradient frost visible behind translucent panes). ✅ (build; visual deferred to Phase 5)

- [x] **3.4 Remove tiled tint — Pass A (`heca/src/app/render.rs`)** — delete the `needs_frosted_backdrop` gate, the frosted-tint `draw_rect` block, and the "Frosted terminal backdrop" comment block. Tiled panes now render translucent (Pass 2, `surface_alpha`) **directly over z=0** — the frost is z=0 showing through, not a tint. Keep the stencil content-clip (Pass 2) intact.
  - **Done (PR #169):** removed the Pass A frosted-tint block, the `needs_frosted_backdrop` gate, the `frost_opacity` local, and the "Frosted terminal backdrop" comment block; replaced with a short "Floating-pane real blur" comment. Stencil content-clip (Pass 2) kept intact. Tiled frost is now z=0 showing through `surface_alpha`.
  - Relations: 3.3 (z=0 must be in place first).
  - Check: `cargo build -p heca` green; **visual** in Phase 5 (tiled frost = z=0). ✅ (build; visual deferred to Phase 5)

- [x] **3.4b Remove the `content_canvas_fill()` stopgap (new, post-PR-#160)** —
  PR #160 added a tactical background-tint in `heca/src/app/render.rs` that
  paints `theme.background` at `appearance.opacity()` over pane-less content
  area, via `content_canvas_fill()` + `FillRect` / `subtract_fill_rect()` /
  `uncovered_fill_rects()` (+ unit tests `content_canvas_fill_*` and
  `uncovered_fill_rects_exclude_pane_rectangles`). `theming-plan.md` flags this
  as a stopgap to fold into the z=0 model. It sits in the exact pre-stencil slot
  z=0 now owns, so leaving it in place would produce **two competing background
  layers**. Once 3.3 wires z=0:
  - delete `content_canvas_fill`, `FillRect`, `subtract_fill_rect`,
    `uncovered_fill_rects`, and the uncovered-region fill pass in `render_frame`;
  - delete the associated unit tests in `heca/src/app/render.rs::tests`;
  - confirm `rg "content_canvas_fill|FillRect|uncovered_fill_rects|subtract_fill_rect" --glob '!*.md'`
    returns no code references (only `theming-plan.md` historical mentions may
    remain — update them in Phase 4).
  - **Done (PR #169):** all four helpers + the uncovered-region fill pass + the 3 unit tests (`content_canvas_fill_is_none_when_window_is_opaque`, `content_canvas_fill_uses_theme_background_and_app_opacity`, `uncovered_fill_rects_exclude_pane_rectangles`) deleted; the `use super::{...}` import in the test module trimmed to `status_mode_parts`. Grep `rg "content_canvas_fill|FillRect|uncovered_fill_rects|subtract_fill_rect" --glob '!*.md'` → no code references (exit 1). `theming-plan.md` historical pointer update is Phase 4 (task 4.1b).
  - Relations: 3.3 (z=0 must replace it).
  - Check: `cargo build -p heca` green; `cargo test -p heca` green; grep clean. ✅

- [x] **3.5 Remove old tint config (`heca-config/src/appearance.rs`)** — delete `terminal_frost_color`, `terminal_frost_opacity()`, and `effective_terminal_frost_color()`. Update any remaining references (grep `terminal_frost` across the workspace).
  - **Done (PR #169):** removed `terminal_frost_color` (AppearanceConfig **and** `Theme` fields), `terminal_frost_opacity()`, `terminal_floating_frost_opacity()`, `effective_terminal_frost_color()`. Also removed `terminal_blur` / `terminal_blur_radius()` / `default_terminal_blur` — dead after Pass A removal; grill-me Q3 mandates dropping tiled `terminal_blur`, and leaving it would break the 0-warning baseline. Fixed `latte.toml` `terminal_background #e6e9ef00 → #e6e9ef` (grill-me Q1 opaque-theme fix, inline TOML comment added). Stripped `terminal_blur` + `terminal_frost_color` from `example.config.toml`. Updated the `terminal_floating_blur` doc comment (it still referenced the removed tiled `terminal_blur`). Doc-only references (`theming-documentation.md` etc.) are Phase 4 (task 4.1b).
  - Relations: 3.4 (render.rs no longer uses them).
  - Check (post-PR-#160):
    - [x] `cargo build --workspace --all-targets` green.
    - [x] `cargo test -p heca-config` green (48 tests).
    - [x] `rg "terminal_frost" --glob '!*.md'` returns no code references. Note
      this now also catches **`heca-theme/src/themes/latte.toml`**
      (`terminal_frost_color = "#e6e9ef"`) and **`example.config.toml`** — both
      must have the key removed (not just docs). `Theme` uses per-field
      `#[serde(default)]` with no `deny_unknown_fields`, so stale keys won't
      break loading, but they must still be stripped to avoid shipping a dead
      knob. Doc-only references (`theming-documentation.md` etc.) are fixed in
      Phase 4. (Also `rg "terminal_blur"` clean in code.)

- [x] **3.6 Floating backdrop-100% fix (`heca/src/app/render.rs`)** — in the floating-pane loop, change the `backdrop.draw` opacity argument from `floating_frost_opacity` → `1.0` (the blurred tiled content fully replaces the raw behind — no sharp leak). Drop `floating_frost_opacity` (and `terminal_floating_frost_opacity()` if it exists); floating frost visibility is now driven solely by `terminal_floating_transparency` (cell surface alpha).
  - **Done (PR #169):** floating `backdrop.draw` opacity arg → `1.0`; removed the `floating_frost_opacity` local + `terminal_floating_frost_opacity()`.
  - Relations: 3.4.
  - Check: `cargo build -p heca` green; **visual** in Phase 5 (no text collision, frost visible at chosen transparency). ✅ (build; visual deferred to Phase 5)

- [x] **3.7 Showcase + examples signature updates (`heca-renderer/examples/showcase.rs`)** — update any `backdrop.draw` / `primitive` / `text.render` calls whose signatures changed (e.g. if a new alpha-blit pipeline altered `Backdrop::draw`). **Note (post-PR-#160):** the showcase now integrates `heca-theme` directly and has a live theme switcher (`⇄ THEME: ...` button) that rebuilds the whole widget tree on theme change. `backdrop.draw` currently has **no** call site in the showcase (only in `heca/src/app/render.rs:747`), so signature changes there mostly affect the app, not the example — but re-verify the showcase still builds with whatever new pipeline 3.3 introduces, and that the theme switcher still rebuilds cleanly with z=0 in mind.
  - **Done (PR #169):** no `Backdrop::draw` signature changed (reused as-is), so no showcase call-site edits needed. `cargo build --workspace --all-targets` green — the showcase builds. The live theme switcher is unaffected (it only composes `heca-grid-ui` widgets + grid scene; z=0 is app-render-only).
  - Relations: 3.3, 3.6.
  - Check: `cargo build --workspace --all-targets` green (examples included); `cargo run -p heca-renderer --example showcase` launches and the theme switcher still cycles `grid_tron`/`mocha`/`latte`. ✅ (build verified; runtime launch deferred to Phase 5)

### Phase 3 exit — tests + review
- [x] `cargo build --workspace --all-targets` green. ✅
- [x] `cargo test --workspace` green. ✅ (except the known pre-existing env-dependent `heca-core` `terminal_backend_bash_integration` test, which fails on plain `origin/main` too — excluded baseline.)
- [x] **Review together (the critical review):** walk the full pipeline order in `render.rs` — clear → z=0 blit → tiled stencil → tiled content (translucent over z=0) → chrome borders → floating blur capture → floating stencil → floating backdrop(100%) + content → grid-ui chrome → sidebar. Confirm no stencil state leaks between the tiled and floating passes, and the z=0 blit does **not** get clipped by the tiled stencil (z=0 is pre-stencil). ✅ Walked in the PR #169 body + user review; z=0 is pre-stencil (`stencil = None`); floating pass uses its own stencil. User review passed (2 doc-only findings applied).
  - **Translucency-channel reconciliation (post-PR-#160, required):** the plan assumes tiled pane translucency is driven solely by the `terminal_transparency` knob → `surface_alpha` in `heca-renderer/src/terminal.rs` (`surface_bg[3] = default_bg[3] * style.surface_alpha`). PR #160's `latte` theme sets `terminal_background = "#e6e9ef00"` (alpha 0), which makes the terminal surface fully transparent **independent of the knob** — this is the prime suspect for the deferred "no blur/transparency" regression. Before Phase 5, decide and record **one** of:
    1. **Theme owns opacity:** bundled theme `terminal_background` values are **opaque** (fix `latte.toml` `#e6e9ef00` → `#e6e9ef`), and `terminal_transparency` / `surface_alpha` is the only translucency channel (cleanest, matches this plan's model). **Recommended.**
    2. **Theme owns translucency:** document that theme-bg alpha is a second channel and reconcile both in the resolver (more complex, risks re-introducing the regression).
    If (1) is chosen, fold the `latte.toml` fix into Task 3.4b's cleanup or a dedicated 3.5 sub-step. Proceed to Phase 4.
  - **RESOLVED (grill-me Q1, applied in PR #169 Task 3.5):** Option **1** chosen — opaque theme `terminal_background` + knob-driven `surface_alpha` is the sole translucency channel. `latte.toml` `terminal_background` fixed to opaque `#e6e9ef` (with inline bug-fix comment). Proceeding to Phase 4.

---

## Phase 4 — Documentation

**Goal:** document the new z-layer model and knobs; remove/replace the tint docs.

### Tasks

- [x] **4.1 `keybindings.toml`** — replace the tiled-tint appearance block with the z=0 knobs (`background_gradient_top/bottom`, `background_blur`, `background_transparency`) + a concise z-layer model explanation + a **migration note** that `terminal_blur` / `terminal_frost_color` are removed (use `background_blur`).
  - **Done (this PR):** the canonical config example is `example.config.toml` (there is no `keybindings.toml` appearance block) — added the z=0 block (`background_blur`, `background_transparency`, optional `background_gradient_top/bottom` with the unset → theme/derived fallback noted) + a migration note that `terminal_blur`/`terminal_frost_color` are removed (use `background_blur`).
  - Relations: Phase 3.
  - Check: doc reads consistently; no references to removed knobs as valid. ✅

- [x] **4.1b Strip removed knobs from theme TOMLs + config examples (new, post-PR-#160)** — Task 3.5 removes `terminal_frost_color` from code; the merge left stale references that must be cleaned in the same doc pass:
  - `heca-theme/src/themes/latte.toml` — remove `terminal_frost_color = "#e6e9ef"` (line ~45).
  - `example.config.toml` — remove the commented `# terminal_frost_color = "#1e1e2e"` block (~line 29).
  - `theming-documentation.md` — remove/replace the `terminal_frost_color` entries in the bundled-theme value tables and the latte section (lines ~152, ~300, ~443, ~488).
  - `theming-plan.md` — update the `content_canvas_fill()` stopgap note to point to this plan's Task 3.4b as the folding target (the note currently says "fold into z=0" without naming the task).
  - **Done (this PR):** `latte.toml` `terminal_frost_color` was already stripped in Phase 3 (3.5); `example.config.toml` commented block was already stripped in Phase 3 and is now replaced by the z=0 block (4.1). `theming-documentation.md` — removed all 4 `terminal_frost_color` entries (grid_tron example, field list, latte block, latte field list) AND fixed the stale `terminal_background = "#e6e9ef00"` → opaque `#e6e9ef` in the latte block. `theming-plan.md` — both `content_canvas_fill` notes (§ stopgaps + 3C.3) now point to Phase 3 Task 3.4b as the folding target and marked done (PR #169).
  - Relations: 3.5.
  - Check: `rg "terminal_frost_color" --glob '!compositor-blur-refactor-plan.md'` returns no references outside historical/migration notes; `rg "content_canvas_fill"` only in `theming-plan.md` as a historical pointer to Task 3.4b. ✅ (verified — remaining hits are README/AGENTS/example.config migration notes + BACKLOG/resume-handoff historical; `theming-documentation.md` clean.)

- [x] **4.2 `README.md`** — update the appearance / blur section to the z-layer model.
  - **Done (this PR):** added a new `### Appearance & Frost (z=0 background layer)` section after the Theme section — documents the heca-owned z=0 gradient, the `background_blur`/`background_transparency`/gradient knobs, how tiled vs floating frost is produced, and the `terminal_blur`/`terminal_frost_color` migration note. Reload via `prefix+Shift+r` noted.
  - Relations: 4.1.
  - Check: review. ✅

- [x] **4.3 `AGENTS.md`** — add the z-layer frost model to the rendering/appearance notes; reinforce the "no hardcoded color/style — read from theme/config" rule with the gradient as an example; note the `BackgroundLayer` is a heca-renderer primitive (headless), not a `heca-grid-ui` widget.
  - **Done (this PR):** added a `### z=0 background frost model (heca-owned, cross-platform)` subsection after the Stack "What NOT to use" table — documents `BackgroundLayer` as a heca-renderer GPU primitive (NOT a `heca-grid-ui` widget), the full render order, the grill-me Q1 opaque-theme translucency rule, the removed knobs, and reinforces the no-hardcoded-color rule with the gradient (`Theme::effective_background_gradient_top/bottom()`) as the canonical example.
  - Relations: 4.1.
  - Check: review. ✅

- [x] **4.4 `.planning/research/ARCHITECTURE.md`** — document the z-layer model, `BackgroundLayer`, static blur cache, and the cross-platform rationale (app vs compositor).
  - **Done (this PR):** updated §3 (Compositor/Renderer) — rewrote the Composite step to the actual z=0 pipeline order (clear → z=0 blit pre-stencil → tiled stencil + translucent content → borders → floating blur capture + backdrop(100%) → grid-ui chrome → overlays → present) and added a `#### z=0 background frost model (heca-owned, cross-platform)` subsection covering `BackgroundLayer`'s `z0_tex`/`cache_tex`/dirty cache + snapshot contract, the translucency channel, removed knobs, and the app-vs-compositor rationale (why heca can't blur the desktop cross-platform).
  - Relations: 4.1.
  - Check: review. ✅

### Phase 4 exit — review together
- [x] Docs consistent across all four files; no stale references to the tint approach in code-facing docs. Proceed to Phase 5. ✅ (gate verified: `rg "terminal_frost_color"` / `content_canvas_fill` outside the plan return only historical/migration notes; `theming-documentation.md` clean.)

---

## Phase 5 — Visual tuning (with user)

**Goal:** iterate the frost to a user-approved look. This is the loop that
determines success — the GPU work is deterministic, the *look* is not.

### Tasks

- [ ] **5.1 Tiled frost baseline** — user sets `background_blur`, `background_transparency`, `background_gradient_top/bottom`, `terminal_transparency`; reload (`prefix+Shift+r`); report whether the tiled frost looks like real frosted glass.
  - **Pre-check (post-PR-#160):** confirm the Phase 3 critical review's translucency-channel decision (option 1 vs 2) was applied. If option 1 was chosen, `latte.toml` `terminal_background` must be opaque (`#e6e9ef`, not `#e6e9ef00`) before this step — otherwise the knob-driven tuning below will not behave as the plan expects and the deferred "no blur/transparency" regression will likely persist.
  - Relations: Phase 3.
  - Check: user confirms "tiled frost looks good."

- [ ] **5.2 Blur strength tuning** — if the frost is too weak (shapes too sharp) or too strong (flat wash), tune `BLUR_PASSES` / `MAX_BLUR_PX` / the `background_blur_radius()` mapping in `appearance.rs`. Re-test.
  - Relations: 5.1.
  - Check: user confirms blur strength is right.

- [ ] **5.3 Floating frost + collision check** — user sets `terminal_floating_blur` + `terminal_floating_transparency`; confirm the floating terminal's text **does not collide** with the tiled content behind (the backdrop-100% fix) and the frost is visible.
  - Relations: 5.1.
  - Check: user confirms "floating looks good, no collision."

- [ ] **5.4 Resize correctness** — resize the window (and toggle chrome); confirm z=0 recomputes (no stale/blurry-at-wrong-resolution artifact).
  - Relations: 5.2.
  - Check: user confirms "resize is ok."

### Phase 5 exit — user sign-off
- [ ] User confirms tiled + floating frost look good, no collision, resize works. Proceed to Phase 6.

---

## Phase 6 — Phase-end review + cleanup + ship

**Goal:** quality gates, then ship (with user approval — no commit until the user
has tested, per AGENTS.md).

### Tasks

- [ ] **6.1 Rust-skill review** — load `/Users/antonio/.agents/skills/rust/SKILL.md` and run a formal review against the full diff (all phases). Fix findings.
  - Relations: Phase 5.
  - Check: review notes addressed; no HIGH/MED findings open.

- [ ] **6.2 Clippy clean** — `cargo clippy --workspace --all-targets --all-features`. Must be clean of **new** lints. **(Post-PR-#160: the pre-merge "9 pre-existing lints" baseline is gone — clippy is currently 0 warnings.)** Any warning that appears is new and must be fixed; the old allowance for the 8 `selection_model.rs` `dead_code` + 2 `too_many_arguments` lints no longer applies.
  - Relations: 6.1.
  - Check: clippy green (only known pre-existing, or zero).

- [ ] **6.3 Test sweep** — `cargo test --workspace` green; add/adjust any tests touched by the refactor (appearance resolvers, background dirty-flag, render behavior).
  - Relations: 6.2.
  - Check: all tests green.

- [ ] **6.4 Planning docs update** — update `.planning/STATE.md` + `PLAN.md` — mark the blur refactor done; reference this plan file.
  - Relations: 6.3.
  - Check: docs updated.

- [ ] **6.5 Commit + push + PR** — stage the change in clean, behavior-preserving slices; commit with a clear message; push `feature/compositor-blur-refactor`; open PR to `main`. **Do not commit until the user has tested and approved** (AGENTS.md gate).
  - Relations: 6.3 + user approval.
  - Check: PR open; CI green (or the repo's equivalent gate).

### Phase 6 exit — merged (after user approval + review)
- [ ] Blur refactor shipped; PR merged.

---

## 5. Risk register

| Risk | Mitigation | Where |
|---|---|---|
| Static blur cache stale after resize | `resize()` sets dirty; Phase 5.4 verifies | background.rs, events.rs |
| Gradient banding (2-color, no blur) | Blur hides it; add dithering only if user reports | gradient.rs |
| wgpu resource leak (2 new fullscreen textures) | Recreate on resize; review in Phase 1 + 3 | background.rs |
| Stencil state leaks between tiled/floating/z0 passes | z=0 blit is **pre-stencil**; Phase 3 critical review confirms | render.rs |
| Shared `Blur` reused for z0 + floating in one dirty frame | Copy z0 blurred view → cache **before** the floating `blur.process` call; documented in `BackgroundLayer::render` | background.rs, render.rs |
| Removing `terminal_blur` breaks user config | Migration note in keybindings.toml (Phase 4) | docs |
| `content_canvas_fill()` stopgap left in place alongside z=0 (two competing background layers) | New Task 3.4b removes it once z=0 is wired | render.rs, Task 3.4b |
| `latte` transparent `terminal_background` (`#e6e9ef00`) bypasses `terminal_transparency` → `surface_alpha`, re-introducing the "no blur/transparency" regression | **DECIDED (grill-me Q1, Option A):** opaque theme `terminal_background` + knob-driven `surface_alpha` is the only translucency channel; fix `latte.toml` `#e6e9ef00` → `#e6e9ef` (fold into Task 3.4b/3.5); 5.1 pre-check verifies | heca-theme/src/themes/latte.toml, render.rs, terminal.rs |
| `defaults.rs` referenced by old Task 2.3 no longer exists | Task 2.3 rewritten to target `heca-theme/src/theme.rs` + TOMLs | heca-theme |

---

## 6. Success criteria (definition of done)

- [ ] **Cross-platform:** tiled + floating frost render identically via wgpu (no OS-vibrancy dependency) — verified by code review (no platform API in the frost path) + user visual check.
- [ ] **Tunable:** `background_blur` visibly changes the frost strength; the user confirms in Phase 5.
- [ ] **No collision:** floating terminal text does not collide with tiled content behind (backdrop-100%); user confirms in Phase 5.3.
- [ ] **Clean:** `cargo build --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features` all green (only known pre-existing lints).
- [ ] **Documented:** z-layer model + knobs documented in keybindings.toml, README, AGENTS, ARCHITECTURE.
- [ ] **Shipped:** PR merged after user approval.