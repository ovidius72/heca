//! Validate native metadata and append the configured report to the owning terminal's file.

use std::{collections::BTreeMap, fs::OpenOptions, io::{self, Write}, path::PathBuf};

use serde::Deserialize;
use serde_json::Value;

use super::{DRIVER, ENDPOINT, EVENTS};
use crate::agent::AgentState;

pub(super) struct Reports {
    pub endpoint: Option<PathBuf>,
    pub events: BTreeMap<String, AgentState>,
    pub limit: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    event: String,
    #[serde(default)]
    turn_id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    session_title: Option<String>,
}

impl Reports {
    pub fn environment(limit: usize) -> io::Result<Self> {
        let endpoint = std::env::var_os(ENDPOINT).filter(|value| !value.is_empty()).map(PathBuf::from);
        let driver = std::env::var(DRIVER).unwrap_or_default();
        let events = std::env::var(EVENTS).unwrap_or_default();
        let mut events: BTreeMap<String, AgentState> = if endpoint.is_none() || events.is_empty() {
            BTreeMap::new()
        } else {
            if events.len() > limit { return Err(io::Error::other("report configuration exceeds max_report_bytes")) }
            serde_json::from_str(&events)?
        };
        events.retain(|_, report| report.driver == driver && !report.status.as_str().is_empty());
        Ok(Self { endpoint, events, limit })
    }

    pub fn report(&self, arguments: Value) -> io::Result<()> {
        let event: Event = serde_json::from_value(arguments)?;
        let Some(mut report) = self.events.get(&event.event).cloned() else { return Ok(()) };
        let Some(endpoint) = &self.endpoint else { return Ok(()) };
        report.turn_id = event.turn_id;
        report.model = event.model;
        report.session_id = event.session_id;
        report.session_title = event.session_title;
        let mut bytes = serde_json::to_vec(&report)?;
        if bytes.len() > self.limit { return Err(io::Error::other("report exceeds max_report_bytes")) }
        bytes.push(b'\n');
        // The terminal creates and owns this file. A closed pane must never be recreated.
        let mut file = match OpenOptions::new().append(true).open(endpoint) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        file.write_all(&bytes)
    }
}
