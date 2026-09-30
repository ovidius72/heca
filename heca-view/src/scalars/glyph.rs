//! The icon names.

use crate::PropValue;

// ── The icon names ─────────────────────────────────────────────────────────────────────────
//
// Mirrored from `heca_grid_ui::Glyph`. This crate cannot depend on the widget library (that is the
// point of it), so the list is a copy — held honest by `every_glyph_name_has_a_mirror` in
// `heca-view-realize`, which compares it against `Glyph::VARIANT_NAMES` in both directions and
// fails, naming what is missing, the moment an icon is added there. To add one: a single line in
// the list below.

named_set! {
    /// A Phosphor icon, by name — mirrors `heca_grid_ui::Glyph`.
    pub enum ViewGlyph {
        Folder => "folder",
        FolderOpen => "folder_open",
        File => "file",
        FileCode => "file_code",
        GitBranch => "git_branch",
        GitCommit => "git_commit",
        GitMerge => "git_merge",
        GitPullRequest => "git_pull_request",
        Terminal => "terminal",
        Gear => "gear",
        Search => "search",
        Close => "close",
        Check => "check",
        CaretRight => "caret_right",
        CaretLeft => "caret_left",
        CaretDown => "caret_down",
        CaretUp => "caret_up",
        Play => "play",
        Pause => "pause",
        Stop => "stop",
        Warning => "warning",
        WarningCircle => "warning_circle",
        Info => "info",
        Circle => "circle",
        Lightning => "lightning",
        List => "list",
        Sidebar => "sidebar",
        DotsThreeVertical => "dots_three_vertical",
        ArrowRight => "arrow_right",
        ArrowLineLeft => "arrow_line_left",
        ArrowLineRight => "arrow_line_right",
        Plus => "plus",
        Minus => "minus",
        SquareSplitVertical => "square_split_vertical",
        XSquare => "x_square",
        FrameCorners => "frame_corners",
        Cards => "cards",
        Pencil => "pencil",
        NotePencil => "note_pencil",
        Backspace => "backspace",
        Trash => "trash",
        XCircle => "x_circle",
        PlusCircle => "plus_circle",
        FolderSimpleMinus => "folder_simple_minus",
        FolderSimplePlus => "folder_simple_plus",
        StackPlus => "stack_plus",
        StackMinus => "stack_minus",
        ColumnsPlusLeft => "columns_plus_left",
        ColumnsPlusRight => "columns_plus_right",
        SquareHalf => "square_half",
        SquareSplitHorizontal => "square_split_horizontal",
        SquareHalfBottom => "square_half_bottom",
    }
}

value_set! {
    /// The **keyboard** glyphs, from the embedded Nerd Font — a separate vocabulary from
    /// [`ViewGlyph`] because it is a separate font (F003/P097/T501).
    ///
    /// These are the keys a shortcut is written with: ⇧ ⌃ ⌥ ⌘, Enter, Escape, Tab, Space, Backspace and
    /// the four arrows. A description names one and the host resolves it against the font, exactly as
    /// it does an icon name — so a plugin can render a keybinding the way heca's own key hints do
    /// instead of typing a character that its user's font may not have.
    ///
    /// Held honest by `every_glyph_name_has_a_mirror`, which compares both vocabularies against the
    /// library's own in both directions.
    pub enum ViewNfGlyph {
        Shift => "shift",
        Control => "control",
        Option => "option",
        Command => "command",
        CapsLock => "caps_lock",
        Enter => "enter",
        Escape => "escape",
        Tab => "tab",
        Space => "space",
        Backspace => "backspace",
        ArrowUp => "arrow_up",
        ArrowDown => "arrow_down",
        ArrowLeft => "arrow_left",
        ArrowRight => "arrow_right",
    }
}

/// A glyph becomes [`PropValue::Glyph`], not `Text`: that variant already exists and says what the
/// string is, so `realize` resolves it against the icon font rather than guessing.
impl From<ViewGlyph> for PropValue {
    fn from(g: ViewGlyph) -> Self {
        PropValue::Glyph(g.name().to_string())
    }
}
