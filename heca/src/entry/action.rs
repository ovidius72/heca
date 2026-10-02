//! **`pro.action("start_worker")`** — an action that belongs to no dock. *Server side.*
//!
//! ```ignore
//! let pro = heca::extension("pro");
//! pro.action("start_worker")
//!     .label("Start worker")
//!     .icon(Glyph::Play)
//!     .run(|cx, args| {
//!         cx.dispatch("spawn_pane", PropMap::new());
//!         Handled::Yes
//!     });
//! ```
//!
//! Its name is `pro.start_worker`: the extension, then the short name. See [`extension`].
//!
//! Only what *nothing else can do*. The test a dock's declared actions already use decides it:
//! remove the thing that owns it — does the action still make sense? An action tied to one row of
//! one dock is that dock's ([`Provider::actions`](crate::providers::Provider::actions)); one that
//! makes sense on its own is declared here.
//!
//! It lands in the same catalog and registry as every built-in, so the palette, the icon, the
//! label, `describe-action`, a key binding, a menu entry, RPC and the confirm gate all find it.
//!
//! **What the handler gets.** A [`ProviderCx`] — read the store, emit intents, nothing else — the
//! same context a mounted component's `perform` gets, so there is one type and one door. It has no
//! placement: the action is not aimed at a dock. What it asks for goes out the way a click does,
//! policy-routed and confirm-gated.
//!
//! **Words are an English fallback.** `.label(..)` and `.description(..)` are the text shown until
//! the app-wide text system (T519) lands; then a plugin's strings come from the language files
//! under its own namespace, and these are what is shown when a translation is missing.

use std::rc::Rc;

use heca_grid_ui::Handled;

use crate::actions::{
    ActionCatalog, ActionMeta, ActionRegistry, DuplicateAction, register_dynamic,
};
use crate::app::conflicts::{ActionConflict, Conflicts};
use crate::app_state::AppState;
use crate::chrome::{Intent, SharedChromeState, StartupQueue};
use crate::providers::{ProviderCx, emit_queued};

/// What an added action does when it runs.
type Run = dyn Fn(&mut ProviderCx<'_>, &Intent) -> Handled;

/// One action on its way in: what it says about itself, and what it does.
struct Declared {
    meta: ActionMeta,
    run: Rc<Run>,
}

thread_local! {
    /// Declarations made before the host took them.
    static QUEUE: StartupQueue<Box<Declared>> = const { StartupQueue::new() };
}

impl ActionMeta {
    /// **What it does** — the last line of an [`Extension::action`](super::Extension::action).
    /// Queues it; the host registers it as it starts.
    ///
    /// Returning [`Handled::No`] is a silent decline — nothing to act on — not an error.
    ///
    /// For an action of an [`Extension`](super::Extension) only: a name whose owner half is not an
    /// extension this program declared is refused, so nothing gets an action into someone else's
    /// namespace by building the metadata by hand. A dock's own actions are declared by the dock
    /// ([`Provider::actions`](crate::providers::Provider::actions)) and run by its `perform`.
    pub fn run(self, run: impl Fn(&mut ProviderCx<'_>, &Intent) -> Handled + 'static) {
        // A refused item carries no name and was already reported where it was refused.
        if self.name.is_empty() {
            return;
        }
        let declared = self
            .name
            .split_once('.')
            .is_some_and(|(owner, _)| super::extension::is_declared(owner));
        if !declared {
            crate::chrome::warn_author(format!(
                "[heca] action '{}' does not belong to an extension — declare one with \
                 `heca::extension(..)` and add it with `.action(..)` — so it was not added",
                self.name
            ));
            return;
        }
        let declared = Declared {
            meta: self,
            run: Rc::new(run),
        };
        if let Err(late) = QUEUE.with(|q| q.add(Box::new(declared))) {
            crate::chrome::warn_author(format!(
                "[heca] action '{}' was added after the app started, so it does nothing — add it \
                 before `heca::run()`",
                late.meta.name
            ));
        }
    }
}

/// Run one added action against `store`, returning what it asked the host to do.
///
/// The whole of the call apart from the app state: split out so it can be checked without a window.
fn run_once(run: &Run, store: SharedChromeState, intent: &Intent) -> Vec<Intent> {
    let mut cx = ProviderCx::new("", store);
    run(&mut cx, intent);
    cx.drain()
}

/// Put every queued action into the catalog and registry, and close the queue. The host calls this
/// once, as it starts, after the mounted components have registered theirs.
pub(crate) fn register_queued_actions(
    registry: &mut ActionRegistry,
    catalog: &mut ActionCatalog,
    conflicts: &mut Conflicts,
) {
    let mut seen = std::collections::HashSet::new();
    for declared in QUEUE.with(StartupQueue::take) {
        let Declared { meta, run } = *declared;
        let id = meta.name.clone();
        // The registry lets a second registration replace the first (a dock remounting is meant
        // to). Two actions added under one name by one program is a mistake, so it is said.
        if !seen.insert(id.clone()) {
            conflicts.action(ActionConflict {
                id: id.clone(),
                kept: "the one added last".to_string(),
                rejected: "an earlier action of the same name".to_string(),
                shadows_builtin: false,
            });
        }
        let handler = Rc::new(move |state: &mut AppState, intent: &Intent| {
            let queued = run_once(&*run, state.chrome_state.clone(), intent);
            emit_queued(state, queued);
        });
        if let Err(duplicate) = register_dynamic(registry, catalog, meta, Some(handler)) {
            conflicts.action(ActionConflict {
                id,
                kept: match duplicate {
                    DuplicateAction::ShadowsBuiltin => "the built-in".to_string(),
                    DuplicateAction::ReplacedDynamic => "the one added last".to_string(),
                },
                rejected: "an action added with an extension's `.action(..)`".to_string(),
                shadows_builtin: matches!(duplicate, DuplicateAction::ShadowsBuiltin),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::interaction::ActionPolicy;
    use crate::chrome::PropMap;
    use heca_grid_ui::Glyph;

    /// A fresh queue, as a new host would start with.
    fn reset() {
        QUEUE.with(StartupQueue::reset);
        super::super::extension::reset();
    }

    fn pro() -> super::super::Extension {
        super::super::extension("pro")
    }

    fn registered() -> (ActionRegistry, ActionCatalog, Conflicts) {
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        let mut conflicts = Conflicts::default();
        register_queued_actions(&mut registry, &mut catalog, &mut conflicts);
        (registry, catalog, conflicts)
    }

    /// **What the author says about an action is what every surface reads** — the label, the
    /// icon, the danger, the owner — from the one catalog, the same one a built-in is in.
    #[test]
    fn an_added_action_is_in_the_catalog_with_what_its_author_said() {
        reset();
        let pro = pro();
        pro.action("start")
            .label("Start agent")
            .icon(Glyph::Play)
            .destructive()
            .run(|_, _| Handled::Yes);
        pro.action("list").run(|_, _| Handled::Yes);

        let (_, catalog, conflicts) = registered();
        let start = catalog.find("pro.start").expect("in the catalog");
        assert_eq!(start.label, "Start agent");
        assert_eq!(start.icon, Some(Glyph::Play));
        assert_eq!(
            start.owner, None,
            "an added action belongs to the app, not a dock"
        );
        assert!(catalog.destructive("pro.start"));
        let go_ahead = catalog.confirm_spec("pro.start").expect("asks first");
        assert!(
            go_ahead.buttons.iter().any(|b| b.label == "Start agent"),
            "the go-ahead button is worded with the label, whichever order they were said in"
        );
        let list = catalog.find("pro.list").expect("in the catalog");
        assert_eq!(list.label, "pro.list", "no label said ⇒ the id");
        assert!(
            !catalog.destructive("pro.list"),
            "danger only when it says so"
        );
        assert_eq!(list.policy, ActionPolicy::AlwaysAllowed);
        assert!(conflicts.is_empty());
    }

    /// **What it asks for is queued for the host, not run behind the router's back** — the
    /// handler gets a context that can only read and emit.
    #[test]
    fn what_a_handler_asks_for_comes_back_as_intents_for_the_host() {
        let store = SharedChromeState::new(240.0, 240.0);
        let out = run_once(
            &|cx, args| {
                assert_eq!(args.action, "pro.start");
                cx.dispatch("focus_pane", PropMap::new());
                Handled::Yes
            },
            store,
            &Intent::new("pro.start"),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].action, "focus_pane");
    }

    /// **Adding the same name twice keeps the last and reports it** — never two silent owners.
    #[test]
    fn an_action_added_twice_keeps_the_last_and_is_reported() {
        reset();
        let pro = pro();
        pro.action("start").label("First").run(|_, _| Handled::Yes);
        pro.action("start").label("Second").run(|_, _| Handled::Yes);
        let (_, catalog, conflicts) = registered();
        assert_eq!(
            catalog.find("pro.start").expect("registered").label,
            "Second"
        );
        assert_eq!(conflicts.actions.len(), 1);
        assert!(!conflicts.actions[0].shadows_builtin);
    }

    /// **Metadata built by hand cannot put an action in someone else's namespace.**
    #[test]
    fn an_action_whose_owner_is_not_a_declared_extension_is_refused() {
        reset();
        let _pro = pro();
        ActionMeta::new("other.start").run(|_, _| Handled::Yes);
        ActionMeta::new("start").run(|_, _| Handled::Yes);
        assert!(QUEUE.with(|q| q.peek(<[_]>::is_empty)));
    }

    /// **After the host has started, a new action is refused out loud** — and the queue stays empty.
    #[test]
    fn an_action_added_after_the_app_started_is_dropped() {
        reset();
        let _ = registered();
        pro().action("too_late").run(|_, _| Handled::Yes);
        assert!(QUEUE.with(|q| q.peek(<[_]>::is_empty)));
    }
}
