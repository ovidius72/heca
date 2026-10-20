//! What part of a retained texture must be redrawn, and which bands of it to copy.

use heca_core::backend::{TerminalDamage, TerminalRowRange, TerminalSnapshot};
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalCopyBand {
    pub(crate) y: u32,
    pub(crate) height: u32,
}

/// Decide what damage to re-render into a retained terminal layer this frame.
///
/// Returns `None` to skip the update entirely — the retained layer already
/// holds the last frame's content, so unchanged rows stay visible and only the
/// dirty rows need repainting. Returns `Some(Full)` when the layer was resized or
/// its style render-key changed (a structural change forces every row to be
/// repainted, otherwise the resized/retinted grid would show stale content).
/// Otherwise the backend's per-frame damage passes through unchanged, so only
/// the rows it reports are redrawn.
///
/// This is the pure policy behind `sync_retained_terminal_layers`; extracting it
/// keeps the retained-presentation contract unit-testable without a GPU.
///
/// `image_rows_now` / `image_rows_prev` are the visible row ranges the pane's
/// inline images cover this frame and last frame. Per-image damage
/// (`terminal-task-23`): an image change (appear / move / clear / animation frame
/// advance) damages only those rows (new ∪ old) instead of the whole pane, and a
/// text change on a pane holding images only re-blits the image where the changed
/// text actually overlaps it.
pub(super) fn retained_damage_to_apply(
    resized: bool,
    style_changed: bool,
    graphics_changed: bool,
    mount_damage: &TerminalDamage,
    image_rows_now: &[TerminalRowRange],
    image_rows_prev: &[TerminalRowRange],
) -> Option<TerminalDamage> {
    // A structural change (resize / style) still repaints every row; the per-row
    // path can't reason about the whole grid moving or re-shaping.
    if resized || style_changed {
        return Some(TerminalDamage::Full);
    }
    // A `Full` text damage subsumes any image rows.
    if matches!(mount_damage, TerminalDamage::Full) {
        return Some(TerminalDamage::Full);
    }

    let text_rows: &[TerminalRowRange] = match mount_damage {
        TerminalDamage::Rows(rows) => rows,
        _ => &[],
    };

    let mut damage: Vec<TerminalRowRange> = text_rows.to_vec();
    if graphics_changed {
        // Appear / move / clear / frame advance: repaint the union of the old and
        // new image rows (old so a removed/moved image leaves no stale pixels).
        damage.extend_from_slice(image_rows_now);
        damage.extend_from_slice(image_rows_prev);
    } else if !text_rows.is_empty() && ranges_overlap(text_rows, image_rows_now) {
        // Text changed under/over an image: re-blit the image on those rows so the
        // glyph pass doesn't overwrite it (or leave the image's old pixels).
        damage.extend_from_slice(image_rows_now);
    }

    let merged = merge_row_ranges(damage);
    if merged.is_empty() {
        None
    } else {
        Some(TerminalDamage::Rows(merged))
    }
}

/// Visible row ranges an image placement list covers, clamped to `[0, rows)`.
pub(super) fn image_row_ranges(
    graphics: &[heca_core::backend::GraphicsPlacement],
    rows: usize,
) -> Vec<TerminalRowRange> {
    let mut ranges: Vec<TerminalRowRange> = graphics
        .iter()
        .filter_map(|g| {
            let start = g.row.min(rows);
            let end = g.row.saturating_add(g.rows).min(rows);
            (end > start).then_some(TerminalRowRange::new(start, end))
        })
        .collect();
    ranges = merge_row_ranges(ranges);
    ranges
}

/// Whether any range in `a` overlaps any range in `b` (half-open `[start, end)`).
pub(super) fn ranges_overlap(a: &[TerminalRowRange], b: &[TerminalRowRange]) -> bool {
    a.iter()
        .any(|ra| b.iter().any(|rb| ra.start < rb.end && rb.start < ra.end))
}

/// Sort and coalesce adjacent/overlapping row ranges into a minimal set.
pub(super) fn merge_row_ranges(mut ranges: Vec<TerminalRowRange>) -> Vec<TerminalRowRange> {
    ranges.retain(|r| r.end > r.start);
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<TerminalRowRange> = Vec::with_capacity(ranges.len());
    for r in ranges {
        match merged.last_mut() {
            Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
            _ => merged.push(r),
        }
    }
    merged
}

/// Stable signature of a pane's inline-image placements, so a layer can detect
/// when images appear, move, resize, or clear and force a full repaint.
pub(super) fn graphics_signature(graphics: &[heca_core::backend::GraphicsPlacement]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    graphics.len().hash(&mut hasher);
    for g in graphics {
        g.image_id.hash(&mut hasher);
        g.row.hash(&mut hasher);
        g.col.hash(&mut hasher);
        g.cols.hash(&mut hasher);
        g.rows.hash(&mut hasher);
        g.z_index.hash(&mut hasher);
        for v in g.src_top_left.iter().chain(g.src_bottom_right.iter()) {
            v.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

pub(super) fn terminal_damage_copy_bands(
    damage: &TerminalDamage,
    snapshot: &TerminalSnapshot,
    logical_height: f32,
    scale_factor: f64,
    texture_height: u32,
) -> Vec<TerminalCopyBand> {
    match damage {
        TerminalDamage::None => Vec::new(),
        TerminalDamage::Full => vec![TerminalCopyBand {
            y: 0,
            height: texture_height.max(1),
        }],
        TerminalDamage::Rows(ranges) => {
            let scale = scale_factor as f32;
            let max_height = logical_height.max(0.0);
            let mut bands = Vec::with_capacity(ranges.len());
            for range in ranges {
                let start = range.start.min(snapshot.rows);
                let end = range.end.min(snapshot.rows);
                if start >= end {
                    continue;
                }
                let top = ((start as f32 * snapshot.cell_h) * scale).floor() as u32;
                let bottom =
                    (((end as f32 * snapshot.cell_h).min(max_height)) * scale).ceil() as u32;
                let y = top.min(texture_height);
                let clipped_bottom = bottom.min(texture_height);
                if clipped_bottom > y {
                    bands.push(TerminalCopyBand {
                        y,
                        height: clipped_bottom - y,
                    });
                }
            }
            bands
        }
    }
}

#[cfg(test)]
mod tests;
