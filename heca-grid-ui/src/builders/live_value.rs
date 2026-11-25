//! **A value a builder takes fixed or live** — the one shape behind `.hidden(..)`, `.visible(..)`
//! and `Icon::glyph(..)`: a value given once, or a function read again whenever a signal it read
//! changes. Written once here, so no builder follows a value its own way.

use crate::reactive::{Signal, SignalGet, SignalUpdate};
use std::rc::Rc;

/// **A `T` that may change**: fixed, or read again whenever a signal it read changes.
pub struct LiveValue<T>(Source<T>);

enum Source<T> {
    Fixed(T),
    Live(Rc<dyn Fn() -> T>),
}

impl<T: Clone> Clone for LiveValue<T> {
    fn clone(&self) -> Self {
        Self(match &self.0 {
            Source::Fixed(value) => Source::Fixed(value.clone()),
            Source::Live(read) => Source::Live(read.clone()),
        })
    }
}

impl<T: Clone + PartialEq + 'static> LiveValue<T> {
    /// This value, for good.
    pub fn fixed(value: T) -> Self {
        Self(Source::Fixed(value))
    }

    /// The value `read` gives, read again whenever a signal it reads changes.
    pub fn live(read: impl Fn() -> T + 'static) -> Self {
        Self(Source::Live(Rc::new(read)))
    }

    /// The value now — tracked, when read inside an effect.
    pub(crate) fn now(&self) -> T {
        match &self.0 {
            Source::Fixed(value) => value.clone(),
            Source::Live(read) => read(),
        }
    }

    /// The value now, read without subscribing whoever is asking.
    pub(crate) fn now_untracked(&self) -> T {
        match &self.0 {
            Source::Fixed(value) => value.clone(),
            Source::Live(read) => crate::reactive::untrack(|| read()),
        }
    }

    /// Whether it can change, so there is something to follow.
    pub(crate) fn changes(&self) -> bool {
        matches!(self.0, Source::Live(_))
    }

    /// **Keep `target` equal to this value**: written now, and again whenever a live value
    /// changes. Writing the first value is part of building the widget and asks for nothing; a
    /// later change asks for the frame that shows it. Equal values are not written again.
    pub(crate) fn show_in(self, target: Signal<T>) {
        match self.0 {
            Source::Fixed(value) => {
                if target.get_untracked() != value {
                    target.set(value);
                }
            }
            Source::Live(read) => {
                crate::reactive::create_effect(move |built: Option<()>| {
                    let value = read();
                    if target.get_untracked() != value {
                        target.set(value);
                        if built.is_some() {
                            crate::component::request_frame();
                        }
                    }
                });
            }
        }
    }
}

/// **What `Icon::glyph(..)` takes**: a [`Glyph`](crate::widgets::Glyph), a `Signal<Glyph>`, a
/// `Memo<Glyph>` or a function returning one.
pub trait IntoGlyphValue {
    /// The glyph to show, and follow when it can change.
    fn into_glyph_value(self) -> LiveValue<crate::widgets::Glyph>;
}

impl IntoGlyphValue for crate::widgets::Glyph {
    fn into_glyph_value(self) -> LiveValue<crate::widgets::Glyph> {
        LiveValue::fixed(self)
    }
}

impl<F: Fn() -> crate::widgets::Glyph + 'static> IntoGlyphValue for F {
    fn into_glyph_value(self) -> LiveValue<crate::widgets::Glyph> {
        LiveValue::live(self)
    }
}

impl IntoGlyphValue for Signal<crate::widgets::Glyph> {
    fn into_glyph_value(self) -> LiveValue<crate::widgets::Glyph> {
        LiveValue::live(move || self.get())
    }
}

impl IntoGlyphValue for crate::reactive::Memo<crate::widgets::Glyph> {
    fn into_glyph_value(self) -> LiveValue<crate::widgets::Glyph> {
        LiveValue::live(move || self.get())
    }
}

/// **What [`Input::suggest`](crate::widgets::Input::suggest) takes as its items** — a fixed list, or one
/// that changes (a `Signal`, a `Memo`, a function reading signals), followed while the list is
/// open.
pub trait IntoSuggestions<T> {
    /// The items, read again whenever a signal they read changes.
    fn into_suggestions(self) -> std::rc::Rc<dyn Fn() -> Vec<T>>;
}

impl<T: Clone + 'static> IntoSuggestions<T> for Vec<T> {
    fn into_suggestions(self) -> std::rc::Rc<dyn Fn() -> Vec<T>> {
        std::rc::Rc::new(move || self.clone())
    }
}

impl<T: Clone + 'static> IntoSuggestions<T> for Signal<Vec<T>> {
    fn into_suggestions(self) -> std::rc::Rc<dyn Fn() -> Vec<T>> {
        std::rc::Rc::new(move || self.get())
    }
}

impl<T: Clone + PartialEq + 'static> IntoSuggestions<T> for crate::reactive::Memo<Vec<T>> {
    fn into_suggestions(self) -> std::rc::Rc<dyn Fn() -> Vec<T>> {
        std::rc::Rc::new(move || self.get())
    }
}

impl<T, F: Fn() -> Vec<T> + 'static> IntoSuggestions<T> for F {
    fn into_suggestions(self) -> std::rc::Rc<dyn Fn() -> Vec<T>> {
        std::rc::Rc::new(self)
    }
}

/// **What `Avatar::ring(..)` takes** — a tone, fixed (`Tone::Warning`, or `None` for no ring) or
/// live (a `Signal<Option<Tone>>`, a `Memo`), followed in place.
pub trait IntoToneValue {
    /// The tone to show, and follow when it can change.
    fn into_tone_value(self) -> LiveValue<Option<crate::Tone>>;
}

impl IntoToneValue for crate::Tone {
    fn into_tone_value(self) -> LiveValue<Option<crate::Tone>> {
        LiveValue::fixed(Some(self))
    }
}

impl IntoToneValue for Option<crate::Tone> {
    fn into_tone_value(self) -> LiveValue<Option<crate::Tone>> {
        LiveValue::fixed(self)
    }
}

impl IntoToneValue for Signal<Option<crate::Tone>> {
    fn into_tone_value(self) -> LiveValue<Option<crate::Tone>> {
        LiveValue::live(move || self.get())
    }
}

impl IntoToneValue for crate::reactive::Memo<Option<crate::Tone>> {
    fn into_tone_value(self) -> LiveValue<Option<crate::Tone>> {
        LiveValue::live(move || self.get())
    }
}

/// **What `Avatar::picture(..)` takes** — an image, fixed (a file path, an `ImageSource`, or `None`
/// for none) or live (a `Signal<Option<ImageSource>>`, a `Memo`), followed in place.
pub trait IntoPictureValue {
    /// The image to show, and follow when it can change.
    fn into_picture_value(self) -> LiveValue<Option<crate::image::ImageSource>>;
}

impl IntoPictureValue for String {
    fn into_picture_value(self) -> LiveValue<Option<crate::image::ImageSource>> {
        LiveValue::fixed(Some(crate::image::ImageSource::file(self)))
    }
}

impl IntoPictureValue for &str {
    fn into_picture_value(self) -> LiveValue<Option<crate::image::ImageSource>> {
        self.to_owned().into_picture_value()
    }
}

impl IntoPictureValue for Option<crate::image::ImageSource> {
    fn into_picture_value(self) -> LiveValue<Option<crate::image::ImageSource>> {
        LiveValue::fixed(self)
    }
}

impl IntoPictureValue for Signal<Option<crate::image::ImageSource>> {
    fn into_picture_value(self) -> LiveValue<Option<crate::image::ImageSource>> {
        LiveValue::live(move || self.get())
    }
}

impl IntoPictureValue for crate::reactive::Memo<Option<crate::image::ImageSource>> {
    fn into_picture_value(self) -> LiveValue<Option<crate::image::ImageSource>> {
        LiveValue::live(move || self.get())
    }
}

/// **What `Label::marks(..)` takes** — the character indices to mark, fixed (`[0, 1, 6]`, a `Vec`)
/// or live (a `Signal<Vec<usize>>`, a function reading signals), followed in place.
pub trait IntoMarks {
    /// The indices to mark, and follow when they can change.
    fn into_marks(self) -> LiveValue<Vec<usize>>;
}

impl IntoMarks for Vec<usize> {
    fn into_marks(self) -> LiveValue<Vec<usize>> {
        LiveValue::fixed(self)
    }
}

impl<const N: usize> IntoMarks for [usize; N] {
    fn into_marks(self) -> LiveValue<Vec<usize>> {
        LiveValue::fixed(self.to_vec())
    }
}

impl IntoMarks for &[usize] {
    fn into_marks(self) -> LiveValue<Vec<usize>> {
        LiveValue::fixed(self.to_vec())
    }
}

impl IntoMarks for Signal<Vec<usize>> {
    fn into_marks(self) -> LiveValue<Vec<usize>> {
        LiveValue::live(move || self.get())
    }
}

impl<F: Fn() -> Vec<usize> + 'static> IntoMarks for F {
    fn into_marks(self) -> LiveValue<Vec<usize>> {
        LiveValue::live(self)
    }
}

/// **What a size builder takes** — `width(..)`, `height(..)`, `max_width(..)`, `max_height(..)`: a
/// fixed length in any spelling a [`Length`](crate::style::Length) has (`Length::Em(30.0)`,
/// `"50%"`, `240.0`), or one that changes — a `Signal<Length>`, a `Memo<Length>` or a function
/// reading signals — which the widget follows from the next frame, nothing rebuilt:
///
/// ```ignore
/// Flex::column().width(move || panel_width(size.get()))
/// ```
pub trait IntoLengthValue {
    /// The length, and follow it when it can change.
    fn into_length_value(self) -> LiveValue<crate::style::Length>;
}

macro_rules! fixed_lengths {
    ($($ty:ty),* $(,)?) => {$(
        impl IntoLengthValue for $ty {
            fn into_length_value(self) -> LiveValue<crate::style::Length> {
                LiveValue::fixed(self.into())
            }
        }
    )*};
}
fixed_lengths!(crate::style::Length, f32, f64, i32, u32, &str, &String);

impl IntoLengthValue for Signal<crate::style::Length> {
    fn into_length_value(self) -> LiveValue<crate::style::Length> {
        LiveValue::live(move || self.get())
    }
}

impl IntoLengthValue for crate::reactive::Memo<crate::style::Length> {
    fn into_length_value(self) -> LiveValue<crate::style::Length> {
        LiveValue::live(move || self.get())
    }
}

impl<F: Fn() -> crate::style::Length + 'static> IntoLengthValue for F {
    fn into_length_value(self) -> LiveValue<crate::style::Length> {
        LiveValue::live(self)
    }
}

/// Which size a followed length goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SizeField {
    Width,
    Height,
    MaxWidth,
    MaxHeight,
    MinWidth,
    MinHeight,
}

impl SizeField {
    fn write(self, layout: &mut crate::style::Layout, value: crate::style::Length) {
        match self {
            Self::Width => layout.width = value,
            Self::Height => layout.height = value,
            Self::MaxWidth => layout.max_width = Some(value),
            Self::MaxHeight => layout.max_height = Some(value),
            Self::MinWidth => layout.min_width = Some(value),
            Self::MinHeight => layout.min_height = Some(value),
        }
    }
}

/// **Set a size on `widget`** — fixed, written now; or followed, written now and again by every
/// layout pass while it changes ([`apply_size_values`]). The one way every size builder sets one.
pub(crate) fn set_size<C: crate::component::Component + ?Sized>(
    widget: &mut C,
    field: SizeField,
    value: impl IntoLengthValue,
) {
    let value = value.into_length_value();
    field.write(
        &mut widget.authored_style_mut().layout,
        value.now_untracked(),
    );
    let follows = &mut widget.base_mut().size_values;
    follows.retain(|(f, _)| *f != field);
    if value.changes() {
        // A layout property: the layout pass applies it; the value only asks for the frame that will.
        let watched = value.clone();
        crate::reactive::create_effect(move |_| {
            watched.now();
            crate::component::request_frame();
        });
        follows.push((field, value));
    }
}

/// **Bring every followed size in line with its value** — run by the layout pass on every node
/// before it lays the node out, so the frame a value changed in also lays the widget out at it.
pub(crate) fn apply_size_values(widget: &mut dyn crate::component::Component) {
    if widget.base().size_values.is_empty() {
        return;
    }
    let follows = widget.base().size_values.clone();
    for (field, value) in follows {
        field.write(
            &mut widget.authored_style_mut().layout,
            value.now_untracked(),
        );
    }
}

/// **What a count builder takes** (`List::visible_items(..)`): a `usize`, or one that changes — a
/// `Signal<usize>`, a `Memo<usize>` or a function reading signals.
pub trait IntoCountValue {
    /// The count, and follow it when it can change.
    fn into_count_value(self) -> LiveValue<usize>;
}

impl IntoCountValue for usize {
    fn into_count_value(self) -> LiveValue<usize> {
        LiveValue::fixed(self)
    }
}

impl IntoCountValue for Signal<usize> {
    fn into_count_value(self) -> LiveValue<usize> {
        LiveValue::live(move || self.get())
    }
}

impl IntoCountValue for crate::reactive::Memo<usize> {
    fn into_count_value(self) -> LiveValue<usize> {
        LiveValue::live(move || self.get())
    }
}

impl<F: Fn() -> usize + 'static> IntoCountValue for F {
    fn into_count_value(self) -> LiveValue<usize> {
        LiveValue::live(self)
    }
}
