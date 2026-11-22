use super::*;
use crate::app::backend_store::{Program, TerminalSpec};
use heca_core::backend::{PaneBackend, TerminalDamage};

fn cell(text: &str, width: usize, fg: f32) -> TerminalCell {
    TerminalCell {
        text: text.into(),
        fg: [fg, 0.5, 0.25, 1.0],
        bg: [0.0, 0.0, 0.0, 1.0],
        bold: fg > 0.5,
        italic: false,
        underline: TerminalUnderlineStyle::None,
        width,
    }
}

fn line(text: &str) -> TerminalLine {
    TerminalLine {
        cells: text
            .chars()
            .enumerate()
            .map(|(i, ch)| cell(&ch.to_string(), 1, (i % 3) as f32 / 2.0))
            .collect(),
    }
}

fn frame(owner: &TerminalOwner, lines: Vec<TerminalLine>) -> Arc<TerminalFrame> {
    let backend = heca_core::backend::FakeBackend::new(20, lines.len());
    let mut screen = backend.terminal_snapshot().unwrap().to_wire();
    screen.rows = lines.len();
    screen.lines = lines;
    Arc::new(TerminalFrame {
        id: 1,
        owner: owner.clone(),
        title: "shell".into(),
        display_title: None,
        spec: TerminalSpec {
            program: Program::Shell,
            cwd: None,
            grid: (20, 3),
        },
        screen,
        damage: TerminalDamage::Full,
        runtime: Default::default(),
        mouse_grabbed: false,
    })
}

/// Through JSON, as the socket carries it.
fn carry(
    writer: &mut Screens,
    reader: &mut Screens,
    frame: Arc<TerminalFrame>,
) -> (usize, Arc<TerminalFrame>) {
    let packed = writer.pack(frame);
    let rows = packed.rows.len();
    let bytes = serde_json::to_vec(&Outgoing::Terminal(&packed)).unwrap();
    let Incoming::Terminal(packed) = serde_json::from_slice(&bytes).unwrap() else {
        panic!("not a terminal")
    };
    (rows, reader.unpack(*packed).unwrap())
}

#[test]
fn only_changed_rows_travel_and_the_reader_gets_the_whole_screen_back() {
    let owner = TerminalOwner::Named("t".into());
    let (mut writer, mut reader) = (Screens::default(), Screens::default());
    let first = vec![line("hello"), line("world"), line("")];
    let (rows, got) = carry(&mut writer, &mut reader, frame(&owner, first.clone()));
    assert_eq!(rows, 3, "the first frame carries every row");
    assert_eq!(got.screen.lines, first);

    let mut second = first.clone();
    second[1] = line("w0rld!");
    let (rows, got) = carry(&mut writer, &mut reader, frame(&owner, second.clone()));
    assert_eq!(rows, 1, "one row changed, one row travels");
    assert_eq!(got.screen.lines, second);

    let (rows, got) = carry(&mut writer, &mut reader, frame(&owner, second.clone()));
    assert_eq!(rows, 0);
    assert_eq!(got.screen.lines, second);
}

#[test]
fn wide_and_multi_character_cells_survive_the_trip() {
    let owner = TerminalOwner::Named("t".into());
    let (mut writer, mut reader) = (Screens::default(), Screens::default());
    let mixed = TerminalLine {
        cells: vec![
            cell("a", 1, 0.0),
            cell("界", 2, 0.0),
            cell("", 0, 0.0),
            cell("e\u{301}", 1, 0.0),
            cell("b", 1, 0.0),
            cell("c", 1, 1.0),
        ],
    };
    let lines = vec![mixed, line("plain")];
    let (_, got) = carry(&mut writer, &mut reader, frame(&owner, lines.clone()));
    assert_eq!(got.screen.lines, lines);
}

#[test]
fn a_new_height_sends_every_row_again() {
    let owner = TerminalOwner::Named("t".into());
    let (mut writer, mut reader) = (Screens::default(), Screens::default());
    carry(
        &mut writer,
        &mut reader,
        frame(&owner, vec![line("a"), line("b")]),
    );
    let taller = vec![line("a"), line("b"), line("c")];
    let (rows, got) = carry(&mut writer, &mut reader, frame(&owner, taller.clone()));
    assert_eq!(rows, 3);
    assert_eq!(got.screen.lines, taller);
}

#[test]
fn terminals_are_remembered_apart_and_forgotten_when_closed() {
    let (a, b) = (
        TerminalOwner::Named("a".into()),
        TerminalOwner::Named("b".into()),
    );
    let (mut writer, mut reader) = (Screens::default(), Screens::default());
    carry(&mut writer, &mut reader, frame(&a, vec![line("one")]));
    let (rows, got) = carry(&mut writer, &mut reader, frame(&b, vec![line("two")]));
    assert_eq!(rows, 1);
    assert_eq!(got.screen.lines, vec![line("two")]);
    writer.forget(&a);
    reader.forget(&a);
    let (rows, got) = carry(&mut writer, &mut reader, frame(&a, vec![line("one")]));
    assert_eq!(
        rows, 1,
        "a forgotten terminal starts again from a whole screen"
    );
    assert_eq!(got.screen.lines, vec![line("one")]);
}

#[test]
fn rows_for_a_screen_the_reader_never_had_are_refused() {
    let owner = TerminalOwner::Named("t".into());
    let mut writer = Screens::default();
    writer.pack(frame(&owner, vec![line("a")]));
    let packed = writer.pack(frame(&owner, vec![line("b")]));
    let error = Screens::default().unpack(packed).map(|_| ()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn a_busy_screen_is_a_fraction_of_its_old_size() {
    let owner = TerminalOwner::Named("t".into());
    let text: String = "the quick brown fox jumps over the lazy dog ".repeat(5);
    let lines: Vec<_> = (0..50).map(|_| line(&text)).collect();
    let whole = serde_json::to_vec(&frame(&owner, lines.clone()))
        .unwrap()
        .len();
    let packed = Screens::default().pack(frame(&owner, lines));
    let first = serde_json::to_vec(&Outgoing::Terminal(&packed))
        .unwrap()
        .len();
    assert!(
        first * 5 < whole,
        "a whole screen packs to {first} bytes from {whole}"
    );
}
