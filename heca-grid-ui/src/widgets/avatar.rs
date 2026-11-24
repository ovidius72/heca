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
//! let logo = signal(None);                     // later: logo.set(Some(ImageSource::file(path)))
//! Avatar::new("claude").picture_signal(logo)   // the picture when there is one, else the letter
//! ```
//!
//! **What it owns:** the box — fill and ring are theme tones, the corner radius and ring width are
//! the theme's, all read at paint, so a theme reload restyles it; the letter, written in the
//! theme's colour for that fill (`on(..)`) so it reads on any fill; and the picture, as tall as the
//! text. **What the caller owns:** the tones it picks and the signals it hands in — a ring or a
//! picture signal changes the tile in place, with no rebuild.
//!
//! Why this is a widget: heca-pro drew this tile by hand for its rooms and members (a `Surface` with
//! a fill, a radius and a padding at the call site). A tile with a letter is not about rooms, so it
//! belongs here, where any list can use it.

use crate::FontWeight;
use crate::Tone;
use crate::builders::{LayoutExt, Parent, PlaceExt, PresenceExt, StyleExt};
use crate::component::{Base, Component, PaintCx, area, paint_child};
use crate::image::ImageSource;
use crate::reactive::{Signal, SignalGet, SignalUpdate, create_effect, signal};
use crate::style::Spacing;
use crate::widgets::{Grid, Image, Label};

/// The tile's two parts, each in its own area, one shown at a time: the letter, or the picture.
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
        Self {
            base,
            tone: Tone::Accent,
            ring: signal(None),
            picture,
        }
    }

    /// **The fill**, in a theme tone — resolved at paint, so a theme reload restyles it. Use it for
    /// what kind of thing the tile stands for. Default: [`Tone::Accent`].
    #[heca_grid_ui_macros::prop]
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// **A ring round the tile**, in a theme tone — the box's edge, as wide as the theme's border.
    /// Use it for a second fact the fill does not carry (how the thing is doing). Default: no ring.
    #[heca_grid_ui_macros::prop]
    pub fn ring(self, tone: Tone) -> Self {
        self.ring.set(Some(tone));
        self
    }

    /// **Drive the ring from a tone the host keeps** — `None` takes the ring away. The ring changes
    /// in place as the signal does; nothing is rebuilt.
    #[heca_grid_ui_macros::host_only("binds a host-owned signal, which static data cannot drive")]
    pub fn ring_signal(mut self, ring: Signal<Option<Tone>>) -> Self {
        self.ring = ring;
        self
    }

    /// **A picture instead of the letter** — an image file (a logo, a photo), as tall as the text.
    /// Default: the letter.
    #[heca_grid_ui_macros::prop]
    pub fn picture(self, path: impl Into<String>) -> Self {
        self.picture.set(Some(ImageSource::file(path.into())));
        self
    }

    /// **Drive the picture from an image the host keeps** — `None` shows the letter again. The tile
    /// changes in place as the signal does; nothing is rebuilt.
    #[heca_grid_ui_macros::host_only("binds a host-owned signal, which static data cannot drive")]
    pub fn picture_signal(self, picture: Signal<Option<ImageSource>>) -> Self {
        // The tile's own signal follows the host's, so the parts it was built with keep working.
        let own = self.picture;
        create_effect(move |_| own.set(picture.get()));
        self
    }
}

/// **The two parts, in their named areas**: the letter, shown while there is no picture, and the
/// picture, shown while there is one. A part that is hidden takes no room, so the tile is always as
/// big as what it shows.
fn parts(name: &str, picture: Signal<Option<ImageSource>>) -> Grid {
    let image = Image::from_source(ImageSource::bytes(Vec::new()))
        .width("1em")
        .height("1em");
    let source = image.source_signal();
    create_effect(move |_| {
        if let Some(shown) = picture.get() {
            source.set(shown);
        }
    });
    Grid::new()
        .template_area([format!("{LETTER} {PICTURE}")])
        .align("center")
        .child(
            Label::new(initial(name))
                .font_weight(FontWeight::Bold)
                .padding_x(Spacing::Xs)
                .area(LETTER)
                .hidden(move || picture.get().is_some()),
        )
        .child(
            image
                .padding(Spacing::Xs)
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
            let fill = self.tone.resolve(theme);
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

    /// The ring follows the signal it is bound to, and `ring(..)` writes into that same signal.
    #[test]
    fn the_ring_follows_its_signal() {
        let ring = signal(None);
        let avatar = Avatar::new("bob").ring_signal(ring);
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
        let mut root = laid(Avatar::new("bob").picture_signal(picture));
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

    /// The tile is as big as its letter, the letter in its middle.
    #[test]
    fn the_tile_is_as_big_as_its_letter() {
        let root = laid(Avatar::new("bob"));
        let letter = area(&root, LETTER).expect("the letter").base().bounds;
        let tile = area(&root, "avatar").expect("the avatar").base().bounds;
        assert!(
            (letter.size.w - tile.size.w).abs() < 0.5,
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
