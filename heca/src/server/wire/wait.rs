//! Startup and headless callers wait on transport wakeups, not a polling interval.

use super::{ClientLink, ServerMsg};
use std::{
    io,
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

pub(crate) fn await_message(
    link: &dyn ClientLink,
    ready: &Receiver<()>,
    config: heca_config::settings::ConnectionConfig,
    mut pump: impl FnMut(),
) -> io::Result<ServerMsg> {
    let deadline = Instant::now() + Duration::from_millis(config.request_timeout_ms.get());
    loop {
        pump();
        if let Some(message) = link.poll()? {
            return Ok(message);
        }
        ready
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| io::Error::new(io::ErrorKind::TimedOut, error))?;
    }
}
