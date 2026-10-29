//! Original process inputs, without live handles or generated integration arguments.

use serde::{Deserialize, Serialize};
use std::{ffi::OsString, path::PathBuf};

/// A process's executable, complete argv and starting directory.
/// Native OS strings preserve arguments without parsing a display command line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessLaunch {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
    pub cwd: Option<PathBuf>,
}
