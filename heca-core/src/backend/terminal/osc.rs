use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OscEvent {
    PromptStart,
    CommandStart,
    PreExec,
    Program(String),
    CommandFinished(i32),
    Cwd(PathBuf),
}

#[derive(Debug, Default)]
pub(super) struct OscSnooper {
    state: ParseState,
}

#[derive(Debug, Default)]
enum ParseState {
    #[default]
    Ground,
    SeenEsc,
    Osc {
        payload: Vec<u8>,
        saw_esc: bool,
    },
}

impl OscSnooper {
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
                        };
                    } else if byte == 0x1b {
                        self.state = ParseState::SeenEsc;
                    } else {
                        self.state = ParseState::Ground;
                    }
                }
                ParseState::Osc { payload, saw_esc } => {
                    if *saw_esc {
                        if byte == b'\\' || byte == 0x9c {
                            if let Some(event) = parse_osc_payload(payload) {
                                events.push(event);
                            }
                            self.state = ParseState::Ground;
                            continue;
                        }
                        payload.push(0x1b);
                        *saw_esc = false;
                    }

                    match byte {
                        0x07 => {
                            if let Some(event) = parse_osc_payload(payload) {
                                events.push(event);
                            }
                            self.state = ParseState::Ground;
                        }
                        0x1b => *saw_esc = true,
                        0x9c => {
                            if let Some(event) = parse_osc_payload(payload) {
                                events.push(event);
                            }
                            self.state = ParseState::Ground;
                        }
                        _ => payload.push(byte),
                    }
                }
            }
        }

        events
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
    None
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
    use super::{OscEvent, OscSnooper};
    use std::path::PathBuf;

    #[test]
    fn parses_osc_133_and_osc_7_with_bel_terminator() {
        let mut snooper = OscSnooper::default();
        let events = snooper.observe(b"\x1b]133;D;1\x07\x1b]7;file://localhost/tmp/project%20x\x07");
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
}
