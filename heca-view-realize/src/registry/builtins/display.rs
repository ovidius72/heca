//! Registered display factories.

use super::*;

pub(super) fn line(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok(heca_grid_ui::WidgetModel::new(with_props(heca_grid_ui::Line::new(), node, theme)))
}

pub(super) fn label(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            // bold / italic / underline / strikethrough / align / font_size all arrive through
            // the generated surface — `Label`'s builders decide which, not a list here.
            heca_grid_ui::WidgetModel::new(with_props(Label::new(text_of(node)), node, theme))
        })
}

pub(super) fn badge(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    Ok(heca_grid_ui::WidgetModel::new(with_props(Badge::new(text_of(node)), node, cx.theme())))
}

pub(super) fn tag(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let mut tag = Tag::new(text_of(node));
    for (index, child) in node.children.iter().enumerate() {
        let content = cx.child(node, index)?;
        tag = if slot_of(child) == Some("leading") { tag.leading(content) } else { tag.segment(content) };
    }
    Ok(heca_grid_ui::WidgetModel::new(with_props(tag, node, cx.theme())))
}

pub(super) fn alert(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    Ok(heca_grid_ui::WidgetModel::new(with_props(Alert::new(text_of(node)), node, cx.theme())))
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct DotInputs { status: heca_grid_ui::DotStatus }

pub(super) fn status_dot(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let input: DotInputs = cx.state(node)?;
    let dot = StatusDot::new(input.status);
    let status = dot.status_signal();
    Ok(heca_grid_ui::WidgetModel::new(dot.on_props(move |input: &DotInputs, _| status.set(input.status))))
}

pub(super) fn separator(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok(heca_grid_ui::WidgetModel::new(with_props(Separator::horizontal(), node, theme)))
}

pub(super) fn gauge(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok(heca_grid_ui::WidgetModel::new(with_props(Gauge::new(), node, theme)))
}

pub(super) fn spinner(_node: &ViewNode, _cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    Ok(heca_grid_ui::WidgetModel::new(Spinner::new()))
}

pub(super) fn progress(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok(heca_grid_ui::WidgetModel::new(with_props(ProgressBar::new(), node, theme)))
}

pub(super) fn tile(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            let mut tile = with_props(heca_grid_ui::widgets::Tile::new(), node, theme);
            for (index, child) in node.children.iter().enumerate() {
                let realized = cx.child(node, index)?;
                tile = match slot_of(child) {
                    Some("status") => tile.status(realized),
                    Some("icon") => tile.icon(realized),
                    Some("title") => tile.title(realized),
                    Some("suffix") => tile.suffix(realized),
                    None => tile.line(realized),
                    other => {
                        warn_unknown_slot(
                            node,
                            child,
                            other,
                            &["status", "icon", "title", "suffix"],
                        );
                        tile.line(realized)
                    }
                };
            }
            heca_grid_ui::WidgetModel::new(tile)
        })
}

pub(super) fn nf_glyph(node: &ViewNode) -> Result<NfGlyph, RealizeError> {
    node.props.get("glyph").and_then(PropValue::as_text)
        .and_then(<NfGlyph as heca_grid_ui::PropName>::from_prop_name)
        .ok_or_else(|| RealizeError::invalid_property(node.kind.clone(), "glyph", "a Nerd Font icon requires a supported glyph"))
}

pub(super) fn nf_icon(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    Ok(heca_grid_ui::WidgetModel::new(with_props(NfIcon::new(nf_glyph(node)?), node, cx.theme())))
}

pub(super) fn icon_glyph(node: &ViewNode) -> Result<Glyph, RealizeError> {
    glyph_prop(node).ok_or_else(|| RealizeError::invalid_property(node.kind.clone(), "icon", "an icon requires a supported glyph"))
}

pub(super) fn icon(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let glyph = icon_glyph(node)?;
    Ok(heca_grid_ui::WidgetModel::new(with_props(Icon::new(glyph), node, cx.theme())))
}

pub(super) fn image(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            let mut image = with_props(Image::load(""), node, theme);
            for (index, child) in node.children.iter().enumerate() {
                match slot_of(child) {
                    Some("loading") => image = image.placeholder(cx.child(node, index)?),
                    Some("error") => image = image.fallback(cx.child(node, index)?),
                    slot => warn_unknown_slot(node, child, slot, &["loading", "error"]),
                }
            }
            heca_grid_ui::WidgetModel::new(image)
        })
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct ScrollBarState {
    content_extent: f32,
    viewport_extent: f32,
    offset: f32,
}

pub(super) fn scroll_bar(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let mut bar = heca_grid_ui::ScrollBar::new();
    let content = bar.content_extent_signal();
    let viewport = bar.viewport_extent_signal();
    let offset = bar.offset_signal();
    bar = bar.on_props(move |state: &ScrollBarState, _base| {
        content.set(state.content_extent);
        viewport.set(state.viewport_extent);
        offset.set(state.offset);
    });
    if let Some(name) = name_prop(node) {
        cx.forms.bind(name, Box::new(move || PropValue::Float(f64::from(offset.get_untracked()))));
    }
    if let Some(intent) = change_intent(node) {
        let emit = cx.emit.clone();
        bar = bar.on_change(move |action| {
            if let heca_grid_ui::SignalData::Float(value) = action.data {
                emit(intent.clone().arg("value", PropValue::Float(value)));
            }
        });
    }
    Ok(heca_grid_ui::WidgetModel::new(bar))
}
