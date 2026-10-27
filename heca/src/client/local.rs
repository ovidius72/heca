//! Standalone hosts the same server and listener, with winit driving its inbox.

use std::io;
use heca_config::loader::AppConfig;
use crate::server::{ServerWaker, run::ServerLoop, runtime::{self, RuntimeHandle, RuntimeInbox}, wire::ServerLink};

pub(super) struct LocalServer {
    server: ServerLoop,
    handle: RuntimeHandle,
    inbox: RuntimeInbox,
    #[cfg(unix)]
    listener: Option<crate::server::socket::RuntimeListener>,
}

impl LocalServer {
    pub(super) fn new(config: &AppConfig, wake: ServerWaker) -> io::Result<Self> {
        let (handle, inbox) = runtime::channel_waking(config.config.settings.connection, wake);
        let server = ServerLoop::from_config(config, handle.waker())?;
        #[cfg(unix)]
        let listener = crate::server::socket::socket_path().and_then(|path| {
            match crate::server::socket::RuntimeListener::bind(&path, handle.clone(), config.config.settings.connection) {
                Ok(listener) => Some(listener),
                Err(error) => { eprintln!("[heca] standalone control socket: {error}"); None },
            }
        });
        Ok(Self { server, handle, inbox, #[cfg(unix)] listener })
    }

    pub(super) fn add_client(&mut self, client: impl ServerLink + 'static) { self.server.add_client(client); }
    pub(super) fn pump(&mut self) { self.inbox.pump(&mut self.server); }
    pub(super) fn next_wake(&self) -> Option<std::time::Instant> { self.server.next_wake() }
    pub(super) fn install_actions(&mut self, registry: crate::actions::ActionRegistry, catalog: crate::actions::ActionCatalog) {
        self.server.install_actions(registry, catalog);
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        let _ = self.handle.stop();
        self.server.shutdown();
        self.inbox.reject_pending();
        #[cfg(unix)]
        self.listener.take();
    }
}
