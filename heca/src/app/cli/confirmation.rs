//! Text rendering of action-owned confirmations, enforced by the CLI dispatcher.

use std::io::{self, BufRead, Write};
use crate::actions::{ConfirmSpec, Outcome};

pub(super) type Prepare = fn(&super::Invocation) -> Result<Option<Confirmation>, String>;

pub(super) struct Confirmation {
    pub spec: ConfirmSpec,
    pub prompt: String,
}

pub(super) fn ask(confirmation: Option<Confirmation>) -> Result<bool, String> {
    let Some(confirmation) = confirmation else { return Ok(true); };
    ask_with(&confirmation, &mut io::stdin().lock(), &mut io::stderr().lock())
        .map_err(|error| format!("{error}\n"))
}

fn ask_with(confirmation: &Confirmation, input: &mut impl BufRead, output: &mut impl Write) -> io::Result<bool> {
    if !confirmation.spec.buttons.iter().any(|button| matches!(button.outcome, Outcome::Proceed)
        && !button.label.trim().is_empty()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "confirmation has no affirmative response"));
    }
    writeln!(output, "{}", confirmation.spec.message)?;
    write!(output, "{}", confirmation.prompt)?;
    output.flush()?;
    let mut answer = String::new();
    let read = input.read_line(&mut answer)?;
    Ok(read > 0 && answer.trim().eq_ignore_ascii_case("y"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt() -> Confirmation {
        let words = heca_config::settings::SessionText {
            terminate: "Stop".into(), confirm_terminate: "Stop Project A, retain its session".into(),
            confirm_prompt: "Proceed? [y/N]: ".into(), ..Default::default()
        };
        Confirmation { spec: ConfirmSpec::for_session("terminate_session", &words).unwrap(), prompt: words.confirm_prompt }
    }

    #[test]
    fn only_y_confirms_regardless_of_the_action_label() {
        for answer in ["", "\n", "n\n", "N\n", "cancel\n", "yes\n", "Stop\n", "Terminate\n"] {
            assert!(!ask_with(&prompt(), &mut answer.as_bytes(), &mut Vec::new()).unwrap(), "{answer:?}");
        }
        for answer in ["y\n", "  Y  \n", "y"] {
            let mut output = Vec::new();
            assert!(ask_with(&prompt(), &mut answer.as_bytes(), &mut output).unwrap());
            let text = String::from_utf8(output).unwrap();
            assert!(text.contains("Project A") && text.contains("Proceed? [y/N]:"));
        }
    }

    #[test]
    fn broken_input_or_output_cannot_confirm() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
            fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
        }
        assert!(ask_with(&prompt(), &mut "y\n".as_bytes(), &mut Broken).is_err());
        let invalid_utf8 = [0xff, b'\n'];
        assert!(ask_with(&prompt(), &mut invalid_utf8.as_slice(), &mut Vec::new()).is_err());
    }
}
