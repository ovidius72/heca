//! Declared palette modes and their search scopes.

use super::*;

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
