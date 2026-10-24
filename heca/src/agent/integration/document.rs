//! Merge and remove owned entries while keeping the surrounding user document.

use std::{io, path::Path};

use heca_core::agent::IntegrationResource;
use serde_json::{Map, Value};
use toml_edit::{DocumentMut, Item, Table, TableLike};

use super::{model::Owned, storage::Files};

fn invalid(path: &Path, detail: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{}: {detail}", path.display()))
}

fn json(bytes: Option<&[u8]>, path: &Path) -> io::Result<Value> {
    let value = match bytes {
        None => Value::Object(Map::new()),
        Some(bytes) => serde_json::from_slice(bytes).map_err(|error| invalid(path, &error.to_string()))?,
    };
    if !value.is_object() { return Err(invalid(path, "configuration must be an object")) }
    Ok(value)
}

fn toml(bytes: Option<&[u8]>, path: &Path) -> io::Result<DocumentMut> {
    let text = std::str::from_utf8(bytes.unwrap_or_default()).map_err(|error| invalid(path, &error.to_string()))?;
    text.parse().map_err(|error: toml_edit::TomlError| invalid(path, &error.to_string()))
}

fn toml_entry(value: &Value) -> io::Result<toml_edit::Value> {
    let mut document = toml_edit::ser::to_document(&serde_json::json!({ "entry": value }))
        .map_err(io::Error::other)?;
    document.remove("entry").and_then(|item| item.into_value().ok())
        .ok_or_else(|| io::Error::other("cannot represent a native registration as TOML"))
}

fn same_item(actual: &Item, expected: &Value) -> io::Result<bool> {
    let mut document = DocumentMut::new();
    document.insert("entry", actual.clone());
    let parsed: Value = toml_edit::de::from_str(&document.to_string()).map_err(io::Error::other)?;
    Ok(parsed.get("entry") == Some(expected))
}

fn same_entry(actual: &toml_edit::Value, expected: &Value) -> io::Result<bool> {
    let parsed: Value = toml_edit::de::from_str(&format!("entry = {actual}")).map_err(io::Error::other)?;
    Ok(parsed.get("entry") == Some(expected))
}

fn json_parent<'a>(value: &'a mut Value, keys: &[String], created: &mut Vec<Vec<String>>) -> io::Result<&'a mut Value> {
    let mut parent = value;
    for (index, key) in keys.iter().enumerate() {
        let map = parent.as_object_mut().ok_or_else(|| io::Error::other("configuration path is not an object"))?;
        if !map.contains_key(key) {
            map.insert(key.clone(), Value::Object(Map::new()));
            created.push(keys[..=index].to_vec());
        }
        parent = &mut map[key];
    }
    Ok(parent)
}

fn toml_parent<'a>(table: &'a mut dyn TableLike, keys: &[String], created: &mut Vec<Vec<String>>) -> io::Result<&'a mut dyn TableLike> {
    let Some((key, rest)) = keys.split_first() else { return Ok(table) };
    if !table.contains_key(key) {
        table.insert(key, Item::Table(Table::new()));
        created.push(keys[..1].to_vec());
    }
    let child = table.get_mut(key).and_then(Item::as_table_like_mut)
        .ok_or_else(|| io::Error::other("configuration path is not a TOML table"))?;
    let mut nested = Vec::new();
    let result = toml_parent(child, rest, &mut nested)?;
    created.extend(nested.into_iter().map(|mut path| { path.insert(0, key.clone()); path }));
    Ok(result)
}

pub(super) fn add(files: &mut Files, resource: &IntegrationResource) -> io::Result<Option<Owned>> {
    let path = resource.path();
    let bytes = files.get(path)?.map(<[u8]>::to_vec);
    let created_file = bytes.is_none();
    let mut owned = Owned { resource: resource.clone(), managed: true, previous: None, created_keys: Vec::new(), created_file };
    let after = match resource {
        IntegrationResource::File { content, .. } => {
            if bytes.is_some() { return Err(invalid(path, "an unowned file already exists")) }
            content.clone()
        }
        IntegrationResource::JsonArrayEntry { keys, value, .. } => {
            let (last, parents) = keys.split_last().ok_or_else(|| invalid(path, "missing registration key"))?;
            let mut document = json(bytes.as_deref(), path)?;
            let parent = json_parent(&mut document, parents, &mut owned.created_keys)?;
            let object = parent.as_object_mut().ok_or_else(|| invalid(path, "registration parent must be an object"))?;
            if !object.contains_key(last) {
                object.insert(last.clone(), Value::Array(Vec::new()));
                owned.created_keys.push(keys.clone());
            }
            let entries = object.get_mut(last).and_then(Value::as_array_mut)
                .ok_or_else(|| invalid(path, "registration must be a list"))?;
            if entries.contains(value) { owned.managed = false; return Ok(Some(owned)) }
            entries.push(value.clone());
            serde_json::to_vec_pretty(&document)?
        }
        IntegrationResource::TomlEntry { keys, value, .. } => {
            let (last, parents) = keys.split_last().ok_or_else(|| invalid(path, "missing registration key"))?;
            let mut document = toml(bytes.as_deref(), path)?;
            let parent = toml_parent(document.as_table_mut(), parents, &mut owned.created_keys)?;
            if let Some(previous) = parent.get(last) {
                if !same_item(previous, value)? { return Err(invalid(path, "an unowned registration already exists")) }
                owned.managed = false;
                return Ok(Some(owned));
            }
            parent.insert(last, Item::Value(toml_entry(value)?));
            document.to_string().into_bytes()
        }
        IntegrationResource::TomlBool { keys, value, .. } => {
            let (last, parents) = keys.split_last().ok_or_else(|| invalid(path, "missing setting key"))?;
            let mut document = toml(bytes.as_deref(), path)?;
            let parent = toml_parent(document.as_table_mut(), parents, &mut owned.created_keys)?;
            if let Some(previous) = parent.get(last) {
                owned.previous = Some(previous.as_bool().ok_or_else(|| invalid(path, "setting must be a boolean"))?);
                if owned.previous == Some(*value) { owned.managed = false; return Ok(Some(owned)) }
            }
            parent.insert(last, toml_edit::value(*value));
            document.to_string().into_bytes()
        }
        IntegrationResource::TomlArrayEntry { keys, value, .. } => {
            let (last, parents) = keys.split_last().ok_or_else(|| invalid(path, "missing hook key"))?;
            let mut document = toml(bytes.as_deref(), path)?;
            let parent = toml_parent(document.as_table_mut(), parents, &mut owned.created_keys)?;
            if !parent.contains_key(last) {
                parent.insert(last, toml_edit::value(toml_edit::Array::new()));
                owned.created_keys.push(keys.clone());
            }
            let item = parent.get_mut(last).ok_or_else(|| invalid(path, "missing hook list"))?;
            if let Some(entries) = item.as_array_mut() {
                for entry in entries.iter() {
                    if same_entry(entry, value)? { owned.managed = false; return Ok(Some(owned)) }
                }
                entries.push(toml_entry(value)?);
            } else if let Some(entries) = item.as_array_of_tables_mut() {
                for entry in entries.iter() {
                    if same_entry(&toml_edit::Value::InlineTable(entry.clone().into_inline_table()), value)? {
                        owned.managed = false; return Ok(Some(owned));
                    }
                }
                let toml_edit::Value::InlineTable(entry) = toml_entry(value)? else {
                    return Err(invalid(path, "hook must be a table"));
                };
                entries.push(entry.into_table());
            } else { return Err(invalid(path, "hooks must be an array or array of tables")) }
            document.to_string().into_bytes()
        }
    };
    files.set(path, Some(after))?;
    Ok(Some(owned))
}

pub(super) fn matches(files: &mut Files, owned: &Owned) -> io::Result<bool> {
    let bytes = files.get(owned.resource.path())?;
    match &owned.resource {
        IntegrationResource::File { content, .. } => Ok(bytes == Some(content.as_slice())),
        IntegrationResource::JsonArrayEntry { path, keys, value } => {
            let document = json(bytes, path)?;
            Ok(json_value(&document, keys).and_then(Value::as_array).is_some_and(|entries| entries.contains(value)))
        }
        IntegrationResource::TomlEntry { path, keys, value } => {
            let document = toml(bytes, path)?;
            let mut current = document.as_item();
            for key in keys {
                let Some(next) = current.get(key) else { return Ok(false) };
                current = next;
            }
            same_item(current, value)
        }
        IntegrationResource::TomlBool { path, keys, value } => {
            let document = toml(bytes, path)?;
            let mut current = document.as_item();
            for key in keys {
                let Some(next) = current.get(key) else { return Ok(false) };
                current = next;
            }
            Ok(current.as_bool() == Some(*value))
        }
        IntegrationResource::TomlArrayEntry { path, keys, value } => {
            let document = toml(bytes, path)?;
            let mut current = document.as_item();
            for key in keys {
                let Some(next) = current.get(key) else { return Ok(false) };
                current = next;
            }
            if let Some(entries) = current.as_array() {
                for entry in entries.iter() { if same_entry(entry, value)? { return Ok(true) } }
            } else if let Some(entries) = current.as_array_of_tables() {
                for entry in entries.iter() {
                    if same_entry(&toml_edit::Value::InlineTable(entry.clone().into_inline_table()), value)? { return Ok(true) }
                }
            }
            Ok(false)
        }
    }
}

fn json_value<'a>(document: &'a Value, keys: &[String]) -> Option<&'a Value> {
    keys.iter().try_fold(document, |value, key| value.get(key))
}

fn prune_json(document: &mut Value, keys: &[String]) {
    let Some((last, parents)) = keys.split_last() else { return };
    let mut value = document;
    for key in parents {
        let Some(child) = value.get_mut(key) else { return };
        value = child;
    }
    if value.get(last).is_some_and(|entry| entry.as_array().is_some_and(Vec::is_empty)
        || entry.as_object().is_some_and(Map::is_empty))
        && let Some(map) = value.as_object_mut()
    { map.remove(last); }
}

fn prune_toml(table: &mut dyn TableLike, keys: &[String]) {
    let Some((key, rest)) = keys.split_first() else { return };
    if rest.is_empty() {
        if table.get(key).is_some_and(|item| item.as_table_like().is_some_and(TableLike::is_empty)
            || item.as_array().is_some_and(toml_edit::Array::is_empty)
            || item.as_array_of_tables().is_some_and(toml_edit::ArrayOfTables::is_empty))
        { table.remove(key); }
    } else if let Some(child) = table.get_mut(key).and_then(Item::as_table_like_mut) {
        prune_toml(child, rest);
    }
}

pub(super) fn remove(files: &mut Files, owned: &Owned) -> io::Result<()> {
    if !owned.managed { return Ok(()) }
    let path = owned.resource.path();
    let Some(bytes) = files.get(path)?.map(<[u8]>::to_vec) else { return Ok(()) };
    let after = match &owned.resource {
        IntegrationResource::File { .. } => None,
        IntegrationResource::JsonArrayEntry { keys, value, .. } => {
            let mut document = json(Some(&bytes), path)?;
            let mut entry = &mut document;
            for key in keys { entry = &mut entry[key]; }
            if let Some(entries) = entry.as_array_mut()
                && let Some(index) = entries.iter().position(|entry| entry == value) { entries.remove(index); }
            for keys in owned.created_keys.iter().rev() { prune_json(&mut document, keys); }
            if owned.created_file && document.as_object().is_some_and(Map::is_empty) { None }
            else { Some(serde_json::to_vec_pretty(&document)?) }
        }
        IntegrationResource::TomlEntry { keys, .. } => {
            let mut document = toml(Some(&bytes), path)?;
            let (last, parents) = keys.split_last().ok_or_else(|| invalid(path, "missing registration key"))?;
            let parent = toml_parent(document.as_table_mut(), parents, &mut Vec::new())?;
            parent.remove(last);
            for keys in owned.created_keys.iter().rev() { prune_toml(document.as_table_mut(), keys); }
            if owned.created_file && document.as_table().is_empty() { None }
            else { Some(document.to_string().into_bytes()) }
        }
        IntegrationResource::TomlBool { keys, .. } => {
            let mut document = toml(Some(&bytes), path)?;
            let (last, parents) = keys.split_last().ok_or_else(|| invalid(path, "missing setting key"))?;
            let parent = toml_parent(document.as_table_mut(), parents, &mut Vec::new())?;
            match owned.previous {
                Some(previous) => { parent.insert(last, toml_edit::value(previous)); }
                None => { parent.remove(last); }
            }
            for keys in owned.created_keys.iter().rev() { prune_toml(document.as_table_mut(), keys); }
            if owned.created_file && document.as_table().is_empty() { None }
            else { Some(document.to_string().into_bytes()) }
        }
        IntegrationResource::TomlArrayEntry { keys, value, .. } => {
            let mut document = toml(Some(&bytes), path)?;
            let (last, parents) = keys.split_last().ok_or_else(|| invalid(path, "missing hook key"))?;
            let parent = toml_parent(document.as_table_mut(), parents, &mut Vec::new())?;
            if let Some(item) = parent.get_mut(last) {
                if let Some(entries) = item.as_array_mut() {
                    let mut found = None;
                    for (index, entry) in entries.iter().enumerate() {
                        if same_entry(entry, value)? { found = Some(index); break }
                    }
                    if let Some(index) = found { entries.remove(index); }
                } else if let Some(entries) = item.as_array_of_tables_mut() {
                    let mut retained = toml_edit::ArrayOfTables::new();
                    let mut removed = false;
                    for entry in entries.iter() {
                        if !removed && same_entry(&toml_edit::Value::InlineTable(entry.clone().into_inline_table()), value)? {
                            removed = true;
                        } else { retained.push(entry.clone()); }
                    }
                    *entries = retained;
                }
            }
            for keys in owned.created_keys.iter().rev() { prune_toml(document.as_table_mut(), keys); }
            if owned.created_file && document.as_table().is_empty() { None }
            else { Some(document.to_string().into_bytes()) }
        }
    };
    files.set(path, after)
}
