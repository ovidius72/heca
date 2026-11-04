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
            let views = self.state.visible_notifications();
            for (i, client) in self.clients.iter().enumerate() {
                if matches!(client.attachment.get(), Attachment::Attached)
                    && client
                        .link
                        .send(ServerMsg::Notifications(views.clone()))
                        .is_err()
                {
                    disconnected.push(i);
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
