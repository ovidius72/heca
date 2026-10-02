//! **Pane header buttons, by name.**
//!
//! A button is *an action, on this pane*: its icon, words, tooltip shortcut and danger all come from
//! the [`ActionCatalog`] by the action's name, so a button another crate adds names its action and
//! writes nothing else (AGENTS § "Chrome buttons → action, tooltip"). heca's own six are registered
//! here too, through the same [`PaneButtonDef`] — the first users of the registry, not a special case.

use std::rc::Rc;

use heca_core::layout::PaneId;
use heca_grid_ui::widgets::Glyph;

use crate::actions::ActionCatalog;
use crate::chrome::{Intent, PropValue, StartupQueue, warn_author};
use crate::input::WmAction;

/// **Where a pane is** — the plain data a button needs to say what a click does. Never `AppState`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PaneIds {
    pub(crate) pane_id: PaneId,
    pub(crate) ws_idx: usize,
    pub(crate) col_idx: usize,
}

/// **What a click sends.** heca's own buttons send a pane-parameterized [`WmAction`]; an added
/// button sends its action by name, with the pane it was clicked on as the `pane` argument.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ButtonEmit {
    Wm(WmAction),
    Intent(Intent),
}

impl ButtonEmit {
    /// Whether the button stays when its pane is **floating**, decided by the action's own policy
    /// rather than a list kept here: tiled-only acts (split, zoom, move) hide, pane-local ones stay.
    pub(crate) fn allowed_when_floating(&self, catalog: &ActionCatalog) -> bool {
        match self {
            ButtonEmit::Wm(wm) => crate::app::interaction::action_allowed_when_floating(wm),
            ButtonEmit::Intent(intent) => catalog.find(&intent.action).is_some_and(|m| {
                matches!(
                    m.policy,
                    crate::app::interaction::ActionPolicy::FocusedPaneLocal
                        | crate::app::interaction::ActionPolicy::Global
                )
            }),
        }
    }
}

/// When a button reads as toggled **on**: the pane's column is zoomed, or the pane is floating.
#[derive(Clone, Copy, Debug)]
enum HeldWhen {
    Zoomed,
    Floating,
}

/// What a click sends, worked out for one pane. Reads the catalog because an added button sends only
/// what its action declares.
type EmitFn = dyn Fn(&ActionCatalog, PaneIds) -> ButtonEmit;

/// One button, by name.
pub(crate) struct PaneButtonDef {
    /// What config lists: `split`, `pro.show_notes`.
    pub(crate) name: String,
    /// The action it stands for — the identity its tooltip's shortcut, its danger and, for an added
    /// button, its icon and words are read by. (`split` stands for `split_vertical`, so it still
    /// hints that key.)
    pub(crate) action_name: String,
    /// The words when the catalog has none of its own for this button.
    label: String,
    icon: Rc<dyn Fn(&ActionCatalog) -> Glyph>,
    emit: Rc<EmitFn>,
    /// Acts on the *focused* pane (zoom, float), so it focuses this pane first.
    needs_focus: bool,
    held_when: Option<HeldWhen>,
    /// Added by another crate rather than shipped with heca.
    pub(crate) from_extension: bool,
}

/// **One of heca's own buttons**, described as data — properties are fields, not a run of positional
/// arguments — and turned into a [`PaneButtonDef`] by [`into_def`](Builtin::into_def).
struct Builtin {
    name: &'static str,
    /// The tooltip/shortcut name; `split` keeps `split_vertical` so it still hints its key.
    action_name: &'static str,
    label: &'static str,
    /// The catalog action whose icon it borrows, and the mark to fall back on if that is unknown.
    icon_of: &'static str,
    fallback: Glyph,
    emit: fn(PaneIds) -> WmAction,
    needs_focus: bool,
    held_when: Option<HeldWhen>,
}

impl Builtin {
    fn into_def(self) -> PaneButtonDef {
        let Builtin {
            name,
            action_name,
            label,
            icon_of,
            fallback,
            emit,
            needs_focus,
            held_when,
        } = self;
        PaneButtonDef {
            name: name.to_string(),
            action_name: action_name.to_string(),
            label: label.to_string(),
            // Icons come from the action catalog (the single source); the literal is a defensive
            // fallback only, so the bar and the context menu can never drift.
            icon: Rc::new(move |catalog| catalog.icon(icon_of).unwrap_or(fallback)),
            emit: Rc::new(move |_, ids| ButtonEmit::Wm(emit(ids))),
            needs_focus,
            held_when,
            from_extension: false,
        }
    }
}

impl super::listing::Named for PaneButtonDef {
    fn name(&self) -> &str {
        &self.name
    }
    fn is_added(&self) -> bool {
        self.from_extension
    }
}

/// A button worked out for one pane — everything the header draws it from.
pub(crate) struct ResolvedButton {
    pub(crate) glyph: Glyph,
    pub(crate) emit: ButtonEmit,
    pub(crate) label: String,
    pub(crate) needs_focus: bool,
}

impl PaneButtonDef {
    /// Work this button out for the pane at `ids`.
    pub(crate) fn resolve(&self, catalog: &ActionCatalog, ids: PaneIds) -> ResolvedButton {
        ResolvedButton {
            glyph: (self.icon)(catalog),
            emit: (self.emit)(catalog, ids),
            label: self.label.clone(),
            needs_focus: self.needs_focus,
        }
    }

    /// Whether it reads as toggled on for a pane whose column is `zoomed` / which is `floating`.
    pub(crate) fn is_held(&self, zoomed: bool, floating: bool) -> bool {
        match self.held_when {
            Some(HeldWhen::Zoomed) => zoomed,
            Some(HeldWhen::Floating) => floating,
            None => false,
        }
    }

    /// One another crate added: the action it names says everything about it.
    fn extension(name: String) -> Self {
        let action = name.clone();
        let for_icon = name.clone();
        Self {
            action_name: name.clone(),
            label: name.clone(),
            icon: Rc::new(move |catalog| {
                catalog
                    .icon(&for_icon)
                    .unwrap_or(crate::actions::GENERIC_ACTION_ICON)
            }),
            emit: Rc::new(move |catalog, ids| {
                let mut intent = Intent::new(action.clone());
                // **The pane is passed only to an action that asks for it.** An action that declares
                // no `pane` argument takes none, and a call that passes one is refused ("unknown
                // argument"), so sending it unasked would break exactly the actions that do not care
                // which pane the button was on.
                let wants_pane = catalog
                    .find(&action)
                    .is_some_and(|m| m.args.iter().any(|a| a.name == PANE_ARG));
                if wants_pane {
                    intent
                        .args
                        .insert(PANE_ARG.to_string(), PropValue::Int(ids.pane_id.0 as i64));
                }
                ButtonEmit::Intent(intent)
            }),
            needs_focus: false,
            held_when: None,
            from_extension: true,
            name,
        }
    }
}

/// The argument an added button's action receives **if it declares it**: the id of the pane whose
/// button was clicked.
pub const PANE_ARG: &str = "pane";

thread_local! {
    /// Buttons added before the app took them.
    static QUEUE: StartupQueue<String> = const { StartupQueue::new() };
}

/// Queue the button `name` (an action's full name) — called by `Extension::pane_button`. The app
/// takes the queue once, as it starts; a call after that is said and dropped.
pub(crate) fn add_extension_button(name: String) {
    if let Err(name) = QUEUE.with(|q| q.add(name)) {
        warn_author(format!(
            "[heca] pane button '{name}' was added after the app started, so it does nothing — add \
             it before `heca::run()`"
        ));
    }
}

/// **Every pane button there is**, by name: heca's own and the ones other crates added.
pub(crate) struct PaneButtons {
    defs: Vec<PaneButtonDef>,
}

impl Default for PaneButtons {
    /// heca's own buttons only — what a test wants.
    fn default() -> Self {
        Self {
            defs: builtin_buttons(),
        }
    }
}

impl PaneButtons {
    /// heca's own, then everything queued. Takes the queue and closes it: the app calls this once.
    pub(crate) fn from_startup() -> Self {
        let mut all = Self::default();
        for name in QUEUE.with(StartupQueue::take) {
            all.add(PaneButtonDef::extension(name));
        }
        all
    }

    /// Add one. A name already taken is refused and said — the first keeps it.
    fn add(&mut self, def: PaneButtonDef) {
        super::listing::add_unique(&mut self.defs, "pane button", def);
    }

    #[cfg(test)]
    pub(crate) fn get(&self, name: &str) -> Option<&PaneButtonDef> {
        self.defs.iter().find(|d| d.name == name)
    }

    /// **The buttons to show, in order** — the user's list by name, with what other crates added
    /// after it while that list is still the default (see [`listing`](super::listing)).
    pub(crate) fn shown(&self, config: &[String]) -> Vec<&PaneButtonDef> {
        super::listing::shown(
            &self.defs,
            config,
            &heca_config::appearance::default_pane_title_actions(),
        )
    }

    /// **Say what the user's list gets wrong or misses**, so nothing is silent — the shared rules of
    /// [`listing::report`](super::listing::report), plus the one that is only true of a button: an
    /// added button whose action does not exist.
    pub(crate) fn report(&self, config: &[String], catalog: &ActionCatalog) {
        super::listing::report(
            "pane button",
            "title_actions",
            &self.defs,
            config,
            &heca_config::appearance::default_pane_title_actions(),
        );
        for def in self.defs.iter().filter(|d| d.from_extension) {
            if catalog.find(&def.action_name).is_none() {
                warn_author(format!(
                    "[heca] pane button '{}' has no action of that name, so it will show a plain \
                     mark and do nothing — declare the action too",
                    def.name
                ));
            }
        }
    }
}

/// heca's own six, registered exactly as another crate's would be.
fn builtin_buttons() -> Vec<PaneButtonDef> {
    [
        Builtin {
            name: "split",
            action_name: "split_vertical",
            label: "New pane",
            icon_of: "add_pane_to_column",
            fallback: Glyph::FolderSimplePlus,
            emit: |ids| WmAction::AddPaneToColumn {
                ws_idx: ids.ws_idx,
                col_idx: ids.col_idx,
            },
            needs_focus: false,
            held_when: None,
        },
        Builtin {
            name: "move_left",
            action_name: "move_pane_left",
            label: "Move left",
            icon_of: "move_pane_left",
            fallback: Glyph::ArrowLineLeft,
            emit: |ids| WmAction::MovePaneLeft {
                pane_id: Some(ids.pane_id),
            },
            needs_focus: false,
            held_when: None,
        },
        Builtin {
            name: "move_right",
            action_name: "move_pane_right",
            label: "Move right",
            icon_of: "move_pane_right",
            fallback: Glyph::ArrowLineRight,
            emit: |ids| WmAction::MovePaneRight {
                pane_id: Some(ids.pane_id),
            },
            needs_focus: false,
            held_when: None,
        },
        Builtin {
            name: "close",
            action_name: "close",
            label: "Close",
            icon_of: "close",
            fallback: Glyph::FolderSimpleMinus,
            emit: |ids| WmAction::ClosePaneById {
                pane_id: ids.pane_id,
            },
            needs_focus: false,
            held_when: None,
        },
        Builtin {
            name: "zoom",
            action_name: "zoom_column",
            label: "Zoom",
            icon_of: "zoom_column",
            fallback: Glyph::FrameCorners,
            emit: |_| WmAction::ZoomColumn,
            needs_focus: true,
            held_when: Some(HeldWhen::Zoomed),
        },
        Builtin {
            name: "float",
            action_name: "float",
            label: "Float",
            icon_of: "float",
            fallback: Glyph::Cards,
            emit: |_| WmAction::Float,
            needs_focus: true,
            held_when: Some(HeldWhen::Floating),
        },
    ]
    .into_iter()
    .map(Builtin::into_def)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> PaneIds {
        PaneIds {
            pane_id: PaneId(7),
            ws_idx: 2,
            col_idx: 3,
        }
    }

    fn shipped(name: &str) -> ResolvedButton {
        let catalog = ActionCatalog::with_builtins();
        PaneButtons::default()
            .get(name)
            .unwrap_or_else(|| panic!("heca ships '{name}'"))
            .resolve(&catalog, ids())
    }

    /// **heca's own six buttons do exactly what they did as a closed list**: the same action for the
    /// same pane, the same icon from the catalog, and focus first only for the two that act on the
    /// focused pane.
    #[test]
    fn the_shipped_buttons_map_to_the_actions_they_always_did() {
        let close = shipped("close");
        assert_eq!(close.glyph, Glyph::FolderSimpleMinus);
        assert_eq!(
            close.emit,
            ButtonEmit::Wm(WmAction::ClosePaneById { pane_id: PaneId(7) })
        );
        assert!(!close.needs_focus);

        let split = shipped("split");
        assert_eq!(split.glyph, Glyph::FolderSimplePlus);
        assert_eq!(
            split.emit,
            ButtonEmit::Wm(WmAction::AddPaneToColumn {
                ws_idx: 2,
                col_idx: 3
            })
        );

        let zoom = shipped("zoom");
        assert_eq!(zoom.glyph, Glyph::FrameCorners);
        assert_eq!(zoom.emit, ButtonEmit::Wm(WmAction::ZoomColumn));
        assert!(zoom.needs_focus, "zoom acts on the focused pane");

        let float = shipped("float");
        assert_eq!(float.glyph, Glyph::Cards);
        assert_eq!(float.emit, ButtonEmit::Wm(WmAction::Float));
        assert!(float.needs_focus);
    }

    /// **A floating pane keeps only float and close**, decided by each action's own policy.
    #[test]
    fn a_floating_pane_keeps_only_the_buttons_its_actions_allow() {
        let catalog = ActionCatalog::with_builtins();
        for (name, kept) in [
            ("float", true),
            ("close", true),
            ("split", false),
            ("zoom", false),
            ("move_left", false),
            ("move_right", false),
        ] {
            assert_eq!(
                shipped(name).emit.allowed_when_floating(&catalog),
                kept,
                "{name}"
            );
        }
    }

    /// **The user's list is the list**, by name and in order; `zoom` and `float` read as held.
    #[test]
    fn the_users_list_decides_which_buttons_show_and_in_what_order() {
        let all = PaneButtons::default();
        let names = |shown: Vec<&PaneButtonDef>| -> Vec<String> {
            shown.into_iter().map(|d| d.name.clone()).collect()
        };
        assert_eq!(
            names(all.shown(&["close".into(), "zoom".into()])),
            ["close", "zoom"]
        );
        assert!(all.shown(&[]).is_empty(), "an empty list hides the side");
        assert_eq!(
            names(all.shown(&["nothing.here".into(), "float".into()])),
            ["float"],
            "a name nothing provides is skipped"
        );
        assert!(all.get("zoom").unwrap().is_held(true, false));
        assert!(!all.get("zoom").unwrap().is_held(false, true));
        assert!(all.get("float").unwrap().is_held(false, true));
    }

    /// An added button: names an action, and a click sends that action the pane it was on.
    fn with_added(name: &str) -> PaneButtons {
        let mut all = PaneButtons::default();
        all.add(PaneButtonDef::extension(name.to_string()));
        all
    }

    /// **An added button is shown by default — only while the list is still the default.**
    #[test]
    fn an_added_button_shows_while_the_list_is_the_default_and_not_after() {
        let all = with_added("pro.show_notes");
        let default = heca_config::appearance::default_pane_title_actions();
        let shown: Vec<_> = all.shown(&default).iter().map(|d| d.name.clone()).collect();
        assert_eq!(shown, ["split", "close", "pro.show_notes"]);

        let own = vec!["close".to_string()];
        let shown: Vec<_> = all.shown(&own).iter().map(|d| d.name.clone()).collect();
        assert_eq!(shown, ["close"], "the user's own list is the whole list");

        let listed = vec!["close".to_string(), "pro.show_notes".to_string()];
        let shown: Vec<_> = all.shown(&listed).iter().map(|d| d.name.clone()).collect();
        assert_eq!(shown, ["close", "pro.show_notes"], "listing it shows it");
    }

    /// **An added button reads everything from its action** — and sends it the pane's id.
    #[test]
    fn an_added_button_takes_its_look_from_its_action_and_sends_the_pane() {
        use crate::actions::ActionMeta;
        let mut catalog = ActionCatalog::with_builtins();
        let mut registry = crate::actions::ActionRegistry::new();
        crate::actions::register_dynamic(
            &mut registry,
            &mut catalog,
            ActionMeta::new("pro.show_notes")
                .label("Show notes")
                .icon(Glyph::Plus)
                .arg(crate::args::ArgSpec {
                    name: PANE_ARG.to_string(),
                    kind: crate::args::ArgKind::Int,
                    required: false,
                    description: "The pane whose button was clicked.".to_string(),
                    values: Vec::new(),
                }),
            None,
        )
        .expect("registered");
        let all = with_added("pro.show_notes");
        let def = all.get("pro.show_notes").expect("added");
        let resolved = def.resolve(&catalog, ids());
        assert_eq!(resolved.glyph, Glyph::Plus, "the action's own icon");
        let ButtonEmit::Intent(intent) = &resolved.emit else {
            panic!("an added button sends its action by name");
        };
        assert_eq!(intent.action, "pro.show_notes");
        assert_eq!(intent.args.get(PANE_ARG), Some(&PropValue::Int(7)));
        assert!(!resolved.needs_focus);
        assert_eq!(catalog.label("pro.show_notes"), Some("Show notes"));
    }

    /// **An action that declares no `pane` argument is not sent one** — the call would be refused as
    /// "unknown argument", which is exactly what happened to an action that did not care which pane.
    #[test]
    fn an_added_button_sends_the_pane_only_to_an_action_that_asks_for_it() {
        use crate::actions::ActionMeta;
        let mut catalog = ActionCatalog::with_builtins();
        let mut registry = crate::actions::ActionRegistry::new();
        crate::actions::register_dynamic(
            &mut registry,
            &mut catalog,
            ActionMeta::new("pro.plain").label("Plain"),
            None,
        )
        .expect("registered");
        let all = with_added("pro.plain");
        let resolved = all.get("pro.plain").unwrap().resolve(&catalog, ids());
        let ButtonEmit::Intent(intent) = resolved.emit else {
            panic!("an added button sends its action by name");
        };
        assert!(
            intent.args.is_empty(),
            "no `pane` argument declared, none sent"
        );
    }

    /// **A second button with a taken name is refused** — the first keeps it.
    #[test]
    fn a_pane_button_name_can_be_taken_only_once() {
        let mut all = with_added("pro.show_notes");
        let before = all.defs.len();
        all.add(PaneButtonDef::extension("pro.show_notes".to_string()));
        assert_eq!(all.defs.len(), before);
    }
}
