//! The UI host's registry: built-ins and program additions share one lookup path.
use std::cell::{Cell, RefCell};
use heca_grid_ui::Theme;
use heca_view::{ViewNode, WidgetId};
use heca_view_realize::{FormBindings, IntentEmitter, RealizeError, RealizeErrorKind, WidgetDefinition, WidgetRegistry};
use serde::{Deserialize, Serialize};

thread_local! {
    static REGISTRY: RefCell<Option<WidgetRegistry>> = const { RefCell::new(None) };
    static STARTED: Cell<bool> = const { Cell::new(false) };
}

fn invalid(id: WidgetId, detail: impl Into<String>) -> RealizeError {
    RealizeError { kind: RealizeErrorKind::InvalidDefinition { widget: id, detail: detail.into() }, path: Vec::new() }
}

#[derive(Default, Serialize, Deserialize)]
struct TerminalInputs { name: String, command: Option<String>, cwd: Option<String>, title: Option<String> }

fn terminal() -> Result<WidgetDefinition, RealizeError> {
    Ok(WidgetDefinition::new("terminal", |node, cx| {
        let input: TerminalInputs = cx.state(node)?;
        let owner = cx.owner().unwrap_or(crate::chrome::HOST_OWNER);
        let name = crate::entry::qualified_name(owner, "terminal", &input.name)
            .ok_or_else(|| invalid(node.kind.clone(), "invalid terminal name"))?;
        let mut terminal = crate::Terminal::declared(name);
        if let Some(command) = input.command { terminal = terminal.command(command); }
        if let Some(cwd) = input.cwd { terminal = terminal.cwd(cwd); }
        if let Some(title) = input.title { terminal = terminal.title(title); }
        Ok(terminal)
    }).names("Terminal", "Terminal").inputs::<TerminalInputs>()?.validate(|node, _| {
        match node.props.get("name").and_then(heca_view::PropValue::as_text) {
            Some(name) if !name.is_empty() && !name.contains('.') => Ok(()),
            _ => Err(invalid(node.kind.clone(), "terminal name must be nonempty and contain no dot")),
        }
    }))
}

fn with_registry<T>(use_registry: impl FnOnce(&mut WidgetRegistry) -> Result<T, RealizeError>) -> Result<T, RealizeError> {
    REGISTRY.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let mut registry = WidgetRegistry::builtins()?.on_diagnostic(|diagnostic|
                crate::chrome::warn_author(format!("{diagnostic}; keeping valid widget data")));
            registry.register(terminal()?)?;
            *slot = Some(registry);
        }
        use_registry(slot.as_mut().expect("initialized above"))
    })
}

/// Register a custom described-widget implementation before [`crate::run`].
/// Ordinary native widget construction never needs registration.
pub fn register_widget(definition: WidgetDefinition) -> Result<(), RealizeError> {
    if STARTED.get() { return Err(invalid(definition.metadata().id.clone(), "widget registration is closed after startup")); }
    with_registry(|registry| registry.register(definition))
}

pub(crate) fn freeze() { STARTED.set(true); }

pub(crate) fn realize(owner: Option<&str>, node: &ViewNode, theme: &Theme, emit: &IntentEmitter, forms: &mut FormBindings)
    -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    // Factories may recursively enter the host. Never hold the registry's RefCell borrow there.
    let registry = with_registry(|registry| Ok(registry.clone()))?;
    registry.realize_for(owner, node, theme, emit, forms)
}

#[cfg(test)]
mod tests;
