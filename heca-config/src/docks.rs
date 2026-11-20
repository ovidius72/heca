//! **`[[docks]]` — where each dock goes and in what order**, as the user says it.
//!
//! ```toml
//! [[docks]]
//! name = "workspaces"          # the kind of dock
//! location = "sidebar.left"    # sidebar.left | sidebar.right | bar.top | bar.bottom | none
//! order = 1                    # lower first; ties keep the order the docks were added in
//!
//! [[docks]]
//! name = "workspaces"
//! id = "aaa"                   # one dock of that kind: wins over the entry without an id
//! location = "sidebar.right"
//! ```
//!
//! An entry for an id no dock has does nothing. `location` and `order` are each optional: what is
//! not said is what the code that adds the dock says.

use serde::{Deserialize, Serialize};

/// One `[[docks]]` entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockEntry {
    /// The kind of dock (`Provider::kind`, e.g. `"workspaces"`).
    pub name: String,
    /// One dock of that kind, by its id. Unset: every dock of the kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Where it goes: `"sidebar.left"`, `"sidebar.right"`, `"bar.top"`, `"bar.bottom"`, or
    /// `"none"` to leave it out. Unset: where the code that adds it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Where it sits among the region's docks, as CSS `order`: lower first, ties as added.
    /// Unset: what the code says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
}

/// **The entry for one dock**: the one naming its id wins over the one for its kind.
pub fn entry_for<'a>(docks: &'a [DockEntry], kind: &str, id: &str) -> Option<&'a DockEntry> {
    docks.iter().find(|e| e.name == kind && e.id.as_deref() == Some(id))
        .or_else(|| docks.iter().find(|e| e.name == kind && e.id.is_none()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, id: Option<&str>, location: &str) -> DockEntry {
        DockEntry { name: name.into(), id: id.map(Into::into), location: Some(location.into()), order: None }
    }

    #[test]
    fn an_entry_for_the_id_wins_over_the_one_for_the_kind() {
        let docks = [entry("workspaces", None, "sidebar.left"), entry("workspaces", Some("aaa"), "sidebar.right")];
        assert_eq!(entry_for(&docks, "workspaces", "aaa").unwrap().location.as_deref(), Some("sidebar.right"));
        assert_eq!(entry_for(&docks, "workspaces", "bbb").unwrap().location.as_deref(), Some("sidebar.left"));
        assert!(entry_for(&docks, "notes", "notes").is_none());
    }

    #[test]
    fn docks_read_as_a_list_of_tables() {
        let text = "[[docks]]\nname = \"workspaces\"\nlocation = \"sidebar.left\"\norder = 1\n\n\
                    [[docks]]\nname = \"workspaces\"\nid = \"aaa\"\norder = -1\n";
        #[derive(Deserialize)]
        struct File { docks: Vec<DockEntry> }
        let file: File = toml::from_str(text).unwrap();
        assert_eq!(file.docks.len(), 2);
        assert_eq!(file.docks[0].order, Some(1));
        assert_eq!(file.docks[1].id.as_deref(), Some("aaa"));
        assert_eq!(file.docks[1].location, None);
    }
}
