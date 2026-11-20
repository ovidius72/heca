//! A transparent wrapper (`Visibility`, `KeyHint`, `FocusScope`, `Tooltip`, `KeyHintGroup`) lays its
//! child out exactly as the child would be laid out without it.

use heca_grid_ui::prelude::*;
use heca_grid_ui::{FocusScope, KeyHint, KeyHintGroup, LayoutEngine, Size, Tooltip, Visibility};

fn item() -> Row { Row::new().child(Label::new("short")) }

/// Puts a row inside one kind of wrapper.
type Wrap = fn(Row) -> heca_grid_ui::WidgetModel;

/// The wrapped row's width in a 300px column, and the same row's width with no wrapper.
fn widths(wrap: impl FnOnce(Row) -> heca_grid_ui::WidgetModel) -> (f64, f64) {
    let measure = |child: heca_grid_ui::WidgetModel| {
        let mut root = Flex::column().width(300.0).child(child);
        LayoutEngine::new().compute(&mut root, Size::new(300.0, 200.0));
        let mut node = &root.base().children[0];
        while node.base().transparent {
            node = &node.base().children[0];
        }
        node.base().bounds.size.w
    };
    (measure(wrap(item())), measure(heca_grid_ui::WidgetModel::new(item())))
}

#[test]
fn a_wrapped_list_item_fills_its_column_as_it_would_unwrapped() {
    let wrappers: [(&str, Wrap); 5] = [
        ("Visibility", |row| heca_grid_ui::WidgetModel::new(Visibility::new().visible(true).child(Flex::column().child(row)))),
        ("KeyHint", |row| heca_grid_ui::WidgetModel::new(KeyHint::new(row))),
        ("FocusScope", |row| heca_grid_ui::WidgetModel::new(FocusScope::new(row))),
        ("Tooltip", |row| heca_grid_ui::WidgetModel::new(Tooltip::new(row, "tip"))),
        ("KeyHintGroup", |row| heca_grid_ui::WidgetModel::new(KeyHintGroup::new(row))),
    ];
    for (name, wrap) in wrappers {
        let (wrapped, plain) = widths(wrap);
        assert_eq!(plain, 300.0);
        assert_eq!(wrapped, plain, "{name} keeps its child as wide as the column");
    }
}

#[test]
fn a_wrapped_child_in_a_row_keeps_its_natural_width_and_a_growing_one_still_grows() {
    let mut root = Flex::row().width(300.0)
        .child(Visibility::new().visible(true).child(Label::new("short")))
        .child(Visibility::new().visible(true).child(Flex::row().grow(1.0).child(Label::new("rest"))));
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 40.0));
    let first = root.base().children[0].base().bounds.size.w;
    let second = root.base().children[1].base().children[0].base().bounds.size.w;
    assert!(first < 100.0, "a label in a row keeps its own width, got {first}");
    assert_eq!(first + second, 300.0, "the growing child takes the rest of the row");
}
