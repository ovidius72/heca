//! Terminal backend using vte for VT parsing and libc PTY on Unix.

use super::{
    BackendRenderData, PaneBackend, PaneType, TerminalCell, TerminalCursor, TerminalDamage,
    TerminalLine, TerminalSnapshot,
};
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver};

/// ANSI color palette (0-15).
const ANSI_COLORS: [[u8; 3]; 16] = [
    [0x00, 0x00, 0x00], // 0: black
    [0xcd, 0x00, 0x00], // 1: red
    [0x00, 0xcd, 0x00], // 2: green
    [0xcd, 0xcd, 0x00], // 3: yellow
    [0x00, 0x00, 0xee], // 4: blue
    [0xcd, 0x00, 0xcd], // 5: magenta
    [0x00, 0xcd, 0xcd], // 6: cyan
    [0xe5, 0xe5, 0xe5], // 7: white
    [0x7f, 0x7f, 0x7f], // 8: bright black
    [0xff, 0x00, 0x00], // 9: bright red
    [0x00, 0xff, 0x00], // 10: bright green
    [0xff, 0xff, 0x00], // 11: bright yellow
    [0x5c, 0x5c, 0xff], // 12: bright blue
    [0xff, 0x00, 0xff], // 13: bright magenta
    [0x00, 0xff, 0xff], // 14: bright cyan
    [0xff, 0xff, 0xff], // 15: bright white
];

fn ansi_to_rgba(idx: u8) -> [f32; 4] {
    let c = ANSI_COLORS[(idx % 16) as usize];
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

fn default_fg() -> [f32; 4] {
    [0.9, 0.9, 0.9, 1.0]
}

fn default_bg() -> [f32; 4] {
    [0.0, 0.0, 0.0, 0.0]
}

/// A single cell in the terminal grid.
#[derive(Clone, Copy)]
struct Cell {
    c: char,
    fg: [f32; 4],
    bg: [f32; 4],
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: default_fg(),
            bg: default_bg(),
        }
    }
}

/// Simple terminal grid that implements vte::Perform.
struct Grid {
    cells: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
    width: usize,
    height: usize,
    current_fg: [f32; 4],
    current_bg: [f32; 4],
    bold: bool,
    title: String,
    scroll_region_top: usize,
    scroll_region_bottom: usize,
}

impl Grid {
    fn new(width: usize, height: usize) -> Self {
        Self {
            cells: vec![vec![Cell::default(); width]; height],
            cursor_row: 0,
            cursor_col: 0,
            width,
            height,
            current_fg: default_fg(),
            current_bg: default_bg(),
            bold: false,
            title: "Terminal".to_string(),
            scroll_region_top: 0,
            scroll_region_bottom: height,
        }
    }

    fn resize(&mut self, width: usize, height: usize) {
        let mut new_cells = vec![vec![Cell::default(); width]; height];
        for (row, new_row) in new_cells
            .iter_mut()
            .enumerate()
            .take(self.height.min(height))
        {
            for (col, cell) in new_row.iter_mut().enumerate().take(self.width.min(width)) {
                *cell = self.cells[row][col];
            }
        }
        self.cells = new_cells;
        self.width = width;
        self.height = height;
        self.cursor_row = self.cursor_row.min(height.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(width.saturating_sub(1));
        self.scroll_region_top = 0;
        self.scroll_region_bottom = height;
    }

    fn put_char(&mut self, c: char) {
        if self.cursor_row < self.height && self.cursor_col < self.width {
            let mut fg = self.current_fg;
            if self.bold {
                fg[0] = (fg[0] * 1.2).min(1.0);
                fg[1] = (fg[1] * 1.2).min(1.0);
                fg[2] = (fg[2] * 1.2).min(1.0);
            }
            self.cells[self.cursor_row][self.cursor_col] = Cell {
                c,
                fg,
                bg: self.current_bg,
            };
            self.cursor_col += 1;
            if self.cursor_col >= self.width {
                self.cursor_col = 0;
                self.cursor_row += 1;
                if self.cursor_row >= self.scroll_region_bottom {
                    self.scroll_up(1);
                    self.cursor_row = self.scroll_region_bottom.saturating_sub(1);
                }
            }
        }
    }

    fn scroll_up(&mut self, n: usize) {
        let n = n.min(self.scroll_region_bottom - self.scroll_region_top);
        for _ in 0..n {
            let top = self.scroll_region_top;
            let bottom = self.scroll_region_bottom;
            for row in top..bottom.saturating_sub(1) {
                self.cells[row] = self.cells[row + 1].clone();
            }
            if bottom > 0 {
                self.cells[bottom - 1] = vec![Cell::default(); self.width];
            }
        }
    }

    fn clear_line_right(&mut self) {
        if self.cursor_row < self.height {
            for col in self.cursor_col..self.width {
                self.cells[self.cursor_row][col] = Cell::default();
            }
        }
    }

    fn clear_line_left(&mut self) {
        if self.cursor_row < self.height {
            for col in 0..=self.cursor_col {
                self.cells[self.cursor_row][col] = Cell::default();
            }
        }
    }

    fn clear_line_all(&mut self) {
        if self.cursor_row < self.height {
            for col in 0..self.width {
                self.cells[self.cursor_row][col] = Cell::default();
            }
        }
    }

    fn clear_screen(&mut self) {
        for row in 0..self.height {
            for col in 0..self.width {
                self.cells[row][col] = Cell::default();
            }
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    fn apply_sgr(&mut self, values: &[u16]) {
        let mut i = 0;
        while i < values.len() {
            match values.get(i).copied().unwrap_or(0) {
                0 => {
                    self.current_fg = default_fg();
                    self.current_bg = default_bg();
                    self.bold = false;
                }
                1 => self.bold = true,
                22 => self.bold = false,
                n @ 30..=37 => self.current_fg = ansi_to_rgba((n - 30) as u8),
                n @ 40..=47 => self.current_bg = ansi_to_rgba((n - 40) as u8),
                n @ 90..=97 => self.current_fg = ansi_to_rgba((n - 90 + 8) as u8),
                n @ 100..=107 => self.current_bg = ansi_to_rgba((n - 100 + 8) as u8),
                38 => {
                    if let Some(mode) = values.get(i + 1).copied() {
                        match mode {
                            5 => {
                                if let Some(idx) = values.get(i + 2).copied() {
                                    self.current_fg = ansi_to_rgba((idx % 256) as u8);
                                }
                                i += 2;
                            }
                            2 => {
                                if let (Some(r), Some(g), Some(b)) =
                                    (values.get(i + 2), values.get(i + 3), values.get(i + 4))
                                {
                                    self.current_fg = [
                                        (*r as f32 / 255.0).clamp(0.0, 1.0),
                                        (*g as f32 / 255.0).clamp(0.0, 1.0),
                                        (*b as f32 / 255.0).clamp(0.0, 1.0),
                                        1.0,
                                    ];
                                }
                                i += 4;
                            }
                            _ => {}
                        }
                    }
                }
                48 => {
                    if let Some(mode) = values.get(i + 1).copied() {
                        match mode {
                            5 => {
                                if let Some(idx) = values.get(i + 2).copied() {
                                    self.current_bg = ansi_to_rgba((idx % 256) as u8);
                                }
                                i += 2;
                            }
                            2 => {
                                if let (Some(r), Some(g), Some(b)) =
                                    (values.get(i + 2), values.get(i + 3), values.get(i + 4))
                                {
                                    self.current_bg = [
                                        (*r as f32 / 255.0).clamp(0.0, 1.0),
                                        (*g as f32 / 255.0).clamp(0.0, 1.0),
                                        (*b as f32 / 255.0).clamp(0.0, 1.0),
                                        1.0,
                                    ];
                                }
                                i += 4;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

impl vte::Perform for Grid {
    fn print(&mut self, c: char) {
        self.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\r' => self.cursor_col = 0,
            b'\n' => {
                self.cursor_row += 1;
                if self.cursor_row >= self.scroll_region_bottom {
                    self.scroll_up(1);
                    self.cursor_row = self.scroll_region_bottom.saturating_sub(1);
                }
            }
            b'\t' => {
                let next_tab = (self.cursor_col / 8 + 1) * 8;
                self.cursor_col = next_tab.min(self.width - 1);
            }
            0x08 if self.cursor_col > 0 => {
                self.cursor_col -= 1;
            }
            _ => {}
        }
    }

    fn hook(&mut self, _params: &vte::Params, _intermediates: &[u8], _ignore: bool, _c: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if let Some(first) = params.first()
            && (first.starts_with(b"0;") || first.starts_with(b"2;"))
        {
            let title_bytes = &first[2..];
            if let Ok(s) = std::str::from_utf8(title_bytes) {
                self.title = s.to_string();
            }
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        _intermediates: &[u8],
        _ignore: bool,
        c: char,
    ) {
        let values: Vec<u16> = params.iter().map(|p| p[0]).collect();
        match c {
            'A' => {
                let n = values.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            'B' => {
                let n = values.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_row = (self.cursor_row + n).min(self.height.saturating_sub(1));
            }
            'C' => {
                let n = values.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_col = (self.cursor_col + n).min(self.width.saturating_sub(1));
            }
            'D' => {
                let n = values.first().copied().unwrap_or(1).max(1) as usize;
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            'H' | 'f' => {
                let row = values.first().copied().unwrap_or(1).max(1) as usize - 1;
                let col = values.get(1).copied().unwrap_or(1).max(1) as usize - 1;
                self.cursor_row = row.min(self.height.saturating_sub(1));
                self.cursor_col = col.min(self.width.saturating_sub(1));
            }
            'J' => {
                let mode = values.first().copied().unwrap_or(0);
                match mode {
                    0 => {
                        for col in self.cursor_col..self.width {
                            self.cells[self.cursor_row][col] = Cell::default();
                        }
                        for row in self.cursor_row + 1..self.height {
                            for col in 0..self.width {
                                self.cells[row][col] = Cell::default();
                            }
                        }
                    }
                    2 => self.clear_screen(),
                    _ => {}
                }
            }
            'K' => {
                let mode = values.first().copied().unwrap_or(0);
                match mode {
                    0 => self.clear_line_right(),
                    1 => self.clear_line_left(),
                    2 => self.clear_line_all(),
                    _ => {}
                }
            }
            'm' => self.apply_sgr(&values),
            'r' => {
                let top = values.first().copied().unwrap_or(1).max(1) as usize - 1;
                let bottom = values.get(1).copied().unwrap_or(self.height as u16).max(1) as usize;
                self.scroll_region_top = top.min(self.height);
                self.scroll_region_bottom = bottom.min(self.height).max(self.scroll_region_top + 1);
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

/// Errors that can occur when creating a PTY.
#[derive(Debug)]
pub enum PtyError {
    /// The `openpty` call failed (Unix).
    OpenPtyFailed,
    /// The `dup` call failed on a slave file descriptor (Unix).
    DupSlaveFailed,
    /// Could not spawn the child shell process.
    SpawnFailed(std::io::Error),
    /// Could not dup the master FD for the reader thread (Unix).
    DupMasterFailed,
    /// Could not take stdin/stdout from the child (non-Unix stub).
    NoStdio(&'static str),
}

impl std::fmt::Display for PtyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PtyError::OpenPtyFailed => write!(f, "openpty failed"),
            PtyError::DupSlaveFailed => write!(f, "dup slave fd failed"),
            PtyError::SpawnFailed(e) => write!(f, "spawn failed: {e}"),
            PtyError::DupMasterFailed => write!(f, "failed to dup PTY master fd"),
            PtyError::NoStdio(which) => write!(f, "no {which} from child process"),
        }
    }
}

impl std::error::Error for PtyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PtyError::SpawnFailed(e) => Some(e),
            PtyError::OpenPtyFailed => None,
            PtyError::DupSlaveFailed => None,
            PtyError::DupMasterFailed => None,
            PtyError::NoStdio(_) => None,
        }
    }
}

/// Cross-platform PTY handle.
enum PtyHandle {
    #[cfg(unix)]
    Unix { master: std::os::fd::RawFd },
    #[cfg(not(unix))]
    Stub {
        child: std::process::Child,
        stdin: std::process::ChildStdin,
        stdout: std::process::ChildStdout,
    },
}

impl PtyHandle {
    #[cfg(unix)]
    fn new_unix(cols: u16, rows: u16) -> Result<Self, PtyError> {
        use std::os::fd::FromRawFd;

        let mut master: libc::c_int = 0;
        let mut slave: libc::c_int = 0;

        // SAFETY: openpty is a POSIX syscall. The pointers to master/slave are valid
        // stack-local i32s. The termios and winsize pointers are NULL (use defaults).
        // The returned fds are checked for <0 (error) before use.
        unsafe {
            if libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) < 0
            {
                return Err(PtyError::OpenPtyFailed);
            }
        }

        let ws = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: master is a valid open PTY fd from openpty. ioctl(TIOCSWINSZ)
        // sets the terminal window size; &ws is a valid pointer to a winsize struct.
        unsafe {
            libc::ioctl(master, libc::TIOCSWINSZ, &ws);
        }

        let shell = std::env::var("SHELL").unwrap_or_else(|_| {
            if std::path::Path::new("/bin/bash").exists() {
                "/bin/bash".to_string()
            } else {
                "/bin/sh".to_string()
            }
        });

        // SAFETY: slave is a valid open fd from openpty. dup() creates a new fd
        // referring to the same PTY slave. Each dup result is checked for <0 below.
        let slave_in = unsafe { libc::dup(slave) };
        let slave_out = unsafe { libc::dup(slave) };
        let slave_err = unsafe { libc::dup(slave) };
        if slave_in < 0 || slave_out < 0 || slave_err < 0 {
            return Err(PtyError::DupSlaveFailed);
        }

        use std::os::unix::process::CommandExt;
        let _child = std::process::Command::new(&shell)
            .arg0(&shell)
            // SAFETY: slave_in/out/err are valid dup'd fds. from_raw_fd takes ownership;
            // the child process inherits these as its stdin/stdout/stderr.
            .stdin(unsafe { std::process::Stdio::from_raw_fd(slave_in) })
            .stdout(unsafe { std::process::Stdio::from_raw_fd(slave_out) })
            .stderr(unsafe { std::process::Stdio::from_raw_fd(slave_err) })
            .spawn()
            .map_err(PtyError::SpawnFailed)?;

        // SAFETY: slave is no longer needed after the child has inherited it.
        // Closing it here prevents the parent from accidentally leaking the slave fd.
        unsafe {
            libc::close(slave);
        }

        Ok(Self::Unix { master })
    }

    #[cfg(not(unix))]
    fn new_stub() -> Result<Self, PtyError> {
        let mut child = std::process::Command::new("cmd.exe")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(PtyError::SpawnFailed)?;

        let stdin = child.stdin.take().ok_or(PtyError::NoStdio("stdin"))?;
        let stdout = child.stdout.take().ok_or(PtyError::NoStdio("stdout"))?;

        Ok(Self::Stub {
            child,
            stdin,
            stdout,
        })
    }

    fn new(cols: u16, rows: u16) -> Result<Self, PtyError> {
        #[cfg(unix)]
        {
            Self::new_unix(cols, rows)
        }
        #[cfg(not(unix))]
        {
            let _ = (cols, rows);
            Self::new_stub()
        }
    }
}

impl Drop for PtyHandle {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let PtyHandle::Unix { master, .. } = self;
            // SAFETY: master is a valid open PTY fd. close() releases it; after this
            // the fd is no longer valid and must not be used.
            unsafe {
                libc::close(*master);
            }
        }
    }
}

impl Read for PtyHandle {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            PtyHandle::Unix { master, .. } => {
                // SAFETY: master is a valid open PTY fd. read() is a POSIX syscall;
                // buf is a valid mutable slice with length passed as the count argument.
                let n = unsafe { libc::read(*master, buf.as_mut_ptr() as *mut _, buf.len()) };
                if n < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(n as usize)
                }
            }
            #[cfg(not(unix))]
            PtyHandle::Stub { stdout, .. } => stdout.read(buf),
        }
    }
}

#[cfg(unix)]
impl std::os::fd::AsRawFd for PtyHandle {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        match self {
            PtyHandle::Unix { master, .. } => *master,
        }
    }
}

impl Write for PtyHandle {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            PtyHandle::Unix { master, .. } => {
                // SAFETY: master is a valid open PTY fd. write() is a POSIX syscall;
                // buf is a valid shared slice, ptr and length are correct.
                let n = unsafe { libc::write(*master, buf.as_ptr() as *const _, buf.len()) };
                if n < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(n as usize)
                }
            }
            #[cfg(not(unix))]
            PtyHandle::Stub { stdin, .. } => stdin.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            PtyHandle::Unix { .. } => Ok(()),
            #[cfg(not(unix))]
            PtyHandle::Stub { stdin, .. } => stdin.flush(),
        }
    }
}

/// A backend that runs a real shell inside a PTY, using vte for VT parsing.
pub struct TerminalBackend {
    grid: Grid,
    parser: vte::Parser,
    pty: PtyHandle,
    /// Channel receiving raw bytes from the PTY reader thread.
    pty_rx: Receiver<Vec<u8>>,
    cols: usize,
    rows: usize,
    exited: bool,
}

impl TerminalBackend {
    /// Spawn a new terminal with the given grid size.
    pub fn new(cols: usize, rows: usize) -> Result<Self, PtyError> {
        let pty = PtyHandle::new(cols as u16, rows as u16)?;

        // Spawn reader thread.
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        #[cfg(unix)]
        {
            use std::os::fd::{AsRawFd, FromRawFd};

            let raw_fd = pty.as_raw_fd();
            // SAFETY: raw_fd is a valid PTY master fd. dup creates a new fd for the
            // reader thread so the original isn't closed when the TerminalBackend is dropped.
            let duped = unsafe { libc::dup(raw_fd) };
            if duped < 0 {
                return Err(PtyError::DupMasterFailed);
            }
            // SAFETY: duped is a valid dup'd fd. from_raw_fd takes ownership, creating
            // a File that owns and will close this fd when dropped.
            let mut reader = unsafe { std::fs::File::from_raw_fd(duped) };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                }
            });
        }
        #[cfg(not(unix))]
        {
            let _ = tx;
        }

        let grid = Grid::new(cols, rows);

        Ok(Self {
            grid,
            parser: vte::Parser::new(),
            pty,
            pty_rx: rx,
            cols,
            rows,
            exited: false,
        })
    }
}

impl PaneBackend for TerminalBackend {
    fn pane_type(&self) -> PaneType {
        PaneType::Terminal
    }

    fn title(&self) -> &str {
        &self.grid.title
    }

    fn set_size(&mut self, cols: usize, rows: usize) {
        self.cols = cols;
        self.rows = rows;
        self.grid.resize(cols, rows);

        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let ws = libc::winsize {
                ws_row: rows as u16,
                ws_col: cols as u16,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            // SAFETY: self.pty.as_raw_fd() returns a valid PTY master fd.
            // ioctl(TIOCSWINSZ) sets terminal dimensions; &ws is a valid winsize pointer.
            // Result is discarded — resize is best-effort.
            let _ = unsafe { libc::ioctl(self.pty.as_raw_fd(), libc::TIOCSWINSZ, &ws) };
        }
    }

    fn process_input(&mut self, data: &[u8]) {
        let _ = self.pty.write_all(data);
        let _ = self.pty.flush();
    }

    fn update(&mut self) -> bool {
        let mut had_data = false;
        while let Ok(bytes) = self.pty_rx.try_recv() {
            had_data = true;
            for byte in bytes {
                self.parser.advance(&mut self.grid, &[byte]);
            }
        }
        had_data
    }

    fn terminal_snapshot(&self) -> Option<TerminalSnapshot> {
        let mut lines = Vec::with_capacity(self.rows);
        for row in 0..self.rows {
            let mut cells = Vec::with_capacity(self.cols);
            for col in 0..self.cols {
                let cell = &self.grid.cells[row][col];
                cells.push(TerminalCell {
                    c: cell.c,
                    fg: cell.fg,
                    bg: cell.bg,
                    bold: false,
                });
            }
            lines.push(TerminalLine { cells });
        }

        let (cell_w, cell_h) = self.cell_size();
        Some(TerminalSnapshot {
            cols: self.cols,
            rows: self.rows,
            cell_w,
            cell_h,
            cursor: TerminalCursor {
                col: self.grid.cursor_col,
                row: self.grid.cursor_row,
                visible: true,
            },
            damage: TerminalDamage::Full,
            lines,
        })
    }

    fn render_data(&self) -> BackendRenderData {
        let snapshot = self
            .terminal_snapshot()
            .expect("terminal backend should always produce a terminal snapshot");
        BackendRenderData::Terminal {
            lines: snapshot.lines,
            cursor_col: snapshot.cursor.col,
            cursor_row: snapshot.cursor.row,
            cell_w: snapshot.cell_w,
            cell_h: snapshot.cell_h,
        }
    }

    fn should_close(&self) -> bool {
        self.exited
    }
}
