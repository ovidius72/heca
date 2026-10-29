//! Typed surface construction; callbacks cross the boundary as ordinary intents.

use super::{Parent, Style};
use crate::{Intent, PropValue, ViewEvent, ViewGlyph, ViewNfGlyph, ViewNode, ViewSeverity, ViewSize, ViewVariant};

/// A modal panel with native focus, input and dismissal behavior.
#[derive(Debug, Clone, PartialEq)]
pub struct Dialog(pub(super) ViewNode);
builder_impls!(Dialog);
impl Parent for Dialog {}
impl Dialog {
    pub fn new(title: impl Into<String>) -> Self { Self(ViewNode::new("dialog").text(title)) }
    pub fn title(mut self, title: impl Into<String>) -> Self { self.0 = self.0.text(title); self }
    pub fn opened(self, open: bool) -> Self { self.prop("opened", open) }
    pub fn default_open(self, open: bool) -> Self { self.prop("default_open", open) }
    /// Compatibility alias; prefer the common `max_width` style builder.
    pub fn panel_max_width(self, width: f32) -> Self { self.prop("panel_max_width", width) }
    pub fn dismissible(self, yes: bool) -> Self { self.prop("dismissible", yes) }
    pub fn danger(self, yes: bool) -> Self { self.prop("danger", yes) }
    pub fn default_action(self, key: &str) -> Self { self.prop("default_action", key) }
    pub fn action(mut self, action: impl Into<ViewNode>) -> Self {
        self.0.children.push(action.into().prop("slot", "actions".into())); self
    }
}
with_event!(Dialog { on_dismiss => Dismiss }
    CommandPalette { on_dismiss => Dismiss }
    ToastStack { on_dismiss => Dismiss, on_action => Action });

builder!(/// A native fuzzy command palette. Entries use the same command data as native code.
    CommandPalette => "command_palette");
impl CommandPalette {
    /// Include the live query in the containing form's values.
    pub fn name(self, name: impl Into<String>) -> Self { self.prop("name", name.into()) }
    pub fn opened(self, open: bool) -> Self { self.prop("opened", open) }
    pub fn default_open(self, open: bool) -> Self { self.prop("default_open", open) }
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.0.props.remove("query_mode"); self.prop("query", query.into())
    }
    pub fn query_in(self, scope: Option<&str>, query: impl Into<String>) -> Self {
        let palette = self.query(query);
        if let Some(scope) = scope { palette.prop("query_mode", scope) } else { palette }
    }
    pub fn placeholder(self, text: impl Into<String>) -> Self { self.prop("placeholder", text.into()) }
    pub fn panel_size(self, size: ViewSize) -> Self { self.prop("panel_size", size) }
    pub fn mode(mut self, sigil: char, name: &str) -> Self {
        let mut modes = match self.0.props.remove("modes") { Some(PropValue::List(modes)) => modes, _ => Vec::new() };
        modes.push(PropValue::Map([("sigil".into(), sigil.to_string().into()), ("name".into(), name.into())].into()));
        self.0.props.insert("modes".into(), PropValue::List(modes));
        self
    }
    pub fn command(mut self, command: Command) -> Self { self.0.children.push(command.0); self }
}

/// Palette entry data, not a renderable widget. An entry's callback is an intent.
#[derive(Debug, Clone, PartialEq)]
pub struct Command(ViewNode);
impl Command {
    pub fn new(label: impl Into<String>, intent: Intent) -> Self {
        Self(ViewNode::new("command").text(label).on_press(intent))
    }
    pub fn id(mut self, id: &str) -> Self { self.0 = self.0.prop("id", id.into()); self }
    pub fn description(mut self, text: &str) -> Self { self.0 = self.0.prop("description", text.into()); self }
    pub fn mode(mut self, name: &str) -> Self { self.0 = self.0.prop("mode", name.into()); self }
    pub fn group(mut self, group: u16) -> Self { self.0 = self.0.prop("group", i64::from(group).into()); self }
    pub fn current(mut self, current: bool) -> Self { self.0 = self.0.prop("current", current.into()); self }
    pub fn preselect(mut self, selected: bool) -> Self { self.0 = self.0.prop("preselect", selected.into()); self }
    pub fn icon(mut self, icon: ViewGlyph) -> Self { self.0 = self.0.prop("icon", icon.into()); self }
    pub fn keys(mut self, chord: impl IntoIterator<Item = KeyCap>) -> Self {
        let mut keys = match self.0.props.remove("keys") { Some(PropValue::List(keys)) => keys, _ => Vec::new() };
        keys.push(PropValue::List(chord.into_iter().map(PropValue::from).collect()));
        self.0.props.insert("keys".into(), PropValue::List(keys));
        self
    }
}

/// One displayed shortcut cap, matching the native text/icon vocabulary.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyCap { Text(String), Nf(ViewNfGlyph) }
impl From<KeyCap> for PropValue {
    fn from(cap: KeyCap) -> Self {
        let (key, value) = match cap { KeyCap::Text(text) => ("text", text), KeyCap::Nf(glyph) => ("nf", glyph.name().into()) };
        PropValue::Map([(key.into(), value.into())].into())
    }
}

builder!(/// A host-driven native toast queue. Updating the description reconciles cards by id.
    ToastStack => "toast_stack");
impl ToastStack {
    pub fn item(mut self, item: ToastSpec) -> Self { self.0.children.push(item.0); self }
    pub fn position(self, position: impl Into<String>) -> Self { self.prop("position", position.into()) }
    pub fn max_lines(self, lines: usize) -> Self { self.prop("max_lines", lines) }
}

/// Serializable notification data. Queue lifetimes and removal remain the host's responsibility.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastSpec(ViewNode);
impl ToastSpec {
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self(ViewNode::new("toast").text(title).key(id.to_string()))
    }
    pub fn severity(mut self, severity: ViewSeverity) -> Self { self.0 = self.0.prop("severity", severity.into()); self }
    pub fn icon(mut self, icon: ViewGlyph) -> Self { self.0 = self.0.prop("icon", icon.into()); self }
    pub fn body(mut self, text: &str) -> Self { self.0 = self.0.prop("body_text", text.into()); self }
    pub fn dismissible(mut self, yes: bool) -> Self { self.0 = self.0.prop("dismissible", yes.into()); self }
    pub fn countdown_ms(mut self, millis: u32) -> Self { self.0 = self.0.prop("countdown_ms", i64::from(millis).into()); self }
    pub fn action(mut self, key: &str, label: &str) -> Self {
        self.0.children.push(ViewNode::new("button").key(key).text(label).prop("slot", "actions".into())); self
    }
    pub fn action_with(mut self, key: &str, label: &str, variant: ViewVariant) -> Self {
        self.0.children.push(ViewNode::new("button").key(key).text(label)
            .prop("variant", variant.into()).prop("slot", "actions".into())); self
    }
}
