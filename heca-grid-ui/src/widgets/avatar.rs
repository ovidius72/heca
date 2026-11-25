//! [`Avatar`] — **a small tile that stands for someone or something**: a name's first letter, or a
//! picture, on a coloured box, with an optional ring round it.
//!
//! ```text
//!  ┏━━━┓
//!  ┃ B ┃      ← the letter (or a picture) on the fill; the ring is the box's edge
//!  ┗━━━┛
//! ```
//!
//! It is what a list shows for a person, a room, a project or a program, and it knows nothing about
//! any of them. The caller gives it meaning: the fill can say what kind of thing it is, the ring how
//! it is doing, the picture what it is (a program's logo, a person's photo).
//!
//! ```ignore
//! Avatar::new("bob")
//!     .tone(Tone::Muted)                       // the fill (default: the accent)
//!     .ring(Tone::Warning)                     // an edge in a tone of its own
//!     .size(WidgetSize::Caption)               // the letter and picture follow the size variant
//!
//! Avatar::new("claude")
//!     .picture(logo)                           // a Signal or Memo: the picture when there is one
//!     .ring(doing)                             // a Signal or Memo: follows it, in place
//! ```
//!
//! **What it owns:** the box — fill and ring are theme tones, the corner radius and ring width are
//! the theme's, all read at paint, so a theme reload restyles it; the letter, written in the
//! theme's colour for that fill (`on(..)`) so it reads on any fill; and the size: a square as big as
//! a line of text, the same with the letter or a picture. **What the caller owns:** the tones it picks and the signals it hands in — a ring or a
//! picture that follows a value changes the tile in place, with no rebuild.
//!
//! Why this is a widget: heca-pro drew this tile by hand for its rooms and members (a `Surface` with
//! a fill, a radius and a padding at the call site). A tile with a letter is not about rooms, so it
//! belongs here, where any list can use it.

use crate::FontWeight;
use crate::Tone;
use crate::builders::{LayoutExt, Parent, PlaceExt, PresenceExt, StyleExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx, area, paint_child};
use crate::font::MONO_LINE_RATIO;
use crate::image::ImageSource;
use crate::reactive::{Signal, SignalGet, signal};
use crate::style::Length;
use crate::widgets::{Flex, Image, Label};

/// The tile's two parts, each with its own name, one shown at a time: the letter, or the picture.
const LETTER: &str = "letter";
const PICTURE: &str = "picture";

/// A letter or a picture on a coloured box, with an optional ring — see the module docs.
pub struct Avatar {
    base: Base,
    /// The fill's tone, resolved against the theme at paint.
    tone: Tone,
    /// The ring's tone; `None` draws no ring. A signal, so a host changes it in place.
    ring: Signal<Option<Tone>>,
    /// The picture; `None` shows the letter. A signal, so a host changes it in place.
    picture: Signal<Option<ImageSource>>,
}

#[heca_grid_ui_macros::props]
impl Avatar {
    /// The tile for `name`: its first letter, upper-cased, on the theme's accent. An empty name
    /// shows an empty tile.
    pub fn new(name: impl AsRef<str>) -> Self {
        let picture: Signal<Option<ImageSource>> = signal(None);
        let mut base = Base::container();
        base.children
            .push(crate::WidgetModel::new(parts(name.as_ref(), picture)));
        // **It never gives way**, like a `StatusDot`: squeezing a tile only crushes its letter, and
        // it is already the smallest thing on a row. Text beside it is what cuts.
        // **A square as big as a line of text**, whatever it shows: the letter sits in the middle
        // and a picture fills it, so neither a loaded image's own size nor a larger font can make
        // the tile claim more room than it draws.
        let side = Length::Em(MONO_LINE_RATIO);
        Self {
            base,
            tone: Tone::Accent,
            ring: signal(None),
            picture,
        }
        .width(side)
        .height(side)
        .shrink(0.0)
    }

    /// **The fill**, in a theme tone — resolved at paint, so a theme reload restyles it. Use it for
    /// what kind of thing the tile stands for. Default: [`Tone::Accent`].
    #[heca_grid_ui_macros::prop]
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// **A ring round the tile**, in a theme tone — the box's edge, as wide as the theme's border.
    /// Use it for a second fact the fill does not carry (how the thing is doing). Takes a tone, or
    /// `None` for no ring, fixed or live (a `Signal`, a `Memo`): a live one changes the ring in
    /// place. Default: no ring.
    ///
    /// ```ignore
    /// Avatar::new("bob").ring(Tone::Warning)
    /// Avatar::new("bob").ring(report.tone())      // a Memo<Option<Tone>> the host keeps
    /// ```
    #[heca_grid_ui_macros::prop]
    pub fn ring(self, tone: impl crate::builders::IntoToneValue) -> Self {
        tone.into_tone_value().show_in(self.ring);
        self
    }

    /// **A picture instead of the letter** — an image file path or source, or `None` for the letter,
    /// fixed or live (a `Signal`, a `Memo`): a live one changes the tile in place. It fills the tile
    /// as the letter does. Default: the letter.
    ///
    /// ```ignore
    /// Avatar::new("claude").picture("icons/claude.svg")
    /// Avatar::new("bob").picture(report.picture())  // a Memo<Option<ImageSource>> the host keeps
    /// ```
    #[heca_grid_ui_macros::prop]
    pub fn picture(self, picture: impl crate::builders::IntoPictureValue) -> Self {
        picture.into_picture_value().show_in(self.picture);
        self
    }
}

/// **The two parts**: the letter, shown while there is no picture, and the picture, shown while
/// there is one. They fill the tile's square: the letter centred in it, the picture covering it.
///
/// They sit in a centring `Flex` row, not a `Grid`: one is always hidden and takes no room, so the
/// row holds exactly what is shown. Two `auto` grid columns side by side let free space leak into
/// one of them, which widened the tile and pushed the picture to one side. Each part still carries
/// a name (its area), so it is found by name, never by position.
fn parts(name: &str, picture: Signal<Option<ImageSource>>) -> Flex {
    // Nothing to load while there is no picture; the part is hidden then.
    let nothing = || ImageSource::bytes(Vec::new());
    Flex::row()
        .align("center")
        .justify("center")
        .fill_parent()
        .child(
            Label::new(initial(name))
                .font_weight(FontWeight::Bold)
                .area(LETTER)
                .hidden(move || picture.get().is_some()),
        )
        // The picture fills the tile: a logo is its own tile, as in a pane row.
        .child(
            Image::from_source(nothing())
                .source_from(move || picture.get().unwrap_or_else(nothing))
                .fill_parent()
                .area(PICTURE)
                .hidden(move || picture.get().is_none()),
        )
}

/// The first letter of a name, upper-cased.
fn initial(name: &str) -> String {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

impl Component for Avatar {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The letter — what a reader and a screen reader take the tile to be.
    fn text_summary(&self) -> Option<String> {
        self.base
            .children
            .iter()
            .find_map(|part| area(part.as_ref(), LETTER))
            .and_then(|letter| letter.text_summary())
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let (fill, ink, ring, radius) = {
            let theme = cx.theme();
            // The fill is the letter's background; a picture is its own tile and covers nothing.
            let fill = match self.picture.get_untracked() {
                Some(_) => Color::TRANSPARENT,
                None => self.tone.resolve(theme),
            };
            let ring = self.ring.get_untracked().map(|tone| tone.resolve(theme));
            // The letter is written in the colour that reads on this fill — the theme's `on(..)`,
            // as a filled `Button` writes its label — not the page's text colour, which on a
            // light fill (the accent) left a white letter on light blue.
            (
                fill,
                theme.colors.on(fill),
                ring,
                theme.colors.control_radius(),
            )
        };
        let ring = ring.and_then(|ring| cx.border(ring));
        cx.rect(self.base.bounds, fill, ring, radius, None);
        cx.with_content_color(ink, |cx| {
            for child in &self.base.children {
                paint_child(child.as_ref(), cx);
            }
        });
    }
}

impl LayoutExt for Avatar {}
impl StyleExt for Avatar {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reactive::SignalUpdate;
    use crate::scene::{DrawCommand, Scene};
    use crate::style::WidgetSize;
    use crate::widgets::Flex;
    use crate::{LayoutEngine, Size};

    fn laid(avatar: Avatar) -> Flex {
        let mut root = Flex::row()
            .width(300.0)
            .height(40.0)
            .align("center")
            .child(avatar.size(WidgetSize::Caption).area("avatar"))
            .child(Label::new("bob").grow(1.0))
            .child(Label::new("W"));
        LayoutEngine::new().compute(&mut root, Size::new(300.0, 40.0));
        root
    }

    fn shown(root: &Flex, part: &str) -> bool {
        area(root, part).is_some_and(|p| !p.base().style.layout.hidden)
    }

    #[test]
    fn the_tile_shows_the_upper_cased_first_letter_or_nothing() {
        assert_eq!(initial("pi"), "P");
        assert_eq!(initial("élan"), "É");
        assert_eq!(initial(""), "");
        assert_eq!(Avatar::new("bob").text_summary().as_deref(), Some("B"));
    }

    /// The ring follows the value it was given, and `ring(..)` sets a fixed one.
    #[test]
    fn the_ring_follows_its_signal() {
        let ring = signal(None);
        let avatar = Avatar::new("bob").ring(ring);
        assert_eq!(avatar.ring.get_untracked(), None);
        ring.set(Some(Tone::Warning));
        assert_eq!(avatar.ring.get_untracked(), Some(Tone::Warning));
        let fixed = Avatar::new("bob").ring(Tone::Success);
        assert_eq!(fixed.ring.get_untracked(), Some(Tone::Success));
    }

    /// The picture follows its signal in place: shown while there is one, the letter otherwise —
    /// and it is the same tile throughout.
    #[test]
    fn the_picture_follows_its_signal_in_place() {
        let picture = signal(None);
        let mut root = laid(Avatar::new("bob").picture(picture));
        assert!(
            shown(&root, LETTER) && !shown(&root, PICTURE),
            "the letter first"
        );
        picture.set(Some(ImageSource::bytes(b"<svg/>".to_vec())));
        LayoutEngine::new().compute(&mut root, Size::new(300.0, 40.0));
        assert!(
            !shown(&root, LETTER) && shown(&root, PICTURE),
            "then the picture"
        );
        picture.set(None);
        LayoutEngine::new().compute(&mut root, Size::new(300.0, 40.0));
        assert!(shown(&root, LETTER), "the letter again");
    }

    /// The tile is the same size with its letter and with a picture — a square as big as a line of
    /// text — so a logo arriving neither widens the tile nor sits in a corner of it.
    #[test]
    fn the_tile_is_the_same_square_with_a_letter_or_a_picture() {
        let picture = signal(None);
        let mut root = laid(Avatar::new("bob").picture(picture));
        let tile = |root: &Flex| area(root, "avatar").expect("the avatar").base().bounds;
        let with_letter = tile(&root);
        picture.set(Some(ImageSource::bytes(b"<svg/>".to_vec())));
        LayoutEngine::new().compute(&mut root, Size::new(300.0, 40.0));
        let with_picture = tile(&root);
        assert_eq!(with_letter.size, with_picture.size, "same size either way");
        // A square to the pixel: a label rounds its line to whole pixels, an `em` length does not.
        assert!(
            (with_letter.size.w - with_letter.size.h).abs() <= 1.0,
            "a square: {with_letter:?}"
        );
        let shown = area(&root, PICTURE).expect("the picture").base().bounds;
        assert_eq!(shown.size, with_picture.size, "the picture fills the tile");
        // With a picture the tile paints no fill under it: a logo is its own tile.
        let theme = crate::theme::Theme::default();
        let mut scene = Scene::new();
        paint_child(&root, &mut PaintCx::new(&mut scene, &theme));
        let accent = Tone::Accent.resolve(&theme);
        assert!(
            !scene
                .iter()
                .any(|command| matches!(command, DrawCommand::Rect(rect) if rect.fill == accent)),
            "no fill under the picture"
        );
    }

    /// In a row too narrow for everything, the tile keeps its size; the text beside it gives way.
    #[test]
    fn the_tile_never_gives_way() {
        let mut root = Flex::row()
            .width(40.0)
            .height(40.0)
            .child(Avatar::new("bob").size(WidgetSize::Caption).area("avatar"))
            .child(Label::new("a-long-name-that-cannot-fit").grow(1.0));
        LayoutEngine::new().compute(&mut root, Size::new(40.0, 40.0));
        let tile = area(&root, "avatar").expect("the avatar").base().bounds;
        let letter = area(&root, LETTER).expect("the letter").base().bounds;
        assert!(
            tile.size.w + 0.5 >= letter.size.w,
            "{tile:?} squeezed under {letter:?}"
        );
    }

    /// The letter sits in the middle of the tile.
    #[test]
    fn the_letter_sits_in_the_middle_of_the_tile() {
        let root = laid(Avatar::new("bob"));
        let letter = area(&root, LETTER).expect("the letter").base().bounds;
        let tile = area(&root, "avatar").expect("the avatar").base().bounds;
        let middle =
            |r: heca_core::layout::Rectangle| (r.loc.x + r.size.w / 2.0, r.loc.y + r.size.h / 2.0);
        let ((lx, ly), (tx, ty)) = (middle(letter), middle(tile));
        assert!(
            (lx - tx).abs() <= 1.0 && (ly - ty).abs() <= 1.0,
            "{letter:?} in {tile:?}"
        );
    }

    /// The letter is written in the colour that reads on the fill (`theme.colors.on(fill)`), not
    /// the page's text colour — a white letter on the light accent could not be read.
    #[test]
    fn the_letter_is_written_in_the_colour_that_reads_on_the_fill() {
        let theme = crate::theme::Theme::default();
        for tone in [Tone::Accent, Tone::Muted, Tone::Success] {
            let root = laid(Avatar::new("bob").tone(tone));
            let mut scene = Scene::new();
            paint_child(&root, &mut PaintCx::new(&mut scene, &theme));
            let ink = scene
                .iter()
                .find_map(|command| match command {
                    DrawCommand::Text(text) if text.text == "B" => Some(text.color),
                    _ => None,
                })
                .expect("the letter is drawn");
            assert_eq!(ink, theme.colors.on(tone.resolve(&theme)), "{tone:?}");
        }
    }
}
