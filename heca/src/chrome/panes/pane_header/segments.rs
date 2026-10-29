//! The info bar's chips: which show, what they say, and how they give way to a narrow bar.

use super::*;

pub(crate) struct HeaderSegment {
    pub(crate) name: String,
    pub(crate) chip: crate::chrome::panes::pane_items::PaneChip,
    pub(crate) fit: crate::chrome::panes::pane_items::Fit,
}

/// **What each chip has to say right now** — the one derivation, read three times.
///
/// The builder turns these into a tag, the header's identity reads only *which chips came back*
/// (that is its shape), and the per-frame update writes the texts into the tree that is already on
/// screen. Three readers, one answer, so a chip cannot be built from one thing and rewritten from
/// another. A chip with nothing to say for this pane is skipped (no empty pill).
///
/// Full chip values travel to their labels; the widget owns fitting and hover text.
pub(crate) fn segment_items(
    chips: &crate::chrome::panes::pane_items::PaneChips,
    defs: &[&crate::chrome::panes::pane_items::PaneChipDef],
    facts: &crate::chrome::panes::pane_items::PaneFacts,
) -> Vec<HeaderSegment> {
    let mut items = Vec::new();
    for def in defs {
        let Some(chip) = chips.chip(def, facts) else {
            continue;
        };
        items.push(HeaderSegment { name: def.name.clone(), chip, fit: def.fit });
    }

    items
}

/// Build the pane info bar's segmented [`Tag`] from what the chips said (`segment_items`). `None`
/// when nothing was produced. Used by the terminal pane shell.
pub(crate) fn build_pane_info_bar(
    items: Vec<HeaderSegment>,
) -> Option<Tag> {
    if items.is_empty() {
        return None;
    }

    let mut tag: Option<Tag> = None;
    for HeaderSegment { name, chip, fit } in items {
        // No explicit size → the icon inherits the bar's base font, so glyph and
        // label stay balanced when the bar font changes.
        use heca_grid_ui::{ToneExt, builders::IdentityExt};
        use heca_grid_ui::widgets::{Ellipsis, Label};
        let mut leading = Icon::new(chip.icon);
        let mut label = Label::new(chip.text).key(segment_text_key(&name)).truncate(match fit {
            crate::chrome::panes::pane_items::Fit::PathLeft => Ellipsis::Start,
            crate::chrome::panes::pane_items::Fit::Keep | crate::chrome::panes::pane_items::Fit::End => Ellipsis::End,
        });
        if let Some(tone) = chip.tone {
            leading = leading.content_tone(tone);
            label = label.content_tone(tone);
        }
        // **Each chip is named by what it is**, so its words can be rewritten without the bar
        // being rebuilt. The first chip is the tag's own label; the rest are
        // child labels. Both answer `set_text`.
        tag = Some(match tag.take() {
            None => Tag::from_label(label).leading(leading),
            Some(existing) => existing.segment_label_with(label, leading),
        }.segment_shrink(fit != crate::chrome::panes::pane_items::Fit::Keep));
    }
    tag
}

/// **What a chip's words are called**, so they can be rewritten in place.
///
/// One derivation, read by the builder and by the update pass, so the two cannot address different
/// nodes. Keyed by the chip's *name* rather than its position: which chips have data changes (a pane
/// outside a repository has no branch), and a positional name would then follow whichever chip
/// happened to take that slot.
pub(super) fn segment_text_key(name: &str) -> String {
    format!("hdr.seg:{name}")
}
