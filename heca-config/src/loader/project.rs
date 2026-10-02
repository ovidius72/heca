//! **The project's settings file** — `.heca/config.toml`, found from the folder heca was started in,
//! read, and judged against the user's trust list. Merging it over the user's config is the loader's
//! job (`super`); reading it is this file's.

use std::path::{Path, PathBuf};

use super::ConfigError;

/// **The project's settings file**: `.heca/config.toml` in the nearest folder, starting at `start`
/// and going up, that has a `.heca/` folder. Nested projects: the nearest wins.
pub fn project_config_path(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|dir| dir.join(".heca"))
        .find(|dir| dir.is_dir())
        .map(|dir| dir.join("config.toml"))
}

/// The project's settings file, read, with whether the user has trusted it as it stands.
#[derive(Clone, Debug)]
pub struct ProjectFile {
    /// The `.heca/config.toml` itself.
    pub path: PathBuf,
    /// The folder that has the `.heca/` — what trust is remembered against.
    pub dir: PathBuf,
    /// Hash of its content — what trust is remembered against.
    pub hash: String,
    pub value: toml::Value,
    pub trusted: bool,
}

/// Read the project's file for the folder heca was started in. `Ok(None)` when there is none.
pub fn read_project() -> Result<Option<ProjectFile>, ConfigError> {
    let Ok(cwd) = std::env::current_dir() else {
        return Ok(None);
    };
    read_project_in(&cwd)
}

/// [`read_project`] for a chosen starting folder.
pub fn read_project_in(start: &Path) -> Result<Option<ProjectFile>, ConfigError> {
    read_project_with(start, crate::trust::is_trusted)
}

/// [`read_project_in`] with the trust question answered by `is_trusted(folder, content hash)` — what
/// a test uses so it never reads or writes the user's real trust list.
pub fn read_project_with(
    start: &Path,
    is_trusted: impl Fn(&Path, &str) -> bool,
) -> Result<Option<ProjectFile>, ConfigError> {
    let Some(path) = project_config_path(start) else {
        return Ok(None);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(ConfigError::Io { path, source }),
    };
    let value = toml::from_str::<toml::Value>(&text).map_err(|source| ConfigError::Parse {
        path: path.clone(),
        source,
    })?;
    let dir = path
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let hash = crate::trust::content_hash(&text);
    let trusted = is_trusted(&dir, &hash);
    Ok(Some(ProjectFile {
        path,
        dir,
        hash,
        value,
        trusted,
    }))
}
