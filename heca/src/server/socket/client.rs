//! **`heca::socket` — talking to a running heca server from outside it**: a test, a probe, a tool.
//!
//! The server speaks two things on its socket, and both have a version the server checks:
//!
//! - one **action line** and its answer — what `heca rpc` sends: [`request`];
//! - an **attached connection**: a preface naming the protocol version, then length-prefixed JSON
//!   messages both ways — what a window speaks: [`Attached`].
//!
//! Use these instead of writing the bytes yourself. They are the same code heca's own window and
//! `heca rpc` run, so a client built on them moves with the protocol; one that wrote the preface by
//! hand stopped working the day the version moved, and failed with a message about frame sizes
//! rather than versions.
//!
//! ```no_run
//! use heca_config::settings::ConnectionConfig;
//!
//! # fn main() -> std::io::Result<()> {
//! let path = std::path::Path::new("/tmp/heca-test/server.sock");
//! let limits = ConnectionConfig::default();
//! let answer = heca::socket::request(path, "list_sessions", &limits)?;
//! let mut window = heca::socket::Attached::open(path, &limits)?;
//! window.send(&serde_json::json!({"AttachTo": {"name": "probe"}}))?;
//! let first = window.receive()?;
//! # let _ = (answer, first);
//! # Ok(())
//! # }
//! ```

use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

use heca_config::settings::ConnectionConfig;
use serde_json::Value;

use super::local::UnixStream;

/// **Run one action line on the server at `path`** and return its answer, as `heca rpc` does —
/// waiting no longer than `limits` says. The limits are the caller's, as for [`Attached::open`]:
/// nothing is read from the user's configuration.
pub fn request(path: &Path, line: &str, limits: &ConnectionConfig) -> io::Result<String> {
    super::request_within(path, line, limits)
}

/// **An attached connection to a heca server** — the protocol a window speaks, opened with the
/// version this heca speaks, carrying JSON messages each way within the configured frame limit.
pub struct Attached {
    stream: UnixStream,
    limit: std::num::NonZeroUsize,
    /// The screens the server's terminal rows are applied to, so a terminal comes out whole.
    memory: crate::server::wire::ServerMemory,
}

impl Attached {
    /// **Attach to the server at `path`.** `Err` of kind [`io::ErrorKind::Unsupported`] when the
    /// server speaks another protocol version — its answer is in the message.
    pub fn open(path: &Path, limits: &ConnectionConfig) -> io::Result<Self> {
        let mut stream = UnixStream::connect(path)?;
        super::runtime::attach_preamble(&mut stream, limits)?;
        Ok(Self {
            stream,
            limit: limits.max_frame_bytes,
            memory: Default::default(),
        })
    }

    /// How long [`receive`](Self::receive) waits for a message; `None` waits as long as it takes.
    pub fn wait_at_most(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.stream.set_read_timeout(timeout)
    }

    /// **Send one message** — refused when it is bigger than the frame limit.
    pub fn send(&mut self, message: &Value) -> io::Result<()> {
        crate::server::wire::write_frame(&mut self.stream, message, self.limit)
    }

    /// **The next message from the server, whole**, as JSON — `{"Attached": …}`, `{"Terminal":
    /// …}` with every line of the screen — the way heca's own window reads it: however the
    /// protocol packs a message to travel, it comes out as the message. `None` when the server
    /// closed the connection.
    pub fn receive(&mut self) -> io::Result<Option<Value>> {
        crate::server::wire::read_server_message(&mut self.memory, &mut self.stream, self.limit)?
            .map(|message| serde_json::to_value(message).map_err(io::Error::other))
            .transpose()
    }
}

impl Read for Attached {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buffer)
    }
}

impl Write for Attached {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
