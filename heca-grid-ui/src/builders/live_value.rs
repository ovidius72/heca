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
