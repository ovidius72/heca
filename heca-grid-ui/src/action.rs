//! Event payloads and action results.
//!
//! Event handlers and signals share one typed payload, [`SignalData`]. An
//! [`Action`] bundles a semantic name with that payload — value-less for a
//! button click, value-carrying for change events (input/checkbox/toggle).

/// A typed value carried by events and signals.
#[derive(Debug, Clone, PartialEq)]
pub enum SignalData {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Usize(usize),
}

/// The semantic result of an event: a named action plus an optional payload.
///
/// For change events (`input-change`, `toggle-change`, …) the new value is
/// carried in [`Action::data`].
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub name: String,
    pub data: SignalData,
}

impl Action {
    /// A value-less action (e.g. a button click).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            data: SignalData::None,
        }
    }

    /// An action carrying a new value (input/checkbox/toggle/slider change).
    pub fn value(name: impl Into<String>, data: SignalData) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_less_action() {
        let a = Action::new("click");
        assert_eq!(a.name, "click");
        assert_eq!(a.data, SignalData::None);
    }

    #[test]
    fn change_action_carries_value() {
        let a = Action::value("toggle-change", SignalData::Bool(true));
        assert_eq!(a.data, SignalData::Bool(true));
    }
}
