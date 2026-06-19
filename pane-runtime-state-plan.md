# Pane Runtime State + Reactive Chrome Store + Plugin Event Bus — Implementation Plan

**Status:** design locked (grilled with the user 2026-06-18) · pre-implementation
**Orchestration board:** `pane-runtime-tasks.md` (assignment + agent responses; do **not** use `shared-tasks.md`)
**Single planning index:** `PLAN.md` references this doc; deep rationale lives here.

> This plan turns each pane into a **live, observable runtime object** (what program it runs, its
> status, cwd, git info), stored in a **reactive chrome store** that drives heca's widgets **and** emits
> **typed events** a plugin can subscribe to (`app.on(...)`). It also makes the existing
> `[[keys.command]]` spawn actually run programs (float + close-on-exit policy). It is the concrete
> build-out of the SharedChromeState foundation + parts of `pluggable-chrome-plugin-plan.md`
> (§2.3 events, §3.3 shared state, §3.5 host API, §8.1/§8.2 tokens — the *token customization* is
> deferred; see §0.3).

---

## 0. Locked decisions (do not re-litigate — settled in design)

### 0.1 State + reactivity model
- **One source of truth.** Canonical runtime truth (program, exit, cwd) lives on the **backend/pane**
  (`heca-core`). The **chrome store** (`heca/src/chrome/state.rs`) holds a **reactive mirror** of it that
  widgets subscribe to. Session stays canonical (existing rule).
- **One mutation chokepoint → two subscriber channels.** Every state change goes through a single place
  and notifies **both**:
  1. **internal `floem_reactive` signals** → heca's own widgets repaint (cheap, fine-grained), and
  2. a **typed event bus** → plugins/providers react via `app.on(name, handler)`. Plugins **cannot**
     subscribe to internal signals (esp. future WASM, across the host boundary), so the bus is mandatory.
- **Event bus is built NOW** (first-party subscribers; the WASM bridge comes later, but the architecture
  must exist from day one — no half-measures). Events are **typed per-slice** (e.g. `pane.process.changed`,
  `pane.status.changed`, `pane.exited`, `chrome.region.changed`, `selection.changed`) and exposed to
  plugins as **string names** with a **catch-all** (`app.on('*' | 'updated', …)`).

### 0.2 Detection model — **event-first, no polling timer (poll deferred)**
- **Exit** → PTY child-wait / reader EOF = event (`child.try_wait()` already exists). No poll.
- **Command success/error + cwd** → arrive as **escape sequences** in the output stream
  (**OSC 133** semantic prompts, **OSC 7** cwd) → parsed as output flows = event. For a **spawned single
  command** the exit code comes straight from the child (no shell hook needed); **OSC 133** is only for
  success/error of commands run *inside an interactive shell pane*.
- **Foreground program + running/idle** → the only source with no clean OS event. Re-sampled **on every
  terminal-output wake** (event-driven — the reader thread already wakes the loop on output) and on **OSC 133
  command-start/-end markers** when shell integration is on; **rate-limited by a 250 ms debounce** so heavy
  output bursts don't hammer `tcgetpgrp`/`libproc`. **No periodic polling timer** — the ~250–500 ms poll
  fallback is **DEFERRED** (signal-first per the user's decision); added later *only* if testing shows silent
  foreground transitions (programs that start/exit with zero output) are missed. **Exit** is event-driven via
  the reader's **EOF wake** → `try_wait` (no timer).
- **Git** → re-checked on **cwd-change** (an OSC 7 event) + optional `.git` filesystem watch, slow-debounced;
  never per-tick.

### 0.3 Display + customization
- **Fixed default pane-info display now**, built as **real `heca-grid-ui` widgets** (theme-driven). Two rows
  on the **sidebar pane card** (optional **pane top-left corner** badge = compact row 1):
  - **Row 1 — program:** `Icon` · `Name (raw)` · `Badge(status)`.
    - **Idle (shell foreground):** terminal icon + the shell name (`zsh` / `bash` / …) — no app program.
    - **Running a program:** the program's catalog icon + display name + `(raw)` + a status badge, e.g.
      ` Neovim (nvim) [Running]`.
  - **Row 2 — git (only if the cwd is in a git repo):** `Tag(branch)` (e.g. ` feature/my-feat` inside a
    badge) + `Badge(+a/-d/Δ)` segments for added/new/deleted counts. Hidden entirely outside a repo.
- **Status set = `Running` · `Idle` · `Success` · `Error`** (NO `Exit` — see §0.6). `Success`/`Error` land in
  Phase 3 (OSC 133). Status→theme colour: Running=accent, Idle=muted, Success=success, Error=danger.
- **Process catalog** = a user-extensible `[programs.<raw>]` map (see §0.7) → `{ name, icon, description,
  color }`. Built-in defaults (shells seeded) + config extension. The **raw program name is always shown**
  verbatim (in parens); the catalog adds a friendly name + icon (fallback = raw name + a default terminal
  icon for shells / no icon for unknown). `color` (optional) tints the pane card/border.
- **DEFERRED → future plan:** the **tmux-style token/segment customization** (`${token}` templates, user-
  authored segment lists). Agreed shape when built = the **hybrid** model (a list of widget-typed segments,
  each a `${token}` template) — record this in `pluggable-chrome-plugin-plan.md` §8.1/§8.2. The fixed
  default above *becomes* the default segment config when customization ships.

### 0.4 Command spawn
- `WmAction::SpawnCommand` (today a **stub** that only titles the pane) becomes a real launch: run the
  program via `CommandBuilder`, place it tiled or floating, and remember its close-policy.
- `[[keys.command]]` config grows (replaces the unused `command_type`):
  ```toml
  [[keys.command]]
  key            = "prefix+Shift+g"
  command        = "lazygit"   # program to run (later: provider id when kind="app")
  kind           = "terminal"  # content-kind seam: "terminal" now; "app"/"plugin" later
  float          = true        # spawn into the floating domain
  close_pane     = true        # close the pane when the process exits …
  keep_on_error  = true        # …but keep it open if it exited non-zero (read the failure)
  keep_on_success= false       # …or keep it open if it exited zero
  ```
- `close_pane` is the master switch; `keep_on_error`/`keep_on_success` are per-outcome overrides applied
  when the `pane.exited` event fires (exit code from the child).

### 0.5 Git library
- **`git2` (libgit2)** behind a small **`GitProvider` trait**, so swapping to `gix` later is one file.
  Rationale: mature, complete `statuses()` + branch/ahead-behind for the `+a/-d/Δ` badge.

### 0.6 Pane persistence + close model (settled 2026-06-18)
- **A pane's lifetime is tied to the terminal (the shell = the PTY child `portable_pty` spawned).** `try_wait`
  waits on *that shell*, not on programs running inside it. So **auto-close only fires when the shell/terminal
  dies** (you `exit` the shell, or it's killed). A foreground program *inside* the shell (nvim, lazygit, yazi)
  exiting does **not** close the pane — the shell keeps running → `try_wait` says "alive" → no close. This is
  already heca's behaviour and is **correct**; Phase 2 keeps auto-close as-is.
- **Foreground program exit → `status = Idle`** (shell is foreground again), pane stays open. **No `Exit`
  status variant** — a dead pane closes, it has nothing to display. The exit *code* is carried by the
  `pane.exited{code}` event (observability + Phase 6 close-policy), not by a status.
- **Close-policy (`keep_on_error`/`keep_on_success`) is Phase 6's concern**, applying only to **direct-spawn**
  panes (`[[keys.command]]` run *as* the PTY child, no shell). In Phase 2 every pane is a shell pane.

### 0.7 Process catalog config (`[programs.<raw>]`)
A map keyed by the **raw foreground program name** (the binary basename, e.g. `nvim`). Value = `ProgramMeta`:
```toml
[programs.nvim]
name        = "Neovim"         # display name (Row 1 "Name")
icon        = "\uE7C4"        # nerd-font glyph (Row 1 icon)
description = "modal editor"   # future usage (not rendered yet)
color       = "#fafafa"        # OPTIONAL pane/card tint
```
- All fields optional; partial entries are valid. `name` falls back to the raw key; `icon` falls back to a
  default terminal icon for known shells, none for unknown programs. Icons are **free-form glyph strings**
  (nerd-font codepoints), not a fixed `Glyph` enum — the renderer draws them as text.
- **Built-in defaults seeded** for common shells (`zsh`/`bash`/`fish`/`sh` → terminal icon) so out-of-the-box
  idle panes show a terminal icon + shell name. User entries **override** same-key defaults; users add their
  own (`nvim`, `lazygit`, `yazi`, …) via config.
- **Resolver:** `ProgramsConfig::resolve(raw) -> ProgramView { raw, name, icon, color }` — used by Phase 7
  display; falls back gracefully (raw name, no icon, no colour) when no entry exists.

### 0.8 Phase 3 implementation shape (settled 2026-06-18, prompted by an agent pre-coding review)
**Note:** the high-level Phase 3 design above was locked earlier, but three implementation-shape decisions
(shell-hook mechanism, config-switch name, OSC parsing ownership) were left open. An agent flagged them as
blockers before coding — correctly; the lead should have locked them up-front. They are now settled here so
no agent has to guess.

- **Shell hook = hybrid, shell-specific invocation wrapping** (NOT a pure env-var hook). There is no uniform
  env-var mechanism across bash/zsh/fish that sources a file for *interactive* shells. Auto-enable =
  shell-specific wrapping that sources the user's real RC + a heca snippet, shipped under
  `assets/shell-integration/`:
  - **bash** → spawn with `--init-file <heca-snippet>`; snippet sources `~/.bashrc` first, then installs
    `PROMPT_COMMAND` + `DEBUG`/`trap` hooks emitting OSC 133 A/B/C/D + OSC 7.
  - **zsh** → set `ZDOTDIR=<heca-dir>`; ship a heca `.zshrc` there that sources the user's real
    `${OLD_ZDOTDIR:-$HOME}/.zshrc`, then installs `precmd`/`preexec` hooks (preserve real ZDOTDIR).
  - **fish** → spawn with `fish -C "source <heca-snippet>"` (no `--init-file`); snippet uses
    `fish_prompt`/`fish_preexec` events.
  - `pty.rs` picks the right flag/env per detected shell. Document the manual-install path for users who
    can't use auto-enable (custom shells / restricted envs).
- **Config switch = a single flat bool in `[settings]`** (matches `mouse`/`auto_scroll_edge`/
  `always_center_single_column`):
  ```toml
  [settings]
  shell_integration = true   # default true; false ⇒ spawn bare shell (no injection); user can still source manually
  ```
  Start with the single bool. If per-shell knobs or a `"manual"` mode are needed later, grow it into a
  `[shell_integration]` table then — not now. Document in `keybindings.toml` + README.
- **OSC parsing = a passive pre-parse snooper in heca-core** before `engine.advance_bytes()`.
  wezterm-term at pinned rev `891bed31` exposes `AlertHandler`/`NotificationHandler` (bells) + `get_title()`
  (OSC 0/2) but **no general OSC-dispatch callback**, so intercept in heca. Contract:
  - **Passive observer**: scan the byte stream, extract OSC 133/7, then forward **all bytes unchanged** to
    wezterm-term. Never strip/consume — wezterm-term stays the canonical renderer/parser.
  - **Minimal correct VT state machine** for OSC: `ESC ]` (0x1B 0x5D) → collect params until terminator
    `BEL` (0x07) **or** `ST` = `ESC \` (0x1B 0x5C); handle C1 `ST` (0x9C) if present. Other OSC passes
    unobserved.
  - **Handle fragmentation**: keep a small residual buffer for an in-progress OSC across `advance_bytes`
    calls (OSC can span PTY reads).
  - **Route**: OSC 133 `D;0` → `Success`, `D;!=0` → `Error` + code (§0.3); OSC 7 `file://host/path` →
    `cwd`; OSC 133 `A`/`B`/`C` → trigger the Phase 2 foreground re-sample. Emit through the Phase-0
    chokepoint (`pane.status.changed` / `pane.cwd.changed`).
  - Rationale: no fork of wezterm's parser, one small unit-testable module (synthetic byte streams —
    already in the Phase 3 test plan), same approach other embedders use. Risk contained to one new module.

---
## 1. Goal & scope

**In scope (this plan):** reactive chrome store completion + typed event bus; per-pane runtime state
(program, status, cwd, git, content-kind); OS-native + OSC-based detection; shipped shell hook; git2
integration; extensible process→icon catalog; real command spawn with float + close-policy; fixed default
pane-info widgets; first-party `app.on` event exposure; doc updates.

**Out of scope (deferred):** tmux-style token/segment **customization** UI; WASM plugin runtime;
non-terminal **content providers** in panes (the `kind` seam is laid, not implemented); the broader
pluggable-chrome ChromeHost/provider system (this plan only adds the **event bus** + **shared-state**
pieces it needs).

---

## 2. Cross-cutting rules (every phase obeys)

- **New UI = a proper `heca-grid-ui` widget** — embed `Base`, read ALL styling from `Theme`, domain-neutral
  name/semantics, no hardcoded sizes/colors. **Never** ad-hoc inline `Flex`/`Surface` in the app.
- **Behavior through registries** — any new user action goes through `ActionRegistry` + `KeymapRegistry` +
  config, reachable from **keyboard + mouse/UI + RPC** where meaningful. No hardcoded key behavior.
- **Mutation chokepoint emits events** — never `signal.set()` scattered; state writes go through the store's
  `set_*`/action path which (a) updates the signal and (b) emits the typed event (guarded: only on change).
- **`heca-core` stays UI-free** — detection that needs OS/PTY lives in `heca-core` backend or a core service;
  the chrome store (UI) only mirrors. Canonical truth never lives only in the UI store.
- **Update the showcase + `docs/widgets.md`** in the *same* change when adding/extending a grid-ui widget.
- **Tests** for pure logic (status mapping, close-policy, catalog resolve, token-free segment build).
  **Don't run `cargo fmt`.** Keep `cargo clippy --workspace` clean; fix warnings even if pre-existing.

---

## 3. Data model (target shapes — agree before coding)

```rust
// heca-core (canonical runtime truth, UI-free)
pub enum ProcessStatus { Running, Idle, Success, Error }   // NO Exit — a dead pane closes (§0.6)
pub struct PaneRuntime {
    pub program: Option<String>,     // raw foreground program name ("nvim"), as-is
    pub status: ProcessStatus,
    pub cwd: Option<PathBuf>,
    pub exit_code: Option<i32>,      // set on Success/Error; also carried by the pane.exited event
    pub git: Option<GitInfo>,
    pub kind: ContentKind,           // Terminal now; App/Plugin later
}
pub struct GitInfo { pub branch: Option<String>, pub ahead: u32, pub behind: u32,
                     pub added: u32, pub modified: u32, pub deleted: u32, pub dirty: bool }
pub enum ContentKind { Terminal /*, App(ProviderId), Plugin(PluginId) */ }

// heca/src/chrome/state.rs — reactive MIRROR keyed by PaneId (signals), in WorkspacesContainerState
//   panes: Signal<HashMap<PaneId, PaneRuntimeView>>  (or per-field signals; see Phase 1 decision)

// event bus (heca/src/chrome or heca/src/events.rs)
pub enum ChromeEvent {
    PaneProcessChanged { pane: PaneId },
    PaneStatusChanged  { pane: PaneId, status: ProcessStatus },
    PaneCwdChanged     { pane: PaneId },
    PaneGitChanged     { pane: PaneId },
    PaneExited         { pane: PaneId, code: Option<i32> },
    RegionChanged      { region: RegionId },
    SelectionChanged,
    // … extend per slice
}
// → exposed to plugins as string names: "pane.process.changed", "pane.exited", …, plus "*"/"updated".
```

---

## 4. Phases & tasks

> Each phase is a board entry in `pane-runtime-tasks.md`. Tasks are checkboxes. Phases gate on
> **Depends-on**. An agent picks an *Open* phase from the board, implements per the task list here,
> verifies, and reports on the board.

### Phase 0 — Chrome event bus + finish the SharedChromeState consumer migration  ⟶ FOUNDATION
**Depends-on:** none. **Everything else depends on this.**

**Why:** the store exists (`heca/src/chrome/state.rs`) but selection/targeting/scroll fields aren't
consumed yet (the file's `#![expect(dead_code)]` marks this), clicks still use an `Rc<Cell>` mailbox
stopgap, and there is **no event bus** at all. This phase makes the store the real reactive hub + adds
the plugin-facing event channel.

**Key files:** `heca/src/chrome/state.rs`, `heca/src/chrome/mod.rs` (`sync_chrome_signals`,
`build_chrome_root`, `ChromeSinks`), `heca/src/app/interaction.rs` (the action chokepoint),
new `heca/src/events.rs` (or `chrome/events.rs`), `heca/src/app_state.rs`.

**Tasks**
- [ ] Add a typed **`ChromeEvent`** enum + a host-owned **event bus** (subscriber registry: `subscribe(name|*, handler)`, `emit(event)`), `!Send`/UI-thread, held in `AppState`.
- [ ] Make the **mutation chokepoint emit**: route store `set_*` + the `ActionRegistry`/`interaction` dispatch so every state-changing action emits the matching `ChromeEvent` (guarded: only on real change). Document the rule in `interaction.rs`.
- [ ] **Consume the unused store fields**: bind `hovered_pane` (from the hover dispatch already feeding the tree), drive `KeyHint` from `workspaces.pick_candidates` **instead of** reading `InputMode` directly (single home for pick letters — pick: store is the mirror, `InputMode` stays the action source → reconcile via Phase-0 sync), and remove the `#![expect(dead_code)]` once consumed.
- [ ] **Retire the `Rc<Cell>` click mailbox** (`ChromeSinks`): sidebar clicks → `InteractionIntent` → action → store, no shared cell.
- [ ] **Reactivity reconciliation:** make signal changes feed the damage/repaint path (`needs_paint`/`collect_damage`) so only the changed widget repaints, instead of the per-frame push in `sync_chrome_signals`. (The hard part — keep `sync_chrome_signals` as a fallback until proven.)
- [ ] Tests: event emitted-on-change-only; subscriber receives typed + catch-all; hover/pick/selection round-trip.

**Acceptance:** store has no dead fields; clicks/hover/pick/collapse/regions all flow store→signals→paint
**and** store→events; a test subscriber sees the right typed + `*` events; no `Rc<Cell>` mailbox; clippy clean.

---

### Phase 1 — Pane runtime state model
**Depends-on:** Phase 0.

**Why:** define the canonical `PaneRuntime` (core) + its reactive mirror (store) that Phases 2–6 fill and
Phase 7 renders. Decide the mirror shape (one `HashMap` signal vs per-pane per-field signals).

**Key files:** `heca-core/src/layout/column.rs` (or a new `heca-core/src/runtime.rs`),
`heca/src/chrome/state.rs`, `heca/src/app_state.rs`.

**Tasks**
- [ ] Add `ProcessStatus`, `GitInfo`, `ContentKind`, `PaneRuntime` to `heca-core` (UI-free; see §3).
- [ ] Store mirror: a `panes` slice in `WorkspacesContainerState`. **Decision:** start with per-pane field
      signals registered like the existing `pane_active`/`pane_hint` (so the damage model already works),
      keyed by `PaneId`; revisit a `HashMap<PaneId, …>` signal only if churn demands it.
- [ ] `set_*` writers on the store for program/status/cwd/git, each emitting its `ChromeEvent`.
- [ ] Update `sync_chrome_signals` (or its successor) to project canonical `PaneRuntime` → mirror signals.
- [ ] Tests: round-trip each field; event emitted on change.

**Acceptance:** a pane's runtime fields can be set on core, are mirrored reactively, emit events; widgets
can bind to them (proven by a throwaway/sidebar read).

---

### Phase 2 — Process detection (OS-native foreground + exit; event-first, poll-fallback)
**Depends-on:** Phase 1.

**Why:** fill `program` + running/idle + capture the child exit code (→ `pane.exited{code}`) from the PTY.
Event-first per §0.2; **no `Exit` status variant** (§0.6 — a dead pane closes).

**Key files:** `heca-core/src/backend/terminal/pty.rs` (already holds `child`, `master`, `try_wait`),
`heca-core/src/backend/terminal.rs`, a new **process-monitor** service (app-side, e.g. `heca/src/app/process_monitor.rs`).

**Tasks**
- [x] **Remove `ProcessStatus::Exit`** (shipped in Phase 1) — per §0.6 a dead pane closes, no Exit status. Update the Phase 1 enum + tests.
- [x] **Auto-close stays as-is** (§0.6): it only fires when the shell/terminal (PTY child) dies — correct persistence. Foreground program exit → `Idle`, **no close**. (Close-policy is Phase 6's concern for direct-spawn panes.)
- [x] **Exit + code (event):** in `TerminalBackend::update()` extend the existing `try_wait`/`reconcile_exit_state` to **capture the child exit code** (currently discarded — only `is_some()` is checked); emit `pane.exited{code}` through the Phase-0 chokepoint. The existing auto-close then fires for shell panes. **No polling loop for exit** — rides the reader's EOF wake.
- [x] **Foreground program + running/idle (OS-specific, event-driven):** from the PTY master, get the foreground process group (`tcgetpgrp` on `master.as_raw_fd()`) and compare to `master.process_group_leader()` (shell pgrp). `pgrp == shell` ⇒ `Idle` (program = shell name); else ⇒ `Running` + resolve the program name (macOS `libproc` `proc_name`/`proc_pidpath`; Linux `/proc/<pid>/comm`) — **basename only**. Put OS code behind a `#[cfg(unix)]` shim (`foreground_process(master) -> Option<(pid, name)>`); stub on Windows.
- [x] **cwd (OS fallback):** read the foreground pid's cwd on-demand when foreground changes. **Linux `/proc/<pid>/cwd` ✓ implemented.** **macOS OS-cwd deferred → Phase 3 OSC 7** — `proc_pidinfo`/`PROC_PIDVNODEPATHINFO` FFI has fragile struct layouts, and OSC 7 (Phase 3) is the preferred macOS cwd source anyway, so the OS fallback was deferred rather than ship risky FFI. **Tracked here + in `pane-runtime-tasks.md` Phase 2 one-liner.**
- [x] **Cadence — event-first, NO periodic timer (§0.2):** detection runs on the existing **output wake** (`BackendWake`) + the reader's **EOF wake**; the foreground re-sample is **debounced to 250 ms** so heavy output doesn't hammer `tcgetpgrp`/`libproc`. **Do NOT add a `ControlFlow::WaitUntil` periodic poll** — that fallback is deferred. No busy-spin; skip panes with no output churn.
- [x] **Architecture:** `TerminalBackend` caches the detected foreground + exit code internally (throttle detail); expose `PaneBackend::runtime() -> PaneRuntime` (default `default()` for non-terminal). A per-wake monitor (`sync_pane_runtime_from_backends`) copies `backend.runtime()` → the canonical `Pane.runtime` (Phase 1); the existing `sync_pane_runtime_state` then mirrors `Pane.runtime` → store + events (unchanged).
- [x] Feed all of the above through the Phase-0 chokepoint (store + events).
- [x] **Tests (FakeBackend):** idle↔running transition (fake foreground pgrp); exit captures code; name resolution (mock the OS shim); debounce skips too-frequent re-samples.

**Acceptance:** opening a shell shows `Idle` (terminal icon + shell name); running `nvim` flips to `Running`
+ program `nvim`; `:q` flips back to `Idle` (pane stays open — auto-close only on shell death); the shell
exiting captures the code + emits `pane.exited{code}` then auto-closes (no `Exit` status variant). **No
periodic polling timer.** **Verification = FakeBackend + TerminalBackend unit tests** (per §0.2 — no temporary debug log); macOS primary, Linux shim compiles. **macOS cwd OS-fallback deferred → Phase 3 (OSC 7)** — Linux `/proc/<pid>/cwd` works now.

---

### Phase 3 — Shell integration (OSC 133 / OSC 7) for success/error + cwd
**Depends-on:** Phase 2 (re-sampling hooks), Phase 1 (status field).

**Why:** in-shell command **success/error** + reliable **cwd** need semantic-prompt sequences. Build the
parser + ship a shell hook (per "do it all now").

**Key files:** the terminal-emulator parser under `heca-core/src/backend/terminal/` (`engine.rs` already
exposes `title`; find the OSC dispatch in the underlying lib), `heca-core/src/backend/terminal.rs`,
a shipped shell snippet (e.g. `assets/shell-integration/{bash,zsh,fish}`), the PTY env injection in `pty.rs`.

**Tasks**
- [x] **OSC snooper** (§0.8): implement the passive pre-parse extractor in heca-core before `engine.advance_bytes()` — forward all bytes unchanged to wezterm-term; handle `BEL` + `ST` (+ C1 `ST`) terminators + fragmentation across reads. Parse **OSC 133** `A`/`B`/`C`/`D;<exit>` (prompt-start / command-start / pre-exec / command-end+code); map `D;0` ⇒ `Success`, `D;!=0` ⇒ `Error` + code.
- [x] Parse **OSC 7** `file://host/path` ⇒ `cwd` (same snooper; macOS preferred cwd source — closes the Phase 2 macOS cwd deferral).
- [x] On **command-start/-end** markers, **trigger a foreground re-sample** (makes Phase 2 event-driven when integration is on).
- [x] **Ship a shell hook** (§0.8): bash `--init-file` / zsh `ZDOTDIR` / fish `-C source`, snippets under `assets/shell-integration/` that source the user real RC + emit OSC 133/7; `pty.rs` picks per shell. **Auto-enable** gated by `settings.shell_integration` (bool, default true); document the manual install path.
- [x] Emit `pane.status.changed` / `pane.cwd.changed` through the chokepoint.
- [x] Tests: feed synthetic OSC 133 D;0 / D;1 / OSC 7 byte streams → expected status/cwd.

**Acceptance:** with the hook active, running `false` in a shell pane shows `Error`; `true` shows `Success`;
`cd /tmp` updates cwd; without the hook, status falls back to running/idle (no breakage; no `Exit` status — see §0.6).

---

### Phase 4 — Git integration (git2 behind a trait, debounced)
**Depends-on:** Phase 3 (cwd) or Phase 2 (OS cwd fallback), Phase 1 (git field).

**Why:** per-pane branch + `+a/-d/Δ` badge from the pane's cwd.

**Key files:** new `heca-core/src/git.rs` (or `heca/src/app/git.rs`), `Cargo.toml` (+`git2`).

**Tasks**
- [x] Define a **`GitProvider` trait** (`status(cwd) -> Option<GitInfo>`); implement with **`git2`** (branch, ahead/behind via upstream, `statuses()` counts → added/modified/deleted/dirty).
- [x] **Debounce:** recompute on **cwd-change** + a slow timer; optional `notify` filesystem watch on `.git`. Cache per repo-root; share across panes in the same repo.
- [x] Non-repo cwd ⇒ `git = None` (display hides the git segments).
- [x] Feed `pane.git.changed` through the chokepoint.
- [x] Tests: a temp repo → branch + dirty counts via the trait (no UI).

**Acceptance:** a pane in a git repo shows branch + counts; leaving the repo clears them; no per-tick `git`
subprocess (in-process via git2); clippy clean.

---

### Phase 5 — Process catalog (extensible raw→{name, icon} map)
**Depends-on:** Phase 1 (program field). Parallel-friendly with 2–4.

**Why:** show `Neovim` + an icon for `nvim`, user-extensible via `[programs.<raw>]`; raw name always shown
in parens. Also seeds shell icons so idle panes show a terminal icon + shell name out-of-the-box.

**Key files:** new `heca-config/src/programs.rs` (`ProgramMeta`/`ProgramsConfig`/`ProgramView`),
`heca-config/src/lib.rs` + `loader.rs` (wire `programs` field), `keybindings.toml` + `README.md` (docs),
resolver consumed by Phase 7.

**Tasks**
- [ ] New `heca-config/src/programs.rs`: `ProgramMeta { name, icon, description, color: Option<Color> }` + `ProgramsConfig` (map keyed by raw program name) + `ProgramView { raw, name, icon, color }` resolver (per §0.7). `Color` deserialises from hex (`#rrggbb`/`#rrggbbaa`).
- [ ] **Built-in defaults seeded** in `ProgramsConfig::default()`: common shells (`zsh`/`bash`/`fish`/`sh` → terminal icon). User entries override same-key defaults; users add `nvim`/`lazygit`/`yazi`/… via `[programs.<raw>]`.
- [ ] **Config plumbing:** add `pub programs: ProgramsConfig` (serde default) to `Config`; `pub mod programs;` in `heca-config/src/lib.rs`. Document in `keybindings.toml` + `README.md`.
- [ ] **Resolver:** `resolve(raw) -> ProgramView` — `name` falls back to raw, `icon` to default-terminal (shells) / none (unknown), `color` passes through. Used by Phase 7; raw always available in `ProgramView.raw`. Icons are free-form glyph strings (no new `Glyph` enum variants needed).
- [ ] Tests: default shell hit; user override wins; miss → raw name + no/default icon; `color` parse + pass-through.

**Acceptance:** `nvim` resolves to "Neovim" + icon; an unknown program shows its raw name + default icon;
a config override wins; new glyphs render in the showcase.

---

### Phase 6 — Command spawn: run real programs + kind seam + float + close-policy
**Depends-on:** Phase 2 (exit event) for `close_pane`; Phase 1 (kind). Phase 5 optional (nicer titles).

**Why:** make `[[keys.command]]` actually launch programs with float + close-on-exit policy.

**Key files:** `heca/src/handlers.rs` (`handle_spawn_command` — the stub), `heca-config/src/keys.rs`
(`CommandKeybindConfig`), `heca/src/app/registry.rs` (config→action), `heca/src/input.rs`
(`WmAction::SpawnCommand` → carry options), `heca-core/src/backend/terminal/pty.rs` (`CommandBuilder`),
`heca/src/rpc.rs` (RPC parity).

**Tasks**
- [x] Extend `CommandKeybindConfig`: `kind` (default `"terminal"`, replaces unused `command_type`), `float`, `close_pane`, `keep_on_error`, `keep_on_success` (all default false).
- [x] Extend `WmAction::SpawnCommand` to carry `{ command, kind, float, close_policy }`; update `action_from_name`, priority, `action_policy`, the config→action map, and **RPC** (`spawn-command` with the same options).
- [x] **Real spawn:** `handle_spawn_command` builds a backend that runs `command` via `CommandBuilder` (extend `pty.rs` with `new_with_command`), tiled or floating per `float`, and stores the **close-policy** on the pane.
- [x] **Close-policy on exit:** when `pane.exited{code}` fires, apply: `close_pane && !(keep_on_error && code!=0) && !(keep_on_success && code==0)` ⇒ close the pane (through the normal close action).
- [x] `kind` seam: only `"terminal"` implemented; `"app"/"plugin"` return a clear "not yet" (no fork).
- [x] Tests: option parsing; close-policy truth table; spawn runs the program (integration/live).

**Acceptance:** the example config runs lazygit in a floating pane; exiting closes it; with `keep_on_error`,
a failing command stays open; reachable from keybind **and** RPC; follows the action checklist.

---

### Phase 7 — Display: fixed default pane-info widgets
**Depends-on:** Phases 1–6 data (degrades gracefully if a source is absent).

**Why:** render the pane info (icon · name · `(raw)` · status badge · git badges) on the sidebar card +
optional pane corner — real grid-ui widgets, theme-driven. The fixed default that customization later replaces.

**Key files:** `heca/src/chrome/mod.rs` (`pane_card`), `heca-grid-ui` (reuse `Icon`/`Label`/`Badge`/`Tag`;
add a small composed widget if warranted), the pane-corner overlay in `heca/src/app/terminal_render.rs`,
`heca-renderer/examples/showcase.rs`, `docs/widgets.md`.

**Tasks**
- [ ] **Sidebar card — Row 1 (program):** `Icon(catalog.icon)` · `Label("{name} ({raw})")` · `Badge(status, severity-toned)`, bound to the store mirror (reactive). Idle ⇒ terminal icon + shell name (no app program); Running ⇒ program icon + Name + `(raw)`. Icons are free-form nerd-font glyph strings from the catalog (§0.7) — render via a glyph label or extend `Icon` to accept a string glyph. Status→theme colour: Running=accent, Idle=muted, Success=success, Error=danger (**no Exit** — a dead pane closes). Reuse `Row`/`Grid`.
- [ ] **Sidebar card — Row 2 (git, only in a repo):** `Tag(branch)` (e.g. ` feature/my-feat` in a badge) + `Badge("+a/-d/Δ")` segments (added/new/deleted), bound to the store mirror. Hidden entirely outside a repo.
- [ ] Optional `color` from the catalog → tint the card/border.
- [ ] **Optional pane top-left corner badge:** compact Row 1 (`Icon + Name (raw)`) overlay, behind a config/appearance switch.
- [ ] **Showcase + `docs/widgets.md`:** demo the pane-info row (all status/git states) — required by the grid-ui rule.
- [ ] Verify reactivity: changing a pane's program/status/git updates only that card (damage), and emits the event.
- [ ] Tests where pure (segment build given a `PaneRuntimeView`); visual verify in the app + showcase.

**Acceptance:** sidebar cards show Row 1 (icon/Name (raw)/status) + Row 2 (git branch + counts) live; Idle
shows terminal icon + shell name; corner badge toggle on; absent sources hide cleanly (no repo ⇒ no Row 2);
showcase + docs updated; only the changed card repaints.

---

### Phase 8 — Plugin event exposure + plan/docs updates
**Depends-on:** Phase 0 (event bus). Finalize after 1–7 settle names.

**Why:** expose the bus through the host API shape (`app.on`) for first-party providers now (WASM later),
and record the deferred token customization + the event/state model in the architecture docs.

**Key files:** `heca/src/...` (a host-facade module for `app.on`/`app.state` read selectors — first-party),
`pluggable-chrome-plugin-plan.md`, `PLAN.md`.

**Tasks**
- [ ] First-party **`app.on(name, handler)`** + **`app.state.*`** read selectors over the event bus + store (no WASM yet; the seam the WASM bridge will reuse).
- [ ] `pluggable-chrome-plugin-plan.md`: document **state access for plugins** (read via selectors, react via events) and mark **§5.4 event bus** + **§3.3 shared state** as *foundation landed*; record the **deferred token/segment customization** shape (hybrid segment list + `${token}` templates) under **§8.1/§8.2**.
- [ ] `PLAN.md`: make this initiative the active near-term entry; link this doc; refresh status.
- [ ] Tests: a first-party subscriber drives a trivial provider end-to-end.

**Acceptance:** a first-party "provider" subscribes via `app.on('pane.status.changed', …)` and reads state via
a selector; the architecture docs reflect events + state access; PLAN.md points here.

---

## 5. Sequencing & parallelism
- **0 first, alone** (foundation). Then **1**.
- After 1: **2, 5** can start in parallel; **3** after 2; **4** after 3 (or 2's cwd fallback); **6** after 2.
- **7** after the data phases land (degrades gracefully meanwhile). **8** finalizes last.
- Suggested order if single-threaded: 0 → 1 → 2 → 3 → 6 → 4 → 5 → 7 → 8.

## 6. Risks / watch-items
- OS foreground-process + cwd code is platform-specific — keep it behind a tested shim; macOS is primary.
- The underlying terminal-emulator lib may already handle some OSC — find its dispatch before adding parsing.
- Reactivity reconciliation (Phase 0 last task) is the deepest item; keep `sync_chrome_signals` as a safety net.
- Don't let `heca-core` gain UI deps; the store is the only reactive layer.
- `git2` pulls a C dep; keep it behind `GitProvider` so `gix` remains a swap.
