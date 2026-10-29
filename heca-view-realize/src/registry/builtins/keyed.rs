//! Keyed descriptions use the native cache and props seam, including lazy child construction.
use super::*;
use heca_grid_ui::{Component, widgets::Keyed};
use heca_grid_ui::builders::Parent;
use std::{cell::RefCell, collections::BTreeMap};

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Inputs { content_key: Option<String>, texts: BTreeMap<String, String> }

struct Update {
    version: String,
    content: RefCell<Option<heca_grid_ui::WidgetModel>>,
    texts: Vec<(String, String)>,
}

pub(super) fn build(_: &ViewNode, _: &mut RealizeCx<'_>) -> Result<Keyed, RealizeError> {
    let keyed = Keyed::new();
    let handle = keyed.clone();
    Ok(keyed.on_props(move |update: &Update, base| {
        handle.show(update.version.clone(), || update.content.borrow_mut().take());
        handle.texts(update.texts.clone());
        base.built_from = Some(update.version.clone());
        base.mark_needs_layout();
    }))
}

pub(super) fn update(component: &mut dyn Component, node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<bool, RealizeError> {
    let input: Inputs = cx.state(node)?;
    let version = match input.content_key {
        Some(key) => key,
        None => serde_json::to_string(&node.children).map_err(|error|
            RealizeError::invalid_property(node.kind.clone(), "children", error.to_string()))?,
    };
    let content = if component.base().built_from.as_deref() == Some(&version) { None }
        else if node.children.len() == 1 { Some(cx.child(node, 0)?) }
        else { Some(heca_grid_ui::WidgetModel::new(Flex::column().child(cx.children(node)?))) };
    let input = Update { version, content: RefCell::new(content), texts: input.texts.into_iter().collect() };
    if !component.set_props(&input) {
        return Err(RealizeError::invalid_property(node.kind.clone(), "state", "native keyed props are not bound"));
    }
    Ok(true)
}
