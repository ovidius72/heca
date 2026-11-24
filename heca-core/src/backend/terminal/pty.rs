use super::ShellIntegrationAssets;
use super::engine::SharedWriter;
use anyhow::Error as AnyError;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::Read;
use std::path::Path;
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
    AgentSetup,
    CloneReader,
    TakeWriter,
    Resize,
}

impl PtyOperation {
    fn context(self) -> &'static str {
        match self {
            Self::OpenPty => "failed to open PTY",
            Self::Spawn => "failed to spawn PTY child",
            Self::AgentSetup => "failed to prepare agent reporting",
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
    pub(super) fn agent_setup(error: crate::agent::AgentError) -> Self {
        Self::new(PtyOperation::AgentSetup, error)
    }
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
    pub(super) launch: crate::backend::ProcessLaunch,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// `None` once the program has ended and the terminal was closed so its last output drains
    /// ([`finish_output`](Self::finish_output)).
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
    writer: SharedWriter,
    rx: Receiver<Vec<u8>>,
    /// The resolved shell path this PTY spawned (e.g. `/bin/zsh`). Used by the
    /// backend to report the foreground program name when the shell itself is
    /// foreground (`Idle`).
    shell: String,
}

/// A folder to start in must be a folder. Checked before the fork: the child's own `chdir` failing
/// does not fail the spawn, it just leaves a dead pane that exited with an error nobody sees.
fn check_cwd(cwd: Option<&Path>) -> Result<(), PtyError> {
    match cwd {
        Some(dir) if !dir.is_dir() => Err(PtyError::new(
            PtyOperation::Spawn,
            anyhow::anyhow!("{} is not a folder", dir.display()),
        )),
        _ => Ok(()),
    }
}

/// Process-launch inputs, kept together for shell and direct-command terminals.
pub(super) struct PtyLaunchOptions<'a> {
    pub(super) shell_override: Option<&'a str>,
    pub(super) shell_integration: Option<&'a ShellIntegrationAssets>,
    pub(super) env_clear: bool,
    pub(super) env: &'a [(String, String)],
    pub(super) cwd: Option<&'a Path>,
    pub(super) agents: Option<&'a crate::agent::AgentSession>,
}

impl PtyHandle {
    pub(super) fn command_launch(
        &self,
        command: &str,
        cwd: Option<&Path>,
    ) -> Result<crate::backend::ProcessLaunch, PtyError> {
        process_launch(&command_for_spawned_command(&self.shell, command, cwd), cwd)
    }

    pub(super) fn new_with_process(
        cols: usize,
        rows: usize,
        cell_px: (f32, f32),
        wake: Option<WakeCallback>,
        process: &crate::backend::ProcessLaunch,
        started_as: Option<&crate::backend::ProcessLaunch>,
    ) -> Result<Self, PtyError> {
        check_cwd(process.cwd.as_deref())?;
        // What is run — `started_as` when the process goes through a launcher — while `process`
        // stays what the pane records and saves.
        let run = started_as.unwrap_or(process);
        let mut command = CommandBuilder::new(&run.executable);
        command.args(&run.arguments);
        if let Some(cwd) = &process.cwd {
            command.cwd(cwd);
        }
        let pair = native_pty_system()
            .openpty(pty_size(cols, rows, cell_px))
            .map_err(|error| PtyError::new(PtyOperation::OpenPty, error))?;
        Self::spawn_with_command_builder(
            pair,
            process.executable.display().to_string(),
            command,
            process.clone(),
            wake,
        )
    }
    pub(super) fn new_with_shell(
        cols: usize,
        rows: usize,
        cell_px: (f32, f32),
        wake_on_output: Option<WakeCallback>,
        options: PtyLaunchOptions<'_>,
    ) -> Result<Self, PtyError> {
        check_cwd(options.cwd)?;
        let pty_system = native_pty_system();
        let size = pty_size(cols, rows, cell_px);
        let pair = pty_system
            .openpty(size)
            .map_err(|err| PtyError::new(PtyOperation::OpenPty, err))?;

        let shell = options
            .shell_override
            .map(str::to_string)
            .unwrap_or_else(default_shell);
        let mut cmd = command_for_shell(
            &shell,
            options.shell_integration,
            options.env_clear,
            options.env,
            options.cwd,
        );
        let original = command_for_shell(&shell, None, options.env_clear, options.env, options.cwd);
        if let Some(agents) = options.agents {
            cmd.env("HECA_AGENT_BIN", agents.bin());
        }
        Self::spawn_with_command_builder(
            pair,
            shell,
            cmd,
            process_launch(&original, options.cwd)?,
            wake_on_output,
        )
    }

    pub(super) fn new_with_command(
        cols: usize,
        rows: usize,
        cell_px: (f32, f32),
        wake_on_output: Option<WakeCallback>,
        command: &str,
        options: PtyLaunchOptions<'_>,
    ) -> Result<Self, PtyError> {
        check_cwd(options.cwd)?;
        let pty_system = native_pty_system();
        let size = pty_size(cols, rows, cell_px);
        let pair = pty_system
            .openpty(size)
            .map_err(|err| PtyError::new(PtyOperation::OpenPty, err))?;

        // An explicit shell wins over `$SHELL`: a caller that needs the same behaviour on every
        // machine cannot have the user's interactive rc in the way.
        let shell = options
            .shell_override
            .map(str::to_string)
            .unwrap_or_else(default_shell);
        let prepared = options
            .agents
            .map(|agents| agent_command(&shell, &agents.bin(), command));
        let cmd = command_for_spawned_command(
            &shell,
            prepared.as_deref().unwrap_or(command),
            options.cwd,
        );
        let original = command_for_spawned_command(&shell, command, options.cwd);
        Self::spawn_with_command_builder(
            pair,
            shell,
            cmd,
            process_launch(&original, options.cwd)?,
            wake_on_output,
        )
    }

    fn spawn_with_command_builder(
        pair: portable_pty::PtyPair,
        shell: String,
        mut cmd: CommandBuilder,
        launch: crate::backend::ProcessLaunch,
        wake_on_output: Option<WakeCallback>,
    ) -> Result<Self, PtyError> {
        strip_host_only(&mut cmd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        // A shell with no locale miscounts every non-ASCII character in its prompt — see `locale`.
        if let Some(lang) = super::locale::missing_lang() {
            cmd.env("LANG", lang);
        }
        // Advertise WezTerm-compatible image support. heca embeds wezterm-term and
        // genuinely renders the same inline-image protocols WezTerm does (iTerm2
        // `OSC 1337` + Sixel), but tools pick their image protocol by sniffing the
        // terminal identity. Without a recognized identity, image viewers like Yazi
        // fall back to Kitty Unicode placeholders — a mode no WezTerm release
        // implements either, which renders as garbage. Presenting as WezTerm makes
        // them choose the iTerm2 protocol heca supports, exactly as they do in real
        // WezTerm. See `terminal-09`.
        cmd.env("TERM_PROGRAM", "WezTerm");
        cmd.env("TERM_PROGRAM_VERSION", "20240203-110809-5046fc22");

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
                    // End of file: the slave side is closed for good.
                    Ok(0) => {
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
                    // **A recoverable read error is not the end of the terminal.** Read on again;
                    // see `read_ends_the_reader` for why this is not a detail.
                    Err(err) if !read_ends_the_reader(&err) => continue,
                    Err(_) => {
                        if let Some(wake) = wake_on_output.as_ref() {
                            wake();
                        }
                        break;
                    }
                }
            }
        });

        Ok(Self {
            launch,
            child,
            master: Some(pair.master),
            writer,
            rx,
            shell,
        })
    }

    /// **Let the last output of an ended program reach end of file.** On unix the terminal hangs up
    /// by itself when the program's side closes, so the reader sees end of file with nothing done
    /// here. A Windows pseudo console keeps its output open until it is closed, so the reader would
    /// never see end of file: it is closed here, which flushes what the program wrote and then ends
    /// the reader.
    pub(super) fn finish_output(&mut self) {
        if cfg!(windows) {
            self.master = None;
        }
    }

    pub(super) fn writer(&self) -> SharedWriter {
        self.writer.clone()
    }

    /// The resolved shell path this PTY spawned (e.g. `/bin/zsh`).
    #[cfg(unix)]
    pub(super) fn shell_path(&self) -> &str {
        &self.shell
    }

    /// The PTY child's process-group leader pid (the shell's pgrp). `None` where
    /// the platform exposes no such accessor.
    #[cfg(unix)]
    pub(super) fn process_group_leader(&self) -> Option<libc::pid_t> {
        self.master.as_ref()?.process_group_leader()
    }

    /// The PTY child's own process id — the shell or program heca started. Unlike
    /// [`process_group_leader`](Self::process_group_leader), it does not move when that shell
    /// hands the terminal to a job it starts.
    // Only the unix reporting test reads it.
    #[cfg(all(test, unix))]
    pub(super) fn child_pid(&self) -> Option<u32> {
        self.child.process_id()
    }

    /// The raw fd of the PTY master, for `tcgetpgrp` (foreground pgrp lookup).
    /// `None` where the platform exposes no such accessor.
    #[cfg(unix)]
    pub(super) fn as_raw_fd(&self) -> Option<std::os::unix::io::RawFd> {
        self.master.as_ref()?.as_raw_fd()
    }

    pub(super) fn resize(
        &self,
        cols: usize,
        rows: usize,
        cell_px: (f32, f32),
    ) -> Result<(), PtyError> {
        // A terminal closed after its program ended has nothing left to resize.
        let Some(master) = &self.master else {
            return Ok(());
        };
        master
            .resize(pty_size(cols, rows, cell_px))
            .map_err(|err| PtyError::new(PtyOperation::Resize, err))
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

fn process_launch(
    command: &CommandBuilder,
    cwd: Option<&Path>,
) -> Result<crate::backend::ProcessLaunch, PtyError> {
    let [executable, arguments @ ..] = command.get_argv().as_slice() else {
        return Err(PtyError::new(
            PtyOperation::Spawn,
            anyhow::anyhow!("missing executable"),
        ));
    };
    let cwd = cwd
        .map(Path::to_owned)
        .or_else(|| std::env::current_dir().ok());
    Ok(crate::backend::ProcessLaunch {
        executable: executable.into(),
        arguments: arguments.to_vec(),
        cwd,
    })
}

fn command_for_shell(
    shell: &str,
    shell_integration: Option<&ShellIntegrationAssets>,
    env_clear: bool,
    env: &[(String, String)],
    cwd: Option<&Path>,
) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    if let Some(cwd) = cwd {
        cmd.cwd(cwd);
    }
    if env_clear {
        cmd.env_clear();
    }
    for (key, value) in env {
        cmd.env(key, value);
    }
    let kind = shell_kind(shell);
    let Some(assets) = shell_integration else {
        if LOGIN_SHELL && kind.is_some() {
            cmd.arg("-l");
        }
        return cmd;
    };

    match kind {
        Some(ShellKind::Bash) => {
            // `--init-file` is ignored by a login bash, so the init file itself reads the login
            // files when asked to (`HECA_LOGIN_SHELL`).
            if LOGIN_SHELL {
                cmd.env("HECA_LOGIN_SHELL", "1");
            }
            cmd.arg("--init-file");
            cmd.arg(&assets.bash_init);
        }
        Some(ShellKind::Zsh) => {
            if LOGIN_SHELL {
                cmd.arg("-l");
            }
            cmd.env(
                "OLD_ZDOTDIR",
                users_zdotdir(std::env::var_os("ZDOTDIR"), std::env::var_os("OLD_ZDOTDIR")),
            );
            cmd.env("ZDOTDIR", &assets.zsh_zdotdir);
        }
        Some(ShellKind::Fish) => {
            if LOGIN_SHELL {
                cmd.arg("-l");
            }
            cmd.arg("-C");
            cmd.arg(format!("source {}", assets.fish_init.display()));
        }
        None => {}
    }

    cmd
}

/// **Whether the user's shell starts as a login shell** — on macOS, as Terminal.app and iTerm2 do.
///
/// An app started from the Dock or the Finder gets launchd's bare `PATH`, not the one a terminal
/// has; the user's `PATH` (Homebrew's `brew shellenv`, for one) is set in the login files
/// (`~/.zprofile`, `~/.bash_profile`, fish's login config). A login shell reads them, so a command
/// typed in heca is found however heca was started. Linux desktops start a session from a login
/// shell already, and terminals there start ordinary shells.
const LOGIN_SHELL: bool = cfg!(target_os = "macos");

fn command_for_spawned_command(shell: &str, command: &str, cwd: Option<&Path>) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    if let Some(cwd) = cwd {
        cmd.cwd(cwd);
    }
    #[cfg(windows)]
    {
        cmd.arg("/C");
        cmd.arg(command);
    }
    #[cfg(not(windows))]
    {
        // Use interactive command mode so terminal-first programs keep shell UX
        // features such as job control and interactive rc sourcing — and a login shell where
        // the user's shell starts as one, so the command is found on the user's `PATH`.
        if LOGIN_SHELL && shell_kind(shell).is_some() {
            cmd.arg("-l");
        }
        cmd.arg("-ic");
        cmd.arg(command);
    }
    cmd
}

#[cfg(not(windows))]
fn agent_command(shell: &str, bin: &Path, command: &str) -> String {
    let bin = crate::agent::quote(&bin.to_string_lossy());
    if shell_kind(shell) == Some(ShellKind::Fish) {
        format!("set -gx PATH {bin} $PATH; {command}")
    } else {
        format!("export PATH={bin}:\"$PATH\"; {command}")
    }
}

#[cfg(windows)]
fn agent_command(_shell: &str, _bin: &Path, command: &str) -> String {
    // The reporting wrappers use Unix shell integration. Keep Windows commands unchanged.
    command.to_owned()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShellKind {
    Bash,
    Zsh,
    Fish,
}

fn shell_kind(shell: &str) -> Option<ShellKind> {
    match Path::new(shell).file_name()?.to_str()? {
        name if name.starts_with("bash") => Some(ShellKind::Bash),
        name if name.starts_with("zsh") => Some(ShellKind::Zsh),
        name if name.starts_with("fish") => Some(ShellKind::Fish),
        _ => None,
    }
}

/// Whether a failed read from the PTY master means the terminal is **over**, or merely that this
/// one call did not complete.
///
/// **Getting this wrong kills the user's shell.** The reader thread owns the only cloned read handle
/// on the master; when it returns, that handle drops, the kernel hangs up the terminal, and the
/// child's process group is sent `SIGHUP`. So ending the thread does not just stop reading — it
/// **terminates whatever was running in the pane**, and heca then closes the pane because its child
/// exited.
///
/// It used to end on **any** error. `EINTR` is the ordinary case: a signal delivered while a read is
/// blocked interrupts it, and the caller is expected to read again. Resizing a PTY raises `SIGWINCH`
/// on the child's process group, and dragging a pane divider with the mouse resizes on **every
/// frame** — hundreds of times a second. One interrupted read in that storm was enough to take the
/// pane down, which is why the keyboard resize (a handful of calls) never did it and the mouse did
/// it "after a few attempts".
///
/// `Interrupted` and `WouldBlock` are retried; everything else is treated as fatal, as before.
fn read_ends_the_reader(err: &std::io::Error) -> bool {
    !matches!(
        err.kind(),
        std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
    )
}

fn pty_size(cols: usize, rows: usize, cell_px: (f32, f32)) -> PtySize {
    let cols = cols.max(1);
    let rows = rows.max(1);
    // Report pixel dimensions in the PTY winsize (`TIOCSWINSZ`). Programs read
    // them back via `ioctl(TIOCGWINSZ)` to size inline images — `kitten icat`
    // errors outright without them, and Yazi's kitty/sixel previewers need them
    // to render at all. A per-cell size of at least 1px keeps the totals valid.
    let cell_w = cell_px.0.round().max(1.0) as usize;
    let cell_h = cell_px.1.round().max(1.0) as usize;
    PtySize {
        rows: rows as u16,
        cols: cols as u16,
        pixel_width: (cols * cell_w).min(u16::MAX as usize) as u16,
        pixel_height: (rows * cell_h).min(u16::MAX as usize) as u16,
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

/// **The user's own zsh config folder**, for heca's startup files to read first.
///
/// It is the inherited `ZDOTDIR` — unless that is a heca folder ([`is_heca_zsh_folder`](super::launch::is_heca_zsh_folder): this
/// heca's, or that of any heca whose pane started this one, whatever its version), which is what a
/// shell started inside a heca pane inherits. Handing a heca folder on would make heca's `.zshenv`
/// read heca's again — itself, until zsh gives up ("job table full or recursion limit exceeded") —
/// so the folder the outer heca saved in `OLD_ZDOTDIR` is the user's.
fn users_zdotdir(
    inherited: Option<std::ffi::OsString>,
    saved: Option<std::ffi::OsString>,
) -> std::ffi::OsString {
    let is_heca =
        |dir: &std::ffi::OsStr| super::launch::is_heca_zsh_folder(std::path::Path::new(dir));
    match inherited {
        Some(dir) if is_heca(&dir) => saved.filter(|saved| !is_heca(saved)).unwrap_or_default(),
        Some(dir) => dir,
        None => Default::default(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_shell_started_inside_heca_still_reads_the_users_own_zsh_folder() {
        use std::ffi::OsString;
        let root = tempfile::tempdir().unwrap();
        let heca = super::super::launch::ShellIntegrationAssets::write_to(root.path())
            .unwrap()
            .zsh_zdotdir;
        let heca = heca.to_str().unwrap();
        let own = root.path().join("own");
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(own.join(".zshenv"), "export EDITOR=vim\n").unwrap();
        let own = own.to_str().unwrap();
        let users = |inherited: Option<&str>, saved: Option<&str>| {
            super::users_zdotdir(inherited.map(OsString::from), saved.map(OsString::from))
        };
        assert_eq!(users(None, None), "");
        assert_eq!(users(Some(own), None), own);
        assert_eq!(
            users(Some(heca), Some(own)),
            own,
            "inside a heca pane, the folder the outer heca saved"
        );
        assert_eq!(users(Some(heca), Some("")), "");
        assert_eq!(users(Some(heca), Some(heca)), "");
    }

    use super::read_ends_the_reader;

    /// **An interrupted read must not end the reader thread**.
    ///
    /// The thread owns the only cloned read handle on the PTY master. When it returns, that handle
    /// drops, the kernel hangs up the terminal and the child's process group gets `SIGHUP` — so
    /// ending the thread *kills whatever is running in the pane*, and heca then closes the pane
    /// because its child exited.
    ///
    /// `EINTR` is routine: a signal arriving while a read is blocked interrupts it and the caller
    /// reads again. Resizing a PTY raises `SIGWINCH`, and a mouse divider drag resizes on every
    /// frame — so a pane would vanish mid-drag after a few attempts, while the keyboard resize
    /// never did it.
    #[test]
    fn an_interrupted_read_does_not_end_the_reader() {
        assert!(
            !read_ends_the_reader(&io::Error::from(io::ErrorKind::Interrupted)),
            "EINTR means read again — ending here SIGHUPs the user's shell",
        );
        assert!(
            !read_ends_the_reader(&io::Error::from(io::ErrorKind::WouldBlock)),
            "nothing to read yet is not the end of the terminal",
        );
    }

    /// The other half: a genuinely broken master still ends the reader, so a dead PTY does not
    /// leave a thread spinning on it forever.
    #[test]
    fn a_real_read_failure_still_ends_the_reader() {
        for kind in [
            io::ErrorKind::BrokenPipe,
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::Other,
        ] {
            assert!(
                read_ends_the_reader(&io::Error::from(kind)),
                "{kind:?} is not recoverable — the reader must stop",
            );
        }
    }

    use super::{
        PtyError, PtyOperation, ShellKind, command_for_spawned_command, resolve_shell, shell_kind,
    };
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
    fn shell_kind_detects_supported_shell_wrappers() {
        assert_eq!(shell_kind("/bin/bash"), Some(ShellKind::Bash));
        assert_eq!(shell_kind("/usr/local/bin/bash-5.2"), Some(ShellKind::Bash));
        assert_eq!(shell_kind("/opt/homebrew/bin/zsh"), Some(ShellKind::Zsh));
        assert_eq!(shell_kind("/usr/bin/fish"), Some(ShellKind::Fish));
        assert_eq!(shell_kind("/bin/sh"), None);
    }

    #[test]
    fn command_for_spawned_command_uses_shell_execution() {
        let rendered = format!(
            "{:?}",
            command_for_spawned_command("/bin/sh", "lazygit", None)
        );
        #[cfg(windows)]
        assert!(rendered.contains("/C"));
        #[cfg(not(windows))]
        assert!(rendered.contains("-ic"));
        assert!(rendered.contains("lazygit"));
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

    mod login_shell {
        use super::super::{LOGIN_SHELL, ShellIntegrationAssets, command_for_shell};
        use std::path::PathBuf;

        fn assets() -> ShellIntegrationAssets {
            ShellIntegrationAssets {
                bash_init: PathBuf::from("/heca/bash_init.sh"),
                fish_init: PathBuf::from("/heca/fish_init.fish"),
                zsh_zdotdir: PathBuf::from("/heca/zsh"),
            }
        }

        fn argv(cmd: &portable_pty::CommandBuilder) -> Vec<String> {
            cmd.get_argv()
                .iter()
                .map(|a| a.to_string_lossy().into_owned())
                .collect()
        }

        /// **On macOS the user's shell is a login shell**, so it reads the login files (Homebrew's
        /// `PATH`, for one) however heca was started — from the Dock it gets launchd's bare `PATH`.
        /// Elsewhere it is an ordinary shell, as terminals there start it.
        #[test]
        fn the_users_shell_is_a_login_shell_where_terminals_start_one() {
            let assets = assets();
            for shell in ["/bin/zsh", "/opt/homebrew/bin/fish"] {
                let cmd = command_for_shell(shell, Some(&assets), false, &[], None);
                assert_eq!(
                    argv(&cmd).contains(&"-l".to_string()),
                    LOGIN_SHELL,
                    "{shell}: {:?}",
                    argv(&cmd)
                );
            }
            for shell in ["/bin/zsh", "/bin/bash", "/opt/homebrew/bin/fish"] {
                let bare = command_for_shell(shell, None, false, &[], None);
                assert_eq!(
                    argv(&bare).contains(&"-l".to_string()),
                    LOGIN_SHELL,
                    "{shell} without integration"
                );
            }
        }

        /// A login bash ignores `--init-file`, so heca keeps the init file and asks it to read the
        /// login files instead.
        #[test]
        fn bash_keeps_its_init_file_and_is_asked_to_read_the_login_files() {
            let assets = assets();
            let cmd = command_for_shell("/bin/bash", Some(&assets), false, &[], None);
            let args = argv(&cmd);
            assert!(!args.contains(&"-l".to_string()), "{args:?}");
            assert!(args.contains(&"--init-file".to_string()), "{args:?}");
            assert_eq!(cmd.get_env("HECA_LOGIN_SHELL").is_some(), LOGIN_SHELL);
        }

        /// A command run in a pane (lazygit, an agent) is found on the same `PATH` as one typed.
        #[cfg(not(windows))]
        #[test]
        fn a_command_runs_under_the_same_kind_of_shell() {
            let cmd = super::super::command_for_spawned_command("/bin/zsh", "lazygit", None);
            assert_eq!(
                argv(&cmd).contains(&"-l".to_string()),
                LOGIN_SHELL,
                "{:?}",
                argv(&cmd)
            );
            assert!(argv(&cmd).ends_with(&["-ic".to_string(), "lazygit".to_string()]));
        }
    }
}

/// The environment variables no program in a pane inherits, as the host named them.
static HOST_ONLY_ENV: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// Remove from `cmd` what belongs to the host and to no program it starts (`keep_from_panes`) —
/// inherited or set, it is gone.
fn strip_host_only(cmd: &mut CommandBuilder) {
    for name in HOST_ONLY_ENV.get().into_iter().flatten() {
        cmd.env_remove(name);
    }
}

/// **Keep these variables from every program started in a pane** — the host's own settings (where
/// its config and data live), which a program, or a heca started inside a pane, must not see. Said
/// once, as the host starts; the names are the host's, this only removes them.
pub fn keep_from_panes(names: &[&str]) {
    let _ = HOST_ONLY_ENV.set(names.iter().map(|name| name.to_string()).collect());
}

#[cfg(test)]
mod host_only_tests {
    use super::*;

    /// **What the host keeps from panes never reaches a program in one** — inherited from the host's
    /// own environment or set on the way, it is removed; everything else passes through.
    #[test]
    fn what_the_host_keeps_from_panes_never_reaches_a_program() {
        keep_from_panes(&["HECA_T588_HOST_ONLY"]);
        let mut cmd = CommandBuilder::new("sh");
        cmd.env("HECA_T588_HOST_ONLY", "/tmp/somewhere");
        cmd.env("HECA_T588_PASSES", "1");
        strip_host_only(&mut cmd);
        assert_eq!(cmd.get_env("HECA_T588_HOST_ONLY"), None, "removed");
        assert!(
            cmd.get_env("HECA_T588_PASSES").is_some(),
            "the rest passes through"
        );
    }
}
