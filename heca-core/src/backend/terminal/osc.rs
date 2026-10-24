use std::path::PathBuf;

use crate::agent::TerminalNotification;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OscEvent {
    PromptStart,
    CommandStart,
    PreExec,
    Program(String),
    CommandFinished(i32),
    Cwd(PathBuf),
    Notification(TerminalNotification),
}

#[derive(Debug, Default)]
pub(super) struct OscSnooper {
    state: ParseState,
    notification_limit: Option<usize>,
}

#[derive(Debug, Default)]
enum ParseState {
    #[default]
    Ground,
    SeenEsc,
    Osc {
        payload: Vec<u8>,
        saw_esc: bool,
        discarded: bool,
    },
}

impl OscSnooper {
    pub(super) fn with_notification_limit(notification_limit: Option<usize>) -> Self {
        Self { state: ParseState::Ground, notification_limit }
    }

    pub(super) fn set_notification_limit(&mut self, limit: usize) {
        self.notification_limit = Some(limit);
    }

    pub(super) fn observe(&mut self, bytes: &[u8]) -> Vec<OscEvent> {
        let mut events = Vec::with_capacity(4);
        for &byte in bytes {
            match &mut self.state {
                ParseState::Ground => {
                    if byte == 0x1b {
                        self.state = ParseState::SeenEsc;
                    }
                }
                ParseState::SeenEsc => {
                    if byte == b']' {
                        self.state = ParseState::Osc {
                            payload: Vec::new(),
                            saw_esc: false,
                            discarded: false,
                        };
                    } else if byte == 0x1b {
                        self.state = ParseState::SeenEsc;
                    } else {
                        self.state = ParseState::Ground;
                    }
                }
                ParseState::Osc { payload, saw_esc, discarded } => {
                    if *saw_esc {
                        if byte == b'\\' || byte == 0x9c {
                            if !*discarded && let Some(event) = parse_osc_payload(payload) {
                                events.push(event);
                            }
                            self.state = ParseState::Ground;
                            continue;
                        }
                        append(payload, discarded, 0x1b, self.notification_limit);
                        *saw_esc = false;
                    }

                    match byte {
                        0x07 => {
                            if !*discarded && let Some(event) = parse_osc_payload(payload) {
                                events.push(event);
                            }
                            self.state = ParseState::Ground;
                        }
                        0x1b => *saw_esc = true,
                        0x9c => {
                            if !*discarded && let Some(event) = parse_osc_payload(payload) {
                                events.push(event);
                            }
                            self.state = ParseState::Ground;
                        }
                        _ => append(payload, discarded, byte, self.notification_limit),
                    }
                }
            }
        }

        events
    }
}

fn append(payload: &mut Vec<u8>, discarded: &mut bool, byte: u8, limit: Option<usize>) {
    if *discarded { return }
    payload.push(byte);
    if limit.is_some_and(|limit| payload.len() > limit)
        && [b"9;".as_slice(), b"99;", b"777;"].iter().any(|prefix| payload.starts_with(prefix))
    {
        payload.clear();
        *discarded = true;
    }
}

fn parse_osc_payload(payload: &[u8]) -> Option<OscEvent> {
    let payload = std::str::from_utf8(payload).ok()?;
    if let Some(rest) = payload.strip_prefix("133;") {
        return parse_osc_133(rest);
    }
    if let Some(rest) = payload.strip_prefix("7;") {
        return parse_osc_7(rest);
    }
    let (code, payload) = payload.split_once(';')?;
    let code = code.parse().ok()?;
    matches!(code, 9 | 99 | 777).then(|| {
        OscEvent::Notification(TerminalNotification { code, payload: payload.to_owned() })
    })
}

fn parse_osc_133(payload: &str) -> Option<OscEvent> {
    if payload == "A" {
        Some(OscEvent::PromptStart)
    } else if payload == "B" {
        Some(OscEvent::CommandStart)
    } else if payload == "C" {
        Some(OscEvent::PreExec)
    } else if let Some(program) = payload.strip_prefix("E;") {
        Some(OscEvent::Program(percent_decode(program)?))
    } else if let Some(code) = payload.strip_prefix("D;") {
        Some(OscEvent::CommandFinished(code.parse().ok()?))
    } else {
        None
    }
}

fn parse_osc_7(payload: &str) -> Option<OscEvent> {
    let path = payload.strip_prefix("file://")?;
    let slash = path.find('/')?;
    let decoded = percent_decode(&path[slash..])?;
    Some(OscEvent::Cwd(PathBuf::from(decoded)))
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_digit(bytes[i + 1])?;
                let lo = hex_digit(bytes[i + 2])?;
                decoded.push((hi << 4) | lo);
                i += 3;
            }
            b => {
                decoded.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{OscEvent, OscSnooper, TerminalNotification};
    use std::path::PathBuf;

    #[test]
    fn parses_osc_133_and_osc_7_with_bel_terminator() {
        let mut snooper = OscSnooper::default();
        let events =
            snooper.observe(b"\x1b]133;D;1\x07\x1b]7;file://localhost/tmp/project%20x\x07");
        assert_eq!(
            events,
            vec![
                OscEvent::CommandFinished(1),
                OscEvent::Cwd(PathBuf::from("/tmp/project x")),
            ]
        );
    }

    #[test]
    fn parses_st_terminated_and_fragmented_sequences() {
        let mut snooper = OscSnooper::default();
        assert!(snooper.observe(b"\x1b]133;").is_empty());
        let events = snooper.observe(b"A\x1b\\\x1b]133;E;nvim\x1b\\\x1b]133;D;0\x1b\\");
        assert_eq!(
            events,
            vec![
                OscEvent::PromptStart,
                OscEvent::Program("nvim".into()),
                OscEvent::CommandFinished(0),
            ]
        );
    }

    #[test]
    fn ignores_non_target_osc_sequences() {
        let mut snooper = OscSnooper::default();
        let events = snooper.observe(b"\x1b]0;title\x07");
        assert!(events.is_empty());
    }

    #[test]
    fn preserves_notification_payloads_for_each_standard_code() {
        for code in [9, 99, 777] {
            let mut snooper = OscSnooper::default();
            let bytes = format!("\x1b]{code};notify;title;body\x07");
            assert_eq!(snooper.observe(bytes.as_bytes()), vec![
                OscEvent::Notification(TerminalNotification {
                    code,
                    payload: "notify;title;body".into(),
                }),
            ]);
        }
    }

    #[test]
    fn notification_survives_every_byte_boundary_and_both_terminators() {
        for terminator in ["\x07", "\x1b\\", "\u{009c}"] {
            // UTF-8 encodes U+009c as two bytes; use the terminal's single-byte ST instead.
            let bytes = if terminator == "\u{009c}" {
                b"\x1b]99;i=work;p=body;ready\x9c".to_vec()
            } else {
                format!("\x1b]99;i=work;p=body;ready{terminator}").into_bytes()
            };
            for split in 0..bytes.len() {
                let mut snooper = OscSnooper::default();
                let mut events = snooper.observe(&bytes[..split]);
                events.extend(snooper.observe(&bytes[split..]));
                assert_eq!(events, vec![OscEvent::Notification(TerminalNotification {
                    code: 99,
                    payload: "i=work;p=body;ready".into(),
                })]);
            }
        }
    }

    #[test]
    fn malformed_notification_does_not_prevent_the_next_shell_event() {
        let mut snooper = OscSnooper::default();
        assert_eq!(
            snooper.observe(b"\x1b]9;\xff\x07\x1b]133;A\x07"),
            vec![OscEvent::PromptStart],
        );
    }

    #[test]
    fn oversized_notifications_are_discarded_until_the_terminator() {
        let mut snooper = OscSnooper::with_notification_limit(Some(8));
        assert!(snooper.observe(b"\x1b]9;this-report-is-too-long").is_empty());
        assert_eq!(snooper.observe(b"133;D;1\x07\x1b]133;A\x07"), [OscEvent::PromptStart]);
    }
}
