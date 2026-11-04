//! Follow-picker rows: the other windows on this session, chosen by title.

use super::*;

/// Rows for the follow picker: one per **other** window on the session — choosing it follows that
/// window by its id (so a rename never breaks the choice) — plus a row to stop following when this
/// window currently is one.
pub(super) fn entries(
    windows: &[crate::server::sessions::WindowInfo],
    self_window: u64,
    following: Option<u64>,
    stop_following: &str,
) -> Vec<PaletteEntry> {
    let mut rows = Vec::new();
    if following.is_some() {
        rows.push(PaletteEntry {
            id: "follow:none".into(),
            rank_id: None,
            pane: None,
            current: false,
            preselect: true,
            label: stop_following.to_string(),
            description: String::new(),
            description_path: false,
            detail: None,
            icon: None,
            shortcuts: Vec::new(),
            intent: InteractionIntent::ActivateAction(WmAction::Follow { target: None }),
        });
    }
    rows.extend(
        windows
            .iter()
            .filter(|window| window.id != self_window)
            .map(|window| PaletteEntry {
                id: format!("follow:{}", window.id),
                rank_id: Some(window.title.clone()),
                pane: None,
                current: following == Some(window.id),
                preselect: false,
                label: window.title.clone(),
                description: String::new(),
                description_path: false,
                detail: None,
                icon: None,
                shortcuts: Vec::new(),
                intent: InteractionIntent::ActivateAction(WmAction::Follow {
                    target: Some(window.id),
                }),
            }),
    );
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::sessions::WindowInfo;

    fn directory() -> Vec<WindowInfo> {
        vec![
            WindowInfo { id: 1, ordinal: 1, title: "~/projects/heca".into() },
            WindowInfo { id: 2, ordinal: 2, title: "~/projects/heca #2".into() },
        ]
    }

    #[test]
    fn the_picker_lists_other_windows_and_omits_the_stop_row_when_not_following() {
        // Window 1 picking: it sees only window 2, each row following that window by id.
        let rows = entries(&directory(), 1, None, "Stop following");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "~/projects/heca #2");
        assert!(!rows[0].current);
        assert!(matches!(
            &rows[0].intent,
            InteractionIntent::ActivateAction(WmAction::Follow { target: Some(2) })
        ));
    }

    #[test]
    fn following_adds_a_stop_row_and_marks_the_followed_window() {
        // Window 1 already following window 2: a stop row leads, and window 2's row reads current.
        let rows = entries(&directory(), 1, Some(2), "Stop following");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "Stop following");
        assert!(matches!(
            &rows[0].intent,
            InteractionIntent::ActivateAction(WmAction::Follow { target: None })
        ));
        assert!(rows[1].current, "the followed window reads as the current choice");
    }
}
