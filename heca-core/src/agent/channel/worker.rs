//! Own the watcher outside its callback so registering a report file cannot block the callback.

use std::{fs, path::PathBuf, sync::{Arc, mpsc}};

use notify::Watcher;

use super::{Inbox, ReportReader};
use crate::agent::{AgentDrivers, launcher, watcher::AgentWatcher};

pub(super) enum Message {
    Event(notify::Result<notify::Event>),
    Configure { drivers: AgentDrivers, reporter: crate::agent::reporting::ReportingHelper },
    Stop,
}

pub(super) fn run(
    mut watcher: AgentWatcher,
    receiver: mpsc::Receiver<Message>,
    root: PathBuf,
    mut drivers: AgentDrivers,
    mut reporter: crate::agent::reporting::ReportingHelper,
    inbox: Arc<Inbox>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
) {
    let mut readers: Vec<ReportReader> = Vec::new();
    while let Ok(message) = receiver.recv() {
        let event = match message {
            Message::Stop => break,
            Message::Configure { drivers: next, reporter: next_reporter } => {
                readers.retain_mut(|reader| {
                    if let Some(driver) = next.driver(reader.driver.id()) {
                        reader.driver = driver.clone();
                        reader.limit = next_reporter.max_report_bytes;
                        true
                    } else {
                        let _ = watcher.unwatch(reader.path());
                        false
                    }
                });
                drivers = next;
                reporter = next_reporter;
                continue;
            }
            Message::Event(Err(error)) => {
                eprintln!("[heca] agent report watcher: {error}");
                continue;
            }
            Message::Event(Ok(event)) => event,
        };
        if !event.kind.is_modify() && !event.kind.is_create() { continue }
        // Backends can coalesce directory writes and name any child, including an existing bin
        // directory. Every direct-child event must check the queue or a launch can be stranded.
        if event.paths.iter().any(|path| path == &root || path.parent() == Some(root.as_path()))
            && let Ok(entries) = fs::read_dir(&root)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if !launcher::is_request(&path, &root) { continue }
                match launcher::prepare(&path, &drivers, &reporter, &mut watcher) {
                    Ok(Some(reader)) => {
                        readers.retain(|previous| {
                            if previous.driver.id() != reader.driver.id() { return true }
                            let _ = watcher.unwatch(previous.path());
                            false
                        });
                        readers.push(reader);
                    }
                    Ok(None) => {}
                    Err(error) => eprintln!("[heca] agent launch handshake: {error}"),
                }
            }
        }
        // Also read after registration: a producer may write before its event is delivered.
        for reader in &mut readers {
            if let Err(error) = reader.read(&inbox) {
                eprintln!("[heca] status report from {}: {error}", reader.driver.id());
            }
        }
        if let Some(wake) = &wake { wake() }
    }
}
