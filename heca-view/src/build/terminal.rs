//! A terminal description; the host supplies ownership and the existing session backend.
use super::Style;
use crate::{PropValue, ViewNode};

/// Named terminal session. Rebuilding the description reuses the session within its owner.
#[derive(Debug, Clone, PartialEq)]
pub struct Terminal(pub(super) ViewNode);
builder_impls!(Terminal);

impl Terminal {
    pub fn new(name: impl Into<String>) -> Self {
        Self(ViewNode::new("terminal").prop("name", PropValue::Text(name.into())))
    }
    pub fn command(self, command: impl Into<String>) -> Self { self.prop("command", command.into()) }
    pub fn cwd(self, directory: impl Into<String>) -> Self { self.prop("cwd", directory.into()) }
    pub fn title(self, title: impl Into<String>) -> Self { self.prop("title", title.into()) }
}
