use std::{fs, io::{self, Read}, path::Path};

use notify::{RecursiveMode, Watcher};

use super::{LaunchRequest, LaunchResponse, REQUEST_SUFFIX, RESPONSE};
use crate::agent::{AgentDrivers, AgentError, AgentLaunchContext};
use crate::agent::channel::ReportReader;

pub(in crate::agent) fn is_request(path: &Path, root: &Path) -> bool {
    path.parent() == Some(root)
        && path.file_name().and_then(|name| name.to_str())
            .is_some_and(|name| name.strip_suffix(REQUEST_SUFFIX).is_some_and(|name| !name.is_empty()))
}

pub(in crate::agent) fn prepare(
    path: &Path,
    drivers: &AgentDrivers,
    reporter: &crate::agent::reporting::ReportingHelper,
    watcher: &mut crate::agent::watcher::AgentWatcher,
) -> io::Result<Option<ReportReader>> {
    let root = path.parent().ok_or_else(|| io::Error::other("missing launch root"))?;
    let name = path.file_name().and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(REQUEST_SUFFIX))
        .ok_or_else(|| io::Error::other("invalid launch request name"))?;
    let directory = root.join(name);
    let mut reader = None;
    let limit = reporter.max_report_bytes;
    let result = read(path, limit).and_then(|request| {
        let driver = drivers.recognize(&request.program)
            .ok_or_else(|| AgentError::Report(format!("no agent driver recognizes {}", request.program)))?;
        let endpoint = directory.join("reports.jsonl");
        reader = Some(ReportReader::create(endpoint.clone(), driver.clone(), limit)?);
        watcher.watch(&endpoint, RecursiveMode::NonRecursive)
            .map_err(|error| io::Error::other(error.to_string()))?;
        driver.prepare(&AgentLaunchContext {
            reporter: reporter.clone(),
            directory: directory.clone(), cwd: request.cwd,
            endpoint: Some(endpoint), environment: request.environment,
        })
    });
    // The request can contain the user's environment. Keep it in memory only while preparing.
    let _ = fs::remove_file(path);
    let response: LaunchResponse = result.map_err(|error| error.to_string());
    if response.is_err()
        && let Some(reader) = reader.take()
    {
        let _ = watcher.unwatch(reader.path());
    }
    let temporary = directory.join("response.tmp");
    fs::write(&temporary, serde_json::to_vec(&response)?)?;
    fs::rename(temporary, directory.join(RESPONSE))?;
    Ok(reader)
}

fn read(path: &Path, limit: usize) -> Result<LaunchRequest, AgentError> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(AgentError::Report("agent launch request exceeds max_report_bytes".into()));
    }
    serde_json::from_slice(&bytes).map_err(|_| AgentError::Report("invalid agent launch request".into()))
}
