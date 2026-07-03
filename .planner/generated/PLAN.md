# Heca — Project Plan

> Consolidare Heca come workspace compositor nativo, keyboard-first e pluggable, completando piattaforma terminale, architettura chrome/plugin, libreria grid-ui, integrazione Neovim GUI e rifiniture app/chrome.

Compositore di workspace GPU-native per sviluppatori, ispirato al layout a colonne scrollabili di Niri, che unifica terminali, editor e strumenti in una singola finestra accelerata via GPU. Think tmux meets Niri meets Neovide: terminali, editor e futuri container/plugin convivono nello stesso frame con animazioni fluide, testo nitido e chrome renderizzato via GPU.

**Last updated:** 2026-07-03T09:58:41.469Z
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

### ✅ 8df6dded-5cfb-42a1-8da1-724f3d36b2b4 — 🎨 Theme System

Migrazione completa a `heca-theme`: crate standalone, temi bundled (`grid_tron`, `mocha`, `latte`), migrazione di `heca-config`, `heca-grid-ui` e app principale, rimozione dei colori hardcoded e migrazione del chrome hand-drawn verso widget grid-ui. Completata.

Status: ✅ `done`

**Phases:**
- ✅ **06906105-3054-4bda-9751-04902e03aefe** theming-02: Migrate heca-config to heca-theme (3/3 tasks)
- ✅ **21cc78bb-e0d6-4b57-a20c-a18cb98cd215** theming-05: Migrate hand-drawn chrome to grid-ui widgets (4/4 tasks)
- ✅ **3f91c60a-4a12-47ce-b210-ba1d85cc032a** theming-01: Showcase visual verification (2/2 tasks)
- ✅ **3fdd622b-1757-40f6-8af0-1767760fa792** theming-04: Migrate main app (heca) to heca-theme (3/3 tasks)
- ✅ **457bdae8-efba-4665-a795-25120ea154bd** theming-03: Migrate heca-grid-ui to heca-theme (2/2 tasks)

### ✅ be986d1d-cc2c-4acc-b703-4b1788a4ff86 — 🖥️ Terminal Platform Completion

Completamento della piattaforma terminale: damage preservation, dirty-region rendering, viewport scrollback host-managed, policy ligature, hook protocolli terminali, test backend/renderer, pane-shell hosting contract, text selection, clipboard/paste/OSC 52, UX terminale, image protocols e per-pane font zoom.

Status: ✅ `done`

**Phases:**
- ✅ **24b9d664-80d0-48c8-ab5d-46d7daa177a8** terminal-08: Terminal UX and attention features (4/4 tasks)
- ✅ **40ca7813-1fa0-4a94-b0e0-cd00f48dca0e** terminal-02: Ligature policy (2/2 tasks)
- ✅ **486119ff-99c9-44d5-a622-66818ca933cc** terminal-00: Damage-preservation foundation (2/2 tasks)
- ✅ **57862b41-e5d9-4a82-8248-2ce6f8c7579b** terminal-07: Clipboard and paste (3/3 tasks)
- ✅ **7120375b-cc0c-49e4-8b62-54cb6a35eadc** terminal-01a-h: Host terminal scrollback viewport (4/4 tasks)
- ✅ **7b69ffc4-bc98-47dd-95ab-1c45eb9b5074** terminal-05: Pane-shell hosting contract (1/1 tasks)
- ✅ **903725c6-bb78-44a1-a25a-a720cdde68ff** terminal-01: Dirty-region terminal rendering (1/1 tasks)
- ✅ **a3be94c5-7427-499f-82d6-4d91bd4c61dc** terminal-06: Text selection (2/2 tasks)
- ✅ **acb2c4bf-e7bf-4aa1-b41b-d908a35801e3** terminal-04: Backend and renderer tests (3/3 tasks)
- ✅ **b23c166e-142d-41c8-80b0-edad5e83d405** terminal-03: Richer terminal protocol hooks (2/2 tasks)
- ✅ **c9038ab9-dcbc-4ec4-99fc-ecd161fc7eb3** terminal-09: Image protocols (9/9 tasks)
- ✅ **e9b3402f-fb1f-45b7-ab95-c939736a3dab** terminal-10: Per-pane font zoom (3/3 tasks)

**Work done:** Piattaforma terminale completata: damage preservation, dirty-region rendering, scrollback host-managed, ligature policy, hook protocolli terminali, test backend/renderer, pane-shell hosting contract, text selection, clipboard/paste/OSC 52, UX terminale, image protocols e per-pane/whole-app font zoom. Il backlog su origin/main chiude anche la validazione manuale runtime (`terminal-task-07`) il 2026-07-02.

**Work remaining:** Nessun lavoro terminale aperto nel backlog principale. Restano al più verifiche osservative/non bloccanti già annotate nel backlog, ma il track è considerato completato.

### 📋 ebb9ceb0-ea12-4374-af6e-12aa4256bcc3 — 🧩 Pluggable Chrome Architecture

Architettura chrome pluggable: `ChromeHost` con regioni left/right/top/bottom, provider built-in, migrazione workspace tree in `WorkspacesContainerProvider`, dynamic action registry, host API per actions/overlay/regions, placeholder token system, plugin semplici da config e runtime WASM.

Status: 📋 `planned`

**Phases:**
- 📋 **099e2eb5-30be-4f3c-ab57-a4f111cdc44c** plugin-06: Placeholder token system (0/3 tasks)
- 📋 **09bcb3cf-f8ea-454a-913e-ea377c53d37e** plugin-08: WASM plugin runtime (0/5 tasks)
- 📋 **73c320aa-b8b7-4fb3-9421-5c741b1771ff** plugin-03: Built-in provider + WorkspacesContainer migration (0/3 tasks)
- 📋 **74abdac5-8a35-4e5f-a82f-3abb9bb0b738** plugin-05: Host API actions/overlay/region (0/3 tasks)
- 📋 **7b50f8c3-bfe6-4e2f-869e-06c1ad9a946c** plugin-02: ChromeHost and region hosts (0/4 tasks)
- 📋 **88b87a32-ff00-4044-9a32-c575e3ed5401** plugin-04: Dynamic action registry (0/3 tasks)
- 📋 **9ec31d60-7dfa-4dcc-9797-b265a4d30562** plugin-01: Formal architecture contracts (0/4 tasks)
- 📋 **cec2ead0-bd28-4ac6-87f7-e3bcdf799ddc** plugin-07: Simple config.toml plugins (0/3 tasks)
- 📋 **f94d6da1-43b0-4b6a-a954-b404164126bd** plugin-09: Multi-region proof + config integration (0/3 tasks)

### 📋 cd083ad1-8310-4368-981b-d14c73c20d96 — 📐 Grid-UI Widget Library

Espansione della libreria UI GPU-free e signal-driven: scroll/list primitive, pane shell header/tabs, icon widget Nerd Font, showcase coverage con visual regression, bloom/custom draw effects, widget aggiuntivi e cleanup del crate.

Status: 📋 `planned`

**Phases:**
- 📋 **35d74cc9-a3df-49c9-a5bb-59e12a88b49c** gridui-06: Additional widgets (0/6 tasks)
- 📋 **36628b50-a0b5-4974-852e-96d68c09f11f** gridui-03: Nerd-Font icon widget (0/4 tasks)
- 📋 **981bc6b0-9529-4a1c-96a3-7c8e34217bc4** gridui-02: Pane shell header and tabs (0/4 tasks)
- 📋 **cb8f1203-9ae5-431d-8e7d-07dad2804396** gridui-05: Bloom and custom draw effects (0/2 tasks)
- 📋 **ce5d5ea4-0f84-4e5a-909a-77d8344a2a87** gridui-04: Showcase coverage and visual regression (0/2 tasks)
- 📋 **ef538743-f412-443d-811b-9e02b66106c1** gridui-01: Scroll/list primitive (1/7 tasks)
- 📋 **f9a8c93b-6938-4644-88e4-f1d2d2f59d6d** gridui-07: Crate-review debt (0/8 tasks)

### ⏸️ b5d04826-7fd8-4094-ace5-96dc93b825a0 — 🌫️ Compositor Frost

Lavoro sulla pipeline z=0 per fondo frosted/blurred: gradient background, cached blur, integrazione pipeline, tuning visivo e ship review finale. La pipeline base è merged, restano tuning e review finale.

Status: ⏸️ `deferred`

**Phases:**
- ⏸️ **1af771d1-e22b-43ce-94d3-b9f228e926db** compositor-06: Ship review finale (0/1 tasks)
- ❌ **2e48e3d1-0cff-4ead-863f-c36ff1077178** compositor-05: Visual tuning (0/4 tasks)
- ✅ **743fd127-624a-44e4-83e8-370a53cc1beb** compositor-01-to-04c: z=0 pipeline (merged) (5/5 tasks)

**Work remaining:** La pipeline base è merged. Il tentativo di visual tuning (compositor-05) è stato rifiutato; la review finale/compositor-06 è differita e non è lavoro attivo in questo momento.

### 📋 2809a7a4-6d53-4a58-b776-bac164bb5d03 — 🛠️ App / Chrome Features

Feature applicative e di chrome: audit di parità con Niri, split di `render.rs`, zoom/font-size controls, pane numbering, workspace drag-to-reorder, sidebar wiring + collapsed rail, damage-region optimization, fix vibrancy warning macOS, leftovers sidebar/chrome e context menu.

Status: 📋 `planned`

**Phases:**
- 📋 **00afc6e8-101c-47e3-8739-5a8b216c81c4** app-04: Pane numbering (0/3 tasks)
- ✅ **3bdc254b-4b5c-4f74-a783-8de4ae1628fa** app-12: Keyboard move-to-target picks + rename override + plugin-observable state (3/3 tasks)
- 📋 **50c22c08-b639-4a1a-a3cb-2a9f7084c0f9** app-05: Workspace drag-to-reorder (0/4 tasks)
- 📋 **5cab2dbb-ce1b-4a33-b6bc-ede8437fca09** app-11: Right-click context menu (0/3 tasks)
- 📋 **621e6f47-a0e2-4d0d-9012-e6feadb70097** app-06: Sidebar wiring + collapsed rail (0/2 tasks)
- 📋 **884c75d3-e99a-4537-b09c-aa9efab3270c** app-08: Fix NSWindow vibrancy warning (0/1 tasks)
- 📋 **9e5ec754-ebf3-47f9-9850-378dd75b8e1c** app-07: Damage-region render optimization (0/5 tasks)
- 📋 **a6831878-0168-4261-935e-818cc9b9a909** app-03: App-wide zoom and font-size controls (0/3 tasks)
- 📋 **c192c5ca-f6ac-493a-bd8d-d95117c430aa** app-10: Sidebar/chrome leftovers (2/3 tasks)
- 📋 **f6476b06-0938-4fa1-88ab-23e0283036fa** app-02: Split render.rs into render folder (0/6 tasks)
- 📋 **f6a5aa17-dbbf-4463-8ae1-744ab1ba886f** app-01: Niri layout parity audit (0/3 tasks)

### 📋 0c704076-6072-4c57-8ee1-9fc11f6eaa3a — 🖋️ Neovim GUI Pane

Integrazione Neovim GUI come pane: `nvim --embed`, msgpack-RPC, render singola grid, multi-istanza, `ext_multigrid`, input routing, image layer dedicato e markdown layer. Feature ancora in fase di idea/progettazione.

Status: 📋 `planned`

**Phases:**
- 📋 **0d4b8645-a638-4774-a9e1-ed8edb37e53f** nvim-01: Embed nvim + RPC + render single grid (0/3 tasks)
- 📋 **11d9a9ac-c22d-4613-9423-7f97a64acbcd** nvim-02: Editor-pane integration (0/3 tasks)
- 📋 **14f9486d-6e6a-4dc3-9315-ca553dfe8721** nvim-03: ext_multigrid (0/4 tasks)
- 📋 **87939acb-c177-4c6b-b4d7-2df14c25390d** nvim-06: Markdown layer (0/2 tasks)
- 📋 **aa6c3a49-f3a4-4330-876d-4156745e1ade** nvim-05: Image layer (0/3 tasks)
- 📋 **dcf766af-eb11-4a78-9dd3-1e1e15663c33** nvim-04: Input routing (0/3 tasks)

### ⏸️ eef184fb-863e-476f-8839-0dff37443acf — 🤖 AI Agent Integration

Integrazione agenti AI: `AgentDriver` trait e registry, built-in drivers (Claude/Codex/pi), status agent nei pane e suoni di transizione. Feature parcheggiata e dipendente dalle fasi iniziali di Pluggable Chrome.

Status: ⏸️ `deferred`

**Phases:**
- ⏸️ **f5251ab1-db02-4c74-a1f3-97380b5909f5** agents-01: Agent status tracking and sounds (0/4 tasks)

**Work remaining:** Feature pianificata ma differita: l’implementazione dipende dal completamento delle fondamenta di Pluggable Chrome, in particolare plugin-01 → plugin-05.

---
## Requirements

_No requirements defined yet._

---
## Phases

### 📋 00afc6e8-101c-47e3-8739-5a8b216c81c4 — app-04: Pane numbering

Numerazione stabile dei pane visibili per workspace e azione FocusPaneByNumber.

Status: 📋 `planned`

**Tasks:** 0/3

### ✅ 06906105-3054-4bda-9751-04902e03aefe — theming-02: Migrate heca-config to heca-theme

Aggiungere `heca-theme` come dipendenza, rimuovere il codice colore legacy e aggiornare loader/default theme.

Status: ✅ `done`

**Tasks:** 3/3

### 📋 099e2eb5-30be-4f3c-ab57-a4f111cdc44c — plugin-06: Placeholder token system

Token `${var}` per pane/column/workspace metadata in chrome e config.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 09bcb3cf-f8ea-454a-913e-ea377c53d37e — plugin-08: WASM plugin runtime

Runtime WASM per plugin, event bus, contributi alle regioni e action registration.

Status: 📋 `planned`

**Tasks:** 0/5

### 📋 0d4b8645-a638-4774-a9e1-ed8edb37e53f — nvim-01: Embed nvim + RPC + render single grid

Spawn `nvim --embed`, msgpack-RPC, `nvim_ui_attach` e render singola grid.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 11d9a9ac-c22d-4613-9423-7f97a64acbcd — nvim-02: Editor-pane integration

Multi-istanza, lifecycle dei pane, focus/input forwarding.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 14f9486d-6e6a-4dc3-9315-ca553dfe8721 — nvim-03: ext_multigrid

Supporto multi-grid/window con geometry, viewport e z-ordering.

Status: 📋 `planned`

**Tasks:** 0/4

### ⏸️ 1af771d1-e22b-43ce-94d3-b9f228e926db — compositor-06: Ship review finale

Review finale differita: quality gates e sign-off finale restano fuori dal lavoro attivo attuale.

Status: ⏸️ `deferred`

**Tasks:** 0/1

### ✅ 21cc78bb-e0d6-4b57-a20c-a18cb98cd215 — theming-05: Migrate hand-drawn chrome to grid-ui widgets

Migrare tab bar, status bar, collapsed rail e tree/sidebar verso widget grid-ui.

Status: ✅ `done`

**Tasks:** 4/4

### ✅ 24b9d664-80d0-48c8-ab5d-46d7daa177a8 — terminal-08: Terminal UX and attention features

Bell/attention, hyperlink open, URL linkify e scrollback search.

Status: ✅ `done`

**Tasks:** 4/4

### ❌ 2e48e3d1-0cff-4ead-863f-c36ff1077178 — compositor-05: Visual tuning

Tentativo di tuning visivo blur/transparency/background esplicitamente non approvato; lasciato come storico ma rifiutato nella forma attuale.

Status: ❌ `rejected`

**Tasks:** 0/4

### 📋 35d74cc9-a3df-49c9-a5bb-59e12a88b49c — gridui-06: Additional widgets

Widget aggiuntivi: DnD reorder, multi-select, HUD Frame, Metric Row, Search Input, Accordion.

Status: 📋 `planned`

**Tasks:** 0/6

### 📋 36628b50-a0b5-4974-852e-96d68c09f11f — gridui-03: Nerd-Font icon widget

Widget NfIcon con glyph catalog per programmi e chrome.

Status: 📋 `planned`

**Tasks:** 0/4

### ✅ 3bdc254b-4b5c-4f74-a783-8de4ae1628fa — app-12: Keyboard move-to-target picks + rename override + plugin-observable state

Completato 2026-06-23: keyboard move-to picks, rename custom-name override e stato osservabile per plugin.

Status: ✅ `done`

**Tasks:** 3/3

### ✅ 3f91c60a-4a12-47ce-b210-ba1d85cc032a — theming-01: Showcase visual verification

Verificare che tutti i widget reagiscano al cambio di tema (colori, border, glow, font) e che il tema light (`latte`) renderizzi correttamente.

Status: ✅ `done`

**Tasks:** 2/2

### ✅ 3fdd622b-1757-40f6-8af0-1767760fa792 — theming-04: Migrate main app (heca) to heca-theme

Migrare l'app principale al nuovo sistema di temi e rimuovere hardcode residui.

Status: ✅ `done`

**Tasks:** 3/3

### ✅ 40ca7813-1fa0-4a94-b0e0-cd00f48dca0e — terminal-02: Ligature policy

Aggiornamento shaping/ligature e toggle configurabile per le ligature nel terminale.

Status: ✅ `done`

**Tasks:** 2/2

### ✅ 457bdae8-efba-4665-a795-25120ea154bd — theming-03: Migrate heca-grid-ui to heca-theme

Comporre `Theme.colors = heca_theme::Theme`, re-export dei token e aggiornamento dei callsite grid-ui.

Status: ✅ `done`

**Tasks:** 2/2

### ✅ 486119ff-99c9-44d5-a622-66818ca933cc — terminal-00: Damage-preservation foundation

Fondazioni per preservare il contenuto terminale non ridisegnato, con damage propagation e retained content.

Status: ✅ `done`

**Tasks:** 2/2

### 📋 50c22c08-b639-4a1a-a3cb-2a9f7084c0f9 — app-05: Workspace drag-to-reorder

Drag-to-reorder delle workspace con DnD e action di riordino.

Status: 📋 `planned`

**Tasks:** 0/4

### ✅ 57862b41-e5d9-4a82-8248-2ce6f8c7579b — terminal-07: Clipboard and paste

Clipboard copy/paste, paste bracketed e OSC 52.

Status: ✅ `done`

**Tasks:** 3/3

### 📋 5cab2dbb-ce1b-4a33-b6bc-ede8437fca09 — app-11: Right-click context menu

Context menu keyboard-navigable con azioni chrome e terminal pane.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 621e6f47-a0e2-4d0d-9012-e6feadb70097 — app-06: Sidebar wiring + collapsed rail

Collegare i bottoni della sidebar e migrare il collapsed rail a grid-ui.

Status: 📋 `planned`

**Tasks:** 0/2

### ✅ 7120375b-cc0c-49e4-8b62-54cb6a35eadc — terminal-01a-h: Host terminal scrollback viewport

Scrollback host-managed completo: viewport state, selection stabile, actions, wheel routing, chrome mirror e GUI widgets.

Status: ✅ `done`

**Tasks:** 4/4

### 📋 73c320aa-b8b7-4fb3-9421-5c741b1771ff — plugin-03: Built-in provider + WorkspacesContainer migration

Provider trait e migrazione del workspace tree in WorkspacesContainerProvider.

Status: 📋 `planned`

**Tasks:** 0/3

### ✅ 743fd127-624a-44e4-83e8-370a53cc1beb — compositor-01-to-04c: z=0 pipeline (merged)

Pipeline z=0 con gradient background, cached blur e integrazione render già merged.

Status: ✅ `done`

**Tasks:** 5/5

### 📋 74abdac5-8a35-4e5f-a82f-3abb9bb0b738 — plugin-05: Host API actions/overlay/region

Host API per actions dispatch, overlay/modal/dropdown e region container management.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 7b50f8c3-bfe6-4e2f-869e-06c1ad9a946c — plugin-02: ChromeHost and region hosts

Introduzione di ChromeHost e region host per left/right/top/bottom con ordering e persistence.

Status: 📋 `planned`

**Tasks:** 0/4

### ✅ 7b69ffc4-bc98-47dd-95ab-1c45eb9b5074 — terminal-05: Pane-shell hosting contract

Montare il terminale come contenuto dentro la shell pane di grid-ui.

Status: ✅ `done`

**Tasks:** 1/1

### 📋 87939acb-c177-4c6b-b4d7-2df14c25390d — nvim-06: Markdown layer

Lettura buffer via RPC e preview markdown nativa/rich.

Status: 📋 `planned`

**Tasks:** 0/2

### 📋 884c75d3-e99a-4537-b09c-aa9efab3270c — app-08: Fix NSWindow vibrancy warning

Silenziare il warning macOS legato alla vibrancy.

Status: 📋 `planned`

**Tasks:** 0/1

### 📋 88b87a32-ff00-4044-9a32-c575e3ed5401 — plugin-04: Dynamic action registry

Dynamic action registry con string action IDs e binding configurabili.

Status: 📋 `planned`

**Tasks:** 0/3

### ✅ 903725c6-bb78-44a1-a25a-a720cdde68ff — terminal-01: Dirty-region terminal rendering

Rendering solo delle righe terminali sporche, usando la foundation di retained content.

Status: ✅ `done`

**Tasks:** 1/1

### 📋 981bc6b0-9529-4a1c-96a3-7c8e34217bc4 — gridui-02: Pane shell header and tabs

Header slot, tab bar, corner brackets e status bar per la pane shell.

Status: 📋 `planned`

**Tasks:** 0/4

### 📋 9e5ec754-ebf3-47f9-9850-378dd75b8e1c — app-07: Damage-region render optimization

Ottimizzazione render su scene damage-aware e scissor union.

Status: 📋 `planned`

**Tasks:** 0/5

### 📋 9ec31d60-7dfa-4dcc-9797-b265a4d30562 — plugin-01: Formal architecture contracts

Contratti formali per ChromeHost, provider lifecycle, overlay ownership e audit geometry types.

Status: 📋 `planned`

**Tasks:** 0/4

### ✅ a3be94c5-7427-499f-82d6-4d91bd4c61dc — terminal-06: Text selection

Modello condiviso di text selection terminale con overlay host-rendered.

Status: ✅ `done`

**Tasks:** 2/2

### 📋 a6831878-0168-4261-935e-818cc9b9a909 — app-03: App-wide zoom and font-size controls

Azioni e keybinding per zoom globale e controlli font-size UI/terminal.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 aa6c3a49-f3a4-4330-876d-4156745e1ade — nvim-05: Image layer

Canale RPC dedicato per immagini con riuso di ImageRenderer.

Status: 📋 `planned`

**Tasks:** 0/3

### ✅ acb2c4bf-e7bf-4aa1-b41b-d908a35801e3 — terminal-04: Backend and renderer tests

Test per lifecycle backend PTY e renderer terminale.

Status: ✅ `done`

**Tasks:** 3/3

### ✅ b23c166e-142d-41c8-80b0-edad5e83d405 — terminal-03: Richer terminal protocol hooks

Hook per OSC 8 e punti di aggancio futuri per protocolli grafici/estesi.

Status: ✅ `done`

**Tasks:** 2/2

### 📋 c192c5ca-f6ac-493a-bd8d-d95117c430aa — app-10: Sidebar/chrome leftovers

Leftovers sidebar/chrome in corso: app-task-29 e app-task-31 sono done; resta app-task-30 column-level pick keycaps.

Status: 📋 `planned`

**Tasks:** 2/3

### ✅ c9038ab9-dcbc-4ec4-99fc-ecd161fc7eb3 — terminal-09: Image protocols

Supporto inline images completato: Sixel, iTerm2 OSC 1337, Kitty graphics, renderer GPU, Yazi preview, row-range damage, animated GIF/APNG e toggle config.

Status: ✅ `done`

**Tasks:** 9/9

### 📋 cb8f1203-9ae5-431d-8e7d-07dad2804396 — gridui-05: Bloom and custom draw effects

Bloom pipeline offscreen e escape hatch `DrawCommand::Custom`.

Status: 📋 `planned`

**Tasks:** 0/2

### 📋 ce5d5ea4-0f84-4e5a-909a-77d8344a2a87 — gridui-04: Showcase coverage and visual regression

Audit dello showcase, demo mancanti e visual regression tests.

Status: 📋 `planned`

**Tasks:** 0/2

### 📋 cec2ead0-bd28-4ac6-87f7-e3bcdf799ddc — plugin-07: Simple config.toml plugins

Plugin semplici definiti in `config.toml` con rendering testuale nelle regioni chrome.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 dcf766af-eb11-4a78-9dd3-1e1e15663c33 — nvim-04: Input routing

Encoding tasti/modifier, mouse forwarding e convivenza col prefix mode.

Status: 📋 `planned`

**Tasks:** 0/3

### ✅ e9b3402f-fb1f-45b7-ab95-c939736a3dab — terminal-10: Per-pane font zoom

Per-pane font zoom completato e merged: zoom whole-app + per-pane, keybinding, mouse wheel gating e sticky font modes.

Status: ✅ `done`

**Tasks:** 3/3

### 📋 ef538743-f412-443d-811b-9e02b66106c1 — gridui-01: Scroll/list primitive

ScrollRegion, ensure_visible, bounds-shift e follow-up su scrolling/nested hit-testing.

Status: 📋 `planned`

**Tasks:** 1/7

### ⏸️ f5251ab1-db02-4c74-a1f3-97380b5909f5 — agents-01: Agent status tracking and sounds

Feature differita: dipende dalle fondamenta di Pluggable Chrome (plugin-01 → plugin-05) prima di poter iniziare l’implementazione.

Status: ⏸️ `deferred`

**Tasks:** 0/4

### 📋 f6476b06-0938-4fa1-88ab-23e0283036fa — app-02: Split render.rs into render folder

Estrarre geometry, terminal pass, selection, overlays e pane passes da `render.rs`.

Status: 📋 `planned`

**Tasks:** 0/6

### 📋 f6a5aa17-dbbf-4463-8ae1-744ab1ba886f — app-01: Niri layout parity audit

Catalogare gap di comportamento/animazione rispetto a Niri e validare column width reflow.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 f94d6da1-43b0-4b6a-a954-b404164126bd — plugin-09: Multi-region proof + config integration

Secondo provider built-in, integrazione con keybinding/RPC e discovery delle actions.

Status: 📋 `planned`

**Tasks:** 0/3

### 📋 f9a8c93b-6938-4644-88e4-f1d2d2f59d6d — gridui-07: Crate-review debt

Debito tecnico del crate: docs, allocazioni, test coverage, helper condivisi.

Status: 📋 `planned`

**Tasks:** 0/8
