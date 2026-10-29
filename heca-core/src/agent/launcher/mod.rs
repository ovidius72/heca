//! A private, event-driven launch handshake. The owning host calls the registered driver;
//! the helper only preserves the actual shell environment, waits for setup, then replaces itself.

mod client;
mod request;

pub use client::run_if_requested;
pub(super) use request::{is_request, prepare};

use std::{collections::BTreeMap, io, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};

use super::AgentSetup;

pub(super) const FLAG: &str = "--heca-agent-launch";
pub(super) const REQUEST_READY: &str = ".requests-ready";
const REQUEST_SUFFIX: &str = ".request.json";
const RESPONSE: &str = "response.json";

fn request_path(directory: &Path) -> io::Result<PathBuf> {
    let root = directory.parent().ok_or_else(|| io::Error::other("missing launch root"))?;
    let name = directory.file_name().ok_or_else(|| io::Error::other("missing launch directory"))?;
    let mut name = name.to_os_string();
    name.push(REQUEST_SUFFIX);
    Ok(root.join(name))
}

#[derive(Serialize, Deserialize)]
struct LaunchRequest {
    executable: PathBuf,
    arguments: Vec<std::ffi::OsString>,
    program: String,
    cwd: PathBuf,
    environment: BTreeMap<String, String>,
}

type LaunchResponse = Result<AgentSetup, String>;

#[cfg(test)]
mod tests;
