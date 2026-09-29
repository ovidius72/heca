use super::assign_letters;
use std::collections::HashMap;

fn ids(names: &[&str]) -> Vec<Option<String>> {
    names.iter().map(|n| Some((*n).to_string())).collect()
}

fn remember(pairs: &[(&str, char)]) -> HashMap<String, char> {
    pairs.iter().map(|(n, c)| ((*n).to_string(), *c)).collect()
}

/// With nothing remembered, letters go out in order — the home row first.
#[test]
fn a_first_pick_hands_out_the_alphabet_in_order() {
    let got = assign_letters(&ids(&["a", "b", "c"]), &HashMap::new());
    assert_eq!(got, vec![Some('a'), Some('s'), Some('d')]);
}

/// ⭐ **The report.** Antonio, 2026-08-17: *"I want to expand a pane, prefix+/ and `k` appears on
/// that icon… then I want to collapse. prefix+/ and `j` appears on that button, while I was
/// expecting `k`."*
///
/// A target appearing **earlier in the tree** used to shift every letter after it, because the
/// letter was the index. Now the newcomer takes a spare and everyone else keeps theirs.
#[test]
fn a_new_target_in_front_does_not_move_anyone_elses_letter() {
    let before = assign_letters(&ids(&["expand", "close"]), &HashMap::new());
    assert_eq!(before, vec![Some('a'), Some('s')]);

    let remembered = remember(&[("expand", 'a'), ("close", 's')]);
    let after = assign_letters(&ids(&["status", "expand", "close"]), &remembered);

    assert_eq!(
        after,
        vec![Some('d'), Some('a'), Some('s')],
        "expand keeps `a` and close keeps `s`; the newcomer takes the first free letter"
    );
}

/// Reopening on an unchanged screen gives exactly the same letters — the plainest form of the
/// promise, and the one a user notices first.
#[test]
fn reopening_an_unchanged_screen_gives_the_same_letters() {
    let first = assign_letters(&ids(&["one", "two", "three"]), &HashMap::new());
    let remembered = remember(&[("one", 'a'), ("two", 's'), ("three", 'd')]);
    assert_eq!(
        assign_letters(&ids(&["one", "two", "three"]), &remembered),
        first
    );
}

/// A target that has gone releases its letter, and the next newcomer may take it — the memory
/// is rebuilt from what is on screen, so it cannot leak the pool away.
#[test]
fn a_departed_targets_letter_returns_to_the_pool() {
    let remembered = remember(&[("gone", 'a'), ("stays", 's')]);
    let got = assign_letters(&ids(&["stays", "new"]), &remembered);
    assert_eq!(
        got,
        vec![Some('s'), Some('a')],
        "`stays` keeps `s`, and `a` is free again for the newcomer"
    );
}

/// **Two views of one thing share its letter.**
///
/// A pane in the scrolling area and the sidebar row for that pane are the same pane, so they
/// answer to one name. Giving each its own letter asks which picture of the same thing you
/// meant, and burns the alphabet twice as fast — which is what pushed the letters into
/// uppercase. Offering already puts one letter on every place a name is shown.
#[test]
fn two_views_of_one_thing_wear_the_same_letter() {
    let remembered = remember(&[("pane:7", 'a')]);
    let got = assign_letters(&ids(&["pane:7", "pane:7"]), &remembered);
    assert_eq!(got, vec![Some('a'), Some('a')]);
}

/// …and a name with no remembered letter still gets one letter for both of its views, not two.
#[test]
fn two_views_of_a_new_thing_also_share_one_letter() {
    let got = assign_letters(&ids(&["pane:7", "pane:7", "pane:8"]), &Default::default());
    assert_eq!(
        got,
        vec![Some('a'), Some('a'), Some('s')],
        "one letter for the pane seen twice, the next letter for the other pane",
    );
}

/// **Open the picker, close it, open it again: the same letters.**
///
/// This walks the full round `handle_hint_pick` performs — assign, then remember — twice over
/// an unchanged set of targets, which is exactly what two presses do. The single-pass tests
/// above cannot see this: the defect only appears once the remember step has written back.
///
/// **It holds only while every target has a distinct name, and that is not this function's to
/// guarantee.** Given two targets with one name, both ask for the same remembered letter, the
/// first takes it, the second is refused and draws a fresh one — and the remember step then
/// saves the loser's letter, so the next opening trades them back, forever. That is why the
/// invariant is enforced where names are built (`heca_grid_ui::nav::identity_of`) rather than
/// patched here: no assignment rule can tell apart two things that claim to be the same thing.
#[test]
fn two_openings_of_an_unchanged_picker_hand_out_the_same_letters() {
    let identities = ids(&["ws:0", "ws:0/pane:2", "ws:0/pane:3"]);
    let mut remembered: HashMap<String, char> = HashMap::new();

    // Press 1 — assign, then remember, the way `handle_hint_pick` does.
    let first = assign_letters(&identities, &remembered);
    remembered.extend(
        identities
            .iter()
            .zip(first.iter())
            .filter_map(|(id, ch)| Some((id.clone()?, (*ch)?))),
    );

    // Esc, then press 2 — nothing about the targets has changed.
    let second = assign_letters(&identities, &remembered);

    assert_eq!(
        second, first,
        "a second opening must repeat the first's letters"
    );
}

/// A target with no identity cannot be remembered, but still gets a letter — it just gets a
/// fresh one each time.
#[test]
fn a_target_without_an_identity_still_gets_a_letter() {
    let got = assign_letters(&[None, Some("keyed".into())], &remember(&[("keyed", 'a')]));
    assert_eq!(got, vec![Some('s'), Some('a')]);
}

/// Past the alphabet a target gets **no letter**, never a longer one (§2a: one keystroke,
/// always; 52 is the cap).
#[test]
fn past_the_alphabet_a_target_gets_no_letter() {
    let names: Vec<String> = (0..60).map(|i| format!("t{i}")).collect();
    let ids: Vec<Option<String>> = names.iter().cloned().map(Some).collect();
    let got = assign_letters(&ids, &HashMap::new());
    assert_eq!(got.iter().filter(|c| c.is_some()).count(), 52);
    assert!(got[52..].iter().all(|c| c.is_none()));
}
