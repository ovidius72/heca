//! The fixed value sets that cross to a widget by name.

// ── The remaining fixed value sets ────────────────────────────────────────────────────────
//
// A property that accepts only a fixed set of words gets a type here, so a misspelling cannot
// compile. Written as free text, `orientation: "vertcal"` is accepted, ignored, and never
// reported — the same silent failure F003/P010/T006 removed from actions, which was still live in
// this vocabulary until F003/P011/T019.
//
// **None of these gets a `PropValue` variant, and neither should the next one.** A fixed set
// travels as its NAME, so `PropValue::Text` already carries every one of them; the type belongs in
// the authoring layer, not in the wire format. `Size`/`Variant`/`Align` above predate that rule and
// are the closed shape `PropValue::Map` was added to escape — do not extend it.

// The three older sets keep their own variants — that is how they already travel, and changing it
// would be a wire-format break. New sets do not get one; see the note above `ViewOrientation`.

/// **A fixed set of words, written once**: the enum, the name each value travels under, and every
/// value in order — from one list, so the three cannot disagree.
///
/// The name is what crosses to the widget, and it is also what serde reads and writes, so a
/// description and the authoring layer can never come to spell a value differently.
macro_rules! named_set {
    ($(
        $(#[$meta:meta])*
        $vis:vis enum $ty:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $name:literal ),* $(,)?
        }
    )*) => {
        $(
            $(#[$meta])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
            $vis enum $ty {
                $( $(#[$vmeta])* #[serde(rename = $name)] $variant, )*
            }

            impl $ty {
                /// Every value in this set, in declaration order.
                pub const ALL: &'static [$ty] = &[$($ty::$variant),*];

                /// The name this value travels under — what the widget's own enum parses.
                pub fn name(self) -> &'static str {
                    match self {
                        $($ty::$variant => $name,)*
                    }
                }
            }
        )*
    };
}

/// A [`named_set!`] that reaches a property as its **name**, in `PropValue::Text` — so
/// `.prop("orientation", ViewOrientation::Vertical)` works for every set without `PropValue`
/// growing a variant per set.
macro_rules! value_set {
    ($(
        $(#[$meta:meta])*
        $vis:vis enum $ty:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $name:literal ),* $(,)?
        }
    )*) => {
        named_set! {
            $(
                $(#[$meta])*
                $vis enum $ty {
                    $( $(#[$vmeta])* $variant => $name, )*
                }
            )*
        }

        $(
            impl From<$ty> for PropValue {
                fn from(v: $ty) -> Self {
                    PropValue::Text(v.name().to_string())
                }
            }
        )*
    };
}

mod event;
mod glyph;
mod layout;
mod look;
mod text;

pub use event::*;
pub use glyph::*;
pub use layout::*;
pub use look::*;
pub use text::*;
