use super::*;

#[test]
fn search_scrollback_matches_case_insensitively_with_columns() {
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
    engine.advance_bytes(b"Hello World\r\nfoo BAR baz\r\n");

    // Case-insensitive, with inclusive start / exclusive end columns.
    let world = engine.search_scrollback("world", 80);
    assert_eq!(world.len(), 1);
    assert_eq!((world[0].start_col, world[0].end_col), (6, 11));

    // Lowercase query matches uppercase content.
    let bar = engine.search_scrollback("bar", 80);
    assert_eq!(bar.len(), 1);
    assert_eq!((bar[0].start_col, bar[0].end_col), (4, 7));

    // No match, and an empty query, both yield nothing.
    assert!(engine.search_scrollback("zzz", 80).is_empty());
    assert!(engine.search_scrollback("", 80).is_empty());
}

#[test]
fn scan_url_byte_ranges_detects_and_trims() {
    let text = "see https://example.com/p?q=1, and http://a.b please";
    let ranges = super::scan_url_byte_ranges(text);
    let urls: Vec<&str> = ranges.iter().map(|&(s, e)| &text[s..e]).collect();
    assert_eq!(urls, vec!["https://example.com/p?q=1", "http://a.b"]);
}

#[test]
fn scan_url_byte_ranges_ignores_plain_text_and_bare_scheme() {
    assert!(super::scan_url_byte_ranges("just some words example.com").is_empty());
    assert!(super::scan_url_byte_ranges("https://").is_empty());
}

#[test]
fn snapshot_auto_detects_plain_urls() {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    let mut engine = TerminalEngine::new(
        40,
        2,
        (8.0, 16.0),
        writer,
        None,
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
    )
    .expect("engine init");
    engine.advance_bytes(b"go https://example.com now");
    let snap = engine.snapshot((8.0, 16.0));
    assert_eq!(snap.hyperlinks.len(), 1);
    assert_eq!(snap.hyperlinks[0].uri, "https://example.com");
    assert_eq!(snap.hyperlinks[0].row, 0);
    assert_eq!(snap.hyperlinks[0].start_col, 3);

    // Toggling detection off clears the auto-detected link.
    engine.set_link_detection(false);
    assert!(engine.snapshot((8.0, 16.0)).hyperlinks.is_empty());
}

#[test]
fn snapshot_captures_osc8_hyperlink_spans() {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    let mut engine = TerminalEngine::new(
        80,
        4,
        (8.0, 16.0),
        writer,
        None,
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
    )
    .expect("terminal engine should initialize");

    // OSC 8 hyperlink: open `https://example.com`, write "link", close.
    engine.advance_bytes(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\");

    let snapshot = engine.snapshot((8.0, 16.0));
    assert_eq!(snapshot.hyperlinks.len(), 1, "one captured link span");
    let span = &snapshot.hyperlinks[0];
    assert_eq!(span.uri, "https://example.com");
    assert_eq!(span.row, 0);
    assert_eq!(span.start_col, 0);
    assert_eq!(span.end_col, 4, "the 4 cells of \"link\" form the span");
    assert!(
        snapshot.graphics.is_empty() && snapshot.images.is_empty(),
        "plain text emits no inline-image placements"
    );
}
