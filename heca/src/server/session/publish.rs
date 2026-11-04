//! Projections sent only to the clients attached to this session.

use super::*;

impl SessionRuntime {
    pub(super) fn publish_feedback(&mut self) -> Vec<usize> {
        if !self.state.feedback.take_dirty() { return Vec::new() }
        let panes = self.state.feedback.snapshot();
        self.clients.iter().enumerate().filter_map(|(index, client)| {
            (matches!(client.attachment.get(), Attachment::Attached)
                && client.link.send(ServerMsg::Feedback {
                    session_id: self.session.id, panes: panes.clone(),
                }).is_err()).then_some(index)
        }).collect()
    }

    /// Tell every attached client what changed — each reacts to it for itself.
    pub(super) fn broadcast(&self, changes: &[Change]) -> Vec<usize> {
        self.broadcast_except(changes, None)
    }

    /// The session's window directory: one entry per attached window, in attach order, with the
    /// title that window reported (empty until it has). Follow and the window list are built from
    /// this (decision e36aba0c).
    pub(super) fn window_directory(&self) -> Vec<super::super::sessions::WindowInfo> {
        self.clients
            .iter()
            .filter(|client| matches!(client.attachment.get(), Attachment::Attached))
            .map(|client| super::super::sessions::WindowInfo {
                id: client.id,
                ordinal: client.window_ordinal.get(),
                title: client.title.borrow().clone().unwrap_or_default(),
            })
            .collect()
    }

    /// Tell each window whether anyone now follows it, but only when that changed — so a followed
    /// window reports its view ([`ClientMsg::ReportView`]) and the rest stay quiet. Recomputed after
    /// a follow request and after a window leaves.
    pub(super) fn refresh_follow(&self) -> Vec<usize> {
        let followed: std::collections::HashSet<u64> =
            self.clients.iter().filter_map(|client| client.follows.get()).collect();
        let mut disconnected = Vec::new();
        for (i, client) in self.clients.iter().enumerate() {
            let active = followed.contains(&client.id);
            if client.was_followed.replace(active) != active
                && matches!(client.attachment.get(), Attachment::Attached)
                && client.link.send(ServerMsg::Followed { active }).is_err()
            {
                disconnected.push(i);
            }
        }
        disconnected
    }

    /// Relay a leader's new view to every window that follows it.
    pub(super) fn relay_follow_view(&self, leader: u64, view: &heca_core::layout::ViewSnapshot) -> Vec<usize> {
        let mut disconnected = Vec::new();
        for (i, client) in self.clients.iter().enumerate() {
            if client.follows.get() == Some(leader)
                && matches!(client.attachment.get(), Attachment::Attached)
                && client.link.send(ServerMsg::FollowView(view.clone())).is_err()
            {
                disconnected.push(i);
            }
        }
        disconnected
    }

    /// Send the current window directory to every attached client, so each can list the others (the
    /// follow picker) and show them. Sent after attach and on every join, leave and title change.
    pub(super) fn publish_windows(&self) -> Vec<usize> {
        let windows = self.window_directory();
        let mut disconnected = Vec::new();
        for (i, client) in self.clients.iter().enumerate() {
            if matches!(client.attachment.get(), Attachment::Attached)
                && client
                    .link
                    .send(ServerMsg::Windows {
                        session_id: self.session.id,
                        windows: windows.clone(),
                    })
                    .is_err()
            {
                disconnected.push(i);
            }
        }
        disconnected
    }

    /// Send the current notification projection to every attached client when the visible set
    /// changed. The notification seam does **not** ride the generic [`broadcast`](Self::broadcast):
    /// its dataless `NotificationsChanged` fact would otherwise drag the whole `SessionContent`
    /// across on every notification raise and every auto-dismiss tick. The window builds its own
    /// toasts from the projection, so this is the one message the seam needs.
    pub(super) fn publish_notifications(&self, changes: &[Change]) -> Vec<usize> {
        let mut disconnected = Vec::new();
        if changes
            .iter()
            .any(|change| matches!(change, Change::NotificationsChanged))
        {
            // Each window gets its own projection: the session-wide cards plus the ones addressed
            // to it. A card raised for one window never reaches the others.
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment.get(), Attachment::Attached) {
                    let views = self.state.visible_notifications_for(client.id);
                    if client
                        .link
                        .send(ServerMsg::Notifications(views))
                        .is_err()
                    {
                        disconnected.push(i);
                    }
                }
            }
        }
        disconnected
    }

    pub(super) fn broadcast_except(
        &self,
        changes: &[Change],
        reply_to: Option<usize>,
    ) -> Vec<usize> {
        let mut disconnected = Vec::new();
        if !changes.is_empty() {
            let content = self.session.content();
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment.get(), Attachment::Attached)
                    && client
                        .link
                        .send(ServerMsg::Content {
                            content: content.clone(),
                        })
                        .is_err()
                {
                    disconnected.push(i);
                }
            }
        }
        for change in changes {
            for (i, client) in self.clients.iter().enumerate() {
                if reply_to != Some(i)
                    && matches!(client.attachment.get(), Attachment::Attached)
                    && client
                        .link
                        .send(ServerMsg::Changed(change.clone()))
                        .is_err()
                {
                    disconnected.push(i);
                }
            }
        }
        disconnected
    }
}
