//! Stable installed hooks read the per-command environment. Selection needs no native config edit.

use std::{collections::BTreeMap, path::PathBuf};

use serde_json::{Value, json};

use super::*;

pub(super) fn registrations(driver: &Driver, path: PathBuf, reporter: &ReportingHelper) -> Result<Vec<IntegrationResource>, AgentError> {
    let mut events: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for native in &driver.native_events {
        let (event, matcher) = native.split_once(':').map_or((native.as_str(), None), |(event, matcher)| (event, Some(matcher)));
        let command = reporter.command_hook(&driver.id, native)?;
        let mut group = json!({ "hooks": [{ "type": "command", "command": command }] });
        if let Some(matcher) = matcher { group["matcher"] = Value::String(matcher.into()); }
        events.entry(event.into()).or_default().push(group);
    }
    Ok(events.into_iter().flat_map(|(event, groups)| {
        let path = &path;
        groups.into_iter().map(move |value| IntegrationResource::JsonArrayEntry {
            path: path.clone(), keys: vec!["hooks".into(), event.clone()], value,
        })
    }).collect())
}
