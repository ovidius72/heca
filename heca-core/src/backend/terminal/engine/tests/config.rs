use super::*;
use crate::backend::DEFAULT_CELL_SIZE;

/// **A new history size reaches a terminal that is already running**: from the next line that
/// scrolls off, it keeps no more than the new size.
#[test]
fn reload_config_changes_how_much_history_a_running_terminal_keeps() {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    let mut engine = TerminalEngine::new(20, 10, (8.0, 16.0), writer, None, 3500)
        .expect("engine should initialize");
    let lines = |n: usize| (1..=n).map(|i| format!("{i}\r\n")).collect::<String>();
    engine.advance_bytes(lines(1000).as_bytes());
    assert!(
        engine.max_viewport_offset() > 900,
        "it kept the history it was given room for"
    );

    engine.reload_config(None, 100);
    engine.advance_bytes(lines(5).as_bytes());

    assert_eq!(engine.max_viewport_offset(), 100);
}

#[test]
fn reload_config_reapplies_palette_over_a_program_fork() {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    let mut engine = TerminalEngine::new(
        6,
        2,
        (8.0, 16.0),
        writer,
        Some(bg_defaults([10, 20, 30, 255])),
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
    )
    .expect("engine should initialize");

    // The configured background is in effect.
    assert_eq!(
        engine.terminal.palette().background,
        rgba_u8([10, 20, 30, 255])
    );

    // A program forks the palette via OSC 11 (set the default background). After this,
    // `palette()` returns the fork, not the config.
    engine.advance_bytes(b"\x1b]11;rgb:ff/00/00\x07");
    assert_ne!(
        engine.terminal.palette().background,
        rgba_u8([10, 20, 30, 255]),
        "OSC 11 should fork the palette away from the configured background"
    );

    // Reloading with a new theme background must win over the fork (the bug: it didn't).
    engine.reload_config(
        Some(bg_defaults([40, 50, 60, 255])),
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
    );
    assert_eq!(
        engine.terminal.palette().background,
        rgba_u8([40, 50, 60, 255]),
        "reload must re-theme even after a program forked the palette"
    );
}

#[test]
fn snapshot_preserves_background_colored_blank_cells_after_clear() {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    let mut engine = TerminalEngine::new(
        6,
        2,
        (8.0, 16.0),
        writer,
        None,
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
    )
    .expect("engine should initialize");

    engine.advance_bytes(b"\x1b[48;2;30;30;46m\x1b[2J");

    let snapshot = engine.snapshot(DEFAULT_CELL_SIZE);
    let expected_bg = [30.0 / 255.0, 30.0 / 255.0, 46.0 / 255.0, 1.0];

    assert!(
        snapshot.lines.iter().all(|line| {
            line.cells.iter().all(|cell| {
                (cell.bg[0] - expected_bg[0]).abs() < 0.001
                    && (cell.bg[1] - expected_bg[1]).abs() < 0.001
                    && (cell.bg[2] - expected_bg[2]).abs() < 0.001
                    && (cell.bg[3] - expected_bg[3]).abs() < 0.001
            })
        }),
        "clear-screen background should be preserved across blank cells"
    );
}

#[test]
fn bell_alert_is_captured_once() {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    let mut engine = TerminalEngine::new(
        80,
        24,
        (8.0, 16.0),
        writer,
        None,
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
    )
    .expect("terminal engine should initialize");

    engine.advance_bytes(b"\x07");
    assert_eq!(engine.take_alerts(), vec![BackendAlert::Bell]);
    assert!(engine.take_alerts().is_empty());
}

#[test]
fn captures_osc52_clipboard_write_once() {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    let mut engine = TerminalEngine::new(
        80,
        24,
        (8.0, 16.0),
        writer,
        None,
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
    )
    .expect("terminal engine should initialize");

    // OSC 52: set clipboard `c` to base64("hello"). wezterm decodes it for us.
    engine.advance_bytes(b"\x1b]52;c;aGVsbG8=\x07");
    assert_eq!(engine.take_clipboard_writes(), vec!["hello".to_string()]);
    // Drained on read.
    assert!(engine.take_clipboard_writes().is_empty());
}

#[test]
fn bracketed_paste_mode_tracks_decset_2004() {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    let mut engine = TerminalEngine::new(
        80,
        24,
        (8.0, 16.0),
        writer,
        None,
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
    )
    .expect("terminal engine should initialize");

    assert!(!engine.bracketed_paste_enabled(), "off by default");
    engine.advance_bytes(b"\x1b[?2004h");
    assert!(engine.bracketed_paste_enabled(), "DECSET 2004 enables it");
    engine.advance_bytes(b"\x1b[?2004l");
    assert!(!engine.bracketed_paste_enabled(), "DECRST 2004 disables it");
}

#[test]
fn map_underline_style_preserves_all_wezterm_variants() {
    assert_eq!(
        map_underline_style(wezterm_term::Underline::None),
        TerminalUnderlineStyle::None
    );
    assert_eq!(
        map_underline_style(wezterm_term::Underline::Single),
        TerminalUnderlineStyle::Single
    );
    assert_eq!(
        map_underline_style(wezterm_term::Underline::Double),
        TerminalUnderlineStyle::Double
    );
    assert_eq!(
        map_underline_style(wezterm_term::Underline::Curly),
        TerminalUnderlineStyle::Curly
    );
    assert_eq!(
        map_underline_style(wezterm_term::Underline::Dotted),
        TerminalUnderlineStyle::Dotted
    );
    assert_eq!(
        map_underline_style(wezterm_term::Underline::Dashed),
        TerminalUnderlineStyle::Dashed
    );
}

#[test]
fn scrollback_size_override_replaces_default_capacity() {
    // Drive the engine with a non-default scrollback size and confirm it is
    // plumbed through `HecaTerminalConfig` by filling past the wezterm default
    // (3500) up to the override. Total retained rows should be bounded by the
    // formula `scrollback_size + visible_rows` (the override caps history).
    let scrollback_size = 50;
    let rows = 2;
    let mut engine = viewport_engine(10, rows, scrollback_size);
    fill_scrollback(&mut engine, 10, 80);
    let snapshot = engine.snapshot((8.0, 14.0));
    assert_eq!(snapshot.rows, rows);
    assert_eq!(snapshot.lines.len(), rows);
    assert!(
        snapshot.scrollback_rows <= scrollback_size + rows,
        "scrollback_size override should cap retained history at scrollback_size + visible_rows"
    );
}

/// A process that reads slowly: every write waits, and what arrived is kept in order.
struct SlowReader(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for SlowReader {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        std::thread::sleep(std::time::Duration::from_millis(2));
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// **Keys, typed text and a paste reach the process in the order they were sent**, however slowly it
/// reads. A space used to be written straight to the process while the letters around it waited
/// in the emulator's own queue, so under load "ciao " arrived as "cia o".
#[test]
fn keys_text_and_paste_arrive_in_the_order_sent() {
    let arrived = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = SharedWriter::new(Box::new(SlowReader(arrived.clone())));
    let mut engine = TerminalEngine::new(80, 24, (8.0, 16.0), writer, None,
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE)
        .expect("terminal engine should initialize");
    let key = |c| crate::backend::BackendKeyEvent { code: crate::backend::BackendKeyCode::Char(c), modifiers: Default::default() };
    assert!(engine.process_key_event(&key('c')));
    assert!(engine.type_text("ia"));
    assert!(engine.process_key_event(&key('o')));
    assert!(engine.type_text(" "));
    assert!(engine.paste("x\ny"));
    assert!(engine.process_key_event(&key('!')));
    assert!(engine.type_text("\x7f"));
    let want = b"ciao x\ny!\x7f";
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while arrived.lock().unwrap().len() < want.len() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(arrived.lock().unwrap().as_slice(), want, "in order, and the paste verbatim");
}
