//! Generic native composition wrappers, available to described trees without host plumbing.

use heca_grid_ui::{ChromeRegion, Component, Flex, FocusScope, Pane};
use heca_grid_ui::builders::{IntoComponent, Parent};
use heca_view::{ViewNode, ViewOrientation};
use serde::{Deserialize, Serialize};
use super::{RealizeCx, RealizeError, with_props};

pub(super) fn pane(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<Pane, RealizeError> {
    let mut pane = with_props(Pane::new(), node, cx.theme());
    pane.base_mut().children.extend(cx.children(node)?);
    Ok(pane)
}

pub(super) fn focus_scope(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<FocusScope, RealizeError> {
    let child = if node.children.len() == 1 {
        cx.child(node, 0)?
    } else {
        Flex::column().child(cx.children(node)?).into_component()
    };
    Ok(with_props(FocusScope::new(child), node, cx.theme()))
}

#[derive(Serialize, Deserialize)]
pub(super) struct RegionInputs {
    orientation: ViewOrientation,
}

impl Default for RegionInputs {
    fn default() -> Self { Self { orientation: ViewOrientation::Vertical } }
}

pub(super) fn chrome_region(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<ChromeRegion, RealizeError> {
    let inputs: RegionInputs = cx.state(node)?;
    let region = match inputs.orientation {
        ViewOrientation::Vertical => ChromeRegion::vertical(),
        ViewOrientation::Horizontal => ChromeRegion::horizontal(),
    };
    let mut region = with_props(region, node, cx.theme());
    region.base_mut().children.extend(cx.children(node)?);
    Ok(region)
}
