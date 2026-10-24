//! One decoder path for filesystem and embedded audio sources.

use std::{fmt, fs::File, io::Cursor, path::PathBuf, sync::Arc};

/// A reusable sound source. Neither constructor opens an audio device or plays anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Audio {
    File(PathBuf),
    Bytes(Arc<[u8]>),
}

impl Audio {
    pub fn load(path: impl Into<PathBuf>) -> Self { Self::File(path.into()) }
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Self { Self::Bytes(Arc::from(bytes.as_ref())) }

    pub(crate) fn decode(&self) -> Result<Box<dyn rodio::Source + Send>, String> {
        match self {
            Self::File(path) => {
                let file = File::open(path).map_err(|error| error.to_string())?;
                rodio::Decoder::try_from(file).map(|source| Box::new(source) as Box<dyn rodio::Source + Send>)
                    .map_err(|error| error.to_string())
            }
            Self::Bytes(bytes) => rodio::Decoder::try_from(Cursor::new(bytes.clone()))
                .map(|source| Box::new(source) as Box<dyn rodio::Source + Send>)
                .map_err(|error| error.to_string()),
        }
    }
}

impl fmt::Display for Audio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(path) => path.display().fmt(f),
            Self::Bytes(_) => f.write_str("embedded sound"),
        }
    }
}
