//! **Telling the user about a project settings file they have not trusted** — and the action that
//! trusts it.
//!
//! A project's `.heca/config.toml` is ignored until trusted (`heca_config::trust`). Printing that to
//! the terminal reaches nobody who launched heca from the Dock, so the app raises a **notification**
//! with a *Trust* action, once per folder and file content (`announced`): a changed file is a new
//! content, so it is asked about again. The action is the registered `trust_project`, so it is also in
//! the palette and reachable from RPC, like any other.

use std::rc::Rc;

use crate::actions::{ActionCatalog, ActionCategory, ActionMeta, ActionRegistry, register_dynamic};
use crate::app::interaction::ActionPolicy;
use crate::args::{ArgKind, ArgSpec};
use crate::chrome::Intent;
use crate::notification::{Notification, NotificationAction};

/// The action's name: the palette, a key binding and RPC all say `trust_project`.
pub(crate) const TRUST_PROJECT: &str = "trust_project";

/// The argument naming which project's folder to trust; absent means the one heca was started in.
const FOLDER_ARG: &str = "folder";

/// The hash of the file **the notice was about**. When present, trust happens only if the file still
/// has that hash — so content that changed after the notice is never trusted unseen. Absent (the
/// palette, the command line) means "as it is now", which is fine: the person is acting right then.
const HASH_ARG: &str = "hash";

/// Raise one notification if the project file here is untrusted or has changed since it was.
/// Called at startup and on every reload; says each (folder, content) once ever.
pub(crate) fn notify_if_untrusted() {
    let Ok(Some(project)) = heca_config::loader::read_project() else {
        return;
    };
    if project.trusted {
        return;
    }
    let key = format!("project_trust:{}:{}", project.dir.display(), project.hash);
    if !crate::announced::first_time(&key) {
        return;
    }
    let mut trust = Intent::new(TRUST_PROJECT);
    trust.args.insert(
        FOLDER_ARG.to_string(),
        heca_view::PropValue::Text(project.dir.display().to_string()),
    );
    trust.args.insert(
        HASH_ARG.to_string(),
        heca_view::PropValue::Text(project.hash.clone()),
    );
    Notification::warning("Project settings not trusted")
        .body(format!(
            "{} is ignored until you trust it. A project file can bind keys to commands, so check \
             it first.",
            project.path.display()
        ))
        .dedup_key(key)
        .action(NotificationAction::new("Trust", trust).dismiss_after(true))
        .sticky()
        .send();
}

/// Register the `trust_project` action: trust the project file, then reload so it applies.
/// Name-keyed, the same door `notify` and a component's declared actions use.
///
/// **It needs a person.** It is confirm-gated, so a script over RPC — or, later, an agent working
/// inside a cloned repository — cannot trust that repository's file silently. When heca-pro arrives,
/// agent roles are not to be allowed this action by default.
pub(crate) fn register_trust_action(registry: &mut ActionRegistry, catalog: &mut ActionCatalog) {
    let mut meta = ActionMeta::new(TRUST_PROJECT)
        .label("Trust project settings")
        .description("Trust this project's .heca/config.toml as it is now, so it applies.")
        .category(ActionCategory::System)
        .policy(ActionPolicy::Global)
        .arg(ArgSpec {
            name: HASH_ARG.to_string(),
            kind: ArgKind::Text,
            required: false,
            description: "Trust only if the file still has this SHA-256 (the notice sets it)."
                .to_string(),
            values: Vec::new(),
        })
        .arg(ArgSpec {
            name: FOLDER_ARG.to_string(),
            kind: ArgKind::Text,
            required: false,
            description: "A folder inside the project; default the one heca was started in."
                .to_string(),
            values: Vec::new(),
        });
    meta.owner = None;
    // The confirm gate asks first, with a verb of its own, on every surface that dispatches it.
    meta.confirm = Some(trust_confirm());
    let handler = Rc::new(|state: &mut crate::app_state::AppState, intent: &Intent| {
        let start = match intent.args.get(FOLDER_ARG) {
            Some(heca_view::PropValue::Text(folder)) => Some(std::path::PathBuf::from(folder)),
            _ => std::env::current_dir().ok(),
        };
        let Some(start) = start else { return };
        let expected = match intent.args.get(HASH_ARG) {
            Some(heca_view::PropValue::Text(hash)) => Some(hash.clone()),
            _ => None,
        };
        match heca_config::loader::read_project_in(&start) {
            Ok(Some(project)) if !hash_matches(expected.as_deref(), &project.hash) => {
                eprintln!(
                    "[heca] {} changed since the notice, so it was not trusted — check it again",
                    project.path.display()
                );
                // The new content is a new question: raise a fresh notice for it.
                notify_if_untrusted();
            }
            Ok(Some(project)) if heca_config::trust::trust(&project.dir, &project.hash) => {
                crate::providers::emit_queued(state, vec![Intent::new("reload_config")]);
            }
            Ok(Some(_)) => eprintln!("[heca] could not remember the trust (no data directory)"),
            Ok(None) => eprintln!("[heca] no .heca/config.toml found from {}", start.display()),
            Err(e) => eprintln!("[heca] project settings: {e}"),
        }
    });
    let _ = register_dynamic(registry, catalog, meta, Some(handler));
}

/// Whether the file may be trusted: with an expected hash (from the notice) only if the file still
/// has it; with none, always (the person is acting on the file as it is right now).
fn hash_matches(expected: Option<&str>, current: &str) -> bool {
    expected.is_none_or(|h| h == current)
}

/// The prompt's body, naming the file and its folder from the call's arguments.
fn describe_trust(intent: &Intent) -> String {
    let folder = match intent.args.get(FOLDER_ARG) {
        Some(heca_view::PropValue::Text(f)) => f.clone(),
        _ => std::env::current_dir()
            .map(|d| d.display().to_string())
            .unwrap_or_default(),
    };
    format!(
        "Trust {folder}/.heca/config.toml? It can bind keys to commands, so only trust a project you know."
    )
}

/// The prompt: a forced `[Cancel] [Trust]` choice.
fn trust_confirm() -> crate::actions::ConfirmSpec {
    use crate::actions::{ConfirmSpec, ResponseButton};
    ConfirmSpec {
        message: "Trust this project's settings file? It can bind keys to commands, so only trust \
                  a project you know."
            .to_string(),
        buttons: vec![
            ResponseButton::cancel("cancel", "Cancel"),
            ResponseButton::proceed("confirm", "Trust", true),
        ],
        dismissible: false,
        config_name: TRUST_PROJECT.to_string(),
        default_enabled: true,
        // **Forced.** This prompt is what stops a script — or an agent working inside a cloned
        // repository — trusting that repository's file silently, so nothing may switch it off: it
        // is not listed under `[confirm]` and no key disables it.
        forced: true,
        describe: Some(describe_trust),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`trust_project` is a registered action**, so it is in the palette and reachable from RPC,
    /// takes an optional `folder`, and is allowed whatever owns the screen (a notification's button
    /// must work while a floating pane is active).
    #[test]
    fn trust_project_is_a_registered_action_with_an_optional_folder() {
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        register_trust_action(&mut registry, &mut catalog);
        let meta = catalog.find(TRUST_PROJECT).expect("in the catalog");
        assert_eq!(meta.policy, ActionPolicy::Global);
        let names: Vec<_> = meta.args.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["hash", "folder"]);
        assert!(meta.args.iter().all(|a| !a.required), "both optional");
    }

    /// **It needs a person**: the action is confirm-gated, so a script or an agent cannot trust a
    /// repository's file silently.
    #[test]
    fn trusting_asks_for_confirmation() {
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        register_trust_action(&mut registry, &mut catalog);
        assert!(
            catalog.destructive(TRUST_PROJECT),
            "the confirm gate asks first"
        );
    }

    /// **The prompt names the file and cannot be switched off.**
    #[test]
    fn the_prompt_names_the_file_and_is_forced() {
        let spec = trust_confirm();
        assert!(spec.forced);
        assert!(
            crate::handlers::prompt_needed(&spec, false),
            "no user setting turns it off"
        );
        let mut intent = Intent::new(TRUST_PROJECT);
        intent.args.insert(
            FOLDER_ARG.to_string(),
            heca_view::PropValue::Text("/work/repo".to_string()),
        );
        let body = crate::handlers::confirm_message(&spec, &intent);
        assert!(body.contains("/work/repo/.heca/config.toml"), "{body}");
        let mut catalog = ActionCatalog::with_builtins();
        register_trust_action(&mut ActionRegistry::new(), &mut catalog);
        let listed = catalog
            .describe_all()
            .into_iter()
            .find(|a| a.name == TRUST_PROJECT)
            .expect("described");
        assert_eq!(listed.confirm, None, "not listed under [confirm]");
    }

    /// **A file that changed after the notice is not trusted.** With the notice's hash, trust only if
    /// the file still has it; from the palette or the command line (no hash) the current content is
    /// trusted.
    #[test]
    fn a_changed_file_is_not_trusted_but_an_unhashed_call_trusts_what_is_there() {
        assert!(hash_matches(Some("abc"), "abc"));
        assert!(
            !hash_matches(Some("abc"), "abd"),
            "changed since the notice"
        );
        assert!(hash_matches(None, "anything"), "no hash: as it is now");
    }
}
