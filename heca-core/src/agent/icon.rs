//! Portable icon metadata. The UI resolves these sources through its shared Image widget.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// A driver-supplied mark, independent of a renderer or a fixed program catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentIcon {
    /// A local SVG or bitmap file. A packaged driver resolves its own asset directory.
    File(PathBuf),
    /// Self-contained SVG contents, including artwork embedded by a native driver.
    Svg(String),
}

impl AgentIcon {
    /// Use a local asset. The file is loaded asynchronously by the UI's shared image service.
    pub fn file(path: impl Into<PathBuf>) -> Self { Self::File(path.into()) }

    /// Embed SVG contents. No runtime source-checkout path is required.
    pub fn svg(contents: impl Into<String>) -> Self { Self::Svg(contents.into()) }
}
