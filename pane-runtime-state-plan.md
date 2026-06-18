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

### 0.2 Detection model — **event-first, poll only as fallback**
- **Exit** → PTY child-wait / reader EOF = event (`child.try_wait()` already exists). No poll.
- **Command success/error + cwd** → arrive as **escape sequences** in the output stream
  (**OSC 133** semantic prompts, **OSC 7** cwd) → parsed as output flows = event. For a **spawned single
  command** the exit code comes straight from the child (no shell hook needed); **OSC 133** is only for
  success/error of commands run *inside an interactive shell pane*.
- **Foreground program + running/idle** → the only source with no clean OS event. Re-sampled **on OSC 133
  command-start/-end markers** when shell integration is on (event-driven); **polled** (~250–500 ms) **only**
  as the fallback when it isn't.
- **Git** → re-checked on **cwd-change** (an OSC 7 event) + optional `.git` filesystem watch, slow-debounced;
  never per-tick.

### 0.3 Display + customization
- **Fixed default pane-info display now**, built as **real `heca-grid-ui` widgets** (theme-driven):
  `Icon(program)` · `Label("{name} ({raw})")` · `Badge(status)` · `Tag(git branch)` · `Badge(+a/-d/Δ)`.
  Shown on the **sidebar pane card** and (optional) the **pane's top-left corner**.
- **Process catalog** = user-extensible map `raw_program → { display_name, icon }`. Built-in defaults +
  config extension. The **raw program name is always available** verbatim; the catalog only adds a friendly
  name + icon (fallback = raw name + a default terminal icon).
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
pub enum ProcessStatus { Running, Idle, Success, Error, Exit }   // see Phase 2/3 for detection
pub struct PaneRuntime {
    pub program: Option<String>,     // raw foreground program name ("nvim"), as-is
    pub status: ProcessStatus,
    pub cwd: Option<PathBuf>,
    pub exit_code: Option<i32>,      // set on Exit/Success/Error
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

**Why:** fill `program` + running/idle + `Exit`/exit-code from the PTY. Event-first per §0.2.

**Key files:** `heca-core/src/backend/terminal/pty.rs` (already holds `child`, `master`, `try_wait`),
`heca-core/src/backend/terminal.rs`, a new **process-monitor** service (app-side, e.g. `heca/src/app/process_monitor.rs`).

**Tasks**
- [ ] **Exit (event):** surface child exit + code from `pty.rs` (`try_wait`) as a `pane.exited`-driving signal; wire into the monitor → store (`status = Exit`, `exit_code`). No polling loop for exit.
- [ ] **Foreground program + running/idle (OS-specific):** from the PTY master, get the foreground process group (`tcgetpgrp` on the master fd) → resolve to a program name. macOS: `libproc` (`proc_name`/`proc_pidpath`); Linux: `/proc/<pid>/comm`. `pgrp == shell` ⇒ `Idle`; else `Running` + that program. Put OS code behind a `#[cfg]` shim (`foreground_process(pty) -> Option<(pid, name)>`).
- [ ] **cwd (OS fallback):** read the foreground pid's cwd (macOS `proc_pidinfo`/`PROC_PIDVNODEPATHINFO`; Linux `/proc/<pid>/cwd`). (OSC 7 in Phase 3 is preferred when available.)
- [ ] **Monitor cadence:** event-first; **poll foreground only** on a ~250–500 ms timer **and** on terminal-output activity (we already wake on output). No busy-spin; skip panes with no output churn.
- [ ] Feed all of the above through the Phase-0 chokepoint (store + events).
- [ ] Tests: idle↔running transition from a fake foreground; exit sets code; name resolution (mock the OS shim).

**Acceptance:** opening a shell shows `Idle`; running `nvim` flips to `Running` + program `nvim`; exiting a
spawned command sets `Exit` + code; verified live on macOS (primary) with the Linux shim compiled.

---

### Phase 3 — Shell integration (OSC 133 / OSC 7) for success/error + cwd
**Depends-on:** Phase 2 (re-sampling hooks), Phase 1 (status field).

**Why:** in-shell command **success/error** + reliable **cwd** need semantic-prompt sequences. Build the
parser + ship a shell hook (per "do it all now").

**Key files:** the terminal-emulator parser under `heca-core/src/backend/terminal/` (`engine.rs` already
exposes `title`; find the OSC dispatch in the underlying lib), `heca-core/src/backend/terminal.rs`,
a shipped shell snippet (e.g. `assets/shell-integration/{bash,zsh,fish}`), the PTY env injection in `pty.rs`.

**Tasks**
- [ ] Parse **OSC 133** `A`/`B`/`C`/`D;<exit>` (prompt-start / command-start / pre-exec / command-end+code) in the engine; map `D;0` ⇒ `Success`, `D;!=0` ⇒ `Error` + code.
- [ ] Parse **OSC 7** `file://host/path` ⇒ `cwd`.
- [ ] On **command-start/-end** markers, **trigger a foreground re-sample** (makes Phase 2 event-driven when integration is on).
- [ ] **Ship a shell hook** for bash/zsh/fish that emits OSC 133/7; **auto-enable** it via PTY env (e.g. source a snippet / set the integration env var), with a config switch to disable. Document the manual install path.
- [ ] Emit `pane.status.changed` / `pane.cwd.changed` through the chokepoint.
- [ ] Tests: feed synthetic OSC 133 D;0 / D;1 / OSC 7 byte streams → expected status/cwd.

**Acceptance:** with the hook active, running `false` in a shell pane shows `Error`; `true` shows `Success`;
`cd /tmp` updates cwd; without the hook, status falls back to running/idle/exit (no breakage).

---

### Phase 4 — Git integration (git2 behind a trait, debounced)
**Depends-on:** Phase 3 (cwd) or Phase 2 (OS cwd fallback), Phase 1 (git field).

**Why:** per-pane branch + `+a/-d/Δ` badge from the pane's cwd.

**Key files:** new `heca-core/src/git.rs` (or `heca/src/app/git.rs`), `Cargo.toml` (+`git2`).

**Tasks**
- [ ] Define a **`GitProvider` trait** (`status(cwd) -> Option<GitInfo>`); implement with **`git2`** (branch, ahead/behind via upstream, `statuses()` counts → added/modified/deleted/dirty).
- [ ] **Debounce:** recompute on **cwd-change** + a slow timer; optional `notify` filesystem watch on `.git`. Cache per repo-root; share across panes in the same repo.
- [ ] Non-repo cwd ⇒ `git = None` (display hides the git segments).
- [ ] Feed `pane.git.changed` through the chokepoint.
- [ ] Tests: a temp repo → branch + dirty counts via the trait (no UI).

**Acceptance:** a pane in a git repo shows branch + counts; leaving the repo clears them; no per-tick `git`
subprocess (in-process via git2); clippy clean.

---

### Phase 5 — Process catalog (extensible raw→{name, icon} map)
**Depends-on:** Phase 1 (program field). Parallel-friendly with 2–4.

**Why:** show `Neovim` + an icon for `nvim`, user-extensible; raw name always available.

**Key files:** new `heca-config` section (e.g. `heca-config/src/process_catalog.rs`),
`heca-grid-ui/src/widgets/icon.rs` (add `Glyph`s), a resolver in `heca/src/chrome` or `heca-config`.

**Tasks**
- [ ] Built-in **defaults** map (nvim→Neovim, vim→Vim, yazi→Yazi, lazygit→LazyGit, htop, git, ssh, python, node, cargo, … ) → `{ display_name, Glyph }`.
- [ ] **Config extension** (`[[process]]` / `[process.catalog]` in `config.toml`): user adds/overrides `name` + `icon` (icon by `Glyph` name or codepoint). Merge defaults ← user overrides.
- [ ] Add the needed **`Glyph`** variants (find Phosphor codepoints; verify by rendering — see the swap-glyph method used earlier) + showcase/icon-gallery + `docs/widgets.md`.
- [ ] **Resolver:** `resolve(raw) -> { name: display_or_raw, icon: glyph_or_default }`. Raw always available separately.
- [ ] Tests: default hit, user override, miss → raw + default icon.

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
- [ ] Extend `CommandKeybindConfig`: `kind` (default `"terminal"`, replaces unused `command_type`), `float`, `close_pane`, `keep_on_error`, `keep_on_success` (all default false).
- [ ] Extend `WmAction::SpawnCommand` to carry `{ command, kind, float, close_policy }`; update `action_from_name`, priority, `action_policy`, the config→action map, and **RPC** (`spawn-command` with the same options).
- [ ] **Real spawn:** `handle_spawn_command` builds a backend that runs `command` via `CommandBuilder` (extend `pty.rs` with `new_with_command`), tiled or floating per `float`, and stores the **close-policy** on the pane.
- [ ] **Close-policy on exit:** when `pane.exited{code}` fires, apply: `close_pane && !(keep_on_error && code!=0) && !(keep_on_success && code==0)` ⇒ close the pane (through the normal close action).
- [ ] `kind` seam: only `"terminal"` implemented; `"app"/"plugin"` return a clear "not yet" (no fork).
- [ ] Tests: option parsing; close-policy truth table; spawn runs the program (integration/live).

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
- [ ] **Sidebar card:** compose `Icon(catalog.icon)` · `Label("{name} ({raw})")` · `Badge(status, severity-toned)` · `Tag(branch)` · `Badge("+a/-d/Δ")`, each bound to the store mirror (reactive; absent data hides its segment). Reuse `Row`/`Grid`. Status→theme color (Running=accent, Idle=muted, Success=success, Error=danger, Exit=muted).
- [ ] **Optional pane top-left corner badge:** compact `Icon + name (program)` overlay, behind a config/appearance switch.
- [ ] **Showcase + `docs/widgets.md`:** demo the pane-info row (all status/git states) — required by the grid-ui rule.
- [ ] Verify reactivity: changing a pane's program/status/git updates only that card (damage), and emits the event.
- [ ] Tests where pure (segment build given a `PaneRuntimeView`); visual verify in the app + showcase.

**Acceptance:** sidebar cards show icon/name/status/git live; corner badge toggentle on; absent sources hide
cleanly; showcase + docs updated; only the changed card repaints.

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
