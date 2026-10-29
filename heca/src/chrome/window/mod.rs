//! **The window** — how it is divided, the root tree everything hangs from, building and painting
//! the chrome scene, the theme it reads, and keeping it in step with the session.

pub(super) mod division;
pub(super) mod retained;
pub(super) mod realize;
pub(super) mod scene;
pub(crate) mod signals;
pub(crate) mod theme;
pub(super) mod window_root;
