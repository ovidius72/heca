//! The confirm step every destructive action goes through, and its wording.

use super::pane::handle_close_pane_by_id;
use super::workspace::{handle_delete_column, handle_delete_workspace};
use crate::app::interaction::focused_pane_id;
use crate::app_state::{AppState, InputMode};
use crate::input::WmAction;

/// Raise a declarative [`ConfirmSpec`](crate::actions::ConfirmSpec) as a host-owned overlay modal
/// (`chrome::overlay`): a [`Dialog`](heca_grid_ui::Dialog) layer whose real buttons (hint targets +
/// focus-traversable) come from the spec's [`ResponseButton`](crate::actions::ResponseButton)s. The
/// **title** is computed dynamically from the concrete `resolved` target ([`confirm_title`]); the
/// body / buttons / dismissibility come from the spec. On resolve, [`run_outcome`] runs the chosen
/// button's [`Outcome`](crate::actions::Outcome). `resume_sidebar` picks the mode to return to;
/// `InputMode::ConfirmDelete` is set purely as a status-bar marker (the modal layer owns input).
fn open_confirm(state: &mut AppState, spec: crate::actions::ConfirmSpec, resolved: WmAction) {
    let title = confirm_title(state, &resolved);
    let body = spec.message.clone();
    raise_confirm(state, title, body, spec, Pending::Wm(resolved));
}

/// What the prompt, once confirmed, runs: a raw built-in action, or a named action's call.
pub(crate) enum Pending {
    Wm(WmAction),
    Intent(crate::chrome::Intent),
}

/// **The one place a confirm prompt is raised**, whatever it guards. Both kinds of action — a
/// built-in's raw [`WmAction`] and a named action's [`Intent`] — arrive here, so the prompt, its
/// buttons and what *go ahead* does cannot differ between them (that difference is exactly how a
/// named action's confirm once went unenforced).
fn raise_confirm(
    state: &mut AppState,
    title: String,
    body: String,
    spec: crate::actions::ConfirmSpec,
    pending: Pending,
) {
    use crate::actions::{ButtonRole, Outcome};
    let mut modal = crate::chrome::ModalSpec::message(title, body)
        .dismissible(spec.dismissible)
        .danger(spec.buttons.iter().any(|b| b.role == ButtonRole::Danger));
    for b in &spec.buttons {
        modal = modal.action(
            crate::chrome::ModalAction::new(b.id.clone(), b.label.clone())
                .danger(b.role == ButtonRole::Danger),
        );
    }
    // A raw action's go-ahead is `Outcome::Proceed` (run `resolved`); a named action's runs the call.
    let (resolved, call) = match pending {
        Pending::Wm(wm) => (wm, None),
        Pending::Intent(intent) => (WmAction::ClosePane, Some(intent)),
    };
    let buttons: Vec<_> = spec
        .buttons
        .iter()
        .cloned()
        .map(|mut b| {
            if let (Some(call), Outcome::Proceed) = (&call, &b.outcome) {
                let call = call.clone();
                b.outcome = Outcome::Callback(std::rc::Rc::new(move |state, registry| {
                    registry.execute_dynamic(&call.action, state, &call);
                }));
            }
            b
        })
        .collect();
    crate::chrome::open_modal(state, modal, move |state, registry, result| {
        // Back to Normal, always: chrome focus is not a mode and the container still has the
        // keyboard if it had it before (F003/P086/T365).
        state.input_mode = InputMode::Normal;
        run_outcome(state, registry, &buttons, &resolved, &result);
        state.needs_redraw = true;
    });
    state.input_mode = InputMode::ConfirmDelete;
    state.needs_redraw = true;
}

/// Run the [`Outcome`](crate::actions::Outcome) of the response button the user chose — or, on Esc /
/// scrim dismiss, the `Cancel`-role button's outcome. `Proceed` runs the original `resolved` action
/// via `registry.execute`, which bypasses the dispatch gate that raised the prompt, so there is no
/// loop. `Dispatch` re-dispatches (policy-routed normally); `Callback` runs the native closure.
fn run_outcome(
    state: &mut AppState,
    registry: &crate::actions::ActionRegistry,
    buttons: &[crate::actions::ResponseButton],
    resolved: &WmAction,
    result: &crate::chrome::ModalResult,
) {
    use crate::actions::{ButtonRole, Outcome};
    let outcome = match result {
        crate::chrome::ModalResult::Action { id, .. } => {
            buttons.iter().find(|b| &b.id == id).map(|b| &b.outcome)
        }
        crate::chrome::ModalResult::Dismissed => buttons
            .iter()
            .find(|b| b.role == ButtonRole::Cancel)
            .map(|b| &b.outcome),
    };
    match outcome {
        Some(Outcome::Proceed) => registry.execute(resolved, state),
        Some(Outcome::Dispatch(action)) => crate::app::interaction::dispatch_action(
            state,
            registry,
            crate::app::interaction::InteractionSource::Keyboard,
            action,
        ),
        Some(Outcome::Callback(cb)) => cb(state, registry),
        Some(Outcome::Cancel) | None => {}
    }
}

/// The body of the prompt for `spec` on this call: built from the call's arguments when the spec says
/// how, else its fixed message.
pub(crate) fn confirm_message(
    spec: &crate::actions::ConfirmSpec,
    intent: &crate::chrome::Intent,
) -> String {
    spec.describe
        .map_or_else(|| spec.message.clone(), |describe| describe(intent))
}

/// Whether a prompt is raised: always when the spec is **forced**, else the user's `[confirm]` choice.
pub(crate) fn prompt_needed(spec: &crate::actions::ConfirmSpec, user_enabled: bool) -> bool {
    spec.forced || user_enabled
}

/// **The confirm gate for a name-keyed action** (a component's, a plugin's, an extension's, or
/// `trust_project`): the same prompt as for a built-in, raised through [`raise_confirm`]. Returns
/// `true` when it raised the prompt (the caller must not run the action); *go ahead* then runs the
/// action itself, past this gate.
pub(crate) fn maybe_confirm_dynamic(state: &mut AppState, intent: &crate::chrome::Intent) -> bool {
    let Some(meta) = state.action_catalog.find(&intent.action) else {
        return false;
    };
    let (Some(spec), title) = (meta.confirm.clone(), meta.label.clone()) else {
        return false;
    };
    if !prompt_needed(
        &spec,
        confirm_enabled(state, &spec.config_name, spec.default_enabled),
    ) {
        return false;
    }
    let body = confirm_message(&spec, intent);
    raise_confirm(state, title, body, spec, Pending::Intent(intent.clone()));
    true
}

/// The built-in actions whose confirm [`maybe_confirm_destructive`] resolves from a raw
/// [`WmAction`]. Every other action with a confirm is named and goes through
/// [`maybe_confirm_dynamic`]; a confirm in neither list would never be asked.
pub(crate) const BUILTIN_CONFIRMED: [&str; 3] = ["close", "delete_column", "delete_workspace"];

/// Does a gate ask for this action's confirm? Used by the test that fails when a spec is declared
/// somewhere nothing enforces it.
#[cfg(test)]
pub(crate) fn gate_covers(catalog: &crate::actions::ActionCatalog, name: &str) -> bool {
    if catalog.is_builtin(name) {
        BUILTIN_CONFIRMED.contains(&name)
    } else {
        true
    }
}

/// Whether the confirm prompt for `config_name` is enabled: the user's `[confirm].<name>` value
/// when set, else the action's declared `default_enabled`. Generic — a plugin action is
/// configurable by name automatically, no dedicated settings field.
fn confirm_enabled(state: &AppState, config_name: &str, default_enabled: bool) -> bool {
    state.confirm.enabled(config_name, default_enabled)
}

/// The confirm config name for a raw destructive action (`None` if it isn't confirmable).
/// The **owner action name** whose [`ActionMeta::confirm`] governs this action — not the toggle key.
/// `ClosePane`/`ClosePaneById` are both owned by `close`, so they confirm identically.
fn confirm_owner_name(action: &WmAction) -> Option<&'static str> {
    match action {
        WmAction::ClosePane | WmAction::ClosePaneById { .. } => Some("close"),
        WmAction::DeleteColumn { .. } => Some("delete_column"),
        WmAction::DeleteWorkspace { .. } => Some("delete_workspace"),
        _ => None,
    }
}

/// Human label for a workspace index (its name, or "ws N").
fn ws_label(state: &AppState, ws_idx: usize) -> String {
    state
        .session
        .workspaces
        .get(ws_idx)
        .and_then(|ws| ws.name.clone())
        .unwrap_or_else(|| format!("ws {}", ws_idx + 1))
}

/// The dynamic confirm-prompt **title** for a raw destructive action (target-specific — the pane /
/// column / workspace name — so it can't live in the static [`ConfirmSpec`]).
///
/// This resolves the *names* only; the wording is [`confirm_title_for`], split out because a dialog
/// has **two** independent sources of wording — the button comes from `ConfirmSpec.buttons`, the
/// title from here — and nothing was comparing them. The pane's title said "Delete" for a while
/// after its button said "Close" (F003/P086/T370). The split is what lets a test hold them together
/// without an `AppState`.
fn confirm_title(state: &AppState, action: &WmAction) -> String {
    match action {
        WmAction::ClosePaneById { pane_id } => {
            // Use the pane's **custom name** if it has one, else a generic "Pane" — never a
            // placeholder/process title, so an unnamed pane reads "Close Pane?" not "Close Yellow?".
            let label = state
                .session
                .workspaces
                .iter()
                .find_map(|ws| ws.find_pane(*pane_id))
                .and_then(|p| p.custom_name.clone())
                .unwrap_or_else(|| "Pane".to_string());
            confirm_title_for(action, &label)
        }
        WmAction::DeleteColumn { ws_idx, .. } | WmAction::DeleteWorkspace { ws_idx } => {
            confirm_title_for(action, &ws_label(state, *ws_idx))
        }
        _ => "Confirm?".to_string(),
    }
}

/// The confirm title's **wording**, given the target's already-resolved display name.
///
/// **Close** for a pane, matching its action id, its binding name, its catalog label and this
/// dialog's own button: one process ends and the layout absorbs the gap. **Delete** for a column or
/// a workspace, which destroy every pane and process inside them — a different act, which should not
/// read the same (decided with the user, 2026-07-29).
/// Each title names **just its target** — its name when it has one, else the type word, exactly as
/// an unnamed pane reads "Close Pane?" rather than its process title. A column has no name, so it is
/// always the type word.
fn confirm_title_for(action: &WmAction, target: &str) -> String {
    match action {
        WmAction::ClosePaneById { .. } => format!("Close {target}?"),
        // No index and no parent workspace: the title says what is about to happen, and the column
        // in question is the one just clicked or focused — it is on screen (F003/P086/T370).
        WmAction::DeleteColumn { .. } => "Delete Column?".to_string(),
        WmAction::DeleteWorkspace { .. } => format!("Delete {target}?"),
        _ => "Confirm?".to_string(),
    }
}

/// Run a raw destructive action immediately (no confirm) by calling its handler
/// directly — the no-dialog branch of [`request_destructive`].
fn run_destructive_now(state: &mut AppState, action: &WmAction) {
    match action {
        WmAction::ClosePaneById { .. } => handle_close_pane_by_id(state, action),
        WmAction::DeleteColumn { .. } => handle_delete_column(state, action),
        WmAction::DeleteWorkspace { .. } => handle_delete_workspace(state, action),
        _ => {}
    }
}

/// The confirm-or-run chokepoint for a **raw** destructive action, used by the sidebar / keyboard
/// resolvers that compute a target then need the same confirm behaviour. Looks up the action's
/// [`ConfirmSpec`](crate::actions::ConfirmSpec): if the prompt is enabled, [`open_confirm`] raises
/// it; otherwise the raw action runs now ([`run_destructive_now`]). The central dispatch gate
/// ([`maybe_confirm_destructive`]) shares this same spec-driven path for directly-dispatched actions.
pub(crate) fn request_destructive(state: &mut AppState, raw_action: WmAction) {
    let spec = confirm_owner_name(&raw_action)
        .and_then(|name| state.action_catalog.confirm_spec(name).cloned());
    match spec {
        Some(spec) if confirm_enabled(state, &spec.config_name, spec.default_enabled) => {
            open_confirm(state, spec, raw_action)
        }
        _ => run_destructive_now(state, &raw_action),
    }
}

/// The **central action-confirmation gate**, called once at the dispatch chokepoint
/// ([`dispatch_intent`](crate::app::interaction::dispatch_intent)) before any action executes.
///
/// Looks the dispatched `action` up in the runtime [`ConfirmSpec`](crate::actions::ConfirmSpec)
/// catalog. If it needs a prompt (and the prompt is enabled), it raises the declarative confirm
/// overlay and returns `true` — meaning **handled**, the caller must NOT run the action. For every
/// other action (or when the prompt is disabled) it returns `false` (run it as usual).
///
/// This is why the guard lives on the **action**, not the call site: every surface that dispatches
/// an action — keyboard, the pane-header close button, a context menu / dropdown, RPC — funnels
/// through this one dispatch chokepoint and gets identical confirm behaviour for free. The
/// confirm's `Proceed` runs the raw action via `registry.execute`, which bypasses this gate (no
/// loop). `ClosePane` (focused) is resolved to the concrete `ClosePaneById` pinned at prompt time.
pub(crate) fn maybe_confirm_destructive(state: &mut AppState, action: &WmAction) -> bool {
    // Resolve the dispatched action to its confirm config name + the concrete action to run on
    // `Proceed` (`ClosePane` → the focused pane's `ClosePaneById`, pinned now).
    let (name, resolved) = match action {
        WmAction::ClosePane => match focused_pane_id(state) {
            Some(pane_id) => ("close", WmAction::ClosePaneById { pane_id }),
            None => return false, // nothing focused → let the normal path no-op
        },
        WmAction::ClosePaneById { .. } => ("close", action.clone()),
        WmAction::DeleteColumn { .. } => ("delete_column", action.clone()),
        WmAction::DeleteWorkspace { .. } => ("delete_workspace", action.clone()),
        _ => return false,
    };
    // `name` is the owner ACTION name; its meta's confirm spec carries the toggle key.
    debug_assert!(
        BUILTIN_CONFIRMED.contains(&name),
        "'{name}' is resolved here but missing from BUILTIN_CONFIRMED"
    );
    let Some(spec) = state.action_catalog.confirm_spec(name).cloned() else {
        return false;
    };
    if !confirm_enabled(state, &spec.config_name, spec.default_enabled) {
        return false; // disabled → let dispatch run the action raw via its handler
    }
    open_confirm(state, spec, resolved);
    true
}

/// The confirm dialog's two halves must agree (F003/P086/T370).
///
/// The button's word lives in `ConfirmSpec.buttons` and the title's lives in [`confirm_title_for`],
/// with nothing between them — which is how the pane came to be *closed* by its button and *deleted*
/// by its title at the same time. These tests are the thing that compares them.
#[cfg(test)]
mod confirm_wording_tests {
    use super::*;
    use crate::actions::ActionCatalog;
    use heca_core::layout::PaneId;

    /// The verb on an action's confirm button, as the user reads it.
    fn button_verb(catalog: &ActionCatalog, action: &str) -> String {
        catalog
            .confirm_spec(action)
            .unwrap_or_else(|| panic!("{action} declares a confirm"))
            .buttons
            .iter()
            .find(|b| matches!(b.outcome, crate::actions::Outcome::Proceed))
            .expect("a confirm has a proceed button")
            .label
            .clone()
    }

    #[test]
    fn the_title_and_the_button_use_the_same_verb() {
        let catalog = ActionCatalog::with_builtins();
        for (owner, action, target) in [
            (
                "close",
                WmAction::ClosePaneById { pane_id: PaneId(1) },
                "Pane",
            ),
            (
                "delete_column",
                WmAction::DeleteColumn {
                    ws_idx: 0,
                    col_idx: 0,
                },
                "ws 1",
            ),
            (
                "delete_workspace",
                WmAction::DeleteWorkspace { ws_idx: 0 },
                "ws 1",
            ),
        ] {
            let verb = button_verb(&catalog, owner);
            let title = confirm_title_for(&action, target);
            assert!(
                title.starts_with(&verb),
                "{owner}: the dialog says '{title}' over a button that says '{verb}'",
            );
        }
    }

    /// A pane is **closed**; the two containers are **deleted**. The split is the decision, so it is
    /// asserted rather than left to the loop above — which would pass if both said the same word.
    #[test]
    fn a_pane_closes_and_a_container_deletes() {
        assert_eq!(
            confirm_title_for(&WmAction::ClosePaneById { pane_id: PaneId(1) }, "Pane"),
            "Close Pane?",
        );
        assert_eq!(
            confirm_title_for(&WmAction::DeleteWorkspace { ws_idx: 0 }, "notes"),
            "Delete notes?",
        );
        assert_eq!(
            confirm_title_for(
                &WmAction::DeleteColumn {
                    ws_idx: 0,
                    col_idx: 2
                },
                "notes"
            ),
            "Delete Column?",
            "a column has no name, so it reads as the type word — the same shape as an unnamed pane",
        );
    }
}

#[cfg(test)]
mod gate_coverage_tests {
    use super::*;
    use crate::actions::ActionCatalog;

    /// **A confirm declared where nothing asks it is a silent hole** — the one a named action's
    /// confirm fell into: its metadata said "ask first" and dispatch never did. Every action in the
    /// catalog that declares a confirm must be covered by a gate: a built-in by
    /// [`maybe_confirm_destructive`] (its owner name is in [`BUILTIN_CONFIRMED`]), a named one by
    /// [`maybe_confirm_dynamic`].
    #[test]
    fn every_declared_confirm_is_enforced_by_a_gate() {
        let mut catalog = ActionCatalog::with_builtins();
        // The named actions heca registers itself, and one an extension would add.
        crate::project_trust::register_trust_action(
            &mut crate::actions::ActionRegistry::new(),
            &mut catalog,
        );
        let _ = crate::actions::register_dynamic(
            &mut crate::actions::ActionRegistry::new(),
            &mut catalog,
            crate::actions::ActionMeta::new("pro.erase").destructive(),
            None,
        );
        let confirmed: Vec<String> = catalog
            .describe_all()
            .iter()
            .filter(|a| catalog.confirm_spec(&a.name).is_some())
            .map(|a| a.name.clone())
            .collect();
        assert!(
            confirmed.contains(&"close".to_string()),
            "built-ins declare confirms"
        );
        assert!(confirmed.contains(&"trust_project".to_string()));
        for name in &confirmed {
            assert!(
                gate_covers(&catalog, name),
                "'{name}' declares a confirm no gate enforces"
            );
        }
    }

    /// A forced prompt is raised whatever the user's `[confirm]` says; an ordinary one follows it.
    #[test]
    fn a_forced_prompt_ignores_the_users_setting() {
        let mut spec = crate::actions::ConfirmSpec::destructive("x", "Go");
        assert!(
            !prompt_needed(&spec, false),
            "ordinary: the user may switch it off"
        );
        spec.forced = true;
        assert!(
            prompt_needed(&spec, false),
            "forced: nothing switches it off"
        );
    }
}
