//! Declared palette modes and their search scopes.

use super::*;

/// A callback waiting for its mode to be entered ([`CommandPalette::on_mode_enter`]).
pub(super) struct ModeEnter {
    pub(super) scope: String,
    pub(super) callback: std::rc::Rc<dyn Fn()>,
    /// Whether it has run for the mode the palette is in now.
    pub(super) announced: Cell<bool>,
}

impl CommandPalette {
    /// Declare a mode and all its commands together.
    ///
    /// The palette assigns `scope` to every command, so each row needs no separate
    /// [`Command::mode`] call. The first declared mode remains the default.
    ///
    /// ```
    /// use heca_grid_ui::{Command, CommandPalette};
    /// let palette = CommandPalette::new().mode_with_commands('#', "session", [
    ///     Command::new("My project", || {}),
    /// ]);
    /// ```
    #[heca_grid_ui_macros::host_only("a mode and its command collection, not a scalar")]
    pub fn mode_with_commands(
        mut self,
        sigil: char,
        scope: impl Into<String>,
        commands: impl IntoIterator<Item = Command>,
    ) -> Self {
        let scope = scope.into();
        self = self.mode(sigil, scope.clone());
        for command in commands {
            self = self.command(command.mode(scope.clone()));
        }
        self.sync_marks();
        self.sync_initial_selection();
        self.sync_wrap();
        self
    }

    /// Prefill a query in a declared mode, using its registered sigil.
    ///
    /// `None` or an unknown scope keeps `text` unchanged. This only composes the
    /// query passed to [`Self::query`]; typing the same sigil follows the same path.
    /// Existing sigils in `text` are kept, just as when the user types them.
    /// The mode may be declared before or after this builder. Calling
    /// [`Self::query`] replaces the named prefill.
    ///
    /// ```
    /// use heca_grid_ui::{Command, CommandPalette};
    /// let palette = CommandPalette::new()
    ///     .mode_with_commands('@', "pane", [Command::new("nvim", || {})])
    ///     .query_in(Some("pane"), "nv")
    ///     .default_open(true);
    /// ```
    #[heca_grid_ui_macros::host_only("a mode name and query text, not a scalar")]
    pub fn query_in(self, scope: Option<&str>, text: impl Into<String>) -> Self {
        let text = text.into();
        let sigil = scope.and_then(|scope| {
            self.modes
                .iter()
                .find(|(_, name)| name == scope)
                .map(|(sigil, _)| *sigil)
        });
        let mut palette = self.query(Self::with_sigil(sigil, &text));
        palette.named_prefill = scope.map(|scope| (scope.to_string(), text));
        palette
    }

    /// Declare a mode: typing `sigil` as the query's first character switches the list to the
    /// commands of `scope`, and the palette's memory to that scope's.
    ///
    /// **The first mode declared is the default** — where an unsigiled query lands — so give it a
    /// sigil too and it can be returned to explicitly. Declare none and the palette has exactly one
    /// mode, [`search`](Self::search)'s scope, with no sigil to type.
    ///
    /// The commands of every mode stay in **one flat list**, filtered: their text is built as real
    /// children indexed by position, so a list per mode would mean re-indexing them on each switch.
    #[heca_grid_ui_macros::host_only("a sigil paired with a scope, not a scalar")]
    pub fn mode(mut self, sigil: char, scope: impl Into<String>) -> Self {
        self.modes.push((sigil, scope.into()));
        if let Some((scope, text)) = self.named_prefill.take() {
            self = self.query_in(Some(&scope), text);
        }
        self
    }

    /// Also match the **description** of this mode's rows, not only their title.
    ///
    /// Off by default, so a mode whose descriptions only explain a row already found (the command
    /// list) is still searched by name. Turn it on for a mode whose second line is a thing a user
    /// remembers — a session's folder, a notification's body. A title match still ranks above a
    /// description-only match, and the highlight is drawn on whichever text matched
    /// ([`SearchModel::rank_with_secondary`]).
    #[heca_grid_ui_macros::host_only("a scope name, not a scalar")]
    pub fn match_description_in(mut self, scope: impl Into<String>) -> Self {
        let scope = scope.into();
        if !self.describe_scopes.contains(&scope) {
            self.describe_scopes.push(scope);
        }
        self.sync_marks();
        self
    }

    /// Run `callback` whenever the palette **enters** the mode `scope` while it is open — when it
    /// opens in it (a prefilled query, a named mode) and when the user types its sigil.
    ///
    /// For a mode whose rows are not known until asked for: the host starts the request here, so
    /// there is one place that knows the mode was entered, whatever route led to it. Entering is
    /// announced once; leaving the mode and coming back announces it again, and so does reopening.
    #[heca_grid_ui_macros::host_only("a callback, not a scalar")]
    pub fn on_mode_enter(
        mut self,
        scope: impl Into<String>,
        callback: impl Fn() + 'static,
    ) -> Self {
        self.mode_enter.push(ModeEnter {
            scope: scope.into(),
            callback: std::rc::Rc::new(callback),
            announced: Cell::new(false),
        });
        self.announce_mode();
        self
    }

    /// Tell `mode_enter` which mode the open palette is now in, if it just changed.
    pub(super) fn announce_mode(&self) {
        if !self.is_open() {
            return;
        }
        let (_, scope, _) = self.active();
        if self.entered.borrow().as_deref() != Some(scope.as_str()) {
            // A different mode: every callback may announce again the next time its own is entered.
            *self.entered.borrow_mut() = Some(scope.clone());
            self.mode_enter
                .iter()
                .for_each(|enter| enter.announced.set(false));
        }
        // One at a time and only the ones not yet announced, so a callback registered after the
        // mode was entered still hears about it, and none hears twice.
        for enter in self
            .mode_enter
            .iter()
            .filter(|enter| enter.scope == scope && !enter.announced.get())
        {
            enter.announced.set(true);
            (enter.callback)();
        }
    }

    /// The scope of the default mode — the first declared, or the model's own when none are.
    pub(super) fn default_scope(&self) -> &str {
        match self.modes.first() {
            Some((_, scope)) => scope.as_str(),
            None => self.search.scope(),
        }
    }

    /// Every scope this palette can be in, default first.
    pub(super) fn scopes(&self) -> Vec<String> {
        match self.modes.is_empty() {
            true => vec![self.search.scope().to_string()],
            false => self.modes.iter().map(|(_, s)| s.clone()).collect(),
        }
    }

    /// Which mode the query is in: the **sigil actually typed** (`None` when the default mode was
    /// reached without one), its scope, and the query with that sigil removed.
    ///
    /// The sigil is read off the first character and nothing else — a leading character that is not
    /// in the table is not a sigil, it is the first letter of a search, and is matched as one.
    pub(super) fn active(&self) -> (Option<char>, String, String) {
        let raw = self.query_text();
        let mut rest = raw.chars();
        if let Some(first) = rest.next()
            && let Some((sigil, scope)) = self.modes.iter().find(|(s, _)| *s == first)
        {
            return (Some(*sigil), scope.clone(), rest.as_str().to_string());
        }
        (None, self.default_scope().to_string(), raw)
    }

    /// Does `cmd` belong to `scope`? A command that named no mode belongs to the default one.
    pub(super) fn in_mode(&self, cmd: &Command, scope: &str) -> bool {
        match &cmd.mode {
            Some(mode) => mode == scope,
            None => scope == self.default_scope(),
        }
    }

    /// Put the active sigil back in front of `text` — what a history recall produces, since the
    /// history stores the **effective** query and the field shows the whole thing.
    pub(super) fn with_sigil(sigil: Option<char>, text: &str) -> String {
        match sigil {
            Some(s) => format!("{s}{text}"),
            None => text.to_string(),
        }
    }
}
