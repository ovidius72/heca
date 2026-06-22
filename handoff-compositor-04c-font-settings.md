# Handoff — `compositor-04c` font settings separation

Last updated: 2026-06-22

## 1. Current status

`compositor-04b` is finished and merged.

Merged work:
- PR **#172** — `feat(compositor): add intensity/glow appearance overrides`
  - `[appearance]` overrides for `glow_size` / `intensity`
  - removed `Intensity::glow_scale()`
  - added `GlowLevel::strength_scale()`
  - corrected docs + config examples
  - folded the old un-PR'd Phase 4 compositor docs into that PR
- PR **#173** also merged afterwards on `main` (unrelated widget migration)

Current next phase:
- **`compositor-04c` — Font settings separation (extract fonts from `Theme`)**
- After that: `compositor-05` visual tuning, then `compositor-06` review/ship

User explicitly confirmed:
- do **`compositor-04c` next** before moving to 5/6

## 2. Git / branch state

At handoff time:
- current branch: `feature/intensity-glow-override`
- local `origin/main`: `7b431fa`
- local branch HEAD: `8c1e091` = `chore: remove intensity/glow handoff`

Important:
- `8c1e091` is **not** on `main`; it only removes the old `handoff-intensity-glow.md`
- current working tree is clean
- local `main` was fast-forwarded to `origin/main`

Recommended start for next session:
- sync `origin/main`
- either:
  1. continue from `feature/intensity-glow-override` if you want to keep the handoff-file removal commit, or
  2. start a fresh branch from `origin/main` and re-remove `handoff-intensity-glow.md` only if still desired

## 3. Why `04c` exists

This was discovered during `compositor-04b`.

Problem:
- font family/size currently live on `heca-theme::Theme`
- bundled theme TOMLs ship specific font names like `Maple Mono Normal NF`
- fonts are **system-local, not theme-portable**
- a color theme should not require a specific installed font on another system

User direction:
- font config should move out of `Theme`
- it should live in `config.toml`
- proposed shape:

```toml
[font.family.ui]
normal = "..."
bold = "..."
italic = "..."
bold_italic = "..."

[font.family.terminal]
normal = "..."
bold = "..."
italic = "..."
bold_italic = "..."

[font.size]
terminal = 12
ui = 14
```

This schema was accepted as the target direction.

## 4. Backlog / plan entries already added

### BACKLOG
`BACKLOG.md` already contains:
- `compositor-04c` phase
- tasks `compositor-task-31` through `compositor-task-36`

### Plan / architecture notes
Also added:
- `plugin-task-10a` in `BACKLOG.md` for the discovered expanded-sidebar selection-state bug
- a note in `pluggable-chrome-plugin-plan.md` §3.3 explaining that bug as a concrete example of missing shared `selected row ids` / focus-selection state

## 5. Scope of `compositor-04c`

Backlog tasks define the phase. In practice the work is:

### 5.1 Add structured font config
Create a dedicated config model in `heca-config`.

Planned file:
- `heca-config/src/font.rs` (new)

Need:
- UI family group
- terminal family group
- size group
- defaults in `FontConfig::default()`
- style slots may fall back to `normal`

### 5.2 Remove font fields from `Theme`
Remove these from `heca-theme::Theme`:
- `font_family`
- `font_size`
- `terminal_font_family`
- `terminal_italic_font_family`
- `terminal_font_size`

Also remove from bundled TOMLs:
- `heca-theme/src/themes/grid_tron.toml`
- `heca-theme/src/themes/mocha.toml`
- `heca-theme/src/themes/latte.toml`

### 5.3 Remove font override plumbing from settings/theme loader
Current override path:
- `heca-config/src/settings.rs` has Option font override fields
- `heca-config/src/loader.rs::apply_overrides()` writes them into `Theme`

This needs to be replaced with the new dedicated font config path.

### 5.4 Rewire all consumers away from `theme.font_*`
Previously identified consumer spread: ~45 call sites across 11 files.

Known hotspots:
- `heca/src/main.rs`
- `heca/src/app/startup.rs`
- `heca/src/chrome/mod.rs`
- `heca/src/app/render.rs`
- `heca/src/app/terminal_render.rs`
- `heca/src/app/terminal_metrics.rs`
- `heca-grid-ui/src/layout.rs`
- `heca-grid-ui` tests
- `heca-renderer/examples/showcase.rs`

### 5.5 Renderer/font-family style support
Current renderer state:
- `heca-renderer/src/text.rs` effectively has one main `font_family` slot + `bold: bool`
- terminal path already passes a separate italic family in some places

Open implementation question for `04c`:
- full real per-style family loading (`normal/bold/italic/bold_italic`) may be done now, or
- config schema can land first with fallback-to-`normal` where renderer support is incomplete

Backlog already allows fallback:
- if full per-style families are deferred, style slots can fall back to `normal`
- renderer can temporarily keep weight-based bold

## 6. Code locations worth checking first

### Theme / config
- `heca-theme/src/theme.rs`
- `heca-config/src/settings.rs`
- `heca-config/src/loader.rs`
- `heca-config/src/lib.rs`
- `heca-config/src/theme.rs`

### Consumer call sites
Use grep first:
```bash
rg "theme\.font_|theme\.terminal_font" --glob '*.rs'
```

### Renderer
- `heca-renderer/src/text.rs`
- `heca/src/app/render.rs`
- `heca/src/app/terminal_render.rs`
- `heca/src/app/terminal_metrics.rs`

## 7. Validation expectations

Use the same gates as `04b` unless `04c` changes the plan:

```bash
cargo build --workspace --all-targets
cargo clippy --workspace --all-targets --all-features
cargo test -p heca-theme -p heca-config -p heca-renderer -p heca
cargo test -p heca-core --lib -- --skip terminal_backend_bash_integration
```

Known baseline notes:
- `heca-core::terminal_backend_bash_integration...` is env-dependent; skip as above
- explicit `heca-grid-ui` integration test `toast_action_press_flashes_only_the_action_not_the_whole_card` was previously confirmed to fail on plain `main`; do not confuse it with a new regression unless `04c` touches that area

Useful grep gate after rewiring:
```bash
rg "theme\.font_|\.font_family|\.font_size" --glob '*.rs'
```
Interpret carefully — this will also match the new config structures if they use those names. Use it to catch old `theme.font_*` reads, not to ban font fields globally.

## 8. Important follow-up discovered during `04b`

### Expanded sidebar selection bug — pre-existing, not introduced by `04b`
Investigation findings:
- sidebar nav input works
- `prefix+e` enters `SIDEBAR`
- `j/k` updates `sidebar_tree.cursor/current_item()` internally
- **expanded** sidebar does not visibly highlight selection

Root cause:
- expanded sidebar render in `heca/src/chrome/mod.rs` uses `active_pane` only
- it does **not** render from `sidebar_tree.cursor/current_item()`
- collapsed sidebar renderer does use `tree.cursor`

Conclusion:
- missing bridge from sidebar-nav selection into shared chrome/workspaces state

Tracked now in:
- `BACKLOG.md` → `plugin-task-10a`
- `pluggable-chrome-plugin-plan.md` §3.3 note

Temporary debug logs used during investigation were removed before finalizing `04b`.

## 9. Docs already updated by `04b`

These are already in main and should be treated as current truth for intensity/glow:
- `README.md`
- `AGENTS.md`
- `example.config.toml`
- `theming-documentation.md`
- `terminal-implementation.md`
- `BACKLOG.md`

Do not reintroduce the old wrong meaning:
- `glow_size` = glow presence + radius + strength
- `intensity` = scanline/CRT overlay opacity only

## 10. Workflow reminders

Project/user rules still apply:
- sync/pull `origin/main` before starting
- do **not** commit until user review/approval
- do **not** merge PR without explicit approval
- run clippy clean
- do not run `cargo fmt`

## 11. Suggested restart prompt

For a new session, a good restart instruction would be:

> Read `AGENTS.md`, `BACKLOG.md`, `pluggable-chrome-plugin-plan.md`, and `handoff-compositor-04c-font-settings.md`. We finished `compositor-04b`; start `compositor-04c` (font settings separation) next.
