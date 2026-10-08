//! **Pane header chips, by name** — the words on the left of a pane's header (`~/projects/heca`,
//! `nvim`, the git branch), each a *chip*: an icon and some text, or nothing.
//!
//! A chip is a function of **facts about its pane** ([`PaneFacts`]), returning what to show
//! ([`PaneChip`]) or `None` when it has nothing to say (git outside a repository). heca's own five
//! are registered here through the same [`PaneChipDef`] another crate uses — the first users of the
//! registry, not a special case — and config lists them by name.
//!
//! *Client side*: a chip is drawn, so it lives in a window. What it reads is plain data the window
//! is handed; after the server/client split that is what the server sends.
//!
//! **An added chip runs again when its pane's facts change, not every frame.** heca's own five are
//! read straight off the facts and are cheap; an added one is arbitrary code, so its answer is kept
//! against the facts it was worked out from and reused until they differ.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use heca_core::layout::PaneId;
use heca_core::runtime::GitInfo;
use heca_grid_ui::widgets::Glyph;

use crate::chrome::{StartupQueue, warn_author};

/// **What is true of one pane right now** — everything a chip is worked out from.
///
/// `#[non_exhaustive]`, so facts can be added as named fields — an agent's status is the next — and
/// no chip written against today's fields breaks. Read fields by name; never construct one outside
/// heca (a chip is handed one).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PaneFacts {
    /// Which pane. Stable for the pane's life, and the key an added chip's answer is kept under.
    pub pane_id: PaneId,
    /// The program's icon, from the program catalog.
    pub icon: Glyph,
    /// The pane's own name: its rename when it has one, else the program's name.
    pub title: String,
    /// The program running in it — always the process, never a rename.
    pub app_name: String,
    /// Its working directory, when known.
    pub cwd: Option<PathBuf>,
    /// The git state of that directory; `None` outside a repository.
    pub git: Option<GitInfo>,
}

impl PaneFacts {
    /// The facts about a pane, from what the host knows of it.
    pub(crate) fn of(
        pane_id: PaneId,
        programs: &heca_config::programs::ProgramsConfig,
        fallback_name: &str,
        custom_name: Option<&str>,
        runtime: Option<&heca_core::runtime::PaneRuntime>,
    ) -> Self {
        // The header never shows the `(process)` suffix — that is a sidebar-card affordance.
        let view =
            crate::chrome::pane_info_view(programs, fallback_name, custom_name, runtime, false);
        Self {
            pane_id,
            icon: view.icon,
            title: view.title,
            app_name: view.app_name,
            cwd: runtime.and_then(|r| r.cwd.clone()),
            git: runtime.and_then(|r| r.git.clone()),
        }
    }
}

/// **What a chip shows**: an icon and some text.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PaneChip {
    pub icon: Glyph,
    pub text: String,
}

impl PaneChip {
    /// A chip showing `text` beside `icon`.
    pub fn new(icon: Glyph, text: impl Into<String>) -> Self {
        Self {
            icon,
            text: text.into(),
        }
    }
}

/// What a chip does to fit when the header is too narrow. Its own property, so the header keeps no
/// list of which chip is the long one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fit {
    /// Left as it is.
    Keep,
    /// The text is a path: shorten it from the left (`…/heca/src`) by the overflow.
    PathLeft,
}

type Produce = dyn Fn(&PaneFacts) -> Option<PaneChip>;

/// One chip, by name.
#[derive(Clone)]
pub(crate) struct PaneChipDef {
    /// What config lists: `location`, `pro.agent`.
    pub(crate) name: String,
    pub(crate) fit: Fit,
    produce: Rc<Produce>,
    /// Added by another crate rather than shipped with heca.
    pub(crate) from_extension: bool,
}

impl super::listing::Named for PaneChipDef {
    fn name(&self) -> &str {
        &self.name
    }
    fn is_added(&self) -> bool {
        self.from_extension
    }
}

thread_local! {
    /// Chips added before the app took them.
    static QUEUE: StartupQueue<(String, Rc<Produce>)> = const { StartupQueue::new() };
}

/// Queue the chip `name` (`<extension>.<short>`) — called by `Extension::pane_chip`. The app takes the
/// queue once, as it starts; a call after that is said and dropped.
pub(crate) fn add_extension_chip(
    name: String,
    produce: impl Fn(&PaneFacts) -> Option<PaneChip> + 'static,
) {
    if let Err((name, _)) = QUEUE.with(|q| q.add((name, Rc::new(produce)))) {
        warn_author(format!(
            "[heca] pane chip '{name}' was added after the app started, so it does nothing — add it \
             before `heca::run()`"
        ));
    }
}

/// An added chip's kept answers: per pane and chip, the facts it came from and what it said.
type Kept = HashMap<(PaneId, String), (PaneFacts, Option<PaneChip>)>;

/// **Every pane chip there is**, by name: heca's own and the ones other crates added.
///
/// A handle: a clone is the same registry, and the answers kept are shared by every clone.
#[derive(Clone)]
pub(crate) struct PaneChips {
    defs: Rc<Vec<PaneChipDef>>,
    /// An added chip's last answer, kept against the facts it came from — see the module docs.
    kept: Rc<RefCell<Kept>>,
}

impl Default for PaneChips {
    /// heca's own chips only — what a test wants.
    fn default() -> Self {
        Self {
            defs: Rc::new(builtin_chips()),
            kept: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}

impl PaneChips {
    /// heca's own, then everything queued. Takes the queue and closes it: the app calls this once.
    pub(crate) fn from_startup() -> Self {
        let mut all = Self::default();
        for (name, produce) in QUEUE.with(StartupQueue::take) {
            all.add(PaneChipDef {
                name,
                fit: Fit::Keep,
                produce,
                from_extension: true,
            });
        }
        all
    }

    /// Add one. A name already taken is refused and said — the first keeps it.
    fn add(&mut self, def: PaneChipDef) {
        super::listing::add_unique(Rc::make_mut(&mut self.defs), "pane chip", def);
    }

    #[cfg(test)]
    pub(crate) fn get(&self, name: &str) -> Option<&PaneChipDef> {
        self.defs.iter().find(|d| d.name == name)
    }

    /// **The chips to show, in order** — the user's list by name, with what other crates added after
    /// it while that list is still the default (see [`listing`](super::listing)).
    pub(crate) fn shown(&self, config: &[String]) -> Vec<&PaneChipDef> {
        super::listing::shown(
            &self.defs,
            config,
            &heca_config::appearance::default_pane_title_segments(),
        )
    }

    /// Say what the user's list gets wrong or misses — the shared rules of
    /// [`listing::report`](super::listing::report).
    pub(crate) fn report(&self, config: &[String]) {
        super::listing::report(
            "pane chip",
            "title_segments",
            &self.defs,
            config,
            &heca_config::appearance::default_pane_title_segments(),
        );
    }

    /// **What `def` shows for the pane these facts are about**, or `None`.
    ///
    /// heca's own are worked out on the spot. An added one is worked out again only when the facts
    /// differ from the ones its last answer came from.
    pub(crate) fn chip(&self, def: &PaneChipDef, facts: &PaneFacts) -> Option<PaneChip> {
        if !def.from_extension {
            return (def.produce)(facts);
        }
        let key = (facts.pane_id, def.name.clone());
        if let Some((seen, answer)) = self.kept.borrow().get(&key)
            && seen == facts
        {
            return answer.clone();
        }
        let answer = (def.produce)(facts);
        self.kept
            .borrow_mut()
            .insert(key, (facts.clone(), answer.clone()));
        answer
    }

    /// Forget the answers kept for panes that are gone.
    pub(crate) fn retain_panes(&self, live: &HashSet<PaneId>) {
        self.kept
            .borrow_mut()
            .retain(|(pane, _), _| live.contains(pane));
    }
}

/// heca's own five, registered exactly as another crate's would be.
fn builtin_chips() -> Vec<PaneChipDef> {
    let own = |name: &str, fit: Fit, produce: fn(&PaneFacts) -> Option<PaneChip>| PaneChipDef {
        name: name.to_string(),
        fit,
        produce: Rc::new(produce),
        from_extension: false,
    };
    vec![
        // The working directory, home-relative. The long one, so it is what gives way.
        own("location", Fit::PathLeft, |f| {
            f.cwd
                .as_ref()
                .map(|cwd| PaneChip::new(Glyph::Folder, crate::chrome::home_relative_path(cwd)))
        }),
        // Always the running program's name, never a rename, so the bar keeps showing what runs.
        own("app_name", Fit::Keep, |f| {
            Some(PaneChip::new(f.icon, f.app_name.clone()))
        }),
        // The pane's own name: a rename wins, else the program.
        own("pane_name", Fit::Keep, |f| {
            (!f.title.is_empty()).then(|| PaneChip::new(f.icon, f.title.clone()))
        }),
        own("git_branch", Fit::Keep, |f| {
            f.git
                .as_ref()
                .and_then(|g| g.branch.clone())
                .map(|branch| PaneChip::new(Glyph::GitBranch, branch))
        }),
        // `+A ~M -D`, and nothing at all when the tree is clean.
        own("git_status", Fit::Keep, |f| {
            let g = f.git.as_ref()?;
            let mut parts = Vec::new();
            if g.added > 0 {
                parts.push(format!("+{}", g.added));
            }
            if g.modified > 0 {
                parts.push(format!("~{}", g.modified));
            }
            if g.deleted > 0 {
                parts.push(format!("-{}", g.deleted));
            }
            (!parts.is_empty()).then(|| PaneChip::new(Glyph::GitCommit, parts.join(" ")))
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_config::programs::ProgramsConfig;
    use heca_core::runtime::{PaneRuntime, ProcessStatus};

    fn facts(rt: &PaneRuntime, custom: Option<&str>) -> PaneFacts {
        PaneFacts::of(
            PaneId(1),
            &ProgramsConfig::default(),
            "shell",
            custom,
            Some(rt),
        )
    }

    fn runtime() -> PaneRuntime {
        PaneRuntime {
            program: Some("nvim".into()),
            status: ProcessStatus::Running,
            cwd: Some(PathBuf::from("/tmp/project")),
            git: Some(GitInfo {
                branch: Some("main".into()),
                added: 2,
                modified: 1,
                ..GitInfo::default()
            }),
            ..PaneRuntime::default()
        }
    }

    fn say(chips: &PaneChips, name: &str, facts: &PaneFacts) -> Option<String> {
        chips
            .chip(chips.get(name).expect("shipped"), facts)
            .map(|c| c.text)
    }

    /// **heca's own five chips say what they always did**, read off the pane's facts.
    #[test]
    fn the_shipped_chips_say_what_they_always_did() {
        let chips = PaneChips::default();
        let f = facts(&runtime(), None);
        assert_eq!(say(&chips, "location", &f).as_deref(), Some("/tmp/project"));
        assert_eq!(say(&chips, "git_branch", &f).as_deref(), Some("main"));
        assert_eq!(say(&chips, "git_status", &f).as_deref(), Some("+2 ~1"));
        assert_eq!(
            say(&chips, "app_name", &f).as_deref(),
            f.app_name.as_str().into(),
            "the running program"
        );
    }

    /// **A chip with nothing to say is skipped** — no empty pill: no git outside a repository, no
    /// status when the tree is clean.
    #[test]
    fn a_chip_with_nothing_to_say_says_nothing() {
        let chips = PaneChips::default();
        let mut rt = runtime();
        rt.git = None;
        rt.cwd = None;
        let f = facts(&rt, None);
        for name in ["location", "git_branch", "git_status"] {
            assert_eq!(say(&chips, name, &f), None, "{name}");
        }
        rt.git = Some(GitInfo {
            branch: Some("main".into()),
            ..GitInfo::default()
        });
        assert_eq!(
            say(&chips, "git_status", &facts(&rt, None)),
            None,
            "a clean tree"
        );
    }

    /// **`pane_name` follows a rename; `app_name` never does.** A renamed pane still shows what runs
    /// in it.
    #[test]
    fn pane_name_follows_a_rename_and_app_name_does_not() {
        let chips = PaneChips::default();
        let f = facts(&runtime(), Some("Editor"));
        assert_eq!(say(&chips, "pane_name", &f).as_deref(), Some("Editor"));
        assert_ne!(say(&chips, "app_name", &f).as_deref(), Some("Editor"));
        // Not renamed: it falls back to the program's name, never empty.
        let plain = facts(&runtime(), None);
        assert!(say(&chips, "pane_name", &plain).is_some());
    }

    /// **The user's list decides which chips show and in what order**; a name nothing provides is
    /// skipped.
    #[test]
    fn the_users_list_decides_which_chips_show() {
        let chips = PaneChips::default();
        let names = |shown: Vec<&PaneChipDef>| -> Vec<String> {
            shown.into_iter().map(|d| d.name.clone()).collect()
        };
        assert_eq!(
            names(chips.shown(&["git_branch".into(), "location".into()])),
            ["git_branch", "location"]
        );
        assert!(chips.shown(&[]).is_empty(), "an empty list hides the side");
        assert_eq!(
            names(chips.shown(&["nothing.here".into(), "app_name".into()])),
            ["app_name"]
        );
    }

    fn with_added(produce: impl Fn(&PaneFacts) -> Option<PaneChip> + 'static) -> PaneChips {
        let mut all = PaneChips::default();
        all.add(PaneChipDef {
            name: "pro.status".into(),
            fit: Fit::Keep,
            produce: Rc::new(produce),
            from_extension: true,
        });
        all
    }

    /// **An added chip shows by default — only while the list is still the default.**
    #[test]
    fn an_added_chip_shows_while_the_list_is_the_default_and_not_after() {
        let all = with_added(|_| None);
        let default = heca_config::appearance::default_pane_title_segments();
        let shown: Vec<_> = all.shown(&default).iter().map(|d| d.name.clone()).collect();
        assert_eq!(shown, ["location", "app_name", "pro.status"]);
        let own = vec!["app_name".to_string()];
        let shown: Vec<_> = all.shown(&own).iter().map(|d| d.name.clone()).collect();
        assert_eq!(shown, ["app_name"], "the user's own list is the whole list");
    }

    /// **An added chip runs again when its pane's facts change — not every frame.** It is arbitrary
    /// code, so its answer is kept against the facts it came from.
    #[test]
    fn an_added_chip_runs_again_only_when_the_facts_change() {
        let runs = Rc::new(std::cell::Cell::new(0));
        let counted = runs.clone();
        let all = with_added(move |f| {
            counted.set(counted.get() + 1);
            Some(PaneChip::new(Glyph::Terminal, f.app_name.clone()))
        });
        let def = all.get("pro.status").unwrap();
        let f = facts(&runtime(), None);

        for _ in 0..5 {
            all.chip(def, &f);
        }
        assert_eq!(runs.get(), 1, "five frames, the same facts, one run");

        let mut changed = runtime();
        changed.program = Some("zsh".into());
        all.chip(def, &facts(&changed, None));
        assert_eq!(runs.get(), 2, "the facts changed, so it runs again");
    }

    /// **Answers kept for a pane that is gone are dropped.**
    #[test]
    fn kept_answers_do_not_outlive_their_pane() {
        let all = with_added(|_| Some(PaneChip::new(Glyph::Terminal, "x")));
        let def = all.get("pro.status").unwrap();
        all.chip(def, &facts(&runtime(), None));
        assert_eq!(all.kept.borrow().len(), 1);
        all.retain_panes(&HashSet::new());
        assert!(all.kept.borrow().is_empty());
    }
}
