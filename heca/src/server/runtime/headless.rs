//! The process entry: configuration and process state, without winit or a GPU.

use std::{io, path::Path};
use heca_config::loader::AppConfig;
use crate::server::{run::ServerLoop, socket::RuntimeListener};

pub(crate) fn serve(socket: Option<&Path>) -> io::Result<()> {
    let config = AppConfig::load();
    let settings = &config.config.settings;
    let (handle, inbox) = super::channel(settings.connection);
    let path = socket.map(Path::to_owned).or_else(crate::server::socket::socket_path)
        .ok_or_else(|| io::Error::other("no runtime folder for the server"))?;
    let _listener = RuntimeListener::bind(&path, handle.clone(), settings.connection)?;
    crate::notification::install_notification_sink({
        let handle = handle.clone();
        move |draft| {
            if let Err(error) = handle.raise(draft) { eprintln!("[heca] notification: {error}"); }
        }
    });
    let mut server = ServerLoop::from_config(&config, handle.waker())?;
    server.register_extensions();
    inbox.run(server);
    Ok(())
}
