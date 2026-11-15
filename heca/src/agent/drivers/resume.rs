//! **Continuing a harness's own conversation from the command it was started with.**
//!
//! A saved session records what each harness was launched with. To continue its conversation the
//! launch is rebuilt: the executable and the user's own options (model, permissions, folders, ...)
//! stay; anything that would start a *new* task goes — an initial prompt, a conversation-choosing
//! option from an earlier launch; and the program's own "resume this session" is added with the
//! native session id. What each program calls those, and which of its options take a value, is a
//! [`Grammar`] — plain data — and this is the one place that turns a launch and a grammar into the
//! resumed launch. A program with no grammar has no resume (`AgentDriver::resume` stays `None`).

use std::ffi::{OsStr, OsString};

use heca_core::backend::ProcessLaunch;

/// How many words after an option belong to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Arity {
    /// None.
    Flag,
    /// Exactly the next word.
    One,
    /// Every following word up to the next option.
    Many,
    /// The next word, unless it is an option.
    Optional,
}

/// What one program's command line is made of, as far as resuming needs.
pub(super) struct Grammar {
    /// The options the program documents, with how many words each takes. An option not listed is
    /// kept as written, and a word after it that is not an option is kept as its value.
    pub options: &'static [(&'static str, Arity)],
    /// Words that, as the first non-option word, choose a different command (not a prompt).
    pub subcommands: &'static [&'static str],
    /// A subcommand that is part of resuming: removed, and put back as [`resume`](Self::resume).
    pub resumes_with: Option<&'static str>,
    /// Options that choose or start a conversation of their own: removed with their values.
    pub replaced: &'static [&'static str],
    /// Options that make the launch something that is not a conversation to continue (a
    /// non-interactive run, a listing): no resume.
    pub refused: &'static [&'static str],
    /// What the program is told, before the session id, to continue that session: an option
    /// (`--resume`) or a subcommand (`resume`).
    pub resume: &'static str,
    /// Whether the first plain word is a place and not a prompt, so it stays.
    pub keeps_first_word: bool,
}

/// One word, or one option with the words that belong to it, as they were written.
enum Part {
    Option { name: String, words: Vec<OsString> },
    Word(OsString),
}

fn is_option(word: &OsStr) -> bool {
    let word = word.to_string_lossy();
    word.len() > 1 && word.starts_with('-')
}

impl Grammar {
    fn arity(&self, name: &str) -> Option<Arity> {
        self.options.iter().find(|(option, _)| *option == name).map(|(_, arity)| *arity)
    }

    /// The launch split the way the program itself would split it.
    fn parts(&self, arguments: &[OsString]) -> Vec<Part> {
        let mut parts = Vec::new();
        let mut rest = arguments.iter().peekable();
        while let Some(word) = rest.next() {
            if word == "--" {
                parts.extend(rest.by_ref().cloned().map(Part::Word));
                break;
            }
            if !is_option(word) {
                parts.push(Part::Word(word.clone()));
                continue;
            }
            let text = word.to_string_lossy().into_owned();
            let (name, attached) = text.split_once('=').map_or((text.as_str(), false), |(name, _)| (name, true));
            let mut words = vec![word.clone()];
            let arity = if attached { Some(Arity::Flag) } else { self.arity(name) };
            match arity {
                Some(Arity::Flag) => {}
                Some(Arity::One) => words.extend(rest.next().cloned()),
                Some(Arity::Many) => {
                    while let Some(value) = rest.next_if(|value| !is_option(value)) {
                        words.push(value.clone());
                    }
                }
                Some(Arity::Optional) | None => words.extend(rest.next_if(|value| !is_option(value)).cloned()),
            }
            parts.push(Part::Option { name: name.to_owned(), words });
        }
        parts
    }

    /// **`original`, continuing the program's conversation `session_id`** — or `None` when it is not
    /// a conversation that can be continued: a non-interactive run, another command, or an id that
    /// could be read as an option.
    pub(super) fn resumed(&self, original: &ProcessLaunch, session_id: &str) -> Option<ProcessLaunch> {
        if session_id.is_empty() || session_id.starts_with('-') {
            return None;
        }
        let mut arguments: Vec<OsString> = Vec::new();
        let mut first_word = true;
        for part in self.parts(&original.arguments) {
            match part {
                Part::Option { name, .. } if self.refused.contains(&name.as_str()) => return None,
                Part::Option { name, .. } if self.replaced.contains(&name.as_str()) => {}
                Part::Option { words, .. } => arguments.extend(words),
                Part::Word(word) => {
                    let text = word.to_string_lossy();
                    let first = std::mem::replace(&mut first_word, false);
                    if first && self.resumes_with == Some(&*text) {
                        continue;
                    }
                    if first && self.subcommands.contains(&&*text) {
                        return None;
                    }
                    // Everything else is a prompt, a file for one, or the id an earlier resume held.
                    if first && self.keeps_first_word {
                        arguments.push(word);
                    }
                }
            }
        }
        let mut resumed = vec![OsString::from(self.resume), OsString::from(session_id)];
        resumed.extend(arguments);
        Some(ProcessLaunch { executable: original.executable.clone(), arguments: resumed, cwd: original.cwd.clone() })
    }
}
