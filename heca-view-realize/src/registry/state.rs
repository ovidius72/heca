//! Serializable inputs for native widgets whose controls expose live signals.

use serde::{Serialize, de::DeserializeOwned};
use heca_grid_ui::Theme;
use heca_view::ViewNode;
use super::RealizeError;

pub(super) fn fields<T: Default + Serialize>() -> Result<Vec<String>, String> {
    let value = serde_json::to_value(T::default()).map_err(|error| error.to_string())?;
    value.as_object().map(|fields| fields.keys().cloned().collect())
        .ok_or_else(|| "state inputs must be a named object".into())
}

pub(super) fn read<T: Default + Serialize + DeserializeOwned>(node: &ViewNode, theme: &Theme) -> Result<T, RealizeError> {
    let invalid = |property: &str, detail: String| RealizeError::invalid_property(node.kind.clone(), property, detail);
    let defaults = serde_json::to_value(T::default()).map_err(|error| invalid("state", error.to_string()))?;
    let Some(mut fields) = defaults.as_object().cloned() else {
        return Err(invalid("state", "inputs must be a named object".into()));
    };
    for (name, current) in fields.clone() {
        let Some(value) = node.props.get(&name) else { continue };
        let mut value = crate::style::prop_to_json(value, theme)
            .ok_or_else(|| invalid(&name, "value cannot be represented".into()))?;
        if current.is_number() && let Some(text) = value.as_str() {
            value = text.parse::<f64>().ok().and_then(serde_json::Number::from_f64)
                .map(serde_json::Value::Number)
                .ok_or_else(|| invalid(&name, "expected a finite number".into()))?;
        }
        fields.insert(name.clone(), value);
        serde_json::from_value::<T>(serde_json::Value::Object(fields.clone()))
            .map_err(|error| invalid(&name, error.to_string()))?;
    }
    serde_json::from_value(serde_json::Value::Object(fields)).map_err(|error| invalid("state", error.to_string()))
}
