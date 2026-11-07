use agent_acp::{
    AcpEvent, MessageId, PermissionRequest, PlanEntry, SessionUpdate, ToolCall, ToolCallContent,
    ToolCallKind, ToolCallLocation, ToolCallStatus, ToolCallUpdate,
};
use std::collections::HashMap;

/// One rendered entry of the conversation timeline. Tool calls are referenced
/// by id and resolved against the state map so a streaming update mutates the
/// entry already shown instead of appending a new one.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineItem {
    UserMessage { content: String },
    AgentMessage { content: String },
    AgentThought { content: String },
    ToolCall { tool_call_id: String },
    Plan { entries: Vec<PlanEntry> },
    System { content: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCallState {
    pub id: String,
    pub title: String,
    pub kind: Option<ToolCallKind>,
    pub status: ToolCallStatus,
    pub content: Vec<ToolCallContent>,
    pub locations: Vec<ToolCallLocation>,
}

#[derive(Clone)]
pub struct PendingPermission {
    pub request_id: MessageId,
    pub request: PermissionRequest,
}

#[derive(Default)]
pub struct Thread {
    items: Vec<TimelineItem>,
    tool_calls: HashMap<String, ToolCallState>,
    pending_permissions: Vec<PendingPermission>,
    current_mode_id: Option<String>,
}

impl Thread {
    pub fn timeline(&self) -> &[TimelineItem] {
        &self.items
    }

    pub fn tool_call_state(&self, tool_call_id: &str) -> Option<&ToolCallState> {
        self.tool_calls.get(tool_call_id)
    }

    pub fn pending_permissions(&self) -> &[PendingPermission] {
        &self.pending_permissions
    }

    pub fn current_mode_id(&self) -> Option<&str> {
        self.current_mode_id.as_deref()
    }

    pub fn push_user_message(&mut self, content: String) {
        self.items.push(TimelineItem::UserMessage { content });
    }

    pub fn push_system_message(&mut self, content: String) {
        self.items.push(TimelineItem::System { content });
    }

    /// Streams a chunk into the last agent message, appending element when the
    /// last item already is one. Returns the affected timeline index.
    pub fn stream_agent_message_chunk(&mut self, text: String) -> Option<usize> {
        self.stream_chunk(text, |s| TimelineItem::AgentMessage { content: s })
    }

    pub fn stream_agent_thought_chunk(&mut self, text: String) -> Option<usize> {
        self.stream_chunk(text, |s| TimelineItem::AgentThought { content: s })
    }

    fn stream_chunk(
        &mut self,
        text: String,
        item_kind: impl Fn(String) -> TimelineItem,
    ) -> Option<usize> {
        if text.is_empty() {
            return None;
        }
        let chunk_kind = item_kind("".into());
        if let Some(last) = self
            .items
            .last_mut()
            .filter(|item| std::mem::discriminant(*item) == std::mem::discriminant(&chunk_kind))
        {
            let item_content = match last {
                TimelineItem::AgentMessage { content } | TimelineItem::AgentThought { content } => {
                    content
                }
                _ => return None,
            };
            item_content.push_str(&text);
            Some(self.items.len() - 1)
        } else {
            self.items.push(item_kind(text));
            Some(self.items.len() - 1)
        }
    }

    pub fn handle_event(&mut self, event: &AcpEvent) {
        match event {
            AcpEvent::SessionUpdate { update, .. } => self.handle_session_update(update),
            AcpEvent::PermissionRequest { .. } => {}
            AcpEvent::Terminated => {}
        }
    }

    fn handle_session_update(&mut self, update: &SessionUpdate) {
        match update {
            SessionUpdate::AgentMessageChunk { content } => {
                if let Some(text) = content.text_content() {
                    self.stream_agent_message_chunk(text.to_string());
                }
            }
            SessionUpdate::AgentThoughtChunk { content } => {
                if let Some(text) = content.text_content() {
                    self.stream_agent_thought_chunk(text.to_string());
                }
            }
            SessionUpdate::UserMessageChunk { .. } => {}
            SessionUpdate::ToolCall(tool_call) => self.record_tool_call(tool_call.clone()),
            SessionUpdate::ToolCallUpdate(update) => self.update_tool_call(update.clone()),
            SessionUpdate::Plan { entries } => self.replace_plan(entries.clone()),
            SessionUpdate::AvailableCommands { .. } => {
                // Ignored; mode selector not yet implemented.
            }
            SessionUpdate::CurrentMode { current_mode_id } => {
                self.current_mode_id = Some(current_mode_id.clone());
            }
            SessionUpdate::Unknown { .. } => {}
        }
    }

    pub fn record_tool_call(&mut self, tool_call: ToolCall) {
        let tool_call_id = tool_call.id.clone();
        self.tool_calls.insert(
            tool_call_id.clone(),
            ToolCallState {
                id: tool_call.id,
                title: tool_call.title,
                kind: tool_call.kind,
                status: tool_call.status,
                content: tool_call.content,
                locations: tool_call.locations,
            },
        );
        let already_last = matches!(
            self.items.last(),
            Some(TimelineItem::ToolCall { tool_call_id: last }) if *last == tool_call_id
        );
        if !already_last {
            self.items.push(TimelineItem::ToolCall { tool_call_id });
        }
    }

    pub fn update_tool_call(&mut self, update: ToolCallUpdate) {
        let Some(state) = self.tool_calls.get_mut(&update.id) else {
            return;
        };
        if let Some(title) = update.title {
            state.title = title;
        }
        if let Some(kind) = update.kind {
            state.kind = Some(kind);
        }
        if let Some(status) = update.status {
            state.status = status;
        }
        if let Some(content) = update.content {
            state.content = content;
        }
        if let Some(locations) = update.locations {
            state.locations = locations;
        }
    }

    /// The plan arrives as full snapshots; each snapshot replaces the previous
    /// one so the timeline holds a single live plan rather than a history.
    pub fn replace_plan(&mut self, entries: Vec<PlanEntry>) {
        if let Some(index) = self.items.iter().rposition(|item| {
            matches!(item, TimelineItem::Plan { .. })
        }) {
            self.items[index] = TimelineItem::Plan { entries };
        } else {
            self.items.push(TimelineItem::Plan { entries });
        }
    }

    pub fn set_session_shape(&mut self, current_mode_id: Option<String>) {
        self.current_mode_id = current_mode_id;
    }

    pub fn queue_permission(&mut self, request_id: MessageId, request: PermissionRequest) {
        self.pending_permissions.push(PendingPermission {
            request_id,
            request,
        });
    }

    pub fn take_permission(&mut self, request_id: &MessageId) -> Option<PendingPermission> {
        if let Some(index) = self
            .pending_permissions
            .iter()
            .rposition(|pending| &pending.request_id == request_id)
        {
            Some(self.pending_permissions.swap_remove(index))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_acp::ToolCallStatus;

    #[test]
    fn chunks_accumulate_into_one_message_per_role() {
        let mut thread = Thread::default();
        assert_eq!(
            thread.stream_agent_message_chunk("hel".into()),
            Some(0)
        );
        assert_eq!(
            thread.stream_agent_thought_chunk("thi".into()),
            Some(1)
        );
        assert_eq!(
            thread.stream_agent_message_chunk("lo".into()),
            Some(2)
        );
        assert_eq!(
            thread.timeline(),
            &[
                TimelineItem::AgentMessage {
                    content: "hel".into()
                },
                TimelineItem::AgentThought {
                    content: "thi".into()
                },
                TimelineItem::AgentMessage {
                    content: "hello".into()
                },
            ]
        );
    }

    #[test]
    fn a_message_item_extends_when_it_is_the_last() {
        let mut thread = Thread::default();
        thread.push_user_message("hi".into());
        assert_eq!(thread.stream_agent_message_chunk("hey".into()), Some(1));
        assert_eq!(thread.stream_agent_message_chunk(" there".into()), Some(1));
        assert_eq!(
            thread.timeline(),
            &[
                TimelineItem::UserMessage { content: "hi".into() },
                TimelineItem::AgentMessage {
                    content: "hey there".into()
                },
            ]
        );
    }

    #[test]
    fn empty_chunks_do_not_create_messages() {
        let mut thread = Thread::default();
        assert_eq!(thread.stream_agent_message_chunk("".into()), None);
        assert!(thread.timeline().is_empty());
    }

    #[test]
    fn tool_calls_are_updated_in_place_by_id() {
        let mut thread = Thread::default();
        let create_json = serde_json::json!({
            "toolCallId": "call-1",
            "title": "Read file",
            "kind": "read",
            "status": "in_progress",
        });
        let tool_call: ToolCall =
            serde_json::from_value(create_json).expect("parse tool call");
        thread.record_tool_call(tool_call);

        let update_json = serde_json::json!({
            "toolCallId": "call-1",
            "title": "Finished",
            "status": "completed",
        });
        let update: ToolCallUpdate =
            serde_json::from_value(update_json).expect("parse tool call update");
        thread.update_tool_call(update);

        assert_eq!(thread.timeline().len(), 1);
        let state = thread
            .tool_call_state("call-1")
            .expect("tracked tool call state");
        assert_eq!(state.title, "Finished");
        assert_eq!(state.status, ToolCallStatus::Completed);
        assert_eq!(
            thread.timeline(),
            &[TimelineItem::ToolCall {
                tool_call_id: "call-1".into()
            }],
        );
    }

    #[test]
    fn plan_snapshots_replace_the_previous_plan() {
        let mut thread = Thread::default();
        thread.replace_plan(vec![plan_entry("first")]);
        thread.replace_plan(vec![plan_entry("first"), plan_entry("second")]);
        assert_eq!(thread.timeline().len(), 1);
        match thread.timeline().first() {
            Some(TimelineItem::Plan { entries }) => {
                assert_eq!(entries.len(), 2)
            }
            other => panic!("expected plan item, found {other:?}"),
        }
    }

    fn plan_entry(content: &str) -> PlanEntry {
        PlanEntry {
            content: content.into(),
            priority: Default::default(),
            status: Default::default(),
        }
    }

    #[test]
    fn permissions_are_queued_and_taken_by_request_id() {
        let mut thread = Thread::default();
        let request = PermissionRequest {
            session_id: "s1".into(),
            tool_call_id: None,
            options: Vec::new(),
        };
        thread.queue_permission(MessageId::Number(7), request);
        let removed = thread.take_permission(&MessageId::Number(7));
        assert!(removed.is_some());
        assert!(thread.pending_permissions().is_empty());
        assert!(thread.take_permission(&MessageId::Number(8)).is_none());
    }
}
