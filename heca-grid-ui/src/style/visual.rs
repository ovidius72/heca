//! **The appearance half of a style.**

use super::*;

/// The **appearance** half of [`Style`] — the pixels. The [`Theme`](crate::theme::Theme) supplies
/// every default; a description may override any of it.
///
/// Unset means "ask the theme": [`fill`](Self::fill), [`border`](Self::border) and
/// [`glow`](Self::glow) are `Option`, and [`radius`](Self::radius) / [`font_size`](Self::font_size)
/// use a `0.0 = inherit` sentinel, so a widget that overrides nothing follows a theme reload.
///
/// The split with [`Layout`] groups *what the caller asked for* apart from *what the theme
/// decided*, so a reader tells arrangement from appearance at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Visual {
    pub fill: Option<Color>,
    /// **A fill named by the theme colour it follows** (`"surface"`), set by
    /// [`StyleExt::background`](crate::builders::StyleExt::background) with
    /// [`Fill::theme`](crate::style::Fill::theme) and resolved at paint by
    /// [`fill_in`](Self::fill_in), so it follows a theme reload with no rebuild. A description
    /// names a token too, but resolves it when the tree is built, so this is not serialized.
    #[serde(skip)]
    pub fill_theme: Option<ThemeFill>,
    pub border: Option<Border>,
    pub glow: Option<Glow>,
    /// **The accent this widget and everything inside it paints its chrome with** — CSS's
    /// inherited custom property, for a hue.
    ///
    /// A composition with a colour of its own — a severity-toned notification card, a pane tinted
    /// by its own frame — sets this and everything inside follows: focus rings, hover fills,
    /// selected washes, scrollbar thumbs, carets. `None` (the default, and almost everywhere) means
    /// inherit whatever an ancestor published, else the theme's accent. Read through
    /// [`PaintCx::accent`](crate::component::PaintCx::accent); applied to the whole subtree by
    /// `paint_child`, so no widget opts in and none can forget.
    ///
    /// It does **not** redefine a declared meaning: a `Badge::accent`, an `Alert::info` or a
    /// destructive button keeps the colour its variant names, exactly as it does inside any other
    /// toned container.
    ///
    /// The alternative it replaces is painting the subtree under a **swapped theme**, which needs a
    /// separate paint call per subtree and is what stops a container from painting its own children.
    pub accent: Option<Color>,
    /// **The colour of the text inside this widget, by meaning** — CSS's inherited `color`.
    ///
    /// Everything inside that paints text or a glyph without a colour of its own — a `Label`, an
    /// `Icon` — uses it; a control that colours its own content (a `Button`, a selected `Choice`)
    /// keeps its own. Named as a [`Tone`](crate::Tone) rather than a colour because a widget has no
    /// theme until paint. Applied to the subtree by `paint_child`, so nothing inside opts in.
    pub content_tone: Option<crate::Tone>,
    pub radius: f32,
    /// Explicit font size in logical px. `0.0` = inherit the theme base font.
    pub font_size: f32,
    /// Multiplier applied to the inherited base font (header ≈ 2.0, caption ≈ 0.8,
    /// body = 1.0). Ignored when [`font_size`](Self::font_size) is set explicitly.
    ///
    /// A raw multiplier, so it is host-only: the semantic route a description *can* take is
    /// [`Layout::size`] (`Small`/`Normal`/`Big`), which scales font and padding together and
    /// cascades to children.
    pub font_scale: f32,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            fill: None,
            fill_theme: None,
            border: None,
            glow: None,
            accent: None,
            content_tone: None,
            radius: 0.0,
            // 0.0 = inherit the theme's `font_size`; a widget's `.font_size(x)`
            // (x > 0) overrides it. Resolved centrally during layout.
            font_size: 0.0,
            font_scale: 1.0,
        }
    }
}

impl Visual {
    /// **The fill to paint with this frame**: the colour written, else the theme colour named,
    /// else none. Every widget that paints its own fill reads it through here.
    pub fn fill_in(&self, theme: &crate::theme::Theme) -> Option<Color> {
        self.fill
            .or_else(|| self.fill_theme.and_then(|fill| fill.resolve(theme)))
    }
}

/// **What [`background`](crate::builders::StyleExt::background) paints**: a colour, or a theme
/// colour by name — optionally at one of the theme's opacities — which follows the theme.
///
/// ```ignore
/// Surface::new().background(Fill::theme("surface"))                        // follows the theme
/// Surface::new().background(Fill::theme("background").alpha("scrim"))      // a theme veil
/// Surface::new().background(Color::rgb(0x20, 0x20, 0x28))   // this colour, whatever the theme
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fill {
    /// This colour, whatever the theme.
    Color(Color),
    /// A colour of the theme, read when it is drawn — see [`ThemeFill`].
    Theme(ThemeFill),
}

impl Fill {
    /// **The theme's colour of this name**, read when it is drawn: a field of the theme
    /// (`"surface"`, `"accent"`, `"border"`, …), and a colour the theme derives when it leaves it
    /// unset (`"top_bottom_pane_background"`) answers with what it derives. A name that is not a
    /// theme colour paints nothing. Add [`alpha`](ThemeFill::alpha) to draw it at one of the
    /// theme's opacities.
    pub const fn theme(name: &'static str) -> ThemeFill {
        ThemeFill { name, alpha: None }
    }
}

/// **A theme colour by name, at a theme opacity by name** — both read from the theme when drawn,
/// so a theme reload repaints it. Made with [`Fill::theme`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeFill {
    name: &'static str,
    alpha: Option<&'static str>,
}

impl ThemeFill {
    /// **Draw the colour at the theme's opacity of this name** — one of its interaction alphas
    /// (`"scrim"`, `"row_hover_fill"`, …). A name that is not one paints nothing.
    pub const fn alpha(self, name: &'static str) -> Self {
        Self {
            name: self.name,
            alpha: Some(name),
        }
    }

    /// The colour, from `theme` now.
    pub fn resolve(&self, theme: &crate::theme::Theme) -> Option<Color> {
        let colour = theme.colors.color_named(self.name)?;
        match self.alpha {
            None => Some(colour),
            Some(alpha) => Some(colour.with_alpha(theme.colors.alpha_named(alpha)?)),
        }
    }
}

impl Fill {
    /// The colour it paints with `theme` now — `None` for a theme name the theme does not have.
    pub fn resolve(&self, theme: &crate::theme::Theme) -> Option<Color> {
        match self {
            Fill::Color(colour) => Some(*colour),
            Fill::Theme(fill) => fill.resolve(theme),
        }
    }
}

impl From<ThemeFill> for Fill {
    fn from(fill: ThemeFill) -> Self {
        Self::Theme(fill)
    }
}

impl From<Color> for Fill {
    fn from(colour: Color) -> Self {
        Self::Color(colour)
    }
}
