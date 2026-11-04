use super::*;

fn pending(revision: u64, seen: u64) -> PaneFeedback {
    PaneFeedback {
        pane_id: PaneId(7),
        feedback_revision: revision,
        seen_through_revision: seen,
        status: "waiting_permission".into(),
    }
}

#[test]
fn a_shared_acknowledgement_clears_every_client_without_moving_their_focus() {
    let mut first = FeedbackView::default();
    let mut second = FeedbackView::default();
    for client in [&mut first, &mut second] {
        client.receive(vec![pending(42, 41)]);
        assert_eq!(
            client.project(None),
            vec![Effect::Pulse(PaneId(7), "waiting_permission".into())]
        );
    }
    assert_eq!(
        first.project(Some(PaneId(7))),
        vec![Effect::Clear(PaneId(7)), Effect::Acknowledge(PaneId(7), 42)]
    );
    for client in [&mut first, &mut second] {
        client.receive(vec![pending(42, 42)]);
    }
    assert!(first.project(Some(PaneId(7))).is_empty());
    assert_eq!(second.project(None), vec![Effect::Clear(PaneId(7))]);
    assert!(second.project(None).is_empty());
}

#[test]
fn already_focused_panes_acknowledge_new_feedback_without_flashing_or_reentering() {
    let mut client = FeedbackView::default();
    client.receive(vec![pending(42, 41)]);
    assert_eq!(
        client.project(Some(PaneId(7))),
        vec![Effect::Acknowledge(PaneId(7), 42)]
    );
    assert!(client.project(Some(PaneId(7))).is_empty());
    client.receive(vec![pending(43, 41)]);
    assert_eq!(
        client.project(Some(PaneId(7))),
        vec![Effect::Acknowledge(PaneId(7), 43)]
    );
}

#[test]
fn reconnect_and_rebuild_do_not_replay_seen_feedback_or_restart_the_same_pulse() {
    let mut client = FeedbackView::default();
    client.receive(vec![pending(42, 42)]);
    assert!(client.project(None).is_empty());
    client.receive(vec![pending(43, 42)]);
    assert_eq!(client.project(None).len(), 1);
    client.receive(vec![pending(43, 42)]);
    assert!(client.project(None).is_empty());
    assert!(client.project(None).is_empty());
    let mut third = FeedbackView::default();
    third.receive(vec![pending(42, 42)]);
    assert!(third.project(None).is_empty());
    third.receive(vec![pending(43, 42)]);
    assert_eq!(third.project(None).len(), 1);
}

#[test]
fn an_older_acknowledgement_does_not_suppress_the_latest_event() {
    let mut client = FeedbackView::default();
    client.receive(vec![pending(42, 41)]);
    client.project(None);
    client.receive(vec![pending(43, 42)]);
    assert_eq!(
        client.project(None),
        vec![Effect::Pulse(PaneId(7), "waiting_permission".into())]
    );
}

#[test]
fn completion_pulses_except_in_the_pane_holding_actual_input_focus() {
    for status in ["success", "error"] {
        let mut event = pending(42, 41);
        event.status = status.into();
        let mut background = FeedbackView::default();
        background.receive(vec![event.clone()]);
        assert_eq!(background.project(Some(PaneId(8))),
            vec![Effect::Pulse(PaneId(7), status.into())]);
        let mut inactive_window = FeedbackView::default();
        inactive_window.receive(vec![event.clone()]);
        assert_eq!(inactive_window.project(None), vec![Effect::Pulse(PaneId(7), status.into())]);
        let mut focused = FeedbackView::default();
        focused.receive(vec![event]);
        assert_eq!(focused.project(Some(PaneId(7))), vec![Effect::Acknowledge(PaneId(7), 42)]);
    }
}

#[test]
fn a_granted_sound_plays_once_and_never_after_focus_or_shared_acknowledgement() {
    let mut client = FeedbackView::default();
    client.receive(vec![pending(42, 41)]);
    client.project(None);
    client.grant_sound(PaneId(7), 42);
    assert_eq!(client.project(None), vec![Effect::Sound(PaneId(7), 42, "waiting_permission".into())]);
    client.grant_sound(PaneId(7), 42);
    assert!(client.project(None).is_empty());
    client.receive(vec![pending(43, 42)]);
    client.project(Some(PaneId(7)));
    client.grant_sound(PaneId(7), 43);
    assert!(client.project(None).is_empty(), "already acknowledged locally");
    client.receive(vec![pending(44, 44)]);
    client.grant_sound(PaneId(7), 44);
    assert!(client.project(None).is_empty(), "shared acknowledgement wins");
}
