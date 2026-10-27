//! **The wire vocabulary round-trips through serde** (F012, P099/T543).
//!
//! Every value the server→client connection carries must survive a serialize →
//! deserialize round-trip unchanged, and no decoded-image or wezterm type may
//! appear in the wire form. These tests pin both: the geometry and ids, the
//! layout effects, the terminal damage, and the terminal [`WireSnapshot`].
//!
//! That `TerminalSnapshot` itself is *not* serializable is the guarantee, not a
//! gap: it owns `images: Vec<TerminalImage>` (`Arc<[u8]>` RGBA pixels), which has
//! no `Serialize`, so the only way onto the wire is `to_wire()`, which drops the
//! bytes and keeps each image referenced by `GraphicsPlacement::image_id`.

use heca_core::backend::{
    GraphicsPlacement, HyperlinkSpan, TerminalCell, TerminalCursor, TerminalCursorShape,
    TerminalDamage, TerminalImage, TerminalImageFrame, TerminalLine, TerminalRowRange,
    TerminalSnapshot, TerminalUnderlineStyle, WireSnapshot,
};
use heca_core::layout::{ColumnEffect, PaneEffect, PaneId, Point, Rectangle, Size, SpaceEffect};
use std::sync::Arc;
use std::time::Duration;

/// Serialize, deserialize, and assert nothing changed.
fn roundtrip<T>(value: T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(&value).expect("serialize");
    let back: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(value, back, "value changed across a serde round-trip");
}

#[test]
fn geometry_and_ids_round_trip() {
    roundtrip(PaneId(7));
    roundtrip(Point::new(1.5, -2.25));
    roundtrip(Size::new(640.0, 480.0));
    roundtrip(Rectangle::new(Point::new(10.0, 20.0), Size::new(300.0, 200.0)));
}

#[test]
fn layout_effects_round_trip() {
    roundtrip(SpaceEffect::Column(ColumnEffect::Moved { from: 2, to: 0 }));
    roundtrip(SpaceEffect::Column(ColumnEffect::Swapped { a: 1, b: 3 }));
    roundtrip(SpaceEffect::Pane(PaneEffect::Inserted { col: 1, row: 4 }));
    roundtrip(SpaceEffect::Pane(PaneEffect::Swapped { col: 0, a: 1, b: 2 }));
    // A move is several effects at once — the shape `Change::Arranged` carries.
    roundtrip(vec![
        SpaceEffect::Column(ColumnEffect::Removed { idx: 2 }),
        SpaceEffect::Pane(PaneEffect::Inserted { col: 1, row: 0 }),
    ]);
}

#[test]
fn terminal_damage_round_trips() {
    roundtrip(TerminalDamage::None);
    roundtrip(TerminalDamage::Full);
    roundtrip(TerminalDamage::Rows(vec![
        TerminalRowRange::new(0, 3),
        TerminalRowRange::new(10, 12),
    ]));
}

/// A two-row snapshot with styled cells, a hyperlink, one inline image and the
/// placement that references it.
fn sample_snapshot() -> TerminalSnapshot {
    let cell = |text: &str| TerminalCell {
        text: text.to_string(),
        fg: [0.9, 0.9, 0.9, 1.0],
        bg: [0.0, 0.0, 0.0, 1.0],
        bold: false,
        italic: false,
        underline: TerminalUnderlineStyle::None,
        width: 1,
    };
    let lines = vec![
        TerminalLine {
            cells: vec![cell("h"), cell("i")],
        },
        TerminalLine { cells: vec![cell("!")] },
    ];
    TerminalSnapshot {
        cols: 2,
        rows: 2,
        cell_w: 8.4,
        cell_h: 14.0,
        default_fg: [0.9, 0.9, 0.9, 1.0],
        default_bg: [0.0, 0.0, 0.0, 1.0],
        cursor_color: [1.0, 1.0, 1.0, 1.0],
        cursor: TerminalCursor {
            col: 1,
            row: 0,
            visible: true,
            shape: TerminalCursorShape::Block,
        },
        lines,
        viewport_offset: 0,
        at_bottom: true,
        scrollback_rows: 2,
        viewport_top_stable_row: 0,
        hyperlinks: vec![HyperlinkSpan {
            row: 0,
            start_col: 0,
            end_col: 2,
            uri: "https://example.test".to_string(),
        }],
        graphics: vec![GraphicsPlacement {
            row: 1,
            col: 0,
            cols: 1,
            rows: 1,
            image_id: 42,
            src_top_left: [0.0, 0.0],
            src_bottom_right: [1.0, 1.0],
            z_index: 0,
        }],
        images: vec![TerminalImage {
            id: 42,
            width: 1,
            height: 1,
            frames: Arc::from(
                vec![TerminalImageFrame {
                    rgba: Arc::from(vec![1u8, 2, 3, 4].into_boxed_slice()),
                    delay: Duration::ZERO,
                }]
                .into_boxed_slice(),
            ),
        }],
    }
}

#[test]
fn wire_snapshot_round_trips() {
    roundtrip(sample_snapshot().to_wire());
}

#[test]
fn to_wire_keeps_image_handles_but_drops_the_bytes() {
    let wire: WireSnapshot = sample_snapshot().to_wire();
    // The placement still names the image it needs; the bytes travel separately.
    assert_eq!(wire.graphics[0].image_id, 42);
    let json = serde_json::to_string(&wire).expect("serialize");
    // No image byte buffer rode along: WireSnapshot has no `images`/`rgba` field.
    assert!(!json.contains("rgba"), "wire form must not carry decoded image bytes");
    assert!(!json.contains("images"), "wire form must not carry the images registry");
}
