use super::*;

fn ask(args: &[&str]) -> Option<String> {
    answer(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>()).map(|reply| reply.unwrap_or_else(|error| error))
}

/// **The help lists exactly what runs, arguments included.** Both are rendered from the table
/// the dispatcher walks, so a command cannot be documented without existing, nor exist without
/// being documented — which is the way a `--help` normally goes stale.
#[test]
fn the_help_lists_every_command_and_every_argument_it_declares() {
    let text = help();
    for c in COMMANDS {
        assert!(
            text.contains(c.name),
            "`{}` runs but --help does not list it",
            c.name
        );
        for a in c.args.iter().chain(c.options) {
            assert!(
                text.contains(a.name) && text.contains(a.description),
                "`{}` declares `{}` but --help does not describe it",
                c.name,
                a.name
            );
        }
    }
}

/// **Nothing is asked for ⇒ open the window.** The only invocation that is not a question.
#[test]
fn no_command_opens_the_window() {
    assert!(ask(&[]).is_none());
}

#[test]
fn session_launches_share_the_checked_cli_declaration() {
    let mode = |args: &[&str]| launch_mode(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>());
    assert_eq!(mode(&["--new"]), LaunchMode::New { path: None, name: None });
    assert_eq!(mode(&["--new", "project A", "--name", "work"]),
        LaunchMode::New { path: Some("project A".into()), name: Some("work".into()) });
    assert_eq!(mode(&["--new", "--name", "work", "~/projects/A"]),
        LaunchMode::New { path: Some("~/projects/A".into()), name: Some("work".into()) });
    assert_eq!(mode(&["--attach", "heca #3"]), LaunchMode::Session("heca #3".into()));
    for args in [&["--new", "--name"][..], &["--new", "--name", "A", "--name", "B"], &["--attach"], &["--standalone", "--name", "A"]] {
        assert!(answer(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>()).is_some_and(|answer| answer.is_err()));
    }
    assert!(ask(&["--new", "--help"]).unwrap().contains("[--name <name>]"));
}

/// **A missing argument is a sentence, not silence** — and the same sentence an action gives,
/// because it is the same check.
#[test]
fn a_missing_argument_says_which_one_and_how_to_call_it() {
    let out = ask(&["--describe-action"]).expect("the command was recognised");
    assert!(
        out.contains("missing required argument 'name'"),
        "got: {out}"
    );
    assert!(
        out.contains("Usage: heca --describe-action <name> [--json]"),
        "got: {out}"
    );
}

/// **A command cannot be handed more than it declared.**
#[test]
fn too_many_arguments_are_refused() {
    let out = ask(&["--describe-action", "resize", "extra"]).expect("recognised");
    assert!(out.contains("takes 1 argument(s), got 2"), "got: {out}");
}

/// **An unrecognised flag is answered, with the nearest real one offered.** Opening the window
/// on a typo makes a misspelling look like a broken app.
#[test]
fn an_unknown_flag_is_answered_and_a_near_one_is_offered() {
    let out = ask(&["--list-action"]).expect("an unknown flag is still a question");
    assert!(
        out.contains("unknown command '--list-action'"),
        "got: {out}"
    );
    assert!(out.contains("did you mean '--list-actions'?"), "got: {out}");
    assert!(out.contains("--help"), "…and says where the list is");
}

/// **A one-letter form runs the same command**, and no two commands claim the same letter — a
/// collision would make one of them unreachable, silently, with nothing failing.
#[test]
fn every_short_form_runs_its_command_and_none_collide() {
    let mut seen: Vec<&str> = Vec::new();
    for c in COMMANDS {
        let Some(short) = c.short else { continue };
        assert_eq!(
            short.len(),
            2,
            "`{short}` is not a single letter after its dash"
        );
        assert!(
            !seen.contains(&short),
            "`{short}` is claimed by two commands"
        );
        seen.push(short);
        assert!(
            ask(&[short]).is_some(),
            "`{short}` is declared for {} but names nothing",
            c.name
        );
        assert!(help().contains(short), "…and --help does not show it");
    }
    // The same command, either spelling, same answer.
    assert_eq!(ask(&["-a"]), ask(&["--list-actions"]));
    assert_eq!(
        ask(&["-d", "resize"]),
        ask(&["--describe-action", "resize"])
    );
    // Including the JSON switch.
    assert_eq!(
        ask(&["-d", "resize", "-j"]),
        ask(&["--describe-action", "resize", "--json"])
    );
}

/// **A negative number is a value, not a flag** — so an argument may start with a dash without
/// the dispatcher mistaking it for one.
#[test]
fn a_negative_number_is_read_as_an_argument() {
    let out = ask(&["--describe-action", "-40"]).expect("recognised");
    assert!(out.contains("no action named '-40'"), "got: {out}");
}

/// Every action is listed, and one can be asked about in full.
#[test]
fn actions_are_listable_and_describable() {
    let listed = ask(&["--list-actions"]).expect("recognised");
    assert!(listed.contains("resize"), "the listing names every action");
    assert!(
        listed.lines().count() > 20,
        "…and it is the whole catalog, not a sample"
    );

    let one = ask(&["--describe-action", "resize"]).expect("recognised");
    assert!(one.contains("edge"), "an argument is named");
    assert!(one.contains("optional"), "…and says whether it is required");
    assert!(
        one.contains("auto, top, bottom, left, right"),
        "…and lists what it accepts"
    );

    let json = ask(&["--describe-action", "resize", "--json"]).expect("recognised");
    assert!(
        json.trim_start().starts_with('{'),
        "--json gives a script JSON"
    );
}

/// A name nobody has heard of is answered, not ignored.
#[test]
fn an_unknown_action_says_so_and_says_where_to_look() {
    let out = ask(&["--describe-action", "nope"]).expect("recognised");
    assert!(out.contains("no action named 'nope'"));
    assert!(
        out.contains("--list-actions"),
        "and points at the way to find one"
    );
}
