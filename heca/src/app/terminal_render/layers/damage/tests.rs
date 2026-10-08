use super::{
    TerminalCopyBand, graphics_signature, image_row_ranges, merge_row_ranges, ranges_overlap,
    retained_damage_to_apply, terminal_damage_copy_bands,
};
use heca_core::backend::{TerminalDamage, TerminalRowRange, TerminalSnapshot};

fn snapshot(rows: usize, cell_h: f32) -> TerminalSnapshot {
    TerminalSnapshot {
        cols: 80,
        rows,
        cell_w: 8.0,
        cell_h,
        default_fg: [1.0; 4],
        default_bg: [0.0, 0.0, 0.0, 1.0],
        cursor_color: [1.0; 4],
        cursor: heca_core::backend::TerminalCursor {
            col: 0,
            row: 0,
            visible: true,
            shape: heca_core::backend::TerminalCursorShape::Block,
        },
        lines: Vec::new(),
        viewport_offset: 0,
        at_bottom: true,
        scrollback_rows: rows,
        viewport_top_stable_row: 0,
        hyperlinks: Vec::new(),
        graphics: Vec::new(),
        images: Vec::new(),
    }
}

#[test]
fn terminal_damage_copy_bands_full_covers_entire_texture() {
    let bands =
        terminal_damage_copy_bands(&TerminalDamage::Full, &snapshot(3, 12.0), 36.0, 2.0, 72);
    assert_eq!(bands, vec![TerminalCopyBand { y: 0, height: 72 }]);
}

#[test]
fn terminal_damage_copy_bands_rows_convert_row_ranges_to_pixel_bands() {
    let damage = TerminalDamage::Rows(vec![
        heca_core::backend::TerminalRowRange::new(1, 3),
        heca_core::backend::TerminalRowRange::new(4, 5),
    ]);
    let bands = terminal_damage_copy_bands(&damage, &snapshot(5, 10.0), 50.0, 2.0, 100);
    assert_eq!(
        bands,
        vec![
            TerminalCopyBand { y: 20, height: 40 },
            TerminalCopyBand { y: 80, height: 20 },
        ]
    );
}

#[test]
fn terminal_damage_copy_bands_clamps_rows_to_visible_height() {
    let damage = TerminalDamage::Rows(vec![TerminalRowRange::new(2, 8)]);
    let bands = terminal_damage_copy_bands(&damage, &snapshot(4, 12.0), 48.0, 1.0, 48);
    assert_eq!(bands, vec![TerminalCopyBand { y: 24, height: 24 }]);
}

fn rng(start: usize, end: usize) -> TerminalRowRange {
    TerminalRowRange::new(start, end)
}

#[test]
fn retained_damage_skips_when_backend_reports_none_and_no_structural_change() {
    // No resize, no style change, backend reports nothing dirty, no images:
    // the retained layer already holds the previous frame, so it is skipped.
    assert_eq!(
        retained_damage_to_apply(false, false, false, &TerminalDamage::None, &[], &[]),
        None
    );
}

#[test]
fn retained_damage_passes_dirty_rows_through_when_stable() {
    // Stable layer + backend row damage, no images: only the reported rows.
    let rows = TerminalDamage::Rows(vec![rng(1, 3)]);
    assert_eq!(
        retained_damage_to_apply(false, false, false, &rows, &[], &[]),
        Some(TerminalDamage::Rows(vec![rng(1, 3)]))
    );
}

#[test]
fn retained_damage_passes_full_through_when_stable() {
    assert_eq!(
        retained_damage_to_apply(false, false, false, &TerminalDamage::Full, &[], &[]),
        Some(TerminalDamage::Full)
    );
}

#[test]
fn retained_damage_upgrades_to_full_on_resize_even_if_backend_reports_none() {
    assert_eq!(
        retained_damage_to_apply(true, false, false, &TerminalDamage::None, &[], &[]),
        Some(TerminalDamage::Full)
    );
}

#[test]
fn retained_damage_upgrades_to_full_on_style_change() {
    assert_eq!(
        retained_damage_to_apply(false, true, false, &TerminalDamage::None, &[], &[]),
        Some(TerminalDamage::Full)
    );
    assert_eq!(
        retained_damage_to_apply(
            false,
            true,
            false,
            &TerminalDamage::Rows(vec![rng(0, 2)]),
            &[],
            &[]
        ),
        Some(TerminalDamage::Full)
    );
}

#[test]
fn retained_damage_resize_dominates_style_and_backend_damage() {
    assert_eq!(
        retained_damage_to_apply(
            true,
            true,
            false,
            &TerminalDamage::Rows(vec![rng(0, 1)]),
            &[],
            &[]
        ),
        Some(TerminalDamage::Full)
    );
}

#[test]
fn retained_damage_idle_image_pane_still_skips() {
    // A static image with no text damage and unchanged placements costs nothing.
    assert_eq!(
        retained_damage_to_apply(
            false,
            false,
            false,
            &TerminalDamage::None,
            &[rng(5, 7)],
            &[rng(5, 7)]
        ),
        None
    );
}

#[test]
fn retained_damage_image_change_damages_only_image_rows() {
    // Image appeared (graphics_changed) with no text damage: repaint just the
    // image rows (terminal-task-23), not the whole pane.
    assert_eq!(
        retained_damage_to_apply(false, false, true, &TerminalDamage::None, &[rng(3, 5)], &[]),
        Some(TerminalDamage::Rows(vec![rng(3, 5)]))
    );
}

#[test]
fn retained_damage_image_move_repaints_old_and_new_rows() {
    // Image moved: union of old and new rows so no stale pixels remain.
    assert_eq!(
        retained_damage_to_apply(
            false,
            false,
            true,
            &TerminalDamage::None,
            &[rng(6, 8)],
            &[rng(3, 5)]
        ),
        Some(TerminalDamage::Rows(vec![rng(3, 5), rng(6, 8)]))
    );
}

#[test]
fn retained_damage_image_clear_repaints_old_rows() {
    // Image cleared (now empty, was present): repaint the old rows to erase it.
    assert_eq!(
        retained_damage_to_apply(false, false, true, &TerminalDamage::None, &[], &[rng(3, 5)]),
        Some(TerminalDamage::Rows(vec![rng(3, 5)]))
    );
}

#[test]
fn retained_damage_text_over_image_reblits_image_rows() {
    // Text changed on rows that overlap an image: re-blit the image there so
    // the glyph pass doesn't clobber it. Merged into one contiguous range.
    assert_eq!(
        retained_damage_to_apply(
            false,
            false,
            false,
            &TerminalDamage::Rows(vec![rng(4, 6)]),
            &[rng(5, 8)],
            &[rng(5, 8)]
        ),
        Some(TerminalDamage::Rows(vec![rng(4, 8)]))
    );
}

#[test]
fn retained_damage_text_away_from_image_leaves_image_alone() {
    // Text changed far from a static image: only the text rows are repainted;
    // the image stays retained (not re-blitted).
    assert_eq!(
        retained_damage_to_apply(
            false,
            false,
            false,
            &TerminalDamage::Rows(vec![rng(1, 2)]),
            &[rng(5, 7)],
            &[rng(5, 7)]
        ),
        Some(TerminalDamage::Rows(vec![rng(1, 2)]))
    );
}

#[test]
fn image_row_ranges_clamps_and_merges() {
    use heca_core::backend::GraphicsPlacement;
    let placement = |row, rows| GraphicsPlacement {
        row,
        col: 0,
        cols: 2,
        rows,
        image_id: 1,
        src_top_left: [0.0, 0.0],
        src_bottom_right: [1.0, 1.0],
        z_index: 0,
    };
    // Two placements (rows 1..3 and 2..4) merge to 1..4; a third past the grid
    // clamps to `rows`.
    let g = vec![placement(1, 2), placement(2, 2), placement(9, 5)];
    assert_eq!(image_row_ranges(&g, 10), vec![rng(1, 4), rng(9, 10)]);
}

#[test]
fn merge_row_ranges_coalesces_adjacent_and_overlapping() {
    assert_eq!(
        merge_row_ranges(vec![rng(3, 5), rng(1, 2), rng(2, 3), rng(5, 6)]),
        vec![rng(1, 6)]
    );
    assert!(ranges_overlap(&[rng(4, 6)], &[rng(5, 8)]));
    assert!(!ranges_overlap(&[rng(1, 2)], &[rng(5, 8)]));
}

#[test]
fn graphics_signature_changes_with_placements() {
    use heca_core::backend::GraphicsPlacement;
    let base = GraphicsPlacement {
        row: 0,
        col: 0,
        cols: 2,
        rows: 1,
        image_id: 7,
        src_top_left: [0.0, 0.0],
        src_bottom_right: [1.0, 1.0],
        z_index: 0,
    };
    let empty = graphics_signature(&[]);
    let one = graphics_signature(std::slice::from_ref(&base));
    assert_ne!(empty, one, "presence of an image changes the signature");
    assert_eq!(
        one,
        graphics_signature(std::slice::from_ref(&base)),
        "stable"
    );

    let mut moved = base.clone();
    moved.col = 4;
    assert_ne!(
        one,
        graphics_signature(&[moved]),
        "moving the image changes it"
    );

    let mut other_image = base.clone();
    other_image.image_id = 8;
    assert_ne!(
        one,
        graphics_signature(&[other_image]),
        "new image id changes it"
    );
}
