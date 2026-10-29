//! Registered containers factories.

use super::*;

pub(super) fn v_stack(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    realize_flex(node, Flex::column(), cx)
}

pub(super) fn h_stack(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    realize_flex(node, Flex::row(), cx)
}

pub(super) fn row(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            let mut row = with_props(GridRow::new(), node, theme);
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                row = row.on_activate(move || emit(carrier.clone()));
            }
            attach_children(heca_grid_ui::WidgetModel::new(row), node, cx)?
        })
}

pub(super) fn card(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    Ok({
            attach_children(heca_grid_ui::WidgetModel::new(Card::new(text_of(node))), node, cx)?
        })
}

pub(super) fn surface(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    attach_children(heca_grid_ui::WidgetModel::new(Surface::new()), node, cx)
}

pub(super) fn panel(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            let panel = with_props(Panel::new().title(text_of(node)), node, theme);
            attach_children(heca_grid_ui::WidgetModel::new(panel), node, cx)?
        })
}

pub(super) fn scroll(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            // `axes` reaches the widget through its own builder, so a declarative region can be
            // horizontal or two-axis — it was vertical-only for as long as this arm named its
            // properties by hand.
            let region = with_props(ScrollRegion::new(), node, theme);
            attach_children(heca_grid_ui::WidgetModel::new(region), node, cx)?
        })
}

pub(super) fn resizable(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            let content: heca_grid_ui::WidgetModel = match node.children.len() {
                1 => cx.child(node, 0)?,
                _ => attach_children(heca_grid_ui::WidgetModel::new(Flex::column()), node, cx)?,
            };
            heca_grid_ui::WidgetModel::new(with_props(heca_grid_ui::Resizable::new(content), node, theme))
        })
}

pub(super) fn tree_layout(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    Ok({
            let parent: heca_grid_ui::WidgetModel = if node.children.is_empty() {
                heca_grid_ui::WidgetModel::new(Flex::empty())
            } else {
                cx.child(node, 0)?
            };
            let mut tree = heca_grid_ui::TreeLayout::new(parent);
            for index in 1..node.children.len() { tree = tree.branch(cx.child(node, index)?); }
            heca_grid_ui::WidgetModel::new(tree)
        })
}

pub(super) fn message_list(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            let mut list = with_props(heca_grid_ui::MessageList::new(), node, theme);
            for index in 0..node.children.len() { list = list.entry(cx.child(node, index)?); }
            heca_grid_ui::WidgetModel::new(list)
        })
}

pub(super) fn item_group(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            // `expanded` arrives through the surface; the widget already defaults to expanded.
            let mut group = with_props(ItemGroup::new(text_of(node)), node, theme);
            if let Some(on_toggle) = toggle_change(node, emit) {
                group = group.on_toggle(on_toggle);
            }
            // The group's own header is `children[0]`; the realized rows follow it.
            attach_children(heca_grid_ui::WidgetModel::new(group), node, cx)?
        })
}

pub(super) fn marker_group(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            let markers = with_props(MarkerGroup::new(), node, theme);
            // An indicator: no events of its own — the rows inside carry their own intents.
            attach_children(heca_grid_ui::WidgetModel::new(markers), node, cx)?
        })
}

pub(super) fn grid(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    realize_grid(node, cx)
}

pub(super) fn card_grid(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    realize_card_grid(node, cx)
}

pub(super) fn dock_frame(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            // Everything DockFrame exposes arrives through the surface, at the widget's own
            // defaults when unset.
            let mut dock = with_props(DockFrame::new(text_of(node)), node, theme);
            if let Some(on_toggle) = toggle_change(node, emit) {
                dock = dock.on_toggle(on_toggle);
            }
            // Slots: `header` is the controls slot (a search field, a count badge); everything else
            // is body content — the body is the **default slot**, so an unslotted child lands there.
            for (index, child) in node.children.iter().enumerate() {
                let realized = cx.child(node, index)?;
                match slot_of(child) {
                    Some("header") => dock = dock.header(realized),
                    None => dock = dock.child(realized),
                    other => {
                        warn_unknown_slot(node, child, other, &["header"]);
                        dock = dock.child(realized);
                    }
                }
            }
            // NB: `.rail(..)` is host-only — it binds a host-owned `Signal<RegionMode>`, which
            // static serializable data cannot drive (same rule as `ScrollBar`).
            heca_grid_ui::WidgetModel::new(dock)
        })
}
