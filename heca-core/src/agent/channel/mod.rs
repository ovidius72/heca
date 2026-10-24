//! An append-only report file per driver. Filesystem events wake the terminal; no frame polling.

use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc, atomic::{AtomicUsize, Ordering}},
};

use notify::{RecursiveMode, Watcher};

use super::{AgentDriver, AgentDrivers, AgentError, AgentInput, AgentState};

pub(super) struct Inbox {
    reports: Mutex<VecDeque<AgentState>>,
    capacity: AtomicUsize,
}

impl Inbox {
    pub(super) fn new(capacity: usize) -> Self {
        Self { reports: Mutex::new(VecDeque::new()), capacity: AtomicUsize::new(capacity) }
    }

    fn push(&self, report: AgentState) {
        let mut queue = self.reports.lock().unwrap_or_else(|error| error.into_inner());
        let capacity = self.capacity.load(Ordering::Relaxed);
        if capacity == 0 { return }
        while queue.len() >= capacity {
            queue.pop_front();
        }
        queue.push_back(report);
    }

    pub(super) fn drain(&self) -> Vec<AgentState> {
        self.reports.lock().unwrap_or_else(|error| error.into_inner()).drain(..).collect()
    }
}

pub(super) struct ReportReader {
    path: PathBuf,
    driver: Arc<dyn AgentDriver>,
    file: File,
    pending: Vec<u8>,
    oversized: bool,
    limit: usize,
}

impl ReportReader {
    pub(super) fn path(&self) -> &Path { &self.path }

    pub(super) fn create(
        path: PathBuf,
        driver: Arc<dyn AgentDriver>,
        limit: usize,
    ) -> io::Result<Self> {
        fs::write(&path, [])?;
        let file = File::open(&path)?;
        Ok(Self { path, driver, file, pending: Vec::new(), oversized: false, limit })
    }

    fn read(&mut self, inbox: &Inbox) -> io::Result<()> {
        if self.file.metadata()?.len() < self.file.stream_position()? {
            self.file.seek(SeekFrom::Start(0))?;
            self.pending.clear();
            self.oversized = false;
        }
        let mut bytes = vec![0; self.limit.min(4096)];
        loop {
            let read = self.file.read(&mut bytes)?;
            if read == 0 { break }
            for &byte in &bytes[..read] {
                if byte == b'\n' {
                    if !self.oversized
                        && let Ok(Some(report)) = super::registry::read_report(self.driver.as_ref(), AgentInput::SideChannel(&self.pending))
                    {
                        inbox.push(report);
                    }
                    self.pending.clear();
                    self.oversized = false;
                } else if self.pending.len() < self.limit && !self.oversized {
                    self.pending.push(byte);
                } else {
                    self.pending.clear();
                    self.oversized = true;
                }
            }
        }
        Ok(())
    }
}

pub(super) struct ReportChannel {
    stop: mpsc::Sender<worker::Message>,
    inbox: Arc<Inbox>,
}

impl ReportChannel {
    pub(super) fn configure(
        &self, drivers: AgentDrivers, reporter: super::reporting::ReportingHelper, capacity: usize,
    ) -> Result<(), AgentError> {
        self.inbox.capacity.store(capacity, Ordering::Relaxed);
        let mut queue = self.inbox.reports.lock().unwrap_or_else(|error| error.into_inner());
        while queue.len() > capacity { queue.pop_front(); }
        drop(queue);
        self.stop.send(worker::Message::Configure { drivers, reporter })
            .map_err(|error| AgentError::Launch(io::Error::other(error.to_string())))
    }

    pub(super) fn watch(
        directory: &Path,
        drivers: AgentDrivers,
        reporter: super::reporting::ReportingHelper,
        capacity: usize,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, AgentError> {
        let inbox = Arc::new(Inbox::new(capacity));
        // macOS reports `/private/var/...` even when the temp directory was opened as `/var/...`.
        let watch_root = fs::canonicalize(directory)?;
        let (stop, receiver) = mpsc::channel();
        let sender = stop.clone();
        // Publish requests separately, then write this existing file. A directory watcher can
        // lose a coalesced rename while auto-registering a newly created child on macOS.
        let ready = watch_root.join(super::launcher::REQUEST_READY);
        fs::write(&ready, [])?;
        let mut watcher = super::watcher::new(move |event| {
            let _ = sender.send(worker::Message::Event(event));
        }).map_err(|error| AgentError::Launch(io::Error::other(error)))?;
        watcher.watch(&ready, RecursiveMode::NonRecursive)
            .map_err(|error| AgentError::Launch(io::Error::other(error)))?;
        let incoming = inbox.clone();
        std::thread::Builder::new().name("heca-agent-reports".into()).spawn(move || {
            worker::run(watcher, receiver, watch_root, drivers, reporter, incoming, wake);
        })?;
        Ok(Self { stop, inbox })
    }

    pub(super) fn drain(&self) -> Vec<AgentState> {
        self.inbox.drain()
    }
}

impl Drop for ReportChannel {
    fn drop(&mut self) { let _ = self.stop.send(worker::Message::Stop); }
}

mod worker;

#[cfg(test)]
mod tests;
