//! Stateful surfaces use native widgets, native signals and the shared typed props seam.
use super::*;
use heca_grid_ui::{Command, CommandPalette, Dialog, PropName, ToastAction, ToastSpec, ToastStack};
use heca_grid_ui::builders::IdentityExt;
use heca_grid_ui::reactive::signal;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub(super) struct OpenInputs { pub opened: Option<bool> }

#[derive(Serialize, Deserialize)]
pub(super) struct PaletteMode { sigil: char, name: String }
#[derive(Default, Serialize, Deserialize)]
pub(super) struct PaletteInputs { modes: Vec<PaletteMode>, query_mode: Option<String> }
#[derive(Default, Serialize, Deserialize)]
pub(super) struct PaletteState { opened: Option<bool>, query: Option<String> }
#[derive(Default, Serialize, Deserialize)]
pub(super) struct CommandInputs { keys: Vec<Vec<heca_view::build::KeyCap>> }

pub(super) fn palette_state(node: &ViewNode, theme: &heca_grid_ui::Theme) -> Result<PaletteState, RealizeError> {
    let mut state: PaletteState = super::super::state::read(node, theme)?;
    let inputs: PaletteInputs = super::super::state::read(node, theme)?;
    if let Some(mode) = inputs.query_mode
        && let Some(sigil) = inputs.modes.iter().find(|entry| entry.name == mode).map(|entry| entry.sigil)
    { state.query.get_or_insert_with(String::new).insert(0, sigil); }
    Ok(state)
}

pub(super) fn dialog(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<Dialog, RealizeError> {
    let mut dialog = Dialog::new(text_of(node));
    for (index, child) in node.children.iter().enumerate() {
        let content = cx.child(node, index)?;
        dialog = if slot_of(child) == Some("actions") { dialog.action(content) } else { dialog.child(content) };
    }
    dialog = with_props(dialog, node, cx.theme());
    let open = dialog.open_signal();
    let dismissed = intent_carrier(node, ViewEvent::Dismiss);
    let emit = cx.emit.clone();
    dialog = dialog.on_dismiss(move || {
        open.set(false);
        if let Some(intent) = &dismissed { emit(intent.clone()); }
    });
    Ok(dialog.on_props(move |state: &OpenInputs, _| { if let Some(value) = state.opened { open.set(value); } }))
}

pub(super) fn command_palette(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<CommandPalette, RealizeError> {
    let input: PaletteInputs = cx.state(node)?;
    let mut palette = CommandPalette::new();
    for mode in input.modes { palette = palette.mode(mode.sigil, mode.name); }
    for child in &node.children {
        let intent = child.events.get(ViewEvent::Press.name()).cloned();
        let emit = cx.emit.clone();
        let mut command = with_props(Command::new(text_of(child), move || {
            if let Some(intent) = &intent { emit(intent.clone()); }
        }), child, cx.theme());
        let inputs: CommandInputs = cx.state(child)?;
        for chord in inputs.keys {
            let caps = chord.into_iter().map(|cap| match cap {
                heca_view::build::KeyCap::Text(text) => Ok(heca_grid_ui::KeyCap::Text(text)),
                heca_view::build::KeyCap::Nf(glyph) => heca_grid_ui::NfGlyph::from_prop_name(glyph.name())
                    .map(heca_grid_ui::KeyCap::Nf).ok_or_else(|| RealizeError::invalid_property(child.kind.clone(), "keys", "unsupported keycap glyph")),
            }).collect::<Result<Vec<_>, _>>()?;
            command = command.keys(caps);
        }
        palette = palette.command(command);
    }
    palette = with_props(palette, node, cx.theme());
    let open = palette.open_signal();
    if let Some(intent) = intent_carrier(node, ViewEvent::Dismiss) {
        let emit = cx.emit.clone();
        palette = palette.on_dismiss(move || emit(intent.clone()));
    }
    if let Some(name) = name_prop(node) {
        let query = palette.query_signal();
        cx.forms.bind(name.clone(), Box::new(move || query.get_untracked().into()));
        cx.forms.bind_text_signal(name, query);
    }
    let write_query = palette.query_writer();
    Ok(palette.on_props(move |state: &PaletteState, _| {
        if let Some(value) = state.opened { open.set(value); }
        if let Some(value) = &state.query { write_query(value.clone()); }
    }))
}

/// Command is a declaration consumed by a palette, never a placeholder render node.
pub(super) fn command(node: &ViewNode, _: &mut RealizeCx<'_>) -> Result<Label, RealizeError> {
    Err(RealizeError::invalid_property(node.kind.clone(), "parent", "a command belongs to a command palette"))
}

#[derive(Default, Serialize, Deserialize)]
pub(super) struct ToastInputs { pub(super) countdown_ms: Option<u64> }

#[derive(Serialize, Deserialize)]
pub(super) struct StackInputs { max_lines: usize }
impl Default for StackInputs { fn default() -> Self { Self { max_lines: usize::MAX } } }
pub(super) struct QueueState { items: Vec<ToastSpec>, max_lines: usize }

pub(super) fn queue(node: &ViewNode, theme: &heca_grid_ui::Theme) -> Result<QueueState, RealizeError> {
    let mut items = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    for (index, child) in node.children.iter().enumerate() {
        let invalid = |detail: &str| {
            let mut error = RealizeError::invalid_property(child.kind.clone(), "key", detail);
            error.path.push(index); error
        };
        let id = child.declared_key().and_then(|key| key.parse::<u64>().ok())
            .ok_or_else(|| invalid("a toast queue entry needs its stable numeric id as key"))?;
        if !ids.insert(id) { return Err(invalid("duplicate toast id")); }
        let mut item = ToastSpec::new(id, text_of(child)).severity(severity_prop(child));
        if let Some(icon) = glyph_prop(child) { item = item.icon(icon); }
        if let Some(body) = child.props.get("body_text").and_then(PropValue::as_text) { item = item.body(body); }
        if let Some(yes) = child.props.get("dismissible").and_then(PropValue::as_bool) { item = item.dismissible(yes); }
        let extra: ToastInputs = super::super::state::read(child, theme)?;
        if let Some(millis) = extra.countdown_ms { item = item.countdown(std::time::Duration::from_millis(millis)); }
        for action in &child.children {
            let key = action.declared_key().unwrap_or("");
            if key.is_empty() { return Err(invalid("toast actions need a stable key")); }
            let mut button = ToastAction::new(key, text_of(action));
            if let Some(variant) = action.props.get("variant")
                .and_then(|value| crate::style::prop_to_input(value, theme))
                .and_then(|value| value.as_text().and_then(heca_grid_ui::ButtonVariant::from_prop_name))
            { button = button.variant(variant); }
            item = item.action_with(button);
        }
        items.push(item);
    }
    let options: StackInputs = super::super::state::read(node, theme)?;
    Ok(QueueState { items, max_lines: options.max_lines })
}

pub(super) fn toast_stack(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<ToastStack, RealizeError> {
    let current = queue(node, cx.theme())?;
    let items = signal(current.items);
    let max_lines = signal(current.max_lines);
    let mut stack = with_props(ToastStack::new(items).max_lines_signal(max_lines), node, cx.theme());
    if let Some(intent) = intent_carrier(node, ViewEvent::Dismiss) {
        let emit = cx.emit.clone();
        stack = stack.on_dismiss(move |id| emit(intent.clone().arg("toast_id", id.to_string().into())));
    }
    if let Some(intent) = intent_carrier(node, ViewEvent::Action) {
        let emit = cx.emit.clone();
        stack = stack.on_action(move |id, key| emit(intent.clone()
            .arg("toast_id", id.to_string().into()).arg("action", key.into())));
    }
    Ok(stack.on_props(move |state: &QueueState, _| {
        items.set(state.items.clone()); max_lines.set(state.max_lines);
    }))
}
