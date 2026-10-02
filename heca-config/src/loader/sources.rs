//! **Which file each setting came from** — the report behind `heca --show-config`. It reads the same
//! layers the loader merges (`super`) and names the file that won each key.

use std::path::{Path, PathBuf};

use super::{ConfigError, config_file_paths, keybindings_file_paths, read_first_toml, read_project};

/// One setting a user's file or the project's file sets, and the file it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSource {
    /// Dotted key, `appearance.sidebar.border_color`. An array (a list of bindings) is one key.
    pub key: String,
    /// The value as TOML spells it.
    pub value: String,
    pub file: PathBuf,
}

/// **Which file each setting came from** — every key the user's files or the project's file set, with
/// the later layer winning, exactly as the merge does. Defaults are not listed: they come from no
/// file of the user's.
///
/// An **untrusted** project file is not listed as applied: it is returned in `untrusted_project`, for
/// the caller to show as "(not trusted, ignored)".
pub fn config_sources() -> Result<ConfigSources, ConfigError> {
    let mut layers = Vec::new();
    for paths in [config_file_paths(), keybindings_file_paths()] {
        layers.extend(read_first_toml(&paths)?);
    }
    let mut untrusted_project = None;
    if let Some(project) = read_project()? {
        if project.trusted {
            layers.push((project.path, project.value));
        } else {
            untrusted_project = Some(project.path);
        }
    }
    Ok(ConfigSources {
        sources: sources_of(layers),
        untrusted_project,
    })
}

/// What [`config_sources`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigSources {
    pub sources: Vec<ConfigSource>,
    /// A project file that exists but is not trusted, so it applies to nothing.
    pub untrusted_project: Option<PathBuf>,
}

/// The sources of `layers` (earliest first): each layer flattened to dotted keys, a later layer's
/// key replacing an earlier one's. Sorted by key.
fn sources_of(layers: Vec<(PathBuf, toml::Value)>) -> Vec<ConfigSource> {
    fn flatten(prefix: &str, value: &toml::Value, file: &Path, out: &mut Vec<ConfigSource>) {
        match value {
            toml::Value::Table(table) => {
                for (k, v) in table {
                    let key = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    flatten(&key, v, file, out);
                }
            }
            leaf => out.push(ConfigSource {
                key: prefix.to_string(),
                value: leaf.to_string(),
                file: file.to_path_buf(),
            }),
        }
    }
    let mut by_key = std::collections::BTreeMap::new();
    for (file, value) in &layers {
        let mut found = Vec::new();
        flatten("", value, file, &mut found);
        for source in found {
            by_key.insert(source.key.clone(), source);
        }
    }
    by_key.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toml_value(text: &str) -> toml::Value {
        toml::from_str(text).expect("valid toml")
    }

    /// **Each setting says which file it came from**, and a project key wins over the user's.
    #[test]
    fn each_setting_names_the_file_it_came_from() {
        let user = (
            PathBuf::from("config.toml"),
            toml_value("[settings]\nmouse = false\n[keys]\nfocus_left = \"prefix+a\""),
        );
        let project = (
            PathBuf::from(".heca/config.toml"),
            toml_value("[keys]\nfocus_left = \"prefix+p\""),
        );
        let got = sources_of(vec![user, project]);
        let of = |key: &str| got.iter().find(|s| s.key == key).map(|s| s.file.clone());
        assert_eq!(of("settings.mouse"), Some(PathBuf::from("config.toml")));
        assert_eq!(
            of("keys.focus_left"),
            Some(PathBuf::from(".heca/config.toml")),
            "the project wins"
        );
        assert_eq!(got.len(), 2, "one line per key");
    }
}
