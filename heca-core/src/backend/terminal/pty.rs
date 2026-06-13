use super::engine::SharedWriter;
use anyhow::Error as AnyError;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

type WakeCallback = Arc<dyn Fn() + Send + Sync>;
type PtyErrorSource = AnyError;

pub(super) enum PtyRead {
    Data(Vec<u8>),
    Empty,
    Disconnected,
}

/// Errors that can occur when creating or managing a PTY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PtyOperation {
    OpenPty,
    Spawn,
    CloneReader,
    TakeWriter,
    Resize,
}

impl PtyOperation {
    fn context(self) -> &'static str {
        match self {
            Self::OpenPty => "failed to open PTY",
            Self::Spawn => "failed to spawn PTY child",
            Self::CloneReader => "failed to clone PTY reader",
            Self::TakeWriter => "failed to take PTY writer",
            Self::Resize => "failed to resize PTY",
        }
    }
}

/// Errors that can occur when creating or managing a PTY.
#[derive(Debug)]
pub struct PtyError {
    operation: PtyOperation,
    source: PtyErrorSource,
}

impl PtyError {
    fn new(operation: PtyOperation, source: impl Into<AnyError>) -> Self {
        Self {
            operation,
            source: source.into(),
        }
    }

    #[must_use]
    pub fn operation(&self) -> PtyOperation {
        self.operation
    }
}

impl std::fmt::Display for PtyError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}: {}", self.operation.context(), self.source)
    }
}

impl std::error::Error for PtyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

/// Cross-platform PTY handle with buffered output delivery.
pub(super) struct PtyHandle {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: SharedWriter,
    rx: Receiver<Vec<u8>>,
}

impl PtyHandle {
    pub(super) fn new(
        cols: usize,
        rows: usize,
        wake_on_output: Option<WakeCallback>,
    ) -> Result<Self, PtyError> {
        Self::new_with_shell(cols, rows, wake_on_output, None)
    }

    pub(super) fn new_with_shell(
        cols: usize,
        rows: usize,
        wake_on_output: Option<WakeCallback>,
        shell_override: Option<&str>,
    ) -> Result<Self, PtyError> {
        let pty_system = native_pty_system();
        let size = pty_size(cols, rows);
        let pair = pty_system
            .openpty(size)
            .map_err(|err| PtyError::new(PtyOperation::OpenPty, err))?;

        let shell = shell_override.map(str::to_string).unwrap_or_else(default_shell);
        let mut cmd = CommandBuilder::new(shell);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|err| PtyError::new(PtyOperation::Spawn, err))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|err| PtyError::new(PtyOperation::CloneReader, err))?;
        let writer = SharedWriter::new(
            pair.master
                .take_writer()
                .map_err(|err| PtyError::new(PtyOperation::TakeWriter, err))?,
        );

        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => {
                        if let Some(wake) = wake_on_output.as_ref() {
                            wake();
                        }
                        break;
                    }
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                        if let Some(wake) = wake_on_output.as_ref() {
                            wake();
                        }
                    }
                }
            }
        });

        Ok(Self {
            child,
            master: pair.master,
            writer,
            rx,
        })
    }

    pub(super) fn writer(&self) -> SharedWriter {
        self.writer.clone()
    }

    pub(super) fn resize(&self, cols: usize, rows: usize) -> Result<(), PtyError> {
        self.master
            .resize(pty_size(cols, rows))
            .map_err(|err| PtyError::new(PtyOperation::Resize, err))
    }

    pub(super) fn write_input(&self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_bytes(data)
    }

    pub(super) fn try_read(&self) -> PtyRead {
        match self.rx.try_recv() {
            Ok(bytes) => PtyRead::Data(bytes),
            Err(TryRecvError::Empty) => PtyRead::Empty,
            Err(TryRecvError::Disconnected) => PtyRead::Disconnected,
        }
    }

    pub(super) fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        self.child.try_wait()
    }
}

fn pty_size(cols: usize, rows: usize) -> PtySize {
    PtySize {
        rows: rows.max(1) as u16,
        cols: cols.max(1) as u16,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn default_shell() -> String {
    #[cfg(windows)]
    {
        resolve_shell(std::env::var("COMSPEC").ok(), "cmd.exe")
    }

    #[cfg(not(windows))]
    {
        resolve_shell(std::env::var("SHELL").ok(), "/bin/sh")
    }
}

fn resolve_shell(shell: Option<String>, fallback: &str) -> String {
    shell
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::{PtyError, PtyOperation, resolve_shell};
    use std::error::Error;
    use std::io;

    #[test]
    fn resolve_shell_falls_back_for_missing_or_empty_values() {
        assert_eq!(resolve_shell(None, "/bin/sh"), "/bin/sh");
        assert_eq!(resolve_shell(Some(String::new()), "/bin/sh"), "/bin/sh");
        assert_eq!(resolve_shell(Some("   ".to_string()), "/bin/sh"), "/bin/sh");
    }

    #[test]
    fn resolve_shell_preserves_non_empty_value() {
        assert_eq!(
            resolve_shell(Some("/bin/zsh".to_string()), "/bin/sh"),
            "/bin/zsh"
        );
    }

    #[test]
    fn pty_error_preserves_operation_and_source() {
        let err = PtyError::new(PtyOperation::Resize, io::Error::other("boom"));

        assert_eq!(err.operation(), PtyOperation::Resize);
        assert_eq!(err.to_string(), "failed to resize PTY: boom");
        assert_eq!(
            err.source()
                .expect("pty error should expose a source")
                .to_string(),
            "boom"
        );
    }
}
