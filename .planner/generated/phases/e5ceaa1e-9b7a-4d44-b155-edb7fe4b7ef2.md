# e5ceaa1e-9b7a-4d44-b155-edb7fe4b7ef2 — P061 — notification-01: Domain Model + Config

**Status:** 📋 `planned`
**Created:** 2026-07-03T22:50:20.126Z
**Updated:** 2026-08-06T14:21:54.468Z

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

### 📋 208ad51d-2f42-436e-83c7-69af2c350b6d — T184 — Define extensible notification source model

Status: 📋 `planned`

Definire nel modulo notifiche app-owned una sorgente descrittiva ed estensibile per log, cronologia e dedup, senza inserire concetti in `heca-grid-ui`. Supportare almeno App/Config/Pane/Provider/Plugin/Command con identificatore opaco opzionale; non aggiungere Agent perché appartiene a un'altra feature. Evitare riferimenti mutabili obbligatori a indici workspace/pane che possono diventare invalidi dopo l'archiviazione. Documentare che la source non decide routing o ActionPolicy.

**Checklist:**
- [ ] Definire tipo ed eventuale Other/custom.
- [ ] Evitare dipendenze widget.
- [ ] Coprire serializzazione/debug se necessaria.
- [ ] Testare sorgenti con e senza identificatore.
- [ ] Documentare differenza da InteractionSource.

### 📋 2167fa5e-750b-42b1-be11-1658c13913b6 — T185 — Define lifecycle semantics and defaults

Status: 📋 `planned`

Definire lifecycle app-owned per notifica: durata automatica, sticky e dismissibility. La durata non produce `expires_at` al push: il timer parte quando lo store promuove la notifica a visibile. Stabilire default documentati per severity/lifecycle e mantenere gli istanti fuori dal builder di dominio quando servono test deterministici. Le notifiche sticky non scadono ma possono essere chiuse se dismissible.

**Checklist:**
- [ ] Definire enum/struttura lifecycle.
- [ ] Documentare avvio timer alla visibilità.
- [ ] Definire default per draft comuni.
- [ ] Test sticky e auto-dismiss.
- [ ] Nessun Instant::now nascosto nei metodi testati.

### 📋 5e0face8-7901-4eba-ac34-12fc1b9e26ad — T186 — Add notification configuration defaults

Status: 📋 `planned`

Aggiungere ai default embedded e a `heca-config/src/settings.rs` `notification_system = "app"`. Aggiungere un limite configurabile e documentato per la cronologia (default 100, con validazione maggiore di zero o semantica zero esplicita). Non aggiungere configurazione visuale che duplichi `Theme`/ToastStack. Il massimo di quattro elementi visibili resta contratto della prima versione, non un numero sparso tra più file.

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

### 📋 7dfd4996-aceb-4501-8670-9d7da7c11025 — T188 — Define canonical AppNotification and NotificationId

Status: 📋 `planned`

Creare tipi app-owned fortemente tipizzati: `NotificationId` monotono e non riutilizzato nella sessione; `AppNotification` con id, dedup_key opzionale, source, severity, title, body opzionale, lifecycle, action opzionale, dismissible e metadati temporali/stato necessari allo store. Separare dati canonici dallo stato di collocazione in attesa/visibile/cronologia quando questo rende invarianti più chiare. Non memorizzare Toast o callback UI. Prevedere clone mirati per la proiezione senza clonare inutilmente l'intero store.

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

### 📋 b2319f2a-2902-4235-adbe-26d64149910c — T191 — Define NotificationSystem config enum

Status: 📋 `planned`

Aggiungere `NotificationSystem { App, System, None }` in `heca-config/src/settings.rs` con serde snake_case, `Default::App`, Clone/Copy/Debug/Eq appropriati e accesso tramite Settings. Parsing invalido deve produrre un errore config normale e non modificare la configurazione runtime esistente durante reload. Non inserire logica di consegna nel crate config.

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
