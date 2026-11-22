//! **A character that is text by default is drawn from a text face** — the way every terminal draws
//! it. Unicode marks some characters as emoji that are *text* unless asked otherwise (`⏺`, `☀`, `✔`:
//! `Emoji_Presentation=No`); a face that lacks one hands it to the fallback, which on macOS reaches
//! the colour emoji face before any text face holding it, so `⏺` came out as a framed colour button
//! instead of a round dot in the text's colour. Such a character, not followed by the emoji selector
//! (U+FE0F), is shaped in the first **monochrome** face that has it; only when none has it does the
//! colour face draw it. A character that is an emoji by default (`⭐`, `⌚`) is left alone.
//!
//! Nothing names a font: the faces are whatever this machine has loaded, asked by what they hold.

use std::cell::RefCell;
use std::collections::HashMap;

use cosmic_text::FontSystem;
use cosmic_text::fontdb::{Family, ID, Query};
use cosmic_text::skrifa::{FontRef, MetadataProvider as _, Tag};
use unicode_properties::{EmojiStatus, UnicodeEmoji as _};

/// The selector asking for the emoji form of the character before it.
const EMOJI_SELECTOR: char = '\u{FE0F}';

/// The tables that make a face draw in colour.
const COLOUR_TABLES: [&[u8; 4]; 4] = [b"COLR", b"sbix", b"CBDT", b"SVG "];

thread_local! {
    /// What each (character, family) resolved to — asked once, since the answer only changes when
    /// fonts are loaded, and heca loads them at start.
    static RESOLVED: RefCell<HashMap<(char, String), Option<String>>> = RefCell::new(HashMap::new());
}

/// Whether `ch` is an emoji that is drawn as **text** unless asked otherwise.
pub(super) fn text_by_default(ch: char) -> bool {
    matches!(
        ch.emoji_status(),
        EmojiStatus::EmojiOther | EmojiStatus::EmojiModifierBase
    )
}

/// **The family to shape `ch` in instead of `family`** — `None` when `family` already has it, when
/// it is not a text-by-default emoji, or when no monochrome face on this machine has it either.
fn text_face(fonts: &FontSystem, ch: char, family: &str) -> Option<String> {
    let key = (ch, family.to_owned());
    if let Some(found) = RESOLVED.with(|cache| cache.borrow().get(&key).cloned()) {
        return found;
    }
    let db = fonts.db();
    let primary = db.query(&Query {
        families: &[Family::Name(family)],
        ..Query::default()
    });
    let found = match primary.is_some_and(|id| holds(fonts, id, ch).unwrap_or(false)) {
        true => None,
        // A monospaced face first — it sits in a terminal cell as the text around it does — then
        // any. Hidden system faces (a leading dot) are never asked.
        false => [true, false].into_iter().find_map(|monospaced| {
            db.faces()
                .filter(|face| face.monospaced == monospaced)
                .filter(|face| !face.families.iter().any(|(name, _)| name.starts_with('.')))
                .find(|face| holds(fonts, face.id, ch) == Some(true) && !in_colour(fonts, face.id))
                .and_then(|face| face.families.first().map(|(name, _)| name.clone()))
        }),
    };
    RESOLVED.with(|cache| cache.borrow_mut().insert(key, found.clone()));
    found
}

/// Whether the face `id` has a glyph for `ch`.
fn holds(fonts: &FontSystem, id: ID, ch: char) -> Option<bool> {
    fonts.db().with_face_data(id, |data, index| {
        FontRef::from_index(data, index)
            .ok()
            .is_some_and(|font| font.charmap().map(ch).is_some())
    })
}

/// Whether the face `id` draws in colour.
fn in_colour(fonts: &FontSystem, id: ID) -> bool {
    fonts
        .db()
        .with_face_data(id, |data, index| {
            FontRef::from_index(data, index).ok().is_some_and(|font| {
                COLOUR_TABLES
                    .iter()
                    .any(|tag| font.table_data(Tag::new(tag)).is_some())
            })
        })
        .unwrap_or(false)
}

/// **`text` cut where the face changes** — each piece with the family to shape it in, `None` for the
/// text's own. `None` overall when nothing in it needs another face, the common case.
pub(super) fn pieces<'t>(
    fonts: &FontSystem,
    text: &'t str,
    family: &str,
) -> Option<Vec<(&'t str, Option<String>)>> {
    if !text.chars().any(text_by_default) {
        return None;
    }
    let mut cut: Vec<(usize, Option<String>)> = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((at, ch)) = chars.next() {
        let emoji_asked = chars
            .peek()
            .is_some_and(|(_, next)| *next == EMOJI_SELECTOR);
        let face = (text_by_default(ch) && !emoji_asked)
            .then(|| text_face(fonts, ch, family))
            .flatten();
        if cut.last().is_none_or(|(_, last)| *last != face) {
            cut.push((at, face));
        }
    }
    if cut.iter().all(|(_, face)| face.is_none()) {
        return None;
    }
    let ends = cut.iter().skip(1).map(|(at, _)| *at).chain([text.len()]);
    Some(
        cut.iter()
            .zip(ends)
            .map(|((start, face), end)| (&text[*start..end], face.clone()))
            .collect(),
    )
}
