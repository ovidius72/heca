//! A command and the data its palette row presents.

use super::{Glyph, KeyCap};

/// One command in a [`CommandPalette`](super::CommandPalette).
pub struct Command {
    /// Stable identity, for ranking by past use. **Optional**: a command without one matches and
    /// sorts normally and simply carries no frecency boost. The label cannot serve — a host prefixes
    /// it with the owning component's title, so it changes with mounting.
    pub(super) id: Option<String>,
    /// Which block this command belongs to while **nothing is typed** — lower sorts first. The
    /// caller's policy, applied only in the browsing case; once there is a query the match decides
    /// and the blocks dissolve.
    pub(super) group: u16,
    pub(super) label: String,
    pub(super) description: Option<String>,
    pub(super) description_path: bool,
    pub(super) detail: Option<String>,
    pub(super) icon: Option<Glyph>,
    /// One entry per **binding**, each a chord of caps (`[λ] [⇧] [e]`). Several bindings stack, one
    /// per line — an action bound twice really is bound twice, and joining them into `"λ h / λ ←"`
    /// was both wider than the label and impossible to draw as chips.
    pub(super) chords: Vec<Vec<KeyCap>>,
    /// Is this the row the user is already on? Drawn in the accent so it can be found at a glance.
    /// Distinct from the *selection*, which is where the keyboard is right now.
    pub(super) current: bool,
    /// Should the selection **start** here when the palette opens with nothing typed?
    pub(super) preselect: bool,
    /// Which mode lists this command, named by its **search scope** — the same vocabulary the
    /// palette's sigil table and the host's `clear_search_history scope=…` already use, rather than
    /// a second spelling of the same thing. `None` means the default mode, which is what every
    /// command written before modes existed says.
    pub(super) mode: Option<String>,
    pub(super) on_run: Box<dyn Fn()>,
}

#[heca_grid_ui_macros::props]
impl Command {
    /// A command with `label` that runs `on_run` when selected.
    pub fn new(label: impl Into<String>, on_run: impl Fn() + 'static) -> Self {
        Self {
            id: None,
            group: 0,
            label: label.into(),
            description: None,
            description_path: false,
            detail: None,
            icon: None,
            chords: Vec::new(),
            current: false,
            preselect: false,
            mode: None,
            on_run: Box::new(on_run),
        }
    }

    /// Mark this row as **the one the user is already on** — the focused pane, the active
    /// workspace. It is drawn in the accent, so you can see where you are in a list of near-identical
    /// names before choosing where to go.
    ///
    /// Not the same thing as the selection: the selection is where the keyboard is and moves as you
    /// type, while this does not move at all. A row can be both, and then it simply reads as
    /// selected — you are on it *and* pointing at it, and there is nothing further to distinguish.
    #[heca_grid_ui_macros::prop]
    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    /// Start the selection on this row, when the palette opens with **nothing typed**.
    ///
    /// For a picker, that is the row that makes the default action worth pressing: the pane you
    /// were last in, not the one you are in — landing on the latter would make Enter focus what is
    /// already focused. Deliberately separate from [`current`](Self::current), which says where you
    /// *are*; the two are different rows and each wants its own answer.
    ///
    /// It applies only to an untyped list, because a query is better context than any default: the
    /// first keystroke hands the lead back to the best match. If several rows claim it the first in
    /// the list wins, since a "default" there can be more than one of is not a default.
    #[heca_grid_ui_macros::prop]
    pub fn preselect(mut self, preselect: bool) -> Self {
        self.preselect = preselect;
        self
    }

    /// Which mode this command belongs to, named by the **scope** the palette declared for it
    /// ([`CommandPalette::mode`](super::CommandPalette::mode)). Unset, it belongs to the default mode.
    ///
    /// The scope name and not the sigil: the sigil is how a *user* reaches a mode and lives in the
    /// palette's table, while the scope is what the memory is filed under — one of them is an
    /// input convention and the other is an identity, and a command declares the identity.
    #[heca_grid_ui_macros::prop]
    pub fn mode(mut self, scope: impl Into<String>) -> Self {
        self.mode = Some(scope.into());
        self
    }

    /// An optional second line, muted — what the command does.
    ///
    /// Filtering still matches the **label** only: the description explains a command the user has
    /// already found, and matching it would rank a command whose label the query never mentioned.
    #[heca_grid_ui_macros::prop]
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self.description_path = false;
        self
    }

    /// A single-line path below the title. Preserve its tail and expose the full value in a tooltip.
    #[heca_grid_ui_macros::prop]
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.description = Some(path.into());
        self.description_path = true;
        self
    }

    /// A separate single-line metadata row below the description or path.
    #[heca_grid_ui_macros::prop]
    pub fn detail(mut self, text: impl Into<String>) -> Self {
        self.detail = Some(text.into());
        self
    }

    /// A stable identity, so past choices can rank this command. Usually the action's name.
    ///
    /// Optional by design: `rank` takes `Option<&str>`, so a command without an id still matches and
    /// sorts — it just carries no boost. Nothing is forced to grow an identity to be searchable.
    #[heca_grid_ui_macros::prop]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Which block this command sorts into **while the query is empty** — lower first.
    ///
    /// The one thing a caller can say about ordering, and it is deliberately confined to the
    /// browsing case: with nothing typed the only context available is whatever the caller knows
    /// (the command palette leads with the focused component's actions), but the moment something is
    /// typed the query is better context than that and the blocks are dropped.
    #[heca_grid_ui_macros::prop]
    pub fn group(mut self, group: u16) -> Self {
        self.group = group;
        self
    }

    /// An optional leading icon.
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Add one **binding**, as the caps of its chord — `[Text("λ"), Nf(Shift), Text("e")]` draws
    /// `[λ] [⇧] [e]`, right-aligned in the list's reserved shortcut column.
    ///
    /// Call it once per binding: a second call stacks a second row of chips under the first, rather
    /// than replacing it or running the two together on one line.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar")]
    pub fn keys(mut self, chord: impl IntoIterator<Item = KeyCap>) -> Self {
        let chord: Vec<KeyCap> = chord.into_iter().collect();
        if !chord.is_empty() {
            self.chords.push(chord);
        }
        self
    }
}
