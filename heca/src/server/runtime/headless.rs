//! The process entry: configuration and process state, without winit or a GPU.

use crate::server::{
    identity::{ServerIdentity, ServerKind},
    run::ServerLoop,
    socket::RuntimeListener,
};
use heca_config::loader::AppConfig;
use std::{io, path::Path};

pub(crate) fn serve(socket: Option<&Path>) -> io::Result<()> {
    // **Who this server is, first**, so even what loading the configuration says lands in its log.
    let identity = ServerIdentity::claim(ServerKind::Background);
    if let Ok(log) = identity.own_file("log") {
        super::log::send_output_to(&log);
    }
    let config = AppConfig::load();
    let settings = &config.config.settings;
    let (handle, inbox) = super::channel(settings.connection);
    let path = socket
        .map(Path::to_owned)
        .or_else(crate::server::socket::socket_path)
        .ok_or_else(|| io::Error::other("no runtime folder for the server"))?;
    let _listener = RuntimeListener::bind(&path, handle.clone(), settings.connection)?;
    crate::notification::install_notification_sink({
        let handle = handle.clone();
        move |draft| {
            if let Err(error) = handle.raise(draft) {
                eprintln!("[heca] notification: {error}");
            }
        }
    });
    let server = ServerLoop::from_config(&config, identity, handle.waker())?;
    inbox.run(server);
    Ok(())
}
