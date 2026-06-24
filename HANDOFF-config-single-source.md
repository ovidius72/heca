# Handoff — config.toml come fonte unica (config.default.toml + keybindings.default.toml)

> Scritto 2026-06-24. Decisioni RISOLTE con l'utente; pronto per implementazione
> in sessione fresca. Niente è ancora implementato.

## Obiettivo
I default della configurazione devono vivere in **file `.toml` versionati** (non
seminati in codice), così:
- l'app li parsa all'avvio → **sempre validi** (un test li parsa);
- sono **completi per costruzione** (sono i default) → niente drift codice↔doc;
- l'utente **copia il file e parte da lì** per le modifiche.

## Decisioni risolte
1. **Due file** (split, non un unico file) — più pulito e coerente con la struttura
   esistente del repo (`keybindings.md` + `default-keybindings.toml` già trattano i
   tasti a parte). Confine netto, nessuna ambiguità.
2. **Naming consistente**: `config.default.toml` + `keybindings.default.toml`
   (rinominare l'attuale `default-keybindings.toml` → `keybindings.default.toml`).
3. **Confine tra i due file** (sezioni disgiunte, niente sovrapposizione):
   - `config.default.toml` → `[settings]`, `[appearance]`, `[font]`, `[program]`.
   - `keybindings.default.toml` → `[keys]` (prefix + binding + `[keys.unbind]`) +
     `[[keys.command]]` + `[[keys.mode]]`.
4. **Meccanismo**: default incorporati via `include_str!` + **deep-merge** del config
   utente (come `toml::Value`) sopra i default, poi `try_into::<Config>()`.
5. I default seminati in **codice** (`KeysConfig::default`, `ProgramsConfig::default`,
   e le `#[serde(default = fn)]` su settings/appearance) restano solo come **backstop**
   minimo; la fonte visibile diventano i file. `Config::default()` = parse dei file
   incorporati.
6. Assorbire/eliminare `example.config.toml` (sostituito da `config.default.toml`) e
   `default-keybindings.toml` (rinominato).

## Architettura del loader (heca-config/src/loader.rs)
Stato attuale: `load_config_file()` (riga ~115) fa `toml::from_str::<Config>(user)`
con `#[serde(default)]` a riempire i mancanti. `Config` (righe 63-77): campi
`settings, appearance, font, programs (alias "program"), keys`.

Nuovo flusso:
```text
CONFIG_DEFAULT  = include_str!("../../config.default.toml")      // settings/appearance/font/program
KEYS_DEFAULT    = include_str!("../../keybindings.default.toml") // keys.*

fn merged_config_value():
    base = parse(CONFIG_DEFAULT) as toml::Value           // top-level: settings, appearance, font, program
    base = deep_merge(base, parse(KEYS_DEFAULT))          // aggiunge top-level: keys (sezioni disgiunte)
    if user config.toml exists:      base = deep_merge(base, parse(user_config))
    if user keybindings.toml exists: base = deep_merge(base, parse(user_keys))
    return base

Config = merged_config_value().try_into::<Config>()       // serde valida; backstop per campi assenti
```
- `Config::default()`: rimpiazzare il `derive(Default)` con `impl Default` che fa
  `parse(CONFIG_DEFAULT) deep_merge parse(KEYS_DEFAULT) -> try_into`. Così il file è
  la fonte anche nel caso "nessun config utente" e nei test.
- Percorsi utente: oggi cerca `~/.config/heca/config.toml`. Aggiungere accanto
  `~/.config/heca/keybindings.toml` (+ il path `config_dir()` come per config.toml).
- `try_load()` (reload) usa lo stesso flusso → il reload prende anche le modifiche ai
  keybinding (oltre al fix chrome già fatto in `main.rs reload_config`).

### deep_merge (nuova funzione, ~25 righe, nessuna dipendenza nuova — `toml` c'è già)
```rust
fn deep_merge(base: toml::Value, over: toml::Value) -> toml::Value {
    match (base, over) {
        (toml::Value::Table(mut b), toml::Value::Table(o)) => {
            for (k, ov) in o {
                let nv = match b.remove(&k) {
                    Some(bv) => deep_merge(bv, ov),
                    None => ov,
                };
                b.insert(k, nv);
            }
            toml::Value::Table(b)
        }
        // scalari e ARRAY: over vince (gli array NON si fondono).
        (_, over) => over,
    }
}
```
**Edge case da documentare**: le tabelle si fondono per-chiave (es. `[keys]`,
`[keys.unbind]`, `[appearance]`) → l'utente sovrascrive un singolo binding/valore e
tiene il resto. Gli ARRAY si sostituiscono → `[[keys.command]]` / `[[keys.mode]]`
dell'utente RIMPIAZZANO l'intera lista di default (comportamento atteso/accettato).

## Cosa togliere/cambiare in codice
- `heca-config/src/keys.rs`: `KeysConfig::default()` (riga ~143) semina **50**
  `bindings.insert(...)` → spostarli in `keybindings.default.toml`. Lasciare un
  `Default` minimo (prefix) come backstop, oppure derivarlo dal file. **Verificare**
  come `KeybindingMap`/`KeysConfig` deserializza i binding (campo `bindings` riga 126
  è un catch-all: confermare se usa `#[serde(flatten)]`). Il merge a livello di
  `toml::Value` avviene PRIMA della deserializzazione, quindi la semantica serde dei
  binding non cambia — ma va verificato che `[keys]` con prefix+binding+unbind si
  deserializzi correttamente dopo il merge.
- `heca-config/src/programs.rs`: `ProgramsConfig::default()` (riga ~236) semina il
  catalogo (shell + nvim/yazi/...) → spostarlo in `[program]` dentro
  `config.default.toml`. Backstop minimo se serve.
- `heca-config/src/{settings,appearance,font}.rs`: le `#[serde(default = fn)]`
  restano (backstop). I valori "veri" ora sono nel file. NON serve rimuoverle.

## File TOML da creare
- `config.default.toml`: prendere l'attuale `example.config.toml` (già completo e
  documentato), **attivare** i valori di default reali (settings/appearance/font),
  aggiungere `[program]` col catalogo completo (da `ProgramsConfig::default`). Gli
  esempi non-default (es. override programma dimostrativo) restano commentati.
- `keybindings.default.toml`: partire dall'attuale `default-keybindings.toml`
  (rinominare) + `src/keybindings.md` come riferimento; deve contenere TUTTI i 50
  binding attivi sotto `[keys]` (prefix, binding, eventuale `[keys.unbind]` vuoto) +
  esempi `[[keys.command]]`/`[[keys.mode]]` **commentati** (non sono default).
  **Verificare** che la struttura mappi 1:1 su `Config.keys` (sezione `[keys]`).

## Test (heca-config)
- Sostituire `example_config_toml_parses_against_the_live_schema` (loader.rs, ~riga
  302) con due test: `config_default_toml_parses` e `keybindings_default_toml_parses`
  (entrambi `try_into::<Config>` ok).
- Test `deep_merge`: tabella fonde per-chiave, array sostituisce.
- Test "config utente parziale fa overlay sui default del file" (es. utente setta
  solo `transparency = 30` → resto = default del file; un solo binding cambiato →
  resto invariato).
- Test `Config::default()` = parse dei file (theme grid_tron, prefix ctrl+b, ~50
  binding presenti, catalogo programmi presente).

## Verifica end-to-end
1. `cargo test -p heca-config` verde.
2. `cargo run -p heca`: avvio senza config utente → default dai file (sidebar, tasti,
   programmi tutti ok).
3. Mettere `~/.config/heca/keybindings.toml` con UN binding diverso → solo quello
   cambia, gli altri restano. `prefix+Shift+r` lo ricarica.
4. Mettere `~/.config/heca/config.toml` con `border_color`/`border_width` → applicati.

## Aggiornare la documentazione (REGOLA: vedi memoria update-showcase-when-changing-grid-ui)
- `src/keybindings.md`: aggiornare il riferimento `default-keybindings.toml` →
  `keybindings.default.toml`.
- README / AGENTS se citano `example.config.toml` o `default-keybindings.toml`.
- `docs/widgets.md` non serve (è grid-ui), ma se cambiano proprietà widget aggiornarlo.

## Branch / PR
- Branch nuovo da `main` aggiornato (la #184 è mergiata: `border_*` + reload-fix +
  doc sono già in main).
- PR dedicata "config single-source (file-driven defaults)".

## Rischi / note
- `Config::default()` ora parsa file incorporati → se un file è malformato l'app
  panica all'avvio: è VOLUTO (un test lo previene), ma tenerlo a mente.
- `programs` ha alias serde `"program"` (loader.rs:73): il deep-merge a livello Value
  usa la chiave reale del TOML (`program`) — assicurarsi che base+utente usino la
  stessa chiave.
- `KeybindingMap` deserializzazione dei binding: confermare che dopo il merge la
  tabella `[keys]` (prefix + N binding) si deserializzi senza perdere i default
  (col merge a Value i default ci sono già nella tabella, quindi dovrebbe filare).
