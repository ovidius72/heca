//! **Which server this is, and the folder that is its own.**
//!
//! A server keeps files — what its extensions' services write to disk — and two servers must never
//! write into the same place. So each one *claims* a name by holding an exclusive lock on it for as
//! long as it runs:
//!
//! - the background server (`heca --server`) is `default`;
//! - a standalone window's server is `standalone-<n>`, the lowest `n` nobody holds. Two standalone
//!   windows at once get different folders, and a window opened again after one closed gets its
//!   folder back.
//!
//! The folder is `<data folder>/heca/servers/<name>/`, and each service has a folder of its own
//! inside it ([`ServerIdentity::service_folder`]). It is the same on every start, unlike the epoch.
//!
//! The entry point that starts a server says which kind it is and claims its identity; the server
//! owns it from then on, so the lock goes when the server does. A server that could not claim one —
//! no data folder on this platform, or the folder is unwritable — still runs; only its services
//! are told why they have no folder.

use std::fs::File;
use std::io;
use std::path::{Component, Path, PathBuf};

use super::private_fs::{private_folder, try_lock};

#[cfg(test)]
mod tests;

/// What kind of server is starting — said by its entry point, never guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServerKind {
    /// The background server: `heca --server`.
    Background,
    /// The server inside one standalone window.
    Standalone,
}

impl ServerKind {
    /// The name of the one slot of a background server, and the category of a standalone one.
    fn category(self) -> &'static str {
        match self {
            Self::Background => "default",
            Self::Standalone => "standalone",
        }
    }

    /// The name of the `n`th slot, counting from 1. A background server has only the first.
    fn slot(self, n: u32) -> Option<String> {
        match (self, n) {
            (Self::Background, 1) => Some(self.category().to_owned()),
            (Self::Background, _) => None,
            (Self::Standalone, n) => Some(format!("{}-{n}", self.category())),
        }
    }
}

/// A claimed name and its folder, with the lock that keeps both this server's.
struct Claim {
    /// `<servers>/<name>`: its last part is the name.
    folder: PathBuf,
    /// Held, never read: the lock goes when this file is closed.
    _lock: File,
}

/// **A server's name and folder** — see the [module](self).
pub(crate) struct ServerIdentity {
    /// What kind of server this is.
    kind: ServerKind,
    /// What a service is told when there is no folder.
    claim: Result<Claim, String>,
    /// A folder made for a test, removed with the identity.
    #[cfg(test)]
    _scratch: Option<tempfile::TempDir>,
}

impl ServerIdentity {
    /// Claim this kind's identity under the user's data folder.
    pub(crate) fn claim(kind: ServerKind) -> Self {
        Self::claim_in(kind, heca_config::state_file::data_path("servers"))
    }

    /// Claim under `servers`, the folder every server's own folder goes in. `None` is a platform
    /// with no data folder: the identity is still made, and its services are told there is none.
    pub(crate) fn claim_in(kind: ServerKind, servers: Option<PathBuf>) -> Self {
        let claimed = servers
            .ok_or_else(|| "this platform has no data folder".to_owned())
            .and_then(|servers| claim_slot(kind, &servers).map_err(|error| error.to_string()));
        match claimed {
            Ok(claim) => Self::new(kind, Ok(claim)),
            Err(why) => {
                eprintln!("[heca] this server has no folder of its own for its services: {why}");
                Self::new(kind, Err(why))
            }
        }
    }

    fn new(kind: ServerKind, claim: Result<Claim, String>) -> Self {
        Self {
            kind,
            claim,
            #[cfg(test)]
            _scratch: None,
        }
    }

    /// An identity under a folder of its own that goes with it, for a test that starts a server and
    /// must never touch the user's data.
    #[cfg(test)]
    pub(crate) fn for_tests(kind: ServerKind) -> Self {
        let scratch = tempfile::tempdir().expect("a scratch folder");
        let mut identity = Self::claim_in(kind, Some(scratch.path().to_owned()));
        identity._scratch = Some(scratch);
        identity
    }

    /// What kind of server this is, as its entry point said.
    pub(crate) fn kind(&self) -> ServerKind {
        self.kind
    }

    /// `default`, or `standalone-<n>`; `None` when no name could be claimed.
    #[cfg(test)]
    pub(crate) fn name(&self) -> Option<String> {
        let claim = self.claim.as_ref().ok()?;
        Some(claim.folder.file_name()?.to_string_lossy().into_owned())
    }

    /// The folder that belongs to this server and to `service` (`<extension>.<service>`), made
    /// owner-only if it is not there. The same path on every start.
    pub(crate) fn service_folder(&self, service: &str) -> io::Result<PathBuf> {
        let claim = self.claim.as_ref().map_err(|why| io::Error::other(why.clone()))?;
        let mut parts = Path::new(service).components();
        if !matches!((parts.next(), parts.next()), (Some(Component::Normal(_)), None)) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("'{service}' is not a folder name")));
        }
        let folder = claim.folder.join(service);
        private_folder(&folder)?;
        Ok(folder)
    }

    /// **A file heca itself keeps for this server**, directly in its folder (made owner-only if it
    /// is not there). `name` is a single file name; the file is `heca.<name>`. A service's folder is
    /// `<extension>.<short>` and `heca` is not an extension's name, so nothing a service is called
    /// can ever be this path. The same path on every start.
    pub(crate) fn own_file(&self, name: &str) -> io::Result<PathBuf> {
        let claim = self.claim.as_ref().map_err(|why| io::Error::other(why.clone()))?;
        let mut parts = Path::new(name).components();
        if !matches!((parts.next(), parts.next()), (Some(Component::Normal(_)), None)) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("'{name}' is not a file name")));
        }
        private_folder(&claim.folder)?;
        Ok(claim.folder.join(format!("heca.{name}")))
    }
}

/// How long a claim waits for a slot that is held only for a moment.
///
/// A server that just stopped has let its lock go, but a process started around the same time —
/// a terminal, any child — can still hold a copy of the lock file for the instant between starting
/// and running its program, and the lock stays taken while it does. A server started right after
/// another stopped (a restart) would then find its own slot "in use" and come up without its saved
/// sessions. Waiting this long covers that instant; a slot held by a running server is still held
/// after it, and the claim then fails as before.
const RELEASE_GRACE: std::time::Duration = std::time::Duration::from_secs(1);

/// Take the lowest slot of `kind` nobody holds — waiting [`RELEASE_GRACE`] for one that is being
/// let go.
fn claim_slot(kind: ServerKind, servers: &Path) -> io::Result<Claim> {
    private_folder(servers)?;
    let deadline = std::time::Instant::now() + RELEASE_GRACE;
    loop {
        for n in 1.. {
            let Some(name) = kind.slot(n) else { break };
            if let Some(lock) = try_lock(&servers.join(format!("{name}.lock")))? {
                return Ok(Claim { folder: servers.join(name), _lock: lock });
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(io::Error::new(io::ErrorKind::AddrInUse,
                format!("another server is already running as '{}'", kind.category())));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
