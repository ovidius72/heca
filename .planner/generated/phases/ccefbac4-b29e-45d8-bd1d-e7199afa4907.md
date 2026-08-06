# ccefbac4-b29e-45d8-bd1d-e7199afa4907 — P059 — notification-04: Intent Dispatch, Dismiss Action + KeyHint

**Status:** 📋 `planned`
**Created:** 2026-07-03T22:50:20.126Z
**Updated:** 2026-08-06T14:21:23.399Z

Instradare azioni e chiusura tramite Intent/ActionRegistry con sorgente ChromeOverlay e parità KeyHint/RPC.

Collegare le affordance Toast al percorso universale delle azioni. L'azione principale recupera dal `NotificationStore` il `heca_view::Intent` associato all'ID e lo invia a `dispatch_view_intent` (`heca/src/app/interaction.rs:956`); può attraversare un `AppEvent` di trasporto, ma non una closure che muta direttamente AppState. La chiusura diventa la nuova azione parametrica registrata `notification.dismiss { id }` / `WmAction::DismissNotification { id }`, completa secondo la checklist azioni: parser/build, priority esaustiva, handler, registry, descriptor, RPC generico e classificazione `ActionPolicy::Global`. Le interazioni provenienti dal livello usano `InteractionSource::ChromeOverlay`; questa sorgente non aggira la policy dell'Intent sottostante. L'adattatore Heca associa target runtime stabili (`notification:<id>:action`, `notification:<id>:dismiss`) agli Intent corretti nel `HintTargetRegistry`, così `prefix+/` invoca esattamente lo stesso percorso del click. I modali davanti occludono questi target.

## Goals
- Nessuna mutazione notification avviata dall'utente fuori ActionRegistry.
- Azioni built-in e dinamiche supportate senza WmAction-only.
- Dismiss raggiungibile da mouse, KeyHint e RPC.
- Policy coerente anche nel dominio floating.

## Non-goals
- Keybinding statico per un ID runtime.
- Confirmation-before-action.
- Far dipendere heca-grid-ui da heca_view::Intent.

## Dependencies
- P060(F009) espone callback/target dal livello persistente.
- HintTargetRegistry globale già atterrato in F003.

## Risks
- Un AppEvent di trasporto non deve diventare un secondo dispatcher.
- `dismiss_after` deve essere applicato con semantica esplicita rispetto all'esito dell'azione; Retry resta non dismissivo fino al successo del producer.
- SourceDependent actions devono mantenere la loro policy, non essere rese globali perché provengono da un toast.

## Completion Criteria
- Click e Hint producono lo stesso Intent osservabile.
- RPC `action notification.dismiss id=…` usa lo stesso handler.
- Unknown dynamic action non causa crash e segue il normale esito Intent.
- Test di policy tiled/floating/modal superati.

## Tasks

### 📋 5bb8ca2d-954b-4a4b-b01e-7a30e1aa2eee — T209 — Resolve and dispatch notification action Intent

Status: 📋 `planned`

Gestire `NotificationActionRequested { id }` sul thread app: clonare la `NotificationAction` visibile dallo store, rilasciare il borrow, chiamare `dispatch_view_intent(state, registry, InteractionSource::ChromeOverlay, &intent)` e poi applicare `dismiss_after` soltanto quando l'esito è `IntentOutcome::Ran`. Per `Blocked`, `Unknown`, `MissingArgs` o `NotRunnable`, mantenere la notifica. Per built-in, documentare l'attuale limite che `dispatch_view_intent` restituisce Ran dopo `dispatch_action`; non ampliare F009 con il refactor declined-action. Retry usa sempre `dismiss_after=false`; il producer reload rimuove l'errore solo sul successo reale.

**Checklist:**
- [ ] Clone action prima del dispatch.
- [ ] Source ChromeOverlay.
- [ ] Esiti non riusciti non chiudono.
- [ ] dismiss_after tramite azione registrata, non store diretto.
- [ ] Retry resta fino a successo producer.

### 📋 893b6411-7b5e-4a94-ac20-e57083ddea1a — T210 — Add registered notification.dismiss action

Status: 📋 `planned`

Aggiungere `WmAction::DismissNotification { notification_id }` e nome pubblico `notification.dismiss` seguendo l'intera checklist del progetto: `heca/src/input.rs` enum, `action_from_name`/`build_action` con argomento ID, `action_priority` esaustiva, handler in `heca/src/handlers.rs`, registrazione in `heca/src/app/registry.rs`, descriptor/catalog in `heca/src/actions.rs`, parsing via generic Intent/RPC e classificazione in `heca/src/app/interaction.rs` come `ActionPolicy::Global`. L'handler applica dismissibility e chiama lo store; non serve un binding statico perché ID è contestuale. KeyHint e RPC costruiscono lo stesso Intent.

**Checklist:**
- [ ] Variant + builder args.
- [ ] Priority + policy exhaustive.
- [ ] Handler + registry + descriptor.
- [ ] RPC generic action verificato.
- [ ] Test floating/modal/unknown/non-dismissible.

### 📋 cfe3e133-0a97-47c6-afaa-eaa75847cece — T211 — Define notification event bridge without a second dispatcher

Status: 📋 `planned`

Rivedere `AppEvent` in `heca/src/app/events.rs:24-30`. Riutilizzare `ChromeIntent { source, intent }` per `notification.dismiss`; aggiungere al massimo `NotificationActionRequested { id }` per il lookup late-bound dell'Intent memorizzato. L'evento è soltanto trasporto verso il thread app: non contiene closure, WmAction pre-risolta o mutazioni. Usare `InteractionSource::ChromeOverlay` e mantenere gli eventi clonabili/debuggabili secondo i requisiti esistenti.

**Checklist:**
- [ ] Riutilizzare ChromeIntent dove possibile.
- [ ] Evento action solo ID se necessario.
- [ ] Nessuna callback/handler nel payload.
- [ ] Source ChromeOverlay.
- [ ] Test costruzione/handling.

### 📋 78be366f-b9f1-4be9-8029-a15e96dced0e — T212 — Add InteractionSource::ChromeOverlay

Status: 📋 `planned`

Aggiungere `ChromeOverlay` a `InteractionSource` in `heca/src/app/interaction.rs` e aggiornare tutti i match esaustivi, `domain_for`, logging e test. La sorgente descrive azioni originate da overlay chrome persistenti; non concede privilegi globali. `notification.dismiss` passa perché la sua policy è Global. Un Intent sottostante continua a essere giudicato dalla propria ActionPolicy e dal focus domain: non trasformare FocusPane/SourceDependent in sempre consentita soltanto perché mostrata in una notifica.

**Checklist:**
- [ ] Enum e match esaustivi.
- [ ] domain_for coerente con Overlay.
- [ ] No bypass modal/floating.
- [ ] Test Global allowed.
- [ ] Test azione TiledOnly/SourceDependent bloccata quando previsto.

### 📋 797eea9b-a15c-4a7b-a5f7-30135fc4cd80 — T213 — Validate notification action policies and outcomes

Status: 📋 `planned`

Aggiungere test mirati al router: ReloadConfig resta Global e funziona da ChromeOverlay con floating attivo; DismissNotification è Global; azioni TiledOnly/WorkspaceLevel restano bloccate nel dominio floating/overlay appropriato; dynamic action usa la policy dichiarata; modale visibile occlude il target prima del dispatch. Verificare `IntentOutcome` per Unknown/MissingArgs/NotRunnable/Blocked e che `dismiss_after` non rimuova la notifica in questi casi. Non introdurre in questa feature notifiche sul rifiuto dell'azione.

**Checklist:**
- [ ] ReloadConfig e dismiss Global.
- [ ] Dynamic declared policy.
- [ ] Blocked/unknown/missing/not-runnable.
- [ ] Modal occlusion.
- [ ] Nessun declined-action producer.

### 📋 5f8838b3-5cb8-4208-a16e-4955f8fd7441 — T396 — Bind toast affordances to the global HintTargetRegistry

Status: 📋 `planned`

Durante la proiezione/mount Heca, associare i target generici prodotti da P058 ai relativi `heca_view::Intent`: action target → Intent memorizzato nella notifica; dismiss target → `notification.dismiss { id }`. Registrare soltanto target visibili e non occlusi, rimuoverli su dismiss/expiry/pending e aggiornare action target su dedup same-ID. L'attivazione via `prefix+/` attraversa `dispatch_view_intent` con la sorgente coerente del chrome overlay e produce lo stesso effetto del click. Non dipingere keycap manualmente e non aggiungere binding hardcoded.

**Checklist:**
- [ ] Target action e dismiss per ogni visible.
- [ ] Intent aggiornato same-ID.
- [ ] Rimozione target obsoleti.
- [ ] Occlusione modale.
- [ ] Test click e Hint equivalenti.
