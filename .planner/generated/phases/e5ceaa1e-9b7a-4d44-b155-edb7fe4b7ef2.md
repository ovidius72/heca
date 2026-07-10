# e5ceaa1e-9b7a-4d44-b155-edb7fe4b7ef2 — P005 — notification-01: Model + Config

**Status:** 📋 `planned`
**Created:** 2026-07-03T22:50:20.126Z
**Updated:** 2026-07-03T22:51:04.232Z

Tipi core delle notifiche, mapping ToastSpec e schema config `notification_system`.

Definire i tipi app-owned del dominio notifiche (`NotificationSystem`, severity, lifecycle, source, action, AppNotification), l'adapter verso `ToastSpec` e la configurazione `[settings].notification_system`. V1 usa in-app notifications come default; `system` e `none` devono essere parseable e deterministici.

## Tasks

### 📋 ae6aecf5-b9dd-4327-9cb6-0306aa40551d — T001 — Define app notification severity and ToastSeverity mapping

Status: 📋 `planned`

Definire `NotificationSeverity` app-owned e mapping completo verso `ToastSeverity`, senza colori o stile nel modello app.

### 📋 2167fa5e-750b-42b1-be11-1658c13913b6 — T002 — Define notification lifecycle defaults

Status: 📋 `planned`

Definire `NotificationLifecycle::AutoDismiss(Duration)` / `Sticky` e policy default per severity: info 4s, success 3s, warning 6s, danger sticky.

### 📋 208ad51d-2f42-436e-83c7-69af2c350b6d — T003 — Define notification source model

Status: 📋 `planned`

Definire `NotificationSource` per System, Config, Action, Pane, Workspace, ChromeProvider e Agent; solo metadata/debug/dedup, non routing.

### 📋 b2319f2a-2902-4235-adbe-26d64149910c — T004 — Define NotificationSystem config enum

Status: 📋 `planned`

Aggiungere `NotificationSystem { App, System, None }` in `heca-config/src/settings.rs`, serde snake_case, default App e test parsing.

### 📋 994de278-bc6a-4744-8865-d0e0f1fb0876 — T005 — Define notification action model

Status: 📋 `planned`

Definire `NotificationAction { label, action: WmAction, dismiss_after }`; vietate closure e mutazioni dirette di AppState.

### 📋 7dfd4996-aceb-4501-8670-9d7da7c11025 — T006 — Define AppNotification canonical model

Status: 📋 `planned`

Definire `AppNotification` come source of truth runtime con id, severity, title/body, source, lifecycle, action, dedup_key, created_at/expires_at.

### 📋 b5a302b6-06ed-4409-b3f2-3a857b410cb0 — T007 — Define NotificationDraft builder API

Status: 📋 `planned`

Aggiungere `NotificationDraft` e builder helpers (`success/info/warning/danger`, body, source, action, dedup_key, sticky, auto_dismiss) per producer ergonomici.

### 📋 f11023cb-6ce5-4962-a971-23d14a858f9e — T008 — Convert AppNotification to ToastSpec

Status: 📋 `planned`

Implementare adapter app→grid-ui: title/body/severity/action label verso `ToastSpec`; nessun comportamento azione dentro `ToastSpec`.

### 📋 5e0face8-7901-4eba-ac34-12fc1b9e26ad — T009 — Add notification_system config default

Status: 📋 `planned`

Aggiungere `notification_system = "app"` a `config.default.toml` con commento conciso e senza promettere OS notifications v1.

### 📋 64b075f7-f924-4811-9e5a-30e3aee7ff42 — T010 — Document notification_system setting

Status: 📋 `planned`

Documentare in README i valori `app`, `system`, `none`; chiarire che `system` è riservato/fallback finché il backend OS non atterra.
