# e5ceaa1e-9b7a-4d44-b155-edb7fe4b7ef2 — P061 — notification-01: Domain Model + Config

**Status:** 🚧 `in-progress`
**Created:** 2026-07-03T22:50:20.126Z
**Updated:** 2026-08-07T11:13:35.326Z

Definire modello app-owned, azioni Intent e configurazione app/system/none.

Definire il dominio notifiche nell'app e la configurazione in `heca-config/src/settings.rs` / default embedded. `NotificationAction` conserva un `heca_view::Intent` name-keyed con argomenti, coerente con la porta universale `dispatch_view_intent` in `heca/src/app/interaction.rs:956`; non conserva closure né soltanto `WmAction`. Il modello canonico include ID runtime monotono, dedup key opzionale, sorgente estensibile, gravità, titolo/corpo, lifecycle, azione opzionale, dismissibility e metadati temporali testabili tramite `Instant` passato dal chiamante. `NotificationSystem { App, System, None }` usa serde snake_case e default `App`. La proiezione verso `ToastSpec` deve restare un adattatore sottile e non trasferire responsabilità di queue/timer a grid-ui.

## Goals
- Un solo modello canonico app-owned e testabile.
- Azioni dynamic-friendly tramite Intent action+args.
- Configurazione deterministica con default app.
- Adapter UI che non duplica lifecycle o policy.

## Non-goals
- Implementare lo store o il livello overlay.
- Backend notifiche del sistema operativo.
- Confirmation-before-action.

## Dependencies
- P058(F009) definisce il contratto ToastSpec/target effettivo da consumare.

## Risks
- Accoppiare heca-grid-ui al dominio Heca; mantenere l'adattatore nell'app.
- Usare indici workspace/pane non più validi nella cronologia; le sorgenti sono descrittive, non riferimenti mutabili obbligatori.

## Completion Criteria
- Tipi e builder documentati e coperti da test.
- Config default e parsing per app/system/none.
- Nessuna closure o WmAction-only nel modello azione.
- Conversione verso ToastSpec completa e domain-neutral.

## Tasks

### ✅ 208ad51d-2f42-436e-83c7-69af2c350b6d — T184 — Define extensible notification source model

Status: ✅ `done`

Definire nel modulo notifiche app-owned una sorgente descrittiva ed estensibile per log, cronologia e dedup, senza inserire concetti in `heca-grid-ui`. Supportare almeno App/Config/Pane/Provider/Plugin/Command con identificatore opaco opzionale; non aggiungere Agent perché appartiene a un'altra feature. Evitare riferimenti mutabili obbligatori a indici workspace/pane che possono diventare invalidi dopo l'archiviazione. Documentare che la source non decide routing o ActionPolicy.

---
**Completion summary:**
Added `heca/src/notification.rs` (new app module, registered in `heca/src/main.rs:10`) with:

- `NotificationSourceKind` enum (serde snake_case): `App | Config | Pane | Provider | Plugin | Command`. Deliberately **no `Agent`** variant (belongs to another feature); a compile-time exhaustive-match guard test (`no_agent_variant_exists`) fails to compile if one is added. `Copy + Default(App)` + `as_label()` stable lowercase label. The opaque-id `Option<String>` on `NotificationSource` is the "Other/custom" extension point (plugin id, command name, pane handle-as-string).
- `NotificationSource` struct (Debug/Clone/PartialEq/Eq/Hash/Serialize/Deserialize): `kind` + opaque `Option<String>` id. Constructors `new`/`of`/`app`/`default`; getters `kind()`/`id()`; `dedup_segment()` -> `<kind>[:<id>]` for log/history/dedup. No mutable refs to workspace/pane indices (opaque strings only, per task constraint).

Contract honored: descriptive only — no routing, no `ActionPolicy`, no closures, no bare `WmAction`, no UI, no `heca_grid_ui` dependency (only `serde`/`std`). Module doc explicitly contrasts `NotificationSource` with `InteractionSource` (input provenance vs. routing classifier). `ToastSpec` projection stays in T193.

Tests: 10 unit tests covering kind labels, no-Agent compile guard, constructors, default, dedup segment with/without id, serde snake_case round-trip, Eq+Hash stability — i.e. sources with and without identifiers. `cargo test -p heca notification::` green.

clippy: workspace clean (`--all-targets --all-features`) after a documented, module-scoped `#![allow(dead_code)]` for the staged phase (consumers land in T185-T188; drop the allow then). No other warnings beyond the known upstream `block v0.1.6` future-incompat.

Files: heca/src/notification.rs (new), heca/src/main.rs (+1 line `mod notification;`).

Next: T185 (Define lifecycle semantics and defaults).

**Checklist:**
- [ ] Definire tipo ed eventuale Other/custom.
- [ ] Evitare dipendenze widget.
- [ ] Coprire serializzazione/debug se necessaria.
- [ ] Testare sorgenti con e senza identificatore.
- [ ] Documentare differenza da InteractionSource.

### ✅ 2167fa5e-750b-42b1-be11-1658c13913b6 — T185 — Define lifecycle semantics and defaults

Status: ✅ `done`

Definire lifecycle app-owned per notifica: durata automatica, sticky e dismissibility. La durata non produce `expires_at` al push: il timer parte quando lo store promuove la notifica a visibile. Stabilire default documentati per severity/lifecycle e mantenere gli istanti fuori dal builder di dominio quando servono test deterministici. Le notifiche sticky non scadono ma possono essere chiuse se dismissible.

---
**Completion summary:**
Extended `heca/src/notification.rs` with lifecycle semantics (app-owned, no UI, no store/timer logic):

- `NotificationLifetime { Auto(Duration) | Sticky }` — serde snake_case, `Default` = Auto 5s. Carries NO `Instant`/`expires_at` (computed lazily by the store on promotion, per T185 contract).
- `NotificationLifecycle { lifetime, dismissible }` — struct with builders `new`/`auto`/`sticky`/`sticky_persistent` + setter `dismissible(bool)`; getters `lifetime()`/`is_dismissible()`/`expires()`. Sticky+dismissible = stays until closed; sticky+non-dismissible = stays until replaced; auto+non-dismissible = expires on its own, cannot be closed early. `Default` = dismissible Auto 5s.
- `default_lifetime_for_severity(NotificationSeverity) -> NotificationLifetime`: Error/Warning → Sticky, Success → Auto 4s, Info → Auto 5s. Domain semantics, not user-tunable (kept out of config).
- `NotificationSeverity { Error | Warning | Success | Info }` with `#[default] Info` — placeholder mirroring `ToastSeverity`'s variants so T190 is a rename/re-export, not a redesign.

Contract honored: no Instant in the domain builder (deterministic tests), timer starts on promote not at push, sticky doesn't expire but is closeable when dismissible, no closures, no WmAction, no UI, no grid-ui dependency.

Tests: 8 new (total 18 in module) — default, sticky dismissible/non-dismissible, dismissible toggle, expires semantics, no-Instant-in-public-API assertion, severity→lifetime mapping, serde snake_case round-trip. `cargo test -p heca notification::` green.

clippy: workspace clean after replacing manual `impl Default for NotificationSeverity` with `#[derive(Default)]` + `#[default]` on `Info` (clippy suggestion). `#![allow(dead_code)]` still covers the staged-phase public API until T188 wires `AppNotification`.

Files: heca/src/notification.rs (extended). No other files touched.

Next: T186 (Add notification configuration defaults — `notification_system = "app"` + history limit in heca-config).

**Checklist:**
- [ ] Definire enum/struttura lifecycle.
- [ ] Documentare avvio timer alla visibilità.
- [ ] Definire default per draft comuni.
- [ ] Test sticky e auto-dismiss.
- [ ] Nessun Instant::now nascosto nei metodi testati.

### ✅ 5e0face8-7901-4eba-ac34-12fc1b9e26ad — T186 — Add notification configuration defaults

Status: ✅ `done`

Aggiungere ai default embedded e a `heca-config/src/settings.rs` `notification_system = "app"`. Aggiungere un limite configurabile e documentato per la cronologia (default 100, con validazione maggiore di zero o semantica zero esplicita). Non aggiungere configurazione visuale che duplichi `Theme`/ToastStack. Il massimo di quattro elementi visibili resta contratto della prima versione, non un numero sparso tra più file.

---
**Completion summary:**
Added notification config defaults to `heca-config/src/settings.rs` + `config.default.toml` (depends on T191's `NotificationSystem` enum, now done):

`heca-config/src/settings.rs`:
- `SettingsConfig.notification_system: NotificationSystem` — `#[serde(default, alias = "notification-system")]` → defaults to `App`. Doc points to `NotificationSystem`.
- `SettingsConfig.notification_history_limit: usize` — `#[serde(default = "default_notification_history_limit", alias = "notification-history-limit")]` → 100. Doc states `0` = keep no history (explicit zero semantics per task).
- `default_notification_history_limit() -> 100` helper.
- `impl Default for SettingsConfig`: added both fields (`NotificationSystem::default()`, `default_notification_history_limit()`).

`config.default.toml` (embedded single source, after `terminal_scrollback_lines`):
- New "Notifications" section: `notification_system = "app"` + `notification_history_limit = 100` with comments explaining app/system/none and that the max-4-visible-toasts is a first-version ToastStack widget contract, NOT a setting (it does not appear here — per task: "il massimo di quattro elementi visibili resta contratto della prima versione, non un numero sparso tra più file").

No visual config duplicating Theme/ToastStack (per task). No delivery logic in the config crate (per T191 contract).

Tests (3 new in `settings::tests`, total 89 in crate): `settings_default_notification_fields` (App + 100), `settings_notification_system_overrides_parse` (kebab alias none + 250), `settings_notification_history_limit_zero_is_valid` (0 is valid, not error). Existing `config_default_toml_parses` guard in loader.rs still passes with the new toml fields.

cargo test -p heca-config: 89 passed. cargo check -p heca: clean (new serde-defaulted fields don't break consumers). clippy --workspace --all-targets --all-features: clean.

Files: heca-config/src/settings.rs (2 fields + helper + Default impl + 3 tests), config.default.toml (Notifications section).

Next in dependency order: T187 (Document notification configuration semantics) or T188 (Define canonical AppNotification and NotificationId). T187 is doc-only.

**Checklist:**
- [ ] Default app nel config embedded.
- [ ] Default history limit 100.
- [ ] Deep merge e reload preservano i default non specificati.
- [ ] Commenti concisi senza promettere backend OS v1.
- [ ] Test default e override.

### 📋 64b075f7-f924-4811-9e5a-30e3aee7ff42 — T187 — Document notification configuration semantics

Status: 📋 `planned`

Documentare vicino ai default config e nelle API config il significato preciso di `app`, `system` e `none`: app usa ToastStack; system, fino a P063, ricade in-app e produce un solo warning log per sessione; none sopprime presentazione ma non stderr/debug log. Documentare `notification_history_limit` e chiarire che la cronologia è runtime, non persistita. La sezione README completa resta P057/T239.

**Checklist:**
- [ ] Commenti default config.
- [ ] Doc comment enum e setting.
- [ ] Fallback system esplicito.
- [ ] None non promette silenzio dei log.
- [ ] History runtime/limit documentati.

### ✅ 7dfd4996-aceb-4501-8670-9d7da7c11025 — T188 — Define canonical AppNotification and NotificationId

Status: ✅ `done`

Creare tipi app-owned fortemente tipizzati: `NotificationId` monotono e non riutilizzato nella sessione; `AppNotification` con id, dedup_key opzionale, source, severity, title, body opzionale, lifecycle, action opzionale, dismissible e metadati temporali/stato necessari allo store. Separare dati canonici dallo stato di collocazione in attesa/visibile/cronologia quando questo rende invarianti più chiare. Non memorizzare Toast o callback UI. Prevedere clone mirati per la proiezione senza clonare inutilmente l'intero store.

---
**Completion summary:**
Added canonical notification model to `heca/src/notification.rs` (T188):

- `NotificationId(u64)` — `Debug/Clone/Copy/PartialEq/Eq/Hash/PartialOrd/Ord/Serialize/Deserialize` (serializes as bare u64). Session-monotonic, never-reused via `NotificationId::next()` using a process-wide `AtomicU64` counter (Relaxed ordering — host store serializes use). `from_raw` is test-only (`#[doc(hidden)]`). `as_u64()` for logging.
- `NotificationPlacement { Queued | Visible { expires_at: Option<Instant> } | History }` — **not** Serialize/Deserialize (Instant is not serializable; runtime-only state). Default = Queued. Separates placement bucket from immutable content so the store moves notifications between buckets without rewriting identity.
- `AppNotification { id, dedup_key, source, severity, title, body, lifecycle, action, created_at, placement }` — **not** Serialize/Deserialize (Instant + runtime state). `Debug/Clone/PartialEq`. Builder setters `dedup_key`/`body`/`lifecycle`/`action`. Accessors `is_visible`/`is_history`/`expires_at`. `new(id, source, severity, title, created_at: Instant)` — `created_at` injected explicitly (deterministic tests per T185 rule). `action: Option<heca_view::Intent>` — name-keyed + args, never a closure (T189 formalizes helpers).

Contract honored: monotonic non-reused id, optional dedup_key, separated canonical data vs placement state (Queued/Visible/History), no Toast widget / no UI callback stored, targeted clone for projection (fields pub for projection reads), instants out of the builder.

Removed `Serialize, Deserialize` from `AppNotification`/`NotificationPlacement` because `Instant` is not serializable and these are runtime state (not persisted; future history persistence is a separate serializable projection).

Tests: 8 new (total 26 in module) — id monotonic/unique, id serde-as-u64, placement default=Queued, new() defaults, builder chain, visible+expiry, history placement, clone-for-projection. `cargo test -p heca notification::` green. clippy clean (fixed a rustdoc false-positive by rewording a "+ args" line).

Files: heca/src/notification.rs (extended). `#![allow(dead_code)]` still covers staged phase until store wiring.

Next: T189 (Define notification actions as heca_view::Intent — formalize the action helpers/builders), then T190 (severity + ToastSeverity mapping, replaces the placeholder), T192 (draft builder), T193 (ToastSpec projection).

**Checklist:**
- [ ] Nuovo ID type.
- [ ] Campi canonici documentati.
- [ ] Nessun widget/callback nel modello.
- [ ] Invarianti temporali esplicite.
- [ ] Test costruzione e identità.

### 📋 994de278-bc6a-4744-8865-d0e0f1fb0876 — T189 — Define notification actions as heca_view::Intent

Status: 📋 `planned`

Definire `NotificationAction { label, intent: heca_view::Intent, dismiss_after }`. L'Intent conserva nome azione e `PropMap` argomenti ed è risolto soltanto all'attivazione tramite `dispatch_view_intent`; non usare closure e non limitare il modello a `WmAction`. Documentare la semantica: `dismiss_after=false` per Retry; per azioni dismissive, chiudere soltanto secondo l'esito concordato del dispatch e senza aggirare ActionRegistry. Il modello non entra in heca-grid-ui; ToastSpec riceve etichetta/target generico.

**Checklist:**
- [ ] Usare Intent name-keyed con args.
- [ ] Supportare azioni dinamiche provider/plugin.
- [ ] Definire dismiss_after.
- [ ] Test clone/equality necessari al dedup update.
- [ ] Nessuna closure o ActionHandler nel modello.

### 📋 ae6aecf5-b9dd-4327-9cb6-0306aa40551d — T190 — Define severity and ToastSeverity mapping

Status: 📋 `planned`

Definire una severity canonica app-owned (Info, Success, Warning, Danger/Error secondo nomenclatura coerente) indipendente dal widget e una conversione totale verso `heca_grid_ui::ToastSeverity`. Severity influenza presentazione e default lifecycle, non riordina continuamente le card già visibili. Tenere la conversione nell'adattatore app e aggiungere match esaustivi/test per evitare fallback silenziosi quando cresce l'enum.

**Checklist:**
- [ ] Enum app-owned.
- [ ] Mapping totale ToastSeverity.
- [ ] Default lifecycle documentati.
- [ ] Nessun colore hardcoded.
- [ ] Test ogni variante.

### ✅ b2319f2a-2902-4235-adbe-26d64149910c — T191 — Define NotificationSystem config enum

Status: ✅ `done`

Aggiungere `NotificationSystem { App, System, None }` in `heca-config/src/settings.rs` con serde snake_case, `Default::App`, Clone/Copy/Debug/Eq appropriati e accesso tramite Settings. Parsing invalido deve produrre un errore config normale e non modificare la configurazione runtime esistente durante reload. Non inserire logica di consegna nel crate config.

---
**Completion summary:**
Added `NotificationSystem` config enum to `heca-config/src/settings.rs` (after `SearchCase`):

- `NotificationSystem { App, System, None }` — `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]`, `#[serde(rename_all = "snake_case")]`, `#[default] App`. Doc explains it's the only notification *delivery* knob in the config crate (no delivery logic here), and that an unknown value is a normal config error that leaves the running config untouched on reload (no silent fallback).

Tests (3 new in `settings::tests`, total 86 in crate): `notification_system_default_is_app`, `notification_system_parses_snake_case` (absent→App, app/system/none), `notification_system_rejects_unknown_value` (carrier-pigeon → Err). A documented `#[allow(dead_code)]` on the test-only `W` struct (its field drives deserialization only).

cargo test -p heca-config: 86 passed. clippy -p heca-config: clean.

Files: heca-config/src/settings.rs (enum + 3 tests). The `SettingsConfig` field + `config.default.toml` default + history limit are T186 (next), which depends on this enum existing.

Next: T186 — add `notification_system: NotificationSystem` field to `SettingsConfig` + `notification_system = "app"` to `config.default.toml` + configurable history limit (default 100, >0 validation).

**Checklist:**
- [ ] Enum serde snake_case.
- [ ] Default App.
- [ ] Campo Settings.
- [ ] Test app/system/none e valore invalido.
- [ ] Nessuna dipendenza app/widget.

### 📋 b5a302b6-06ed-4409-b3f2-3a857b410cb0 — T192 — Define NotificationDraft builder API

Status: 📋 `planned`

Creare un `NotificationDraft` ergonomico per i producer con campi obbligatori minimi e builder per body, severity, source, lifecycle, dismissible, dedup_key e `NotificationAction`. Il draft non assegna ID, non decide il canale, non legge `Instant::now()` e non converte in ToastSpec: il router/store compiono queste operazioni. Usare ownership/borrowing idiomatici e impedire stati impossibili dove pratico.

**Checklist:**
- [ ] Costruttore minimo title/source.
- [ ] Builder per tutti gli opzionali.
- [ ] Nessun ID/canale/clock nel draft.
- [ ] Test default e override.
- [ ] Doc example reload failure.

### 📋 f11023cb-6ce5-4962-a971-23d14a858f9e — T193 — Project visible AppNotification into ToastSpec

Status: 📋 `planned`

Implementare l'adattatore app da una notifica visibile a `ToastSpec` usando il contratto corretto da P058: stesso ID runtime, titolo/corpo/severity, action label, dismissibility e target generici stabili per action/dismiss. Non copiare l'`Intent` dentro grid-ui e non calcolare timer/queue nella conversione. La mappatura target deve poter essere registrata dall'host nel `HintTargetRegistry`. Aggiornamenti same-ID devono produrre uno spec diverso che ToastStack applica in loco.

**Checklist:**
- [ ] Mapping campi totale.
- [ ] Target ID stabili per affordance.
- [ ] Intent resta nello store/app.
- [ ] Nessuna logica expiry.
- [ ] Test update same-ID e assenza action/dismiss.
