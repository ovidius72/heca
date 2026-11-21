//! **One RPC line: how it is split into words, and how a script's arguments are written as one.**
//!
//! A line is the action name followed by arguments, separated by whitespace. This module is the
//! only place a line is cut into words ([`tokenize`]) and the only place words are written back
//! into a line ([`format_line`], [`from_argv`]). The parser, the queries, the socket and
//! `heca rpc` all go through it, so a value that holds a space, a quote or a Windows path means the
//! same thing everywhere and no action has to know the problem exists.
//!
//! ```text
//! focus_pane 42                         plain words, exactly as before
//! rename name="ciao mondo"              key="a b": the quotes belong to the value
//! open C:\dir\file                      a backslash outside quotes is just a backslash
//! open "C:\dir\\"                       …and inside double quotes it is written \\ at the end
//! note text='no \escapes here'          single quotes keep everything literal
//! note text=""                          an empty value
//! ```
//!
//! ## The rules
//!
//! 1. Whitespace separates words, except inside quotes. A quote may start in the middle of a word:
//!    `key="a b"` and `ab"c d"e` are one word each.
//! 2. Inside `"…"` only `\"` (a quote) and `\\` (a backslash) are escapes. Any other backslash
//!    sequence stays as written: `\n` is a backslash followed by an `n`.
//! 3. Inside `'…'` everything is literal. There are no escapes.
//! 4. A backslash outside quotes is literal (`C:\dir\file`).
//! 5. `name=value` is a **named** argument when the text before the first unquoted `=` is a valid
//!    name: the first character an ASCII letter or `_`, the rest ASCII letters, digits, `_` or `-`.
//!    Anything else is **positional**, whole: `C:\x=y` and `"a=b"` are positional values, while
//!    `key="a=b"` is the name `key` with the value `a=b`. The name is judged on the text as read,
//!    so quotes inside the name part do not count against it (`"key"=v` names `key`).
//! 6. `""` and `''` are an empty value: a positional `""`, or `key=""` (also written `key=`).
//! 7. A quote that never closes, or a `\` that ends the line inside `"…"`, is an error before
//!    anything runs.
//! 8. Nothing is expanded: no variables, no globs, no commands.
//! 9. A carriage return, a line feed or a NUL anywhere in the line is an error. The protocol's own
//!    line terminator is not part of the line: [`without_terminator`] takes it off first.
//!
//! Every error names where it is: the column, counted in characters from 1, of the quote that never
//! closed, of the backslash left with nothing to escape, or of the refused character. A line that
//! cannot be read is answered by the server itself, before any window is asked.
//!
//! ## Writing a line
//!
//! [`from_argv`] is `heca rpc`'s side. One argument is a whole line and goes out as it is. Several
//! are one word each: the first is the action, and each of the others is written so it reads back
//! as that same word. A named value is quoted on its own (`text="ciao mondo"`, never
//! `"text=ciao mondo"`); a positional is quoted whole. Words that need nothing stay bare, so every
//! line that worked before is sent byte for byte as it was. Inside double quotes the writer
//! escapes every `\` and `"`, so a Windows path that ends in a backslash comes out as `"C:\dir\\"`
//! and reads back as `C:\dir\`.

use std::fmt;

/// Why a line could not be read or written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineError {
    /// A `'` or `"` was opened at `column` and the line ended before it was closed.
    UnclosedQuote { quote: char, column: usize },
    /// The line ended on the `\` at `column`, inside double quotes: nothing is there to escape.
    DanglingEscape { column: usize },
    /// A carriage return, line feed or NUL at `column`: they would end or garble the one-line
    /// request.
    ControlCharacter { character: char, column: usize },
    /// A named argument whose name is not a valid name (only the writer can be given one).
    InvalidName(String),
    /// An action name that would not read back as itself: empty, or with a space, a quote or a `=`.
    InvalidAction(String),
}

impl fmt::Display for LineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineError::UnclosedQuote { quote, column } => {
                write!(
                    f,
                    "a {quote} quote was opened at column {column} and never closed"
                )
            }
            LineError::DanglingEscape { column } => {
                write!(
                    f,
                    "the line ends in a backslash at column {column} inside double quotes; write \\\\ for a backslash"
                )
            }
            LineError::ControlCharacter { character, column } => {
                let what = match character {
                    '\r' => "a carriage return",
                    '\n' => "a line break",
                    _ => "a NUL character",
                };
                write!(f, "{what} at column {column} is not allowed in a line")
            }
            LineError::InvalidName(name) => write!(
                f,
                "'{name}' is not a valid argument name: it starts with a letter or _ and has only letters, digits, _ or -"
            ),
            LineError::InvalidAction(action) => write!(
                f,
                "'{action}' is not an action name: it cannot be empty or hold a space, a quote or a ="
            ),
        }
    }
}

impl std::error::Error for LineError {}

/// One argument: a bare value in the action's declared order, or one given by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arg {
    Positional(String),
    Named { name: String, value: String },
}

impl Arg {
    /// How one word of a script's argument list reads: named when the text before its first `=` is a
    /// valid name, positional otherwise. The same test [`tokenize`] applies to a line, so a word
    /// written by [`format_line`] reads back as the arg it was.
    pub fn from_word(word: &str) -> Arg {
        word.find('=')
            .and_then(|at| named_at(word, at))
            .unwrap_or_else(|| Arg::Positional(word.to_string()))
    }

    /// The word as plain text — `name=value` for a named one, quotes gone.
    pub fn text(&self) -> String {
        match self {
            Arg::Positional(value) => value.clone(),
            Arg::Named { name, value } => format!("{name}={value}"),
        }
    }
}

/// One word of a line, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub arg: Arg,
    /// Whether any quote was used in writing it. A quoted `--dock` is a value, not the flag.
    pub quoted: bool,
}

impl Word {
    /// The flag that names a placement, written bare as `--dock`.
    pub fn is_dock_flag(&self) -> bool {
        !self.quoted && self.arg == Arg::Positional("--dock".to_string())
    }
}

/// Cut a line into words by the rules in the module doc.
pub fn tokenize(line: &str) -> Result<Vec<Word>, LineError> {
    reject_control(line)?;
    let mut words = Vec::new();
    let mut building: Option<Building> = None;
    let mut chars = columns(line);
    while let Some((column, c)) = chars.next() {
        if c.is_whitespace() {
            words.extend(building.take().map(Building::finish));
            continue;
        }
        let word = building.get_or_insert_with(Building::default);
        match c {
            '"' => {
                word.quoted = true;
                read_double_quoted(&mut chars, column, &mut word.value)?;
            }
            '\'' => {
                word.quoted = true;
                read_single_quoted(&mut chars, column, &mut word.value)?;
            }
            '=' => {
                word.split.get_or_insert(word.value.len());
                word.value.push('=');
            }
            c => word.value.push(c),
        }
    }
    words.extend(building.map(Building::finish));
    Ok(words)
}

/// The line without the protocol's own terminator: one `\n`, and the `\r` before it if there is one.
/// Whatever is left is read as it is, so a `\r` or `\n` inside it is still refused by [`tokenize`].
pub fn without_terminator(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

/// Write `action` and `args` as one line that [`tokenize`] reads back as the same words.
pub fn format_line<'a>(
    action: &str,
    args: impl IntoIterator<Item = &'a Arg>,
) -> Result<String, LineError> {
    reject_control(action)?;
    // The action is never quoted: it must read back as itself, one bare word.
    if action.is_empty()
        || needs_quotes(action)
        || Arg::from_word(action) != Arg::Positional(action.to_string())
    {
        return Err(LineError::InvalidAction(action.to_string()));
    }
    let mut line = action.to_string();
    for arg in args {
        line.push(' ');
        match arg {
            Arg::Positional(value) => {
                reject_control(value)?;
                // Bare would read as named, or as nothing at all: say it is a value.
                if value.is_empty() || needs_quotes(value) || Arg::from_word(value) != *arg {
                    push_quoted(&mut line, value);
                } else {
                    line.push_str(value);
                }
            }
            Arg::Named { name, value } => {
                if !is_name(name) {
                    return Err(LineError::InvalidName(name.clone()));
                }
                reject_control(value)?;
                line.push_str(name);
                line.push('=');
                if needs_quotes(value) {
                    push_quoted(&mut line, value);
                } else {
                    line.push_str(value);
                }
            }
        }
    }
    Ok(line)
}

/// **`heca rpc`'s argument list as one line.** One argument is a whole line and is sent as it is.
/// Several are the action and one word each, written by [`format_line`].
pub fn from_argv(argv: &[String]) -> Result<String, LineError> {
    match argv {
        [] => Ok(String::new()),
        [line] => reject_control(line).map(|()| line.clone()),
        [action, rest @ ..] => {
            let args: Vec<Arg> = rest.iter().map(|word| Arg::from_word(word)).collect();
            format_line(action, &args)
        }
    }
}

/// A word under construction: its text so far, where its first unquoted `=` fell, and whether a
/// quote was used.
#[derive(Default)]
struct Building {
    value: String,
    split: Option<usize>,
    quoted: bool,
}

impl Building {
    fn finish(self) -> Word {
        let Building {
            value,
            split,
            quoted,
        } = self;
        let arg = split
            .and_then(|at| named_at(&value, at))
            .unwrap_or(Arg::Positional(value));
        Word { arg, quoted }
    }
}

/// `text` as a named argument split at byte `at` (a `=`), when what stands before it is a name.
fn named_at(text: &str, at: usize) -> Option<Arg> {
    let name = &text[..at];
    is_name(name).then(|| Arg::Named {
        name: name.to_string(),
        value: text[at + 1..].to_string(),
    })
}

fn is_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Each character of `text` with its column, counted in characters from 1 — the position an error
/// names.
fn columns(text: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    text.chars().enumerate().map(|(index, c)| (index + 1, c))
}

fn reject_control(text: &str) -> Result<(), LineError> {
    match columns(text).find(|(_, c)| matches!(c, '\r' | '\n' | '\0')) {
        Some((column, character)) => Err(LineError::ControlCharacter { character, column }),
        None => Ok(()),
    }
}

/// Up to the closing `"` opened at `opened`; `\"` and `\\` are the only escapes, any other `\` is
/// kept as written.
fn read_double_quoted(
    chars: &mut impl Iterator<Item = (usize, char)>,
    opened: usize,
    into: &mut String,
) -> Result<(), LineError> {
    loop {
        match chars.next() {
            None => {
                return Err(LineError::UnclosedQuote {
                    quote: '"',
                    column: opened,
                });
            }
            Some((_, '"')) => return Ok(()),
            Some((column, '\\')) => match chars.next() {
                None => return Err(LineError::DanglingEscape { column }),
                Some((_, escaped @ ('"' | '\\'))) => into.push(escaped),
                Some((_, other)) => {
                    into.push('\\');
                    into.push(other);
                }
            },
            Some((_, c)) => into.push(c),
        }
    }
}

/// Up to the closing `'` opened at `opened`; everything is literal.
fn read_single_quoted(
    chars: &mut impl Iterator<Item = (usize, char)>,
    opened: usize,
    into: &mut String,
) -> Result<(), LineError> {
    loop {
        match chars.next() {
            None => {
                return Err(LineError::UnclosedQuote {
                    quote: '\'',
                    column: opened,
                });
            }
            Some((_, '\'')) => return Ok(()),
            Some((_, c)) => into.push(c),
        }
    }
}

fn needs_quotes(value: &str) -> bool {
    value
        .chars()
        .any(|c| c.is_whitespace() || c == '"' || c == '\'')
}

fn push_quoted(line: &mut String, value: &str) {
    line.push('"');
    for c in value.chars() {
        if matches!(c, '"' | '\\') {
            line.push('\\');
        }
        line.push(c);
    }
    line.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(value: &str) -> Arg {
        Arg::Positional(value.to_string())
    }

    fn named(name: &str, value: &str) -> Arg {
        Arg::Named {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    fn args(line: &str) -> Vec<Arg> {
        tokenize(line)
            .unwrap()
            .into_iter()
            .map(|word| word.arg)
            .collect()
    }

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn a_line_without_quotes_is_split_on_whitespace_as_before() {
        assert_eq!(
            args("  place_pane 5\t0  1 pane_idx=0 "),
            [
                pos("place_pane"),
                pos("5"),
                pos("0"),
                pos("1"),
                named("pane_idx", "0")
            ],
        );
        assert!(args("   ").is_empty());
    }

    #[test]
    fn a_quoted_value_keeps_its_spaces() {
        assert_eq!(args(r#"say "ciao mondo""#), [pos("say"), pos("ciao mondo")]);
        assert_eq!(
            args(r#"say text="ciao mondo""#),
            [pos("say"), named("text", "ciao mondo")]
        );
    }

    #[test]
    fn a_quote_may_start_in_the_middle_of_a_word() {
        assert_eq!(args(r#"ab"c d"e"#), [pos("abc de")]);
        assert_eq!(args(r#"key="a b"c"#), [named("key", "a bc")]);
        assert_eq!(args(r#"a"b"'c'"#), [pos("abc")]);
    }

    #[test]
    fn whitespace_inside_quotes_does_not_split() {
        assert_eq!(
            args("a 'b  c'\t\"d\te\" f"),
            [pos("a"), pos("b  c"), pos("d\te"), pos("f")]
        );
    }

    #[test]
    fn only_a_quote_and_a_backslash_can_be_escaped_in_double_quotes() {
        assert_eq!(args(r#""say \"hi\"""#), [pos(r#"say "hi""#)]);
        assert_eq!(args(r#""one \\ two""#), [pos(r"one \ two")]);
    }

    #[test]
    fn any_other_backslash_sequence_inside_double_quotes_stays_as_written() {
        // `\n` is a backslash and an n, not a line break.
        assert_eq!(args(r#""a\nb""#), [pos(r"a\nb")]);
        assert_eq!(args(r#""\t\x\ ""#), [pos(r"\t\x\ ")]);
    }

    #[test]
    fn single_quotes_keep_everything_literal() {
        assert_eq!(
            args(r#"'a\nb \" \\ "q" $HOME *'"#),
            [pos(r#"a\nb \" \\ "q" $HOME *"#)]
        );
        assert_eq!(args(r"'C:\dir\'"), [pos(r"C:\dir\")]);
        assert_eq!(args(r#"it"'"s"#), [pos("it's")]);
    }

    #[test]
    fn a_backslash_outside_quotes_is_a_backslash() {
        assert_eq!(
            args(r"open C:\dir\file"),
            [pos("open"), pos(r"C:\dir\file")]
        );
        assert_eq!(args(r"open C:\dir\"), [pos("open"), pos(r"C:\dir\")]);
    }

    #[test]
    fn a_windows_path_in_double_quotes_ends_in_two_backslashes() {
        assert_eq!(args(r#"open "C:\dir\\""#), [pos("open"), pos(r"C:\dir\")]);
        assert_eq!(
            args(r#"open "C:\Program Files\app""#),
            [pos("open"), pos(r"C:\Program Files\app")]
        );
        assert_eq!(
            args(r#"open path="C:\my dir\\""#),
            [pos("open"), named("path", r"C:\my dir\")]
        );
    }

    #[test]
    fn a_name_is_a_letter_or_underscore_then_letters_digits_underscore_or_dash() {
        for (word, name) in [
            ("a=1", "a"),
            ("_x=1", "_x"),
            ("pane-idx_2=1", "pane-idx_2"),
            ("A9=1", "A9"),
        ] {
            assert_eq!(args(word), [named(name, "1")], "{word}");
        }
        for word in [
            "1a=x", "-a=x", "=x", "a.b=x", "é=x", r"C:\x=y", "--dock=x", "/p=1",
        ] {
            assert_eq!(args(word), [pos(word)], "{word}");
        }
    }

    #[test]
    fn a_word_splits_at_its_first_equals_sign_only() {
        assert_eq!(args("a=b=c"), [named("a", "b=c")]);
        assert_eq!(args("a=="), [named("a", "=")]);
        // The first `=` is the judge: an invalid name makes the whole word positional.
        assert_eq!(args(r"C:\x=y=z"), [pos(r"C:\x=y=z")]);
    }

    #[test]
    fn a_quoted_equals_sign_does_not_make_a_named_value() {
        assert_eq!(args(r#""a=b""#), [pos("a=b")]);
        assert_eq!(args(r#"a"="b"#), [pos("a=b")]);
        assert_eq!(args("a='='"), [named("a", "=")]);
    }

    #[test]
    fn a_named_value_may_hold_an_equals_sign_when_quoted() {
        assert_eq!(args(r#"key="a=b""#), [named("key", "a=b")]);
    }

    #[test]
    fn a_quoted_name_part_is_judged_as_read() {
        assert_eq!(args(r#""key"=v"#), [named("key", "v")]);
        assert_eq!(args(r#""a b"=v"#), [pos("a b=v")]);
    }

    #[test]
    fn empty_quotes_are_an_empty_value() {
        assert_eq!(args(r#"a "" b"#), [pos("a"), pos(""), pos("b")]);
        assert_eq!(args("a '' b"), [pos("a"), pos(""), pos("b")]);
        assert_eq!(args(r#"key="""#), [named("key", "")]);
        assert_eq!(args("key=''"), [named("key", "")]);
        assert_eq!(args("key= x"), [named("key", ""), pos("x")]);
    }

    #[test]
    fn an_unclosed_quote_is_an_error() {
        assert_eq!(
            tokenize(r#"say "hello"#),
            Err(LineError::UnclosedQuote {
                quote: '"',
                column: 5
            })
        );
        assert_eq!(
            tokenize("say 'hello"),
            Err(LineError::UnclosedQuote {
                quote: '\'',
                column: 5
            })
        );
        assert_eq!(
            tokenize(r#"say key="a b"#),
            Err(LineError::UnclosedQuote {
                quote: '"',
                column: 9
            })
        );
        // An escaped quote does not close.
        assert_eq!(
            tokenize(r#"say "a\""#),
            Err(LineError::UnclosedQuote {
                quote: '"',
                column: 5
            })
        );
        // The column counts characters, not bytes.
        assert_eq!(
            tokenize(r#"dì "x"#),
            Err(LineError::UnclosedQuote {
                quote: '"',
                column: 4
            })
        );
    }

    #[test]
    fn a_backslash_ending_the_line_inside_double_quotes_is_an_error() {
        assert_eq!(
            tokenize(r#"say "abc\"#),
            Err(LineError::DanglingEscape { column: 9 })
        );
        // Inside single quotes, and outside quotes, it is only a character.
        assert_eq!(
            tokenize(r"say 'abc\"),
            Err(LineError::UnclosedQuote {
                quote: '\'',
                column: 5
            })
        );
        assert_eq!(args(r"say abc\"), [pos("say"), pos(r"abc\")]);
    }

    #[test]
    fn a_carriage_return_a_line_feed_or_a_nul_is_refused_even_inside_quotes() {
        for (line, character, column) in [
            ("a\nb", '\n', 2),
            ("a\rb", '\r', 2),
            ("a\0b", '\0', 2),
            ("a \"x\ny\"", '\n', 5),
            ("a 'x\ry'", '\r', 5),
            ("a key=\"x\0\"", '\0', 9),
            ("a\r\n", '\r', 2),
        ] {
            assert_eq!(
                tokenize(line),
                Err(LineError::ControlCharacter { character, column }),
                "{line:?}"
            );
        }
    }

    #[test]
    fn the_protocols_line_terminator_comes_off_before_a_line_is_read() {
        assert_eq!(without_terminator("focus_left\n"), "focus_left");
        assert_eq!(without_terminator("focus_left\r\n"), "focus_left");
        assert_eq!(without_terminator("focus_left"), "focus_left");
        // Only the terminator: one more is content, and still refused.
        assert_eq!(
            tokenize(without_terminator("a\n\n")),
            Err(LineError::ControlCharacter {
                character: '\n',
                column: 2
            })
        );
        assert_eq!(
            tokenize(without_terminator("a\r\r\n")),
            Err(LineError::ControlCharacter {
                character: '\r',
                column: 2
            })
        );
    }

    #[test]
    fn nothing_is_expanded() {
        assert_eq!(
            args("a $HOME ~ * `x` $(y) {1,2}"),
            [
                pos("a"),
                pos("$HOME"),
                pos("~"),
                pos("*"),
                pos("`x`"),
                pos("$(y)"),
                pos("{1,2}")
            ]
        );
        assert_eq!(args(r#"a "$HOME" '*'"#), [pos("a"), pos("$HOME"), pos("*")]);
    }

    #[test]
    fn only_a_bare_dock_flag_is_the_flag() {
        let words = tokenize(r#"--dock "--dock" '--dock' --dock= x"#).unwrap();
        let flags: Vec<bool> = words.iter().map(Word::is_dock_flag).collect();
        assert_eq!(flags, [true, false, false, false, false]);
    }

    // The writer

    #[test]
    fn words_that_need_no_quotes_are_written_bare() {
        let line = format_line(
            "place_pane",
            &[
                pos("5"),
                pos("0"),
                pos(r"C:\dir\file"),
                named("pane_idx", "0"),
                named("name", ""),
                named("p", r"C:\x=y"),
            ],
        );
        assert_eq!(
            line.unwrap(),
            r"place_pane 5 0 C:\dir\file pane_idx=0 name= p=C:\x=y"
        );
    }

    #[test]
    fn a_named_value_is_quoted_on_its_own() {
        assert_eq!(
            format_line("say", &[named("text", "ciao mondo")]).unwrap(),
            r#"say text="ciao mondo""#
        );
        assert_eq!(
            format_line("say", &[named("text", r#"a "b""#)]).unwrap(),
            r#"say text="a \"b\"""#
        );
    }

    #[test]
    fn a_positional_is_quoted_whole_when_it_needs_it() {
        let line = format_line(
            "say",
            &[
                pos("ciao mondo"),
                pos(""),
                pos("a=b"),
                pos("it's"),
                pos("a\tb"),
            ],
        )
        .unwrap();
        assert_eq!(line, "say \"ciao mondo\" \"\" \"a=b\" \"it's\" \"a\tb\"");
    }

    #[test]
    fn a_windows_path_ending_in_a_backslash_is_written_with_two() {
        assert_eq!(
            format_line("open", &[pos(r"C:\my dir\")]).unwrap(),
            r#"open "C:\\my dir\\""#
        );
        assert_eq!(
            format_line("open", &[named("path", r"C:\my dir\")]).unwrap(),
            r#"open path="C:\\my dir\\""#
        );
    }

    #[test]
    fn the_writer_refuses_what_a_line_cannot_hold() {
        assert_eq!(
            format_line("a", &[pos("x\ny")]),
            Err(LineError::ControlCharacter {
                character: '\n',
                column: 2
            })
        );
        assert_eq!(
            format_line("a", &[named("k", "x\ry")]),
            Err(LineError::ControlCharacter {
                character: '\r',
                column: 2
            })
        );
        assert_eq!(
            format_line("a", &[pos("x\0")]),
            Err(LineError::ControlCharacter {
                character: '\0',
                column: 2
            })
        );
        assert_eq!(
            format_line("a\n", &[]),
            Err(LineError::ControlCharacter {
                character: '\n',
                column: 2
            })
        );
        assert_eq!(
            format_line("a", &[named("1k", "v")]),
            Err(LineError::InvalidName("1k".into()))
        );
    }

    /// What goes in comes out: write words, read the line, get the same words back.
    #[test]
    fn a_written_line_reads_back_as_the_same_words() {
        let tricky = [
            "",
            " ",
            "a b",
            "ciao mondo",
            "a=b",
            "=",
            "=x",
            "k=",
            "k=v",
            "a\tb",
            "it's",
            r#"say "hi""#,
            r"C:\dir\file",
            r"C:\dir\",
            r"C:\my dir\",
            r"\\server\share\x y",
            r"\",
            r"\\",
            r#"\""#,
            r#""\"#,
            r"a\nb",
            r"a\nb c",
            "'",
            "\"",
            "$HOME *",
            "--x=1",
            "é ü",
            "a=b=c",
            "k=a b=c",
        ];
        for value in tricky {
            for arg in [pos(value), named("key", value), named("_k-9", value)] {
                // A positional the CLI would have read as named is only reachable through the typed API.
                let line = format_line("act", [&pos("first"), &arg, &pos("last")]).unwrap();
                let read = args(&line);
                assert_eq!(
                    read,
                    [pos("act"), pos("first"), arg.clone(), pos("last")],
                    "{line}"
                );
            }
        }
    }

    #[test]
    fn one_argument_is_a_whole_line_sent_as_it_is() {
        for line in [
            r#"say text="ciao mondo""#,
            "focus_pane 42",
            r"open C:\dir\",
            "  spaced  ",
        ] {
            assert_eq!(from_argv(&argv(&[line])).unwrap(), line);
        }
        assert_eq!(from_argv(&[]).unwrap(), "");
    }

    #[test]
    fn several_arguments_are_one_word_each() {
        let line = from_argv(&argv(&["say", "text=ciao mondo", "a b", "C:\\dir\\", "n="])).unwrap();
        assert_eq!(line, "say text=\"ciao mondo\" \"a b\" C:\\dir\\ n=");
        assert_eq!(
            args(&line),
            [
                pos("say"),
                named("text", "ciao mondo"),
                pos("a b"),
                pos(r"C:\dir\"),
                named("n", "")
            ],
        );
    }

    #[test]
    fn lines_that_worked_before_are_sent_byte_for_byte() {
        for line in [
            "split_horizontal",
            "focus_pane 42",
            "place_pane 5 0 1 pane_idx=0",
            "terminal_rename_to terminal=demo.shell name=",
            "docker.stop_selected --dock docker.right",
        ] {
            let words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
            assert_eq!(from_argv(&words).unwrap(), line);
        }
    }

    #[test]
    fn the_argument_list_refuses_a_carriage_return_a_line_feed_or_a_nul() {
        for words in [
            &["say", "a\nb"][..],
            &["say", "k=a\rb"],
            &["say", "a\0"],
            &["say\n", "x"],
            &["say\nx"],
            &["say x\r"],
        ] {
            assert!(
                matches!(
                    from_argv(&argv(words)),
                    Err(LineError::ControlCharacter { .. })
                ),
                "{words:?}"
            );
        }
    }

    #[test]
    fn the_argument_list_round_trips() {
        let lists: [&[&str]; 6] = [
            &["act"],
            &["act", "1", "2", "k=v"],
            &["act", "text=ciao mondo", "C:\\my dir\\", "plain"],
            &["act", "", "k=", "k=\"", "it's"],
            &["act", r#"say "hi""#, r"a\nb", "x=y z=w"],
            &["act", "--dock", "docker.right", "text=a b"],
        ];
        for list in lists {
            let line = from_argv(&argv(list)).unwrap();
            let read: Vec<Arg> = args(&line).into_iter().skip(1).collect();
            let sent: Vec<Arg> = list[1..].iter().map(|word| Arg::from_word(word)).collect();
            assert_eq!(read, sent, "{line}");
        }
    }

    #[test]
    fn an_action_that_would_not_read_back_as_itself_is_refused() {
        for action in ["", "two words", "say\"", "k=v"] {
            assert!(
                matches!(
                    from_argv(&argv(&[action, "x"])),
                    Err(LineError::InvalidAction(_))
                ),
                "{action:?}"
            );
        }
        assert_eq!(
            from_argv(&argv(&["docker.stop_selected", "x"])).unwrap(),
            "docker.stop_selected x"
        );
    }
}
