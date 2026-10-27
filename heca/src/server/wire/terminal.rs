//! Plain process input and terminal state. Scrolling and selection stay in each client.

use heca_core::{backend::{BackendKeyEvent, BackendMouseEvent, TerminalDamage, WireSnapshot}, runtime::PaneRuntime};
use crate::app::backend_store::TerminalOwner;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum ProcessInput {
    Bytes(Vec<u8>),
    Key(BackendKeyEvent),
    Mouse(BackendMouseEvent),
    Paste(String),
    Focus(bool),
    Resize { cols: usize, rows: usize, cell: (f32, f32), scale: f32 },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct TerminalFrame {
    pub(crate) id: u64,
    pub(crate) owner: TerminalOwner,
    pub(crate) spec: crate::app::backend_store::TerminalSpec,
    pub(crate) title: String,
    pub(crate) screen: WireSnapshot,
    pub(crate) damage: TerminalDamage,
    pub(crate) runtime: PaneRuntime,
    pub(crate) mouse_grabbed: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum ReadData {
    Lines(Vec<heca_core::backend::TerminalLine>),
    Matches(Vec<heca_core::backend::SearchMatch>),
}
