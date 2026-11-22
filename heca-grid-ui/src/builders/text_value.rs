//! **A text that may change** — what `.text(..)`, `.label(..)` and `.value(..)` take: a fixed string,
//! a `Signal<String>`, a `Memo<String>`, or a function reading signals. One door for a constant and a
//! live text, the way [`hidden`](super::PresenceExt::hidden) takes a `bool` or a live one, so no
//! widget needs a second `with_*_signal` builder.
//!
//! ```ignore
//! Label::new("").text(title)                         // a Signal<String>: followed
//! Tag::new("").label(move || format!("{}", n.get())) // derived from other signals
//! Input::new().value(query)                          // a Signal<String>: shared both ways
//! ```

use crate::reactive::{Memo, Signal, SignalGet, SignalUpdate};

/// A text to show, fixed or live. See the module docs.
#[derive(Clone)]
pub struct TextValue(Source);

#[derive(Clone)]
enum Source {
    Fixed(String),
    /// A signal the caller owns — the widget may share it (an input writes what is typed into it).
    Signal(Signal<String>),
    Live(std::rc::Rc<dyn Fn() -> String>),
}

impl TextValue {
    /// **Show it in `target`** — the widget's own text signal: a fixed text is written once, a live
    /// one is followed (the widget's signal is written whenever it changes, and a frame is asked
    /// for). For a widget that only shows the text.
    pub(crate) fn show_in(self, target: Signal<String>) {
        match self.0 {
            Source::Fixed(text) => target.set(text),
            Source::Signal(signal) => follow(target, move || signal.get()),
            Source::Live(read) => follow(target, move || read()),
        }
    }

    /// **Bind it to `target`, both ways where it can be** — an input field: the caller's
    /// `Signal<String>` becomes the field's own, so what is typed is written into it; a fixed or
    /// derived text is shown as for [`show_in`](Self::show_in).
    pub(crate) fn bind(self, target: &mut Signal<String>) {
        match self.0 {
            Source::Signal(signal) => *target = signal,
            other => TextValue(other).show_in(*target),
        }
    }
}

/// Write `read()` into `target` now and whenever a signal it reads changes.
fn follow(target: Signal<String>, read: impl Fn() -> String + 'static) {
    crate::reactive::create_effect(move |_| {
        let text = read();
        if target.get_untracked() != text {
            target.set(text);
            crate::component::request_frame();
        }
    });
}

/// **What a text builder takes**: `&str`, `String`, `Signal<String>`, `Memo<String>` or a function
/// returning a `String`.
pub trait IntoTextValue {
    /// The text, and whether (and how) it changes.
    fn into_text_value(self) -> TextValue;
}

impl IntoTextValue for &str {
    fn into_text_value(self) -> TextValue {
        TextValue(Source::Fixed(self.to_owned()))
    }
}

impl IntoTextValue for String {
    fn into_text_value(self) -> TextValue {
        TextValue(Source::Fixed(self))
    }
}

impl IntoTextValue for &String {
    fn into_text_value(self) -> TextValue {
        TextValue(Source::Fixed(self.clone()))
    }
}

impl IntoTextValue for Signal<String> {
    fn into_text_value(self) -> TextValue {
        TextValue(Source::Signal(self))
    }
}

impl IntoTextValue for Memo<String> {
    fn into_text_value(self) -> TextValue {
        TextValue(Source::Live(std::rc::Rc::new(move || self.get())))
    }
}

impl<F: Fn() -> String + 'static> IntoTextValue for F {
    fn into_text_value(self) -> TextValue {
        TextValue(Source::Live(std::rc::Rc::new(self)))
    }
}

impl IntoTextValue for TextValue {
    fn into_text_value(self) -> TextValue {
        self
    }
}
