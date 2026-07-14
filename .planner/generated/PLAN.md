# Heca — Project Plan

> Consolidare Heca come workspace compositor nativo, keyboard-first e pluggable, completando piattaforma terminale, architettura chrome/plugin, libreria grid-ui, integrazione Neovim GUI e rifiniture app/chrome.

Compositore di workspace GPU-native per sviluppatori, ispirato al layout a colonne scrollabili di Niri, che unifica terminali, editor e strumenti in una singola finestra accelerata via GPU. Think tmux meets Niri meets Neovide: terminali, editor e futuri container/plugin convivono nello stesso frame con animazioni fluide, testo nitido e chrome renderizzato via GPU.

**Last updated:** 2026-07-14T21:30:39.171Z
**Version:** 1
**Project ID:** `4fa9f09d-ec66-47a9-a37b-b6bf459ef747`

---

## Stack & Tools

### Technologies
- Rust
- wgpu
- cosmic-text
- winit
- portable-pty
- wezterm-term
- tokio
- serde
- TOML

## Architectural Decisions

- Usare il BACKLOG come fonte primaria e più aggiornata dello stato dei lavori.
- Mantenere nel planner anche le feature completate, marcate come done.
- Mantenere Pluggable Chrome come una singola feature con tutte le sue fasi, non spezzata in sotto-feature.
- Separare Grid-UI Widget Library da Pluggable Chrome: workstream distinti.
- Le widget G1–G8 di grid-ui esistono già nel codebase e non vanno ricreate.

## Global Rules

- Per UI nuove usare widget esistenti di heca-grid-ui o introdurre widget generici theme-driven in heca-grid-ui, non composizioni ad-hoc nell'app.
- Ogni azione utente deve passare per ActionRegistry e KeymapRegistry, con keybinding configurabili.
- Nessun hardcode di colori, keybinding o stile: leggere da Theme/config.
- Ogni capability significativa deve essere raggiungibile da mouse/UI, keyboard/action e RPC quando appropriato.

## Workflow Rules

---
## Features

### ✅ 8df6dded-5cfb-42a1-8da1-724f3d36b2b4 — F001 — 🎨 Theme System

Migrazione completa a `heca-theme`: crate standalone, temi bundled (`grid_tron`, `mocha`, `latte`), migrazione di `heca-config`, `heca-grid-ui` e app principale, rimozione dei colori hardcoded e migrazione del chrome hand-drawn verso widget grid-ui. Completata.

Status: ✅ `done`

**Phases:**
- ✅ **3fdd622b-1757-40f6-8af0-1767760fa792** P001 — theming-04: Migrate main app (heca) to heca-theme (3/3 tasks)
- ✅ **06906105-3054-4bda-9751-04902e03aefe** P002 — theming-02: Migrate heca-config to heca-theme (3/3 tasks)
- ✅ **457bdae8-efba-4665-a795-25120ea154bd** P003 — theming-03: Migrate heca-grid-ui to heca-theme (2/2 tasks)
- ✅ **21cc78bb-e0d6-4b57-a20c-a18cb98cd215** P004 — theming-05: Migrate hand-drawn chrome to grid-ui widgets (4/4 tasks)
- ✅ **3f91c60a-4a12-47ce-b210-ba1d85cc032a** P005 — theming-01: Showcase visual verification (2/2 tasks)

### ✅ be986d1d-cc2c-4acc-b703-4b1788a4ff86 — F002 — 🖥️ Terminal Platform Completion

Completamento della piattaforma terminale: damage preservation, dirty-region rendering, viewport scrollback host-managed, policy ligature, hook protocolli terminali, test backend/renderer, pane-shell hosting contract, text selection, clipboard/paste/OSC 52, UX terminale, image protocols e per-pane font zoom.

Status: ✅ `done`

**Phases:**
- ✅ **7120375b-cc0c-49e4-8b62-54cb6a35eadc** P001 — terminal-01a-h: Host terminal scrollback viewport (4/4 tasks)
- ✅ **903725c6-bb78-44a1-a25a-a720cdde68ff** P002 — terminal-01: Dirty-region terminal rendering (1/1 tasks)
- ✅ **40ca7813-1fa0-4a94-b0e0-cd00f48dca0e** P003 — terminal-02: Ligature policy (2/2 tasks)
- ✅ **486119ff-99c9-44d5-a622-66818ca933cc** P004 — terminal-00: Damage-preservation foundation (2/2 tasks)
- ✅ **7b69ffc4-bc98-47dd-95ab-1c45eb9b5074** P005 — terminal-05: Pane-shell hosting contract (1/1 tasks)
- ✅ **a3be94c5-7427-499f-82d6-4d91bd4c61dc** P006 — terminal-06: Text selection (2/2 tasks)
- ✅ **acb2c4bf-e7bf-4aa1-b41b-d908a35801e3** P007 — terminal-04: Backend and renderer tests (3/3 tasks)
- ✅ **c9038ab9-dcbc-4ec4-99fc-ecd161fc7eb3** P008 — terminal-09: Image protocols (9/9 tasks)
- ✅ **b23c166e-142d-41c8-80b0-edad5e83d405** P009 — terminal-03: Richer terminal protocol hooks (2/2 tasks)
- ✅ **57862b41-e5d9-4a82-8248-2ce6f8c7579b** P010 — terminal-07: Clipboard and paste (3/3 tasks)
- ✅ **24b9d664-80d0-48c8-ab5d-46d7daa177a8** P011 — terminal-08: Terminal UX and attention features (4/4 tasks)
- ✅ **e9b3402f-fb1f-45b7-ab95-c939736a3dab** P012 — terminal-10: Per-pane font zoom (3/3 tasks)

**Work done:** Piattaforma terminale completata: damage preservation, dirty-region rendering, scrollback host-managed, ligature policy, hook protocolli terminali, test backend/renderer, pane-shell hosting contract, text selection, clipboard/paste/OSC 52, UX terminale, image protocols e per-pane/whole-app font zoom. Il backlog su origin/main chiude anche la validazione manuale runtime (`terminal-task-07`) il 2026-07-02.

**Work remaining:** Nessun lavoro terminale aperto nel backlog principale. Restano al più verifiche osservative/non bloccanti già annotate nel backlog, ma il track è considerato completato.

### 📋 ebb9ceb0-ea12-4374-af6e-12aa4256bcc3 — F003 — 🧩 Pluggable Chrome Architecture

Architettura chrome pluggable: `ChromeHost` con regioni left/right/top/bottom, provider built-in, migrazione workspace tree in `WorkspacesContainerProvider`, dynamic action registry, host API per actions/overlay/regions, placeholder token system, plugin semplici da config e runtime WASM.

Status: 📋 `planned`

**Phases:**
- 📋 **09bcb3cf-f8ea-454a-913e-ea377c53d37e** P001 — plugin-08: WASM plugin runtime (0/5 tasks)
- 📋 **099e2eb5-30be-4f3c-ab57-a4f111cdc44c** P002 — plugin-06: Placeholder token system (0/3 tasks)
- ✅ **88b87a32-ff00-4044-9a32-c575e3ed5401** P003 — plugin-04: Dynamic action registry (3/3 tasks)
- ✅ **73c320aa-b8b7-4fb3-9421-5c741b1771ff** P004 — plugin-03: Built-in provider + WorkspacesContainer migration (6/6 tasks)
- ✅ **9ec31d60-7dfa-4dcc-9797-b265a4d30562** P005 — plugin-01: Formal architecture contracts (4/4 tasks)
- ✅ **7b50f8c3-bfe6-4e2f-869e-06c1ad9a946c** P006 — plugin-02: ChromeHost and region hosts (4/4 tasks)
- 📋 **f94d6da1-43b0-4b6a-a954-b404164126bd** P007 — plugin-09: Multi-region proof + config integration (0/3 tasks)
- 📋 **74abdac5-8a35-4e5f-a82f-3abb9bb0b738** P008 — plugin-05: Host API actions/overlay/region (0/3 tasks)
- 📋 **cec2ead0-bd28-4ac6-87f7-e3bcdf799ddc** P009 — plugin-07: Simple config.toml plugins (0/3 tasks)
- 📋 **3e04a8f5-3c51-4f2d-8fab-d51b392e80bc** P010 — action-interaction: Declarative action interaction (confirm + response buttons) (2/3 tasks)
- 📋 **99d19246-1ba9-4720-9895-e70c66acf144** P011 — plugin-ui: Declarative widget-tree UI model (ViewNode) (4/9 tasks)
- 📋 **8f760a9b-3ce8-4932-8286-67d742397f2e** P012 — context-menu: Contextual menu → OverlayHost + plugin-declarable (7/8 tasks)
- 📋 **05c9295a-0ab7-48a0-97a5-c2a5bbf2c5d9** P013 — menu-nav: Shared list/menu navigation keybindings (0/1 tasks)
- 📋 **7a3c634c-5aba-4de1-a01a-5c07f4ee4cb4** P014 — topbar-menu: Top-bar Menu (menubar) — STUB (0/1 tasks)
- ✅ **68a01ad8-1026-4c09-b516-2e2838723903** P015 — viewnode-all-widgets: ViewNode → all widgets (compositional refactor) (3/4 tasks)
- ✅ **1df9c36c-5c29-4288-9fe1-7a6e9c72179b** P016 — viewnode-choice: Choice primitive + compose the remaining widgets (full ViewNode coverage) (9/9 tasks)

### ⏸️ cd083ad1-8310-4368-981b-d14c73c20d96 — F004 — 📐 Grid-UI Widget Library

Espansione della libreria UI GPU-free e signal-driven: scroll/list primitive, pane shell header/tabs, icon widget Nerd Font, showcase coverage con visual regression, bloom/custom draw effects, widget aggiuntivi e cleanup del crate.

Status: ⏸️ `deferred`

**Phases:**
- ⏸️ **ef538743-f412-443d-811b-9e02b66106c1** P001 — gridui-01: Scroll/list primitive (1/7 tasks)
- 📋 **cb8f1203-9ae5-431d-8e7d-07dad2804396** P002 — gridui-05: Bloom and custom draw effects (0/2 tasks)
- 📋 **981bc6b0-9529-4a1c-96a3-7c8e34217bc4** P003 — gridui-02: Pane shell header and tabs (0/4 tasks)
- 📋 **ce5d5ea4-0f84-4e5a-909a-77d8344a2a87** P004 — gridui-04: Showcase coverage and visual regression (0/2 tasks)
- 📋 **35d74cc9-a3df-49c9-a5bb-59e12a88b49c** P005 — gridui-06: Additional widgets (0/6 tasks)
- 📋 **f9a8c93b-6938-4644-88e4-f1d2d2f59d6d** P006 — gridui-07: Crate-review debt (0/9 tasks)
- 📋 **36628b50-a0b5-4974-852e-96d68c09f11f** P007 — gridui-03: Nerd-Font icon widget (0/4 tasks)
- 📋 **402d22d4-a658-47d7-8481-fb074ce43a00** P008 — button-shortcut: Button accelerator / shortcut (0/2 tasks)

### 📋 b5d04826-7fd8-4094-ace5-96dc93b825a0 — F005 — 🌫️ Compositor Frost

Lavoro sulla pipeline z=0 per fondo frosted/blurred: gradient background, cached blur, integrazione pipeline, tuning visivo e ship review finale. La pipeline base è merged, restano tuning e review finale.

Status: 📋 `planned`

**Phases:**
- ❌ **2e48e3d1-0cff-4ead-863f-c36ff1077178** P001 — compositor-05: Visual tuning (0/4 tasks)
- ✅ **743fd127-624a-44e4-83e8-370a53cc1beb** P002 — compositor-01-to-04c: z=0 pipeline (merged) (5/5 tasks)
- ✅ **1af771d1-e22b-43ce-94d3-b9f228e926db** P003 — compositor-06: Ship review finale (1/1 tasks)

**Work remaining:** La pipeline base è merged. Il tentativo di visual tuning (compositor-05) è stato rifiutato; la review finale/compositor-06 è differita e non è lavoro attivo in questo momento.

### 📋 2809a7a4-6d53-4a58-b776-bac164bb5d03 — F006 — 🛠️ App / Chrome Features

Feature applicative e di chrome: audit di parità con Niri, split di `render.rs`, zoom/font-size controls, pane numbering, workspace drag-to-reorder, sidebar wiring + collapsed rail, damage-region optimization, fix vibrancy warning macOS, leftovers sidebar/chrome e context menu.

Status: 📋 `planned`

**Phases:**
- 📋 **f6476b06-0938-4fa1-88ab-23e0283036fa** P001 — app-02: Split render.rs into render folder (0/6 tasks)
- 📋 **621e6f47-a0e2-4d0d-9012-e6feadb70097** P002 — app-06: Sidebar wiring + collapsed rail (0/3 tasks)
- 📋 **f6a5aa17-dbbf-4463-8ae1-744ab1ba886f** P003 — app-01: Niri layout parity audit (0/3 tasks)
- 📋 **9e5ec754-ebf3-47f9-9850-378dd75b8e1c** P004 — app-07: Damage-region render optimization (0/5 tasks)
- 📋 **c192c5ca-f6ac-493a-bd8d-d95117c430aa** P005 — app-10: Sidebar/chrome leftovers (3/6 tasks)
- 📋 **5cab2dbb-ce1b-4a33-b6bc-ede8437fca09** P006 — app-11: Right-click context menu (2/3 tasks)
- 📋 **00afc6e8-101c-47e3-8739-5a8b216c81c4** P007 — app-04: Pane numbering (0/3 tasks)
- 📋 **50c22c08-b639-4a1a-a3cb-2a9f7084c0f9** P008 — app-05: Workspace drag-to-reorder (0/4 tasks)
- 📋 **a6831878-0168-4261-935e-818cc9b9a909** P009 — app-03: App-wide zoom and font-size controls (0/3 tasks)
- 📋 **884c75d3-e99a-4537-b09c-aa9efab3270c** P010 — app-08: Fix NSWindow vibrancy warning (0/1 tasks)
- ✅ **3bdc254b-4b5c-4f74-a783-8de4ae1628fa** P011 — app-12: Keyboard move-to-target picks + rename override + plugin-observable state (3/3 tasks)
- 📋 **9cefac90-9fea-427c-9106-9a41e71e8acd** P012 — app-13: Mouse/input leftovers (0/2 tasks)

### 📋 0c704076-6072-4c57-8ee1-9fc11f6eaa3a — F007 — 🖋️ Neovim GUI Pane

Integrazione Neovim GUI come pane: `nvim --embed`, msgpack-RPC, render singola grid, multi-istanza, `ext_multigrid`, input routing, image layer dedicato e markdown layer. Feature ancora in fase di idea/progettazione.

Status: 📋 `planned`

**Phases:**
- 📋 **14f9486d-6e6a-4dc3-9315-ca553dfe8721** P001 — nvim-03: ext_multigrid (0/4 tasks)
- 📋 **dcf766af-eb11-4a78-9dd3-1e1e15663c33** P002 — nvim-04: Input routing (0/3 tasks)
- 📋 **87939acb-c177-4c6b-b4d7-2df14c25390d** P003 — nvim-06: Markdown layer (0/2 tasks)
- 📋 **aa6c3a49-f3a4-4330-876d-4156745e1ade** P004 — nvim-05: Image layer (0/3 tasks)
- 📋 **0d4b8645-a638-4774-a9e1-ed8edb37e53f** P005 — nvim-01: Embed nvim + RPC + render single grid (0/3 tasks)
- 📋 **11d9a9ac-c22d-4613-9423-7f97a64acbcd** P006 — nvim-02: Editor-pane integration (0/3 tasks)

### ⏸️ eef184fb-863e-476f-8839-0dff37443acf — F008 — 🤖 AI Agent Integration

Integrazione agenti AI: `AgentDriver` trait e registry, built-in drivers (Claude/Codex/pi), status agent nei pane e suoni di transizione. Feature parcheggiata e dipendente dalle fasi iniziali di Pluggable Chrome.

Status: ⏸️ `deferred`

**Phases:**
- ⏸️ **f5251ab1-db02-4c74-a1f3-97380b5909f5** P001 — agents-01: Agent status tracking and sounds (0/4 tasks)

**Work remaining:** Feature pianificata ma differita: l’implementazione dipende dal completamento delle fondamenta di Pluggable Chrome, in particolare plugin-01 → plugin-05.

### 📋 fc19894f-c2aa-4ed1-81f3-24d82670dbae — F009 — 🔔 Notification System

Sistema notifiche configurabile per Heca. V1 implementa notifiche in-app tramite i widget esistenti `Toast`, `ToastStack`, `ToastSpec` e `ToastSeverity`; Heca possiede modello, store, queue, lifecycle, timer, dedup, routing e dispatch azioni. Backend OS/system notifications e integrazione global KeyHint per azioni toast restano differiti. Out of scope: confirmation modals e confirmation-before-action flow.

Status: 📋 `planned`

**Phases:**
- 📋 **93379c9a-43b7-43eb-8988-be505251fe79** P001 — notification-02: Store + Lifecycle (0/9 tasks)
- 📋 **03b297bb-a915-4712-a6b4-dc688dc7ddc5** P002 — notification-06: Routing + Additional Producers (0/7 tasks)
- 📋 **e8d54d82-7357-4436-9b81-22cf2eaef545** P003 — notification-05: Reload Config Producer (0/5 tasks)
- 📋 **ccefbac4-b29e-45d8-bd1d-e7199afa4907** P004 — notification-04: Toast Action Dispatch (0/5 tasks)
- 📋 **e5ceaa1e-9b7a-4d44-b155-edb7fe4b7ef2** P005 — notification-01: Model + Config (0/10 tasks)
- 📋 **e298f926-f6b3-4147-abcc-eea87171cd81** P006 — notification-03: In-App Toast Rendering (0/6 tasks)
- 📋 **461b88ca-5faa-4bea-8734-6c98587f115d** P007 — notification-09: Tests + Documentation (0/8 tasks)
- 📋 **941d82d3-2770-4e6f-b42f-76b36e160c37** P008 — notification-07: OS/System Notification Backend (0/5 tasks)
- 📋 **949d48a4-6001-42b2-a12c-25ba69a95c24** P009 — notification-08: Toast Keyboard Hint Integration (0/4 tasks)

### 📋 2272d270-40ae-47fe-bc64-a173b51a097c — F010 — Improvements

Quality-of-life improvements track, separate from the full Notification System feature. Small, focused enhancements that surface app feedback to the user: notifications for blocked keybindings (policy) and config-reload outcomes. Each improvement is self-contained and ships a visible user-facing signal.

Scope starts with a Notification phase (toast surface + two hooks). May grow with other improvement slices over time. Kept distinct from the planned Notification System feature (fc19894f, 9 phases) which is the full notification architecture — Improvements delivers lightweight, immediate feedback first.

Status: 📋 `planned`

**Phases:**
- 📋 **dcf9ff70-cf6e-416e-8e46-8cc36b4ec233** P001 — Notification (0/2 tasks)
- 📋 **6d582a8a-3b10-4157-8009-2b2db2825e29** P002 — Actions/Keybindings (0/4 tasks)
- 📄 **13be12b1-e912-4117-88bb-6ac85e230bcf** P003 — WhichKey Like Modal (0/0 tasks)

---
## Requirements

### 37773571-97a9-4732-9472-a4b1091508a7 — widget-keys-config: In-widget keys should be customizable

DONE (2026-07-12) — unified `WidgetIntent` + host-owned `Keymap` + `[keys.widgets]`. Branch `feat/widget-keymap-unify` (off `feat/widget-keymap` PR #234). The three per-widget event vocabularies (`MenuNav`/`DialogNav`/`InputEdit`) and the three app maps (`menu_keymap`/`dialog_keymap`/`input_keymap`) were collapsed into **one** `heca_grid_ui::WidgetIntent` (`ItemPrevious`/`ItemNext` = horizontal, `MenuUp`/`MenuDown` = vertical, `Activate`/`Dismiss`, `EditDeleteBackward`/`EditDeleteToLineStart`/`EditSelectAll`) delivered as `Event::Widget(..)`, and a **host-owned** `heca_grid_ui::Keymap` (`keymap.rs`): `key chord → Vec<WidgetIntent>` + `Keymap::dispatch` (raw key field-first, then resolved intents; the `Ctrl+h` overload disambiguates by focus). NOT a global — each host owns its `Keymap` (`AppState.widget_keymap` in the app, `Keymap::with_defaults()` in the showcase). Config moved to a `[keys.widgets]` table; `heca-config` `KeysConfig.widgets` field; `build_widget_keymap` (`registry.rs`) + `combo_to_grid`. Tab/Shift+Tab stay the universal focus primitive (FocusManager + trapped in a modal Dialog), **not** configurable.

Bug fixes (2026-07-12) after user testing, on `feat/widget-keymap-unify`:
- `5127604` — macOS `Ctrl+letter` gave a control char via winit logical key → resolve from the normalized `event_combo` (physical-key fallback) + `combo_to_grid`.
- `83f8361` — `normalize_key_text` names `NamedKey`s capitalised (`"ArrowDown"`/`"Tab"`/`"Enter"`), but `combo_to_grid` matched lowercase → returned `None` for every named key → overlay dispatch was skipped entirely. FIX: lowercase the key name in `combo_to_grid`. Trap: any KeyCombo→grid conversion MUST be case-insensitive on the key name.

Sub-tasks:
- [x] widget-keys-config-1 — audit every heca-grid-ui widget that matches literal keys in `event()`; propose the configurable model (intents + injected keymap); wire `Dialog` + `Input` first. DONE (model chosen 2026-07-11: host-driven semantic events, menu-nav parity).
- [~] widget-keys-config-2 — apply the same host-driven pattern to the remaining literal-key widgets. `Select` DONE (open-list nav consumes `Event::MenuNav`, folded into shared menu-nav vocabulary). `Tabs` DEFERRED (showcase-only; convert when actually mounted in-app).
- [ ] widget-focusable-centralize — 18 widgets re-implement `Component::focusable()`; make it a `Base.focusable` property defaulted per widget (set in constructor / when a callback is wired), with `disabled` handled centrally in the trait default (`base.focusable && !base.disabled`). Drop the 18 overrides; keep a small override only for genuinely dynamic cases (`Dialog` = while open). Top next task.
- [ ] widget-keys-bug-dialog-ok-focus — BUG (needs running app): rename-dialog Tab traverses Input↔Cancel but never the OK/primary button. Check the `Tooltip` wrapper in `overlay.rs build_modal_root` (~L338–363) and the `spec.actions` order. Do after focusable-centralize.
- [ ] widget-keys-verify-inapp — verify the key fixes in the GPU app + showcase.

Docs: `docs/widgets.md` (Input/Select/Tabs/Dialog/ContextMenu/CommandPalette), README (`[keys.widgets]`), rustdoc. Gates: workspace clippy 0, all tests green (grid-ui 128+69, heca 333+89, heca-config 78). In-app visual pass by the user pending. Full detail + resume steps: repo-root `HANDOFF.md`.

Source: BACKLOG.md (Requirement, requested 2026-07-09).

Status: ✅ `done`
Phases: `05c9295a-0ab7-48a0-97a5-c2a5bbf2c5d9`, `99d19246-1ba9-4720-9895-e70c66acf144`

### 4f66a18a-8805-4dd1-a9d6-63f5c51e9be3 — terminal-theming: Terminal color reload + theme integration (BUGS)

Two terminal color bugs found during the pane-naming session (2026-07-10). User: fix on a separate branch after pane-naming. Transparency was **off** when Bug 2 was observed (so it is a palette/default-bg issue, not z=0 frost compositing).

Sub-tasks:
- [ ] terminal-theming-1 — BUG: terminal colors are not reapplied on config reload (`prefix+Shift+r`). Root-caused: `reload_config` recomputes the palette and calls `engine.reload_config` → `terminal.set_config(...)`, but wezterm-term's `set_config` only swaps the config Arc — it does **not** reset the *forked* palette override. `TerminalState::palette()` returns `self.palette` (the fork) when set, else `config.color_palette()`. Any program that uses a dynamic-color escape (OSC 4/10/11/104…) — nvim always does, many shell prompts too — forks `self.palette`, so a heca theme reload updates the config but `palette()` (read by the snapshot, `engine.rs` ~578/643) keeps returning the stale fork. Pristine shells (never touched colors) *do* re-theme. Fix: in `TerminalEngine::reload_config` (`heca-core/.../engine.rs`), after `set_config`, force the new palette to win — e.g. `*self.terminal.palette_mut() = self.terminal.get_config().color_palette()` (or reset the fork to `None`). wezterm's `implicit_palette_reset_if_same_as_configured` is insufficient (only resets when the fork already equals config). A running nvim reasserts its own colors on its next redraw.
- [ ] terminal-theming-2 — BUG: an nvim dark colorscheme has "no effect" under a light UI theme (latte), transparency off. Contributing facts: (1) the UI theme does **not** drive the terminal palette — `terminal_palette_defaults` (`heca/src/app/backend_factory.rs`) reads only explicit `theme.terminal_foreground/background/ansi/brights/…` overrides; unset → wezterm `ColorPalette::default()` regardless of mocha/latte. (2) Likely the same fork mechanism as terminal-theming-1 interacts with a light `terminal_background`. Needs deeper investigation on the fix branch (repro: latte + opaque terminal + dark nvim colorscheme; check whether heca's default bg overrides nvim's OSC-set bg, and whether the UI theme *should* map to terminal defaults when no explicit `terminal_*` override exists).

Source: BACKLOG.md (Requirement, BUGS 2026-07-10).

Status: 📋 `planned`

### 31551e80-42a5-4e36-ac31-a71594de2545 — available-actions: Context-aware available-actions query

A single query answering "which actions are applicable right now", given the current context — so a surface (first consumer: the command palette; also future context-aware help / which-key) can list exactly the actions the user can take at this moment. This is the *general* form of what the context menu already does per-target: the context menu maps a `ContextPath`/`ContextTarget` to a curated entry list; this maps the **whole live context** (focus domain, `InputMode`, focused pane / sidebar cursor, floating-vs-tiled) to the **full set of currently-available `WmAction`s**.

Build on the pieces already in place — do NOT invent a parallel system:
- `action_policy()` (`app/interaction.rs`) already classifies every `WmAction` by where it is allowed (Tiled/Floating/Workspace/etc.); the availability filter is the same predicate the interaction router uses (`route_action`), so "available" == "the router would Allow it now".
- `ContextPath`/`ContextTarget` + `resolve_active_context` (`chrome/context_menu.rs`) already resolve the active context; extend that resolution to also drive action availability.
- `ActionRegistry::ALL` + `ActionCatalog` (`actions.rs`) is the enumerable action set with names/icons/descriptions the palette renders.

Target shape (subject to design): `available_actions(state) -> Vec<ActionAvailability { action_name, allowed: bool, reason }>` (or an iterator of allowed actions), computed from `action_policy` + current focus domain + `InputMode` + context, reachable from keyboard/RPC and consumed by the command palette. Unit-testable as a pure mapping (mirror `resolve_context_for`). Defer the palette UI itself; this requirement is the **query/infrastructure** it will read.

Sub-tasks:
- [ ] available-actions-1 — design + implement the context→available-actions query on top of `action_policy`/`resolve_active_context`/`ActionCatalog`; pure + unit-tested; RPC-introspectable. (Feeds the command palette — a later phase.)

Source: BACKLOG.md (Requirement, asked by user 2026-07-09 while context-menu context system was being built).

Status: 📋 `planned`
Phases: `8f760a9b-3ce8-4932-8286-67d742397f2e`

### 2458a02c-31e2-4b09-9bdb-fe1e56d9d68e — pane-naming: Pane naming + sidebar pane rows

Follow-ups from the rename-dialog session (2026-07-10). Full detail + resume steps in the repo-root `HANDOFF-pane-rename-naming.md` §1. Done this session (uncommitted on `feat/planner-backlog-sync`): rename→modal dialog, by-id rename actions, sidebar-mode rename bindings, `AppName` segment shows the program name (not the rename), prefill fixes, sidebar-rename mode-restore, `pane_renamed_add_process_name` config (rendering not yet wired).

Sub-tasks:
- [ ] pane-naming-1 — new `[appearance.pane] title_segments` value **`pane_name`** (shows `view.title` = original/renamed name; NOT default). `PaneSegment::PaneName` in `heca-config/src/appearance.rs` + `build_pane_info_bar` arm + README/config.default.toml.
- [ ] pane-naming-2 — sidebar pane card: append the small dimmed **`(process)`** suffix when the pane has a custom name + `pane_renamed_add_process_name`. Carry the flag on `WorkspacesContainerState` (signal, set in `sync_chrome_state`) — read in `pane_card`; then remove the now-dead info-bar `add_process_name` plumbing.
- [ ] pane-naming-3 — new `[settings] pane_show_cwd: bool` + a **cwd row** (folder icon + `home_relative_path(cwd)`) between the name row and the git row in the sidebar pane card.
- [x] pane-naming-4 — column rename reachability. Added `RenameColumnByIdx { ws_idx, col_idx }` (full wiring). Menu entry removed by decision (2026-07-10): a column's name is not displayed anywhere (columns render as a `MarkerGroup` with no header/label), so a Rename-column menu entry renames something invisible. The action stays wired (RPC + handler) for when columns surface a name.
- [ ] column-name-display (follow-up, from pane-naming-4) — surface a column's name in the sidebar (a per-column header/label on the `MarkerGroup`, theme-driven, domain-neutral widget per the grid-ui rules). Only then does renaming a column pay off — re-add the Rename-column context-menu entry at that point.
- [ ] pane-naming-5 — declare **icons on EVERY action** used in menus/buttons, in its `ActionDescriptor` (`actions.rs` `ActionRegistry::ALL`) — not just rename. Audit all entries built by `chrome/context_menu.rs` (pane + sidebar.pane/column/workspace) and any button: `split_horizontal`, `split_vertical`, `zoom_column`, `float`, `close`, `create_workspace`, `add_pane_to_column`, `add_column_to_workspace`, `delete_column`, `delete_workspace`, `rename_pane`/`rename_workspace`/`rename_column`, `open_link`, … Each gets `icon: Some(Glyph::…)`. Add any missing glyphs (e.g. `Pencil` for rename) to the central `Glyph` enum + icon-font mapping + `docs/widgets.md`.
- [ ] pane-naming-6 — BUG (needs repro): renaming from the sidebar renames the wrong pane (hypothesis: `pending_context` cursor on a non-pane row → falls back to focused pane).
- [x] pane-naming-7 — Remove/clear a custom pane/workspace name (revert to process/default name). DONE (2026-07-10). 4 dedicated actions (`ResetPaneName`/`ResetPaneNameById`, `ResetWorkspaceName`/`ResetWorkspaceNameByIdx`), full wiring + RPC + descriptors (icon `Backspace`, labels "Use process name"/"Use default name"), reusing `apply_rename(target, "")`. Menu entries conditional (only shown when a custom name exists). Original decision (2026-07-10, user): dedicated action (Option B) — NOT empty-submit in the rename dialog.

Source: BACKLOG.md (Requirement).

Status: 🚧 `in-progress`
Phases: `3bdc254b-4b5c-4f74-a783-8de4ae1628fa`

### 7340431a-5569-4383-ab54-978c2b2b338f — reload-bugs: Config-reload consistency bugs

Two reload-staleness bugs found during the pane-naming session (2026-07-10) — things that only partly re-apply on `prefix+Shift+r`. Same family: a retained/cached tree not invalidated on reload. Worked on branch `fix/reload-bugs` (off `main`, after #228/#229 merged). DONE 2026-07-11.

Sub-tasks:
- [x] reload-bug-header-icons — BUG: pane-header action icons stay **faint/stale on existing panes** after a theme reload (e.g. mocha→latte); a newly-created pane looks correct. Root-caused + FIXED. Retained per-pane headers (`state.pane_headers`) bake theme colors + font into their widget tree at build time, and `pane_header_key` deliberately carries no theme identity (themes only change on reload), so a theme swap changed no header's key and the stale trees were kept. `reload_config` invalidated the other retained trees (`terminal_layers.clear()`, `chrome_tree = None`) but not `pane_headers`. Fix: new `chrome::clear_pane_headers` (drops each header's `HintTargetRegistry` range, then clears) called from `reload_config`; also de-dups the identical inline cleanup in `sync_pane_headers`.
- [x] reload-bug-terminal-transparency — REPORTED: changing `[appearance.terminal] transparency` + reload only partially applies (old alpha persists until a new pane opens). Resolved — no code change needed; user-verified fixed on current `main` (2026-07-11). `terminal_layer_render_key` (`app/terminal_render.rs`) already hashes `surface_alpha` (since the `terminal-00b` foundation, test `terminal_layer_render_key_changes_with_surface_alpha`), and `retained_damage_to_apply` maps any `style_changed` to `TerminalDamage::Full`, so a transparency change forces a full repaint of every pane's retained layer on the next frame. The symptom predated the merged terminal-theming-2 fix; on current `main` it no longer reproduces.

Source: BACKLOG.md (Requirement, found 2026-07-10, done 2026-07-11).

Status: ✅ `done`

### de43129f-eb37-4e1a-bd7f-2ba52c5a9c99 — chrome-bugs: Chrome interaction bugs

UI/interaction bugs found during the pane-naming session (2026-07-10). Not diagnosed yet — capture + repro first.

Sub-tasks:
- [~] chrome-bug-collapsed-sidebar-picks — BUG: with the sidebar **collapsed** (rail mode), clicking a rail cell focuses the **wrong pane** / opens the **delete dialog** / shows the wrong highlight. Root cause found (grill-me, 2026-07-11): the collapsed rail is hand-drawn (`render_sidebar_collapsed`) and `sidebar_hit_test` re-derives the rail geometry with its own magic numbers (`ITEM_HEIGHT`, a phantom `BTN_ROW_HEIGHT` top offset the rail never draws) — the two copies drifted, so clicks map one row off. Resolution (decided with the user): DROP the collapsed rail entirely rather than rebuild it — a region is now **Expanded ⇄ Hidden** (no rail). Deleting `render_sidebar_collapsed` + the collapsed branch of `sidebar_hit_test` removes the broken code and closes this bug by construction. Tracked as the "drop the collapsed rail" work under `app-task-21`. Full design + rationale: `docs/sidebar-provider-modes.md`.
- [ ] chrome-bug-titlebar-doubleclick-fullscreen — BUG (macOS): double-clicking the top-bar sidebar-toggle button enters OS full screen. No app fullscreen/titlebar code exists — the window uses `Window::default_attributes()` (native macOS titlebar) and the vibrancy path doesn't touch the style mask, so this is macOS's native "double-click title bar to zoom/fill/full screen" firing because the top-bar interactive regions sit in the OS titlebar's draggable band (button eats the first click, the OS titlebar gets the second). Fix is macOS-specific: exclude the top-bar buttons from the drag/titlebar region (`mouseDownCanMoveWindow = NO` on those NSViews) or disable titlebar double-click zoom for the window. Files: `heca/src/app/startup.rs` (window/NSWindow setup). Needs on-machine repro.

Source: BACKLOG.md (Requirement, found 2026-07-10).

Status: 🚧 `in-progress`
Phases: `c192c5ca-f6ac-493a-bd8d-d95117c430aa`, `5cab2dbb-ce1b-4a33-b6bc-ede8437fca09`

---
## Phases

### 📄 227d7422-e493-41ff-b786-8e659bf2f88c — P001 — notification-03

Status: 📄 `draft`

### 📄 e544983e-887a-4978-985d-4f9a279496ff — P001 — notification-06

Status: 📄 `draft`

### 📋 14f9486d-6e6a-4dc3-9315-ca553dfe8721 — P001 — nvim-03: ext_multigrid

Supporto multi-grid/window con geometry, viewport e z-ordering.

Status: 📋 `planned`

**Tasks:** 0/4

### 📋 dcf766af-eb11-4a78-9dd3-1e1e15663c33 — P002 — nvim-04: Input routing

Encoding tasti/modifier, mouse forwarding e convivenza col prefix mode.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 87939acb-c177-4c6b-b4d7-2df14c25390d — P003 — nvim-06: Markdown layer

Lettura buffer via RPC e preview markdown nativa/rich.

Status: 📋 `planned`

**Tasks:** 0/2

### 📋 aa6c3a49-f3a4-4330-876d-4156745e1ade — P004 — nvim-05: Image layer

Canale RPC dedicato per immagini con riuso di ImageRenderer.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 0d4b8645-a638-4774-a9e1-ed8edb37e53f — P005 — nvim-01: Embed nvim + RPC + render single grid

Spawn `nvim --embed`, msgpack-RPC, `nvim_ui_attach` e render singola grid.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 11d9a9ac-c22d-4613-9423-7f97a64acbcd — P006 — nvim-02: Editor-pane integration

Multi-istanza, lifecycle dei pane, focus/input forwarding.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 dcf9ff70-cf6e-416e-8e46-8cc36b4ec233 — P001 — Notification

Status: 📋 `planned`

**Tasks:** 0/2

### 📋 6d582a8a-3b10-4157-8009-2b2db2825e29 — P002 — Actions/Keybindings

Status: 📋 `planned`

**Tasks:** 0/4

### 📄 13be12b1-e912-4117-88bb-6ac85e230bcf — P003 — WhichKey Like Modal

Implement a neovim emacs wich-key feature like.

Status: 📄 `draft`

### 📋 f6476b06-0938-4fa1-88ab-23e0283036fa — P001 — app-02: Split render.rs into render folder

Estrarre geometry, terminal pass, selection, overlays e pane passes da `render.rs`.

Status: 📋 `planned`

**Tasks:** 0/6

### 📋 621e6f47-a0e2-4d0d-9012-e6feadb70097 — P002 — app-06: Sidebar wiring + collapsed rail

Collegare i bottoni della sidebar e migrare il collapsed rail a grid-ui.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 f6a5aa17-dbbf-4463-8ae1-744ab1ba886f — P003 — app-01: Niri layout parity audit

Catalogare gap di comportamento/animazione rispetto a Niri e validare column width reflow.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 9e5ec754-ebf3-47f9-9850-378dd75b8e1c — P004 — app-07: Damage-region render optimization

Ottimizzazione render su scene damage-aware e scissor union.

Status: 📋 `planned`

**Tasks:** 0/5

### 📋 c192c5ca-f6ac-493a-bd8d-d95117c430aa — P005 — app-10: Sidebar/chrome leftovers

Leftovers sidebar/chrome in corso: app-task-29 e app-task-31 sono done; resta app-task-30 column-level pick keycaps.

Status: 📋 `planned`

**Tasks:** 3/6

### 📋 5cab2dbb-ce1b-4a33-b6bc-ede8437fca09 — P006 — app-11: Right-click context menu

Context menu keyboard-navigable con azioni chrome e terminal pane.

Status: 📋 `planned`

**Tasks:** 2/3

### 📋 00afc6e8-101c-47e3-8739-5a8b216c81c4 — P007 — app-04: Pane numbering

Numerazione stabile dei pane visibili per workspace e azione FocusPaneByNumber.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 50c22c08-b639-4a1a-a3cb-2a9f7084c0f9 — P008 — app-05: Workspace drag-to-reorder

Drag-to-reorder delle workspace con DnD e action di riordino.

Status: 📋 `planned`

**Tasks:** 0/4

### 📋 a6831878-0168-4261-935e-818cc9b9a909 — P009 — app-03: App-wide zoom and font-size controls

Azioni e keybinding per zoom globale e controlli font-size UI/terminal.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 884c75d3-e99a-4537-b09c-aa9efab3270c — P010 — app-08: Fix NSWindow vibrancy warning

Silenziare il warning macOS legato alla vibrancy.

Status: 📋 `planned`

**Tasks:** 0/1

### ✅ 3bdc254b-4b5c-4f74-a783-8de4ae1628fa — P011 — app-12: Keyboard move-to-target picks + rename override + plugin-observable state

Completato 2026-06-23: keyboard move-to picks, rename custom-name override e stato osservabile per plugin.

Status: ✅ `done`

**Tasks:** 3/3

### 📋 9cefac90-9fea-427c-9106-9a41e71e8acd — P012 — app-13: Mouse/input leftovers

I due punti ANCORA APERTI della vecchia lista mouse (da redo-mouse.md, file cancellato 2026-07-14): il config focus_follows_mouse_delay_ms (mai implementato) e la delega di handle_swap_param agli helper condivisi (swap cross-workspace).

Status: 📋 `planned`

**Tasks:** 0/2

### ✅ 3fdd622b-1757-40f6-8af0-1767760fa792 — P001 — theming-04: Migrate main app (heca) to heca-theme

Migrare l'app principale al nuovo sistema di temi e rimuovere hardcode residui.

Status: ✅ `done`

**Tasks:** 3/3

### ✅ 06906105-3054-4bda-9751-04902e03aefe — P002 — theming-02: Migrate heca-config to heca-theme

Aggiungere `heca-theme` come dipendenza, rimuovere il codice colore legacy e aggiornare loader/default theme.

Status: ✅ `done`

**Tasks:** 3/3

### ✅ 457bdae8-efba-4665-a795-25120ea154bd — P003 — theming-03: Migrate heca-grid-ui to heca-theme

Comporre `Theme.colors = heca_theme::Theme`, re-export dei token e aggiornamento dei callsite grid-ui.

Status: ✅ `done`

**Tasks:** 2/2

### ✅ 21cc78bb-e0d6-4b57-a20c-a18cb98cd215 — P004 — theming-05: Migrate hand-drawn chrome to grid-ui widgets

Migrare tab bar, status bar, collapsed rail e tree/sidebar verso widget grid-ui.

Status: ✅ `done`

**Tasks:** 4/4

### ✅ 3f91c60a-4a12-47ce-b210-ba1d85cc032a — P005 — theming-01: Showcase visual verification

Verificare che tutti i widget reagiscano al cambio di tema (colori, border, glow, font) e che il tema light (`latte`) renderizzi correttamente.

Status: ✅ `done`

**Tasks:** 2/2

### ❌ 2e48e3d1-0cff-4ead-863f-c36ff1077178 — P001 — compositor-05: Visual tuning

Tentativo di tuning visivo blur/transparency/background esplicitamente non approvato; lasciato come storico ma rifiutato nella forma attuale.

Status: ❌ `rejected`

**Tasks:** 0/4

### ✅ 743fd127-624a-44e4-83e8-370a53cc1beb — P002 — compositor-01-to-04c: z=0 pipeline (merged)

Pipeline z=0 con gradient background, cached blur e integrazione render già merged.

Status: ✅ `done`

**Tasks:** 5/5

### ✅ 1af771d1-e22b-43ce-94d3-b9f228e926db — P003 — compositor-06: Ship review finale

Review finale differita: quality gates e sign-off finale restano fuori dal lavoro attivo attuale.

Status: ✅ `done`

**Tasks:** 1/1

### ✅ 7120375b-cc0c-49e4-8b62-54cb6a35eadc — P001 — terminal-01a-h: Host terminal scrollback viewport

Scrollback host-managed completo: viewport state, selection stabile, actions, wheel routing, chrome mirror e GUI widgets.

Status: ✅ `done`

**Tasks:** 4/4

### ✅ 903725c6-bb78-44a1-a25a-a720cdde68ff — P002 — terminal-01: Dirty-region terminal rendering

Rendering solo delle righe terminali sporche, usando la foundation di retained content.

Status: ✅ `done`

**Tasks:** 1/1

### ✅ 40ca7813-1fa0-4a94-b0e0-cd00f48dca0e — P003 — terminal-02: Ligature policy

Aggiornamento shaping/ligature e toggle configurabile per le ligature nel terminale.

Status: ✅ `done`

**Tasks:** 2/2

### ✅ 486119ff-99c9-44d5-a622-66818ca933cc — P004 — terminal-00: Damage-preservation foundation

Fondazioni per preservare il contenuto terminale non ridisegnato, con damage propagation e retained content.

Status: ✅ `done`

**Tasks:** 2/2

### ✅ 7b69ffc4-bc98-47dd-95ab-1c45eb9b5074 — P005 — terminal-05: Pane-shell hosting contract

Montare il terminale come contenuto dentro la shell pane di grid-ui.

Status: ✅ `done`

**Tasks:** 1/1

### ✅ a3be94c5-7427-499f-82d6-4d91bd4c61dc — P006 — terminal-06: Text selection

Modello condiviso di text selection terminale con overlay host-rendered.

Status: ✅ `done`

**Tasks:** 2/2

### ✅ acb2c4bf-e7bf-4aa1-b41b-d908a35801e3 — P007 — terminal-04: Backend and renderer tests

Test per lifecycle backend PTY e renderer terminale.

Status: ✅ `done`

**Tasks:** 3/3

### ✅ c9038ab9-dcbc-4ec4-99fc-ecd161fc7eb3 — P008 — terminal-09: Image protocols

Supporto inline images completato: Sixel, iTerm2 OSC 1337, Kitty graphics, renderer GPU, Yazi preview, row-range damage, animated GIF/APNG e toggle config.

Status: ✅ `done`

**Tasks:** 9/9

### ✅ b23c166e-142d-41c8-80b0-edad5e83d405 — P009 — terminal-03: Richer terminal protocol hooks

Hook per OSC 8 e punti di aggancio futuri per protocolli grafici/estesi.

Status: ✅ `done`

**Tasks:** 2/2

### ✅ 57862b41-e5d9-4a82-8248-2ce6f8c7579b — P010 — terminal-07: Clipboard and paste

Clipboard copy/paste, paste bracketed e OSC 52.

Status: ✅ `done`

**Tasks:** 3/3

### ✅ 24b9d664-80d0-48c8-ab5d-46d7daa177a8 — P011 — terminal-08: Terminal UX and attention features

Bell/attention, hyperlink open, URL linkify e scrollback search.

Status: ✅ `done`

**Tasks:** 4/4

### ✅ e9b3402f-fb1f-45b7-ab95-c939736a3dab — P012 — terminal-10: Per-pane font zoom

Per-pane font zoom completato e merged: zoom whole-app + per-pane, keybinding, mouse wheel gating e sticky font modes.

Status: ✅ `done`

**Tasks:** 3/3

### ⏸️ ef538743-f412-443d-811b-9e02b66106c1 — P001 — gridui-01: Scroll/list primitive

ScrollRegion widget (gridui-task-01, PR #177) shipped — vertical-only, wheel + draggable thumb, focus-gated keyboard scroll, ensure_visible/scroll_to_child API, showcase + docs. Follow-ups tracked as separate tasks (sidebar scroll wiring overlaps plugin-03; pick-a-region mode small/optional; PageUp/PageDown + nested hit-testing deferred; horizontal scroll + scrollbar token cut).

Status: ⏸️ `deferred`

**Tasks:** 1/7

### 📋 cb8f1203-9ae5-431d-8e7d-07dad2804396 — P002 — gridui-05: Bloom and custom draw effects

Bloom pipeline offscreen e escape hatch `DrawCommand::Custom`.

Status: 📋 `planned`

**Tasks:** 0/2

### 📋 981bc6b0-9529-4a1c-96a3-7c8e34217bc4 — P003 — gridui-02: Pane shell header and tabs

Header slot, tab bar, corner brackets e status bar per la pane shell.

Status: 📋 `planned`

**Tasks:** 0/4

### 📋 ce5d5ea4-0f84-4e5a-909a-77d8344a2a87 — P004 — gridui-04: Showcase coverage and visual regression

Audit dello showcase, demo mancanti e visual regression tests.

Status: 📋 `planned`

**Tasks:** 0/2

### 📋 35d74cc9-a3df-49c9-a5bb-59e12a88b49c — P005 — gridui-06: Additional widgets

Widget aggiuntivi: DnD reorder, multi-select, HUD Frame, Metric Row, Search Input, Accordion.

Status: 📋 `planned`

**Tasks:** 0/6

### 📋 f9a8c93b-6938-4644-88e4-f1d2d2f59d6d — P006 — gridui-07: Crate-review debt

Debito tecnico del crate: docs, allocazioni, test coverage, helper condivisi.

Status: 📋 `planned`

**Tasks:** 0/9

### 📋 36628b50-a0b5-4974-852e-96d68c09f11f — P007 — gridui-03: Nerd-Font icon widget

Widget NfIcon con glyph catalog per programmi e chrome.

Status: 📋 `planned`

**Tasks:** 0/4

### 📋 402d22d4-a658-47d7-8481-fb074ce43a00 — P008 — button-shortcut: Button accelerator / shortcut

Button::shortcut(char) → composed Icon(NfIcon ⌃)+Label(letter) trailing slot inside the button; Ctrl+<c> self-submit + FocusManager::deliver_accelerator. Blocked on viewnode-all-widgets (Button) + gridui-03 NerdFont.

Status: 📋 `planned`
Dependencies: 68a01ad8-1026-4c09-b516-2e2838723903, 36628b50-a0b5-4974-852e-96d68c09f11f

**Tasks:** 0/2

### 📋 09bcb3cf-f8ea-454a-913e-ea377c53d37e — P001 — plugin-08: WASM plugin runtime

Runtime WASM per plugin, event bus, contributi alle regioni e action registration.

Status: 📋 `planned`

**Tasks:** 0/5

### 📋 099e2eb5-30be-4f3c-ab57-a4f111cdc44c — P002 — plugin-06: Placeholder token system

Token `${var}` per pane/column/workspace metadata in chrome e config.

Status: 📋 `planned`

**Tasks:** 0/3

### ✅ 88b87a32-ff00-4044-9a32-c575e3ed5401 — P003 — plugin-04: Dynamic action registry

Dynamic action registry: string action IDs + runtime registration, ACCANTO all'enum WmAction (compatibilità piena). Una sola porta d'ingresso (dispatch per nome), un solo registry runtime (il const ALL viene ritirato), policy DICHIARATA per le azioni name-keyed, keybinding config verso ID dinamici. Sblocca context-menu-5, plugin-05 (app.actions.dispatch) e plugin-08 (WASM).

Status: ✅ `done`

**Tasks:** 3/3

### ✅ 73c320aa-b8b7-4fb3-9421-5c741b1771ff — P004 — plugin-03: Built-in provider + WorkspacesContainer migration

Migrare la sidebar attuale (façade hardcoded in sidebar/model.rs) nel primo provider built-in (WorkspacesContainerProvider) montato via ChromeHost + ContainerContribution, implementando il render seam build_contribution. Prova che l'architettura pluggable chrome ospita un container reale, non solo un TestProvider.

Status: ✅ `done`

**Tasks:** 6/6

### ✅ 9ec31d60-7dfa-4dcc-9797-b265a4d30562 — P005 — plugin-01: Formal architecture contracts

Contratti formali per ChromeHost, provider lifecycle, overlay ownership e audit geometry types.

Status: ✅ `done`

**Tasks:** 4/4

### ✅ 7b50f8c3-bfe6-4e2f-869e-06c1ad9a946c — P006 — plugin-02: ChromeHost and region hosts

Introduzione di ChromeHost e region host per left/right/top/bottom con ordering e persistence.

Status: ✅ `done`

**Tasks:** 4/4

### 📋 f94d6da1-43b0-4b6a-a954-b404164126bd — P007 — plugin-09: Multi-region proof + config integration

Secondo provider built-in, integrazione con keybinding/RPC e discovery delle actions.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 74abdac5-8a35-4e5f-a82f-3abb9bb0b738 — P008 — plugin-05: Host API actions/overlay/region

Host API per actions dispatch, overlay/modal/dropdown e region container management.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 cec2ead0-bd28-4ac6-87f7-e3bcdf799ddc — P009 — plugin-07: Simple config.toml plugins

Plugin semplici definiti in `config.toml` con rendering testuale nelle regioni chrome.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 3e04a8f5-3c51-4f2d-8fab-d51b392e80bc — P010 — action-interaction: Declarative action interaction (confirm + response buttons)

Actions declare as data whether they need a prompt (confirm/choice) and which response buttons + outcomes. One central gate at the dispatch chokepoint reads it and drives OverlayHost::open_modal; built-ins and plugins declare it the same way. Design locked in action-interaction-plan.md (2026-07-07). Prerequisite (central destructive gate maybe_confirm_destructive) already landed; this phase generalizes from 4 hardcoded variants to data.

Status: 📋 `planned`

**Tasks:** 2/3

### 📋 99d19246-1ba9-4720-9895-e70c66acf144 — P011 — plugin-ui: Declarative widget-tree UI model (ViewNode)

The serializable widget tree plugins author, SwiftUI/Flutter-style — a container node holds a vector of child widgets — plus the host mapper that realizes it into the retained heca-grid-ui tree. Gate/consumers: the rich overlay body (plugin-task-15), config-plugin render (plugin-task-21), and the WASM contribution description (plugin-task-26) all build on this. Source: pluggable-chrome-plugin-plan.md §2.6.1–2.6.2.

Status: 📋 `planned`

**Tasks:** 4/9

### 📋 8f760a9b-3ce8-4932-8286-67d742397f2e — P012 — context-menu: Contextual menu → OverlayHost + plugin-declarable

Context menus as a host-owned, context-resolved overlay. Foundation: ContextPath (dotted) + ContextMenuRegistry (built-in providers + plugin Contribution::ContextMenu) + resolve_active_context (keyboard implicit) + unified open_context_menu_for + overlay_origin_mode (mode-restore after close). Keyboard prefix+> works everywhere via prefix arm in SidebarNav; after close the user returns to the origin mode (e.g. stays in sidebar). Mouse right-click refactored to the same unified path (parity preserved, Open-link via target hyperlink). Plugin menus attach by context_path and appear when that context is active/clicked.

Status: 📋 `planned`

**Accepted decisions:**
- **ContextPath dotted + target opaco**
  - Decision: ContextPath è una dotted string (es. "pane", "sidebar.workspace", "docker.container"); target è un dato opaco (pane_id/ws_idx/...). Lookup del registry per path.
  - Rationale: Plugin-friendly (stringa in config/Contribution), type-safe lato host.
  - Implementation: ContextPath: String; target: enum/opaque struct passato al provider build(ctx, target).
  - Accepted at: 2026-07-09T16:00:00Z
- **Binding prefix+> ovunque (non tasto diretto in sidebar)**
  - Decision: prefix+> ovunque via prefix arm in handle_sidebar_nav_mode (speculare a handle_selection_mode). Non tasto diretto in sidebar mode (scelta controversa scartata).
  - Rationale: Stesso binding ovunque, zero sorprese, coerente col pattern selection-mode.
  - Implementation: else if ctx.is_prefix arm: InputMode::Prefix + arm timeout + cattura pending_context(path,target,origin=SidebarNav).
  - Accepted at: 2026-07-09T16:00:00Z
- **Mode-restore via overlay_origin_mode**
  - Decision: overlay_origin_mode: Option<InputMode> catturato all'apertura (se restorable: SidebarNav whitelist; Normal non catturato → no-op) o via origin esplicito (keyboard da mode). Ripristinato quando l'ULTIMO overlay si chiude in resolve(). Stacked overlay: origin settato una volta, non sovrascritto.
  - Rationale: L'utente vuole: apri menu dalla sidebar → chiudi → rimani in sidebar. Modal possiede i tasti mentre aperto (events.rs:125), il mode è irrilevante durante; il restore serve solo alla chiusura.
  - Implementation: Campo su AppState/overlay state; capture in open_context_menu_for; restore in chrome::overlay::resolve() quando layers vuoto.
  - Accepted at: 2026-07-09T16:00:00Z
- **pending_context preserva il target attraverso la dispatch**
  - Decision: Keyboard da sidebar cattura pending_context=(path,target,origin=SidebarNav) nel prefix arm PRIMA di transizionare a Prefix. handle_open_context_menu legge pending_context.take() (override di resolve_active_context); altrimenti resolve_active_context(state) computa da InputMode+focus/selection. Mouse: hit-test produce (path,target) direttamente.
  - Rationale: handle_prefix_mode setta Normal prima della dispatch (input.rs:277), quindi l'handler non può leggere InputMode::SidebarNav. Segnale esplicito disaccoppia dal mode.
  - Implementation: pending_context: Option<(ContextPath, Target, Option<InputMode>)> su AppState.
  - Accepted at: 2026-07-09T16:00:00Z
- **Mouse refattorizzato al path unified (parità + Open link via target)**
  - Decision: ContextMenuRegistry: provider built-in registrati per pane/sidebar.pane/sidebar.column/sidebar.workspace (migrazione di open_context_menu + open_sidebar_context_menu item-build). Mouse right-click refattorizzato a produrre (path,target) e chiamare open_context_menu_for. Parità comportamentale per built-in; 'Open link' (mouse-only) preservato via target che porta Option<hyperlink_uri> (keyboard None → niente voce).
  - Rationale: Unifica mouse+keyboard nel path; nessun regresso visibile per built-in; abilita plugin su right-click.
  - Implementation: open_context_menu(state,pane_id,pos) e open_sidebar_context_menu(state,item,pos) rimpiazzati da open_context_menu_for. PaneTarget { pane_id, hyperlink: Option<String> }.
  - Accepted at: 2026-07-09T16:00:00Z
- **Plugin: passa context_path+build, non mode/origin**
  - Decision: Il plugin dichiara context_path+weight+build(target); NON passa mode/origin (host-internal). L'host include il menu del plugin quando il context è attivo (keyboard resolve_active_context) o cliccato (mouse hit-test). Costruisce su context-menu-5.
  - Rationale: Plugin non decide 'quando'; dichiara 'dove'. L'host triggera.
  - Implementation: Contribution::ContextMenu { context_path, weight: Vec<i64>, build } (C1/C2/C3 già locked in context-menu-5).
  - Accepted at: 2026-07-09T16:00:00Z

**Tasks:** 7/8

### 📋 05c9295a-0ab7-48a0-97a5-c2a5bbf2c5d9 — P013 — menu-nav: Shared list/menu navigation keybindings

Single configurable binding set for list/menu navigation on overlay-layer navigable surfaces: contextual menu, command palette, future top-bar menu. Sidebar is OUT — it has its own configurable [keys.mode] name="sidebar" and does NOT migrate.

Status: 📋 `planned`

**Tasks:** 0/1

### 📋 7a3c634c-5aba-4de1-a01a-5c07f4ee4cb4 — P014 — topbar-menu: Top-bar Menu (menubar) — STUB

STUB. A top-bar Menu/menubar is a separate activity, similar to the contextual menu. Not scoped yet — recorded so the shared infrastructure built for context-menu is made reusable, not forked.

Status: 📋 `planned`

**Tasks:** 0/1

### ✅ 68a01ad8-1026-4c09-b516-2e2838723903 — P015 — viewnode-all-widgets: ViewNode → all widgets (compositional refactor)

TOP PRIORITY standing: every widget's content composed from child Components (the tree realize produces) and ViewNode-realizable; refactor touched widgets toward this.

Status: ✅ `done`

**Tasks:** 3/4

### ✅ 1df9c36c-5c29-4288-9fe1-7a6e9c72179b — P016 — viewnode-choice: Choice primitive + compose the remaining widgets (full ViewNode coverage)

One value-carrying, content-composable Choice primitive; refactor Select/Tabs onto it; then realize arms for every remaining WidgetKind (ItemGroup, MarkerGroup, Grid, DockFrame, Toast) — closing ViewNode coverage.

Status: ✅ `done`

**Tasks:** 9/9

### ⏸️ f5251ab1-db02-4c74-a1f3-97380b5909f5 — P001 — agents-01: Agent status tracking and sounds

Feature differita: dipende dalle fondamenta di Pluggable Chrome (plugin-01 → plugin-05) prima di poter iniziare l’implementazione.

Status: ⏸️ `deferred`

**Tasks:** 0/4

### 📋 93379c9a-43b7-43eb-8988-be505251fe79 — P001 — notification-02: Store + Lifecycle

Store app-owned, queue, dedup, expiry e scheduling timer.

Status: 📋 `planned`

**Tasks:** 0/9

### 📋 03b297bb-a915-4712-a6b4-dc688dc7ddc5 — P002 — notification-06: Routing + Additional Producers

Choke point `notify`, routing per app/system/none e producer aggiuntivi.

Status: 📋 `planned`

**Tasks:** 0/7

### 📋 e8d54d82-7357-4436-9b81-22cf2eaef545 — P003 — notification-05: Reload Config Producer

Primo producer reale: reload config success/failure con Retry.

Status: 📋 `planned`

**Tasks:** 0/5

### 📋 ccefbac4-b29e-45d8-bd1d-e7199afa4907 — P004 — notification-04: Toast Action Dispatch

Azioni toast dispatchate tramite ActionRegistry/policy path.

Status: 📋 `planned`

**Tasks:** 0/5

### 📋 e5ceaa1e-9b7a-4d44-b155-edb7fe4b7ef2 — P005 — notification-01: Model + Config

Tipi core delle notifiche, mapping ToastSpec e schema config `notification_system`.

Status: 📋 `planned`

**Tasks:** 0/10

### 📋 e298f926-f6b3-4147-abcc-eea87171cd81 — P006 — notification-03: In-App Toast Rendering

Montare ToastStack nel chrome retained tree usando segnali app-owned.

Status: 📋 `planned`

**Tasks:** 0/6

### 📋 461b88ca-5faa-4bea-8734-6c98587f115d — P007 — notification-09: Tests + Documentation

Copertura test e documentazione utente/planner.

Status: 📋 `planned`

**Tasks:** 0/8

### 📋 941d82d3-2770-4e6f-b42f-76b36e160c37 — P008 — notification-07: OS/System Notification Backend

Backend OS notifications differito.

Status: 📋 `planned`

**Tasks:** 0/5

### 📋 949d48a4-6001-42b2-a12c-25ba69a95c24 — P009 — notification-08: Toast Keyboard Hint Integration

Integrazione differita con global prefix+/ hint system.

Status: 📋 `planned`

**Tasks:** 0/4
