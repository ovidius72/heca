# ccefbac4-b29e-45d8-bd1d-e7199afa4907 — P004 — notification-04: Toast Action Dispatch

**Status:** 📋 `planned`
**Created:** 2026-07-03T22:50:20.126Z
**Updated:** 2026-07-27T11:40:15.531Z

Azioni toast dispatchate tramite ActionRegistry/policy path.

Aggiungere eventi app per dismiss/action, lookup delle azioni notification nello store e dispatch tramite il path registrato delle `WmAction`, senza closure dirette e senza bypass degli handler.

## Tasks

### 📋 cfe3e133-0a97-47c6-afaa-eaa75847cece — T001 — Add notification AppEvent variants

Status: 📋 `planned`

Estendere `AppEvent` con `NotificationDismiss { id }` e `NotificationAction { id }`.

### 📋 893b6411-7b5e-4a94-ac20-e57083ddea1a — T002 — Handle notification dismiss event

Status: 📋 `planned`

Gestire dismiss rimuovendo dallo store e chiedendo redraw; nessun accesso UI diretto.

### 📋 5bb8ca2d-954b-4a4b-b01e-7a30e1aa2eee — T003 — Handle notification action event through registry path

Status: 📋 `planned`

Clonare `NotificationAction`, applicare dismiss_after se richiesto e dispatchare `WmAction` tramite `dispatch_action`/ActionRegistry.

### 📋 797eea9b-a15c-4a7b-a5f7-30135fc4cd80 — T004 — Validate notification action policies

Status: 📋 `planned`

Verificare che azioni usate dalle notifiche abbiano `ActionPolicy` corretta, es. ReloadConfig Global e FocusPane coerente.

### 📋 78be366f-b9f1-4be9-8029-a15e96dced0e — T005 — Decide and wire InteractionSource for toast actions

Status: 📋 `planned`

Decidere se aggiungere `InteractionSource::ChromeOverlay` o riusare una sorgente esistente; evitare blocchi errati in floating domain.
