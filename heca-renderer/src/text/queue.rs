//! **Asking for text**: plain, in a box, in a line box, and terminal row-runs — each becomes one
//! queued command for the next frame.

use heca_grid_ui::scene::TextAlign;

use super::{TextBox, TextGlow, TextRenderer, TextStyle};

pub(super) struct TextCommand {
    pub(super) text: String,
    /// Position: top-left `(x, y)` when `centered` is false, otherwise the box
    /// `(x, y, w, h)` the text is centered within.
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    pub(super) h: f32,
    pub(super) font_size: f32,
    pub(super) color: [f32; 4],
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) faux_italic: bool,
    pub(super) align: TextAlign,
    /// Center within `(w, h)` (grid scene), or place at `(x, y)` (app labels).
    pub(super) centered: bool,
    /// Align within a stable line box without using the generic UI centered-box path.
    pub(super) line_box: bool,
    /// Shape with the embedded icon font instead of the text family.
    pub(super) icon: bool,
    /// Override font family for this run when needed.
    pub(super) font_family: Option<String>,
    /// Clip rect (logical px) this label is scissored to, if any.
    pub(super) clip: Option<[f32; 4]>,
    /// Additive halo behind this run, or `None` for flat text. Always `None` for
    /// terminal runs: cell glyphs are the hottest path in the app and a halo would
    /// multiply their vertex count.
    pub(super) glow: Option<TextGlow>,
    /// Terminal run metadata. When set, the command is one shaped row-run of
    /// terminal cells: the shaper sees the whole run text (so OpenType ligatures
    /// like `->`/`!=` can form), and the emission snaps every glyph back to its
    /// originating cell column so the monospace grid stays exact. `None` for all
    /// UI/chrome labels (the generic align/ink-box path).
    pub(super) run: Option<TerminalRun>,
}

/// Per-run terminal layout metadata carried alongside a [`TextCommand`].
pub(super) struct TerminalRun {
    /// Logical cell width; each glyph is snapped to `run_x + col * cell_w`.
    pub(super) cell_w: f32,
    /// Per-byte column offset within the run text: `byte_cols[b]` is the cell
    /// column (relative to the run start) of the cell that byte `b` belongs to.
    /// Length equals the run text's byte length. Derived from cell display
    /// widths, so it is a pure function of the run's cells.
    pub(super) byte_cols: Vec<u16>,
    /// Per-column foreground color: `col_colors[col]` tints the glyph snapped to
    /// that column. Lets a ligature span a color boundary while each glyph keeps
    /// its own cell's color.
    pub(super) col_colors: Vec<[f32; 4]>,
    /// Whether `calt`/`liga`/`clig` are applied. `false` disables them so the run
    /// shapes character-by-character (no ligatures).
    pub(super) ligatures: bool,
}

impl TextCommand {
    /// **A command for `text` over `rect`**: upright, regular, start-aligned, placed at the box's
    /// top-left, no icon face, no halo, clipped to `clip`. Every way of queueing text starts here
    /// and says only what it changes, so a field added later is set in one place.
    fn new(
        text: &str,
        rect: TextBox,
        font_size: f32,
        color: [f32; 4],
        clip: Option<[f32; 4]>,
    ) -> Self {
        Self {
            text: text.to_string(),
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
            font_size,
            color,
            bold: false,
            italic: false,
            faux_italic: false,
            align: TextAlign::Start,
            centered: false,
            line_box: false,
            icon: false,
            font_family: None,
            clip,
            glow: None,
            run: None,
        }
    }
}

impl TextRenderer {
    /// Queue text with its top-left at `(x, y)` (logical px) — the simple
    /// point-positioned form used throughout the app (sidebar, chrome, …).
    pub fn queue_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: [f32; 4]) {
        let at = TextBox { x, y, w: 0.0, h: 0.0 };
        let cmd = TextCommand::new(text, at, font_size, color, self.current_clip);
        self.commands.push(cmd);
    }

    /// Queue text centered within the box `(x, y, w, h)` (logical px):
    /// horizontally per `align`, always centered vertically. Used by the grid
    /// scene renderer ([`crate::scene`]).
    #[expect(
        clippy::too_many_arguments,
        reason = "Public text-box API keeps geometry, styling, and alignment explicit for renderer call sites."
    )]
    pub fn queue_text_in_box(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        font_size: f32,
        color: [f32; 4],
        bold: bool,
        italic: bool,
        align: TextAlign,
        icon: bool,
        // Family override for this run — `None` shapes with the theme's text family. The Nerd Font
        // role arrives here (see `scene.rs`); the terminal path already used the same field.
        family: Option<&str>,
        glow: Option<TextGlow>,
    ) {
        let rect = TextBox { x, y, w, h };
        self.commands.push(TextCommand {
            bold,
            // Italic is the **synthesized oblique**, not `Style::Italic`. The embedded UI family
            // (Geist Mono) ships no italic face: asking cosmic-text for one would either render
            // upright or substitute a *proportional* fallback, and a proportional fallback breaks
            // the monospace advances every grid-ui measure assumes. The shear keeps the same face —
            // same glyphs, same widths, just slanted.
            faux_italic: italic,
            align,
            centered: true,
            icon,
            font_family: family.map(str::to_string),
            glow,
            ..TextCommand::new(text, rect, font_size, color, self.current_clip)
        });
    }

    pub fn queue_text_in_line_box_with_style(
        &mut self,
        text: &str,
        rect: TextBox,
        font_size: f32,
        style: TextStyle<'_>,
        align: TextAlign,
        icon: bool,
    ) {
        self.commands.push(TextCommand {
            bold: style.bold,
            italic: style.italic,
            faux_italic: style.faux_italic,
            align,
            line_box: true,
            icon,
            font_family: style.font_family.map(str::to_string),
            ..TextCommand::new(text, rect, font_size, style.color, self.current_clip)
        });
    }

    /// Queue one shaped terminal row-run: consecutive same-style cells whose text
    /// is shaped together so OpenType ligatures/contextual alternates can form.
    /// `cell_w` (logical) and `byte_cols` (per-byte cell-column offsets) let the
    /// emission snap every glyph back to its originating cell column, preserving
    /// the monospace grid even when a ligature collapses several cells into one
    /// glyph. `rect.x`/`rect.y` are the run origin; `rect.h` is the cell height
    /// used for vertical centering. Used only by the terminal text path.
    #[expect(
        clippy::too_many_arguments,
        reason = "Terminal run path threads grid metadata (cell width, per-byte columns, per-column colors, ligature flag) explicitly."
    )]
    pub fn queue_terminal_run(
        &mut self,
        text: &str,
        rect: TextBox,
        font_size: f32,
        style: TextStyle<'_>,
        cell_w: f32,
        byte_cols: &[u16],
        col_colors: &[[f32; 4]],
        ligatures: bool,
    ) {
        self.commands.push(TextCommand {
            bold: style.bold,
            italic: style.italic,
            faux_italic: style.faux_italic,
            line_box: true,
            font_family: style.font_family.map(str::to_string),
            // No halo, ever (the default): cell glyphs are the hottest path in the app (thousands
            // per frame) and a halo multiplies their vertex count.
            run: Some(TerminalRun {
                cell_w,
                byte_cols: byte_cols.to_vec(),
                col_colors: col_colors.to_vec(),
                ligatures,
            }),
            ..TextCommand::new(text, rect, font_size, style.color, self.current_clip)
        });
    }
}
