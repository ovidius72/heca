//! Component style: layout (mapped to taffy) + Tron visual tokens.
//!
//! The layout enums here ([`Direction`], [`Justify`], [`Align`], [`Length`])
//! are our own, mapped to `taffy` internally — so `taffy` never leaks into the
//! public API and could be swapped without breaking widget code.

use crate::color::Color;
use crate::scene::{Border, Glow};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

mod flow;
mod grid;
mod layout;
mod length;
mod space;
mod visual;

pub use flow::*;
pub use grid::*;
pub use layout::*;
pub use length::*;
pub use space::*;
pub use visual::*;

/// **Read a layout keyword the way a stylesheet writes it** — one parser for every unit enum here.
///
/// It goes through the type's **own serde names**, so there is exactly one vocabulary: a spelling
/// that works in a plugin's JSON works in Rust, and neither side can learn a word the other does
/// not know. `-` and `_` are the same character to it, because CSS writes `space-between` and
/// serde's derived names write `space_between`, and nobody should have to remember which side of
/// the wire they are standing on.
///
/// Unreadable input is `None`, and every caller turns that into the type's default rather than a
/// panic — these arrive from config and from plugins.
fn keyword<T: serde::de::DeserializeOwned>(t: &str) -> Option<T> {
    let name = t.trim().to_ascii_lowercase().replace('-', "_");
    T::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(&name)).ok()
}

macro_rules! keyword_from_str {
    ($($t:ty),+ $(,)?) => {$(
        impl std::str::FromStr for $t {
            type Err = KeywordParseError;
            /// **The one parser** — the type's own serde names, hyphen or underscore.
            fn from_str(t: &str) -> Result<Self, Self::Err> {
                keyword(t).ok_or(KeywordParseError)
            }
        }
        impl From<&str> for $t {
            /// ⚠️ An unreadable keyword is the **default**, never a panic. Use
            /// [`from_str`](std::str::FromStr::from_str) when you want to be told.
            fn from(t: &str) -> Self {
                t.parse().unwrap_or_default()
            }
        }
        impl From<&String> for $t {
            fn from(t: &String) -> Self { Self::from(t.as_str()) }
        }
    )+};
}

/// What a layout keyword's `from_str` returns when the word is not in that type's vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeywordParseError;

// `Spacing` is deliberately NOT here: its five words are already read by [`Space::from_str`], and
// a second parser for the same vocabulary is the defect this whole change removes. Write
// `.gap("sm")`, which goes through that one.
keyword_from_str!(Direction, Justify, Align);
