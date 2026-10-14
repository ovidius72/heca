//! **Finding links** in plain text a program printed.

use super::*;

/// URL schemes we linkify (kept in sync with the open-side allowlist).
pub(super) const LINK_SCHEMES: &[&str] = &[
    "https://", "http://", "ftps://", "ftp://", "file://", "mailto:",
];

/// Characters allowed inside a detected URL body (RFC 3986 unreserved + reserved
/// + `%`). Whitespace and most quotes/brackets end the URL.
pub(super) fn is_url_body_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b':'
                | b'/'
                | b'?'
                | b'#'
                | b'['
                | b']'
                | b'@'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b'%'
        )
}

/// Trailing characters trimmed off a detected URL (sentence punctuation that is
/// almost never part of the link).
pub(super) fn is_url_trailing_byte(b: u8) -> bool {
    matches!(
        b,
        b'.' | b',' | b';' | b':' | b'!' | b'?' | b')' | b']' | b'}' | b'\'' | b'"' | b'>'
    )
}

/// Scan `text` for URL byte ranges `[start, end)` beginning with a known scheme.
/// Conservative: a scheme only starts a match at a non-URL boundary, needs at
/// least one body char, and trailing sentence punctuation is trimmed.
pub(super) fn scan_url_byte_ranges(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let scheme_len = LINK_SCHEMES
            .iter()
            .find(|s| text[i..].starts_with(**s))
            .map(|s| s.len());
        if let Some(scheme_len) = scheme_len {
            let boundary = i == 0 || !is_url_body_byte(bytes[i - 1]);
            if boundary {
                let mut j = i + scheme_len;
                while j < text.len() && is_url_body_byte(bytes[j]) {
                    j += 1;
                }
                if j > i + scheme_len {
                    let mut end = j;
                    while end > i + scheme_len && is_url_trailing_byte(bytes[end - 1]) {
                        end -= 1;
                    }
                    out.push((i, end));
                    i = j;
                    continue;
                }
            }
        }
        i += text[i..].chars().next().map_or(1, |c| c.len_utf8());
    }
    out
}

/// Detect plain-text URLs in one snapshot line, mapped to cell columns.
pub(super) fn detect_row_links(line: &TerminalLine, row: usize, out: &mut Vec<HyperlinkSpan>) {
    // Row string + per-byte (start col, end col) of the owning cell.
    let mut text = String::new();
    let mut byte_start_col: Vec<usize> = Vec::new();
    let mut byte_end_col: Vec<usize> = Vec::new();
    let mut col = 0usize;
    for cell in &line.cells {
        let end = col + cell.width.max(1);
        for _ in 0..cell.text.len() {
            byte_start_col.push(col);
            byte_end_col.push(end);
        }
        text.push_str(&cell.text);
        col = end;
    }
    for (b_start, b_end) in scan_url_byte_ranges(&text) {
        if b_end == 0 || b_end > byte_start_col.len() {
            continue;
        }
        out.push(HyperlinkSpan {
            row,
            start_col: byte_start_col[b_start],
            end_col: byte_end_col[b_end - 1],
            uri: text[b_start..b_end].to_string(),
        });
    }
}

/// Append auto-detected URL spans, skipping any cell already covered by an
/// explicit OSC 8 span on the same row (OSC 8 wins).
pub(super) fn detect_plain_links(lines: &[TerminalLine], hyperlinks: &mut Vec<HyperlinkSpan>) {
    let explicit = hyperlinks.len();
    let mut detected: Vec<HyperlinkSpan> = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        detect_row_links(line, row, &mut detected);
    }
    detected.retain(|d| {
        !hyperlinks[..explicit]
            .iter()
            .any(|e| e.row == d.row && d.start_col < e.end_col && e.start_col < d.end_col)
    });
    hyperlinks.extend(detected);
}
