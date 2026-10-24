use std::{env, ffi::OsString, fs, io, path::PathBuf, process::Command, sync::mpsc, time::{Duration, Instant}};

use notify::{RecursiveMode, Watcher};

use super::{FLAG, LaunchRequest, LaunchResponse, RESPONSE, request_path};
use crate::agent::{AgentError, AgentSetup};

/// Handle Heca's private launch and reporting helpers before normal CLI or window startup.
/// `None` means this is an ordinary host invocation. Successful Unix launches replace this process.
pub fn run_if_requested() -> Option<Result<i32, AgentError>> {
    let mut arguments = env::args_os().skip(1);
    let flag = arguments.next()?;
    if flag.as_os_str() == FLAG { Some(run(arguments.collect())) }
    else if flag.as_os_str() == crate::agent::reporting::FLAG {
        Some(crate::agent::reporting::run(arguments.collect()))
    } else { None }
}

fn run(arguments: Vec<OsString>) -> Result<i32, AgentError> {
    let [root, timeout, limit, program, executable, rest @ ..] = arguments.as_slice() else {
        return Err(AgentError::Report("incomplete private agent launch".into()));
    };
    let timeout = timeout.to_str().and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0).ok_or_else(|| AgentError::Report("invalid agent launch timeout".into()))?;
    let limit = limit.to_str().and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0).ok_or_else(|| AgentError::Report("invalid agent launch limit".into()))?;
    let root = PathBuf::from(root);
    let directory = tempfile::Builder::new().prefix("run-").tempdir_in(root)?;
    let request = LaunchRequest {
        program: program.to_str().ok_or_else(|| AgentError::Report("agent executable name must be UTF-8".into()))?.to_owned(),
        cwd: env::current_dir()?,
        environment: env::vars_os().filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?))).collect(),
    };
    let setup = request_setup(&directory, &request, Duration::from_millis(timeout), limit)?;
    // The terminal owns the outer directory and removes all run assets when it closes.
    let _assets = directory.keep();
    let mut command = Command::new(executable);
    command.args(setup.arguments).args(rest).envs(setup.environment);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec().into())
    }
    #[cfg(not(unix))]
    {
        Ok(command.status()?.code().unwrap_or(1))
    }
}

pub(super) fn request_setup(directory: &tempfile::TempDir, request: &LaunchRequest, timeout: Duration, limit: usize) -> Result<AgentSetup, AgentError> {
    let (sender, receiver) = mpsc::channel();
    let response = directory.path().join(RESPONSE);
    // Watch the existing response file. Watching its directory can lose a coalesced event
    // when reports.jsonl and the response are created together. Atomic replacement wakes this
    // watch with a remove/rename event on every supported backend.
    fs::write(&response, [])?;
    let mut watcher = crate::agent::watcher::new(move |event: notify::Result<notify::Event>| {
        if event.is_ok() { let _ = sender.send(()); }
    }).map_err(|error| io::Error::other(error.to_string()))?;
    watcher.watch(&response, RecursiveMode::NonRecursive)
        .map_err(|error| io::Error::other(error.to_string()))?;
    let bytes = serde_json::to_vec(request).map_err(|error| AgentError::Report(error.to_string()))?;
    if bytes.len() > limit { return Err(AgentError::Report("agent launch environment exceeds max_report_bytes".into())) }
    let path = request_path(directory.path())?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, &path)?;
    fs::write(path.with_file_name(super::REQUEST_READY), [1])?;
    let deadline = Instant::now() + timeout;
    loop {
        match fs::read(&response) {
            Ok(bytes) if bytes.is_empty() => {}
            Ok(bytes) => {
                let reply: LaunchResponse = serde_json::from_slice(&bytes)
                    .map_err(|_| AgentError::Report("invalid private launch response".into()))?;
                let _ = fs::remove_file(&response);
                return reply.map_err(AgentError::Report);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())).is_err() {
            let _ = fs::remove_file(&path);
            return Err(AgentError::Report("agent launch setup timed out".into()));
        }
    }
}
