//! Validate before acquiring resources, retaining usable data with explicit diagnostics.

use heca_grid_ui::Theme;
use heca_view::ViewNode;
use super::{RealizeError, RealizeErrorKind, WidgetRegistry};

impl WidgetRegistry {
    pub(super) fn prepare(&self, node: &ViewNode, theme: &Theme) -> Result<ViewNode, RealizeError> {
        let mut prepared = node.clone();
        let mut diagnostics = Vec::new();
        self.prepare_node(&mut prepared, theme, &mut Vec::new(), &mut diagnostics)?;
        for diagnostic in &diagnostics {
            (self.diagnostic)(diagnostic);
        }
        Ok(prepared)
    }

    fn prepare_node(
        &self,
        node: &mut ViewNode,
        theme: &Theme,
        path: &mut Vec<usize>,
        diagnostics: &mut Vec<RealizeError>,
    ) -> Result<(), RealizeError> {
        let definition = self.get(&node.kind).ok_or_else(|| RealizeError {
            kind: RealizeErrorKind::UnknownWidget(node.kind.clone()), path: path.clone(),
        })?;
        let metadata = definition.metadata();
        if !(metadata.minimum_version..=metadata.maximum_version).contains(&node.schema_version) {
            return Err(RealizeError { kind: RealizeErrorKind::UnsupportedVersion {
                widget: node.kind.clone(), requested: node.schema_version,
                minimum: metadata.minimum_version, maximum: metadata.maximum_version,
            }, path: path.clone() });
        }
        for validate in &definition.validators {
            loop {
                match validate(node, theme) {
                    Ok(()) => break,
                    Err(mut error) => {
                        error.path = path.clone();
                        // Removing only the rejected input leaves the native builder/adapter's
                        // own default in charge. Retry to diagnose all bad fields on this node.
                        if let RealizeErrorKind::InvalidProperty { property, .. } = &error.kind
                            && node.props.remove(property).is_some()
                        {
                            diagnostics.push(error);
                        } else {
                            return Err(error);
                        }
                    }
                }
            }
        }
        let children = std::mem::take(&mut node.children);
        for (index, mut child) in children.into_iter().enumerate() {
            path.push(index);
            // Even an unusable child must identify a known implementation/version. No
            // registration or compatibility error is hidden by the parent's child contract.
            self.prepare_node(&mut child, theme, path, diagnostics)?;
            path.pop();
            if metadata.accepts_children
                && (metadata.child_types.is_empty() || metadata.child_types.contains(&child.kind))
            {
                node.children.push(child);
            } else {
                diagnostics.push(RealizeError { kind: RealizeErrorKind::InvalidChild {
                    widget: node.kind.clone(), index,
                    detail: format!("child '{}' is not accepted by this definition", child.kind),
                }, path: path.clone() });
            }
        }
        Ok(())
    }
}
