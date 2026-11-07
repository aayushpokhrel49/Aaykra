use std::{path::PathBuf, sync::Arc};

use agent_acp::{
    AcpAgent, AcpEvent, AcpSession, AgentSpawnConfig, ClientCapabilities, ContentBlock, MessageId,
    PermissionOutcome, SessionUpdate, StopReason,
};
use editor::{Editor, EditorMode, MultiBuffer};
use fs::Fs;
use futures::StreamExt as _;
use gpui::{
    actions, App, AppContext as _, AsyncApp, AsyncWindowContext, Context, Entity, EventEmitter,
    Focusable, ScrollHandle, SharedString, Task, TextStyleRefinement, WeakEntity, Window, div,
    prelude::*,
};
use language::Buffer;
use markdown::{Markdown, MarkdownElement, MarkdownStyle};
use settings::{Settings, update_settings_file};
use theme::ActiveTheme as _;
use ui::{IconButton, IconButtonShape, SpinnerLabel, Tooltip, prelude::*};
use workspace::{
    HideStatusItem, Workspace,
    dock::{DockPosition, Panel, PanelEvent},
};

use crate::{
    agents::{AgentProfile, BUILTIN_PROFILES},
    settings::AgentPanelSettings,
    thread::{PendingPermission, Thread, TimelineItem},
};

const AGENT_PANEL_KEY: &str = "agent_panel";

actions!(
    agent_panel,
    [
        /// Toggles the agent panel.
        Toggle,
        /// Toggles focus on the agent panel.
        ToggleFocus,
        /// Sends the prompt currently typed into the agent panel editor.
        SendPrompt,
        /// Stops the current agent turn.
        CancelRun,
        /// Approves the pending permission request once.
        AllowOnce,
        /// Approves the pending permission request for the rest of the session.
        AllowAlways,
        /// Rejects the pending permission request once.
        RejectOnce,
    ]
);

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConnectionState {
    /// No agent has been spawned yet.
    Idle,
    /// Spawning or authenticating the agent process.
    Starting { label: SharedString },
    /// Session alive; the agent waits for the user.
    Running { label: SharedString },
    /// Session alive; a turn is being streamed.
    Working { label: SharedString },
    /// The agent could not be started; another one can be selected.
    Failed { error: SharedString },
}

pub struct AgentPanel {
    workspace: WeakEntity<Workspace>,
    fs: Arc<dyn Fs>,
    focus_handle: gpui::FocusHandle,
    message_buffer: Entity<Buffer>,
    message_editor: Entity<Editor>,
    thread: Thread,
    markdown_by_item: std::collections::HashMap<usize, Entity<Markdown>>,
    scroll_handle: ScrollHandle,
    agent: Option<Arc<AcpAgent>>,
    session: Option<AcpSession>,
    connection_task: Option<Task<()>>,
    driver_task: Option<Task<()>>,
    turn_task: Option<Task<()>>,
    status: ConnectionState,
    pending_first_message: Option<String>,
}

impl AgentPanel {
    fn new(
        workspace: &mut Workspace,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Entity<Self> {
        let fs = workspace.app_state().fs.clone();
        let message_buffer = cx.new(|cx| Buffer::local(String::new(), cx));
        let message_editor = {
            let message_buffer = message_buffer.clone();
            cx.new(move |cx| {
                let multi_buffer = cx.new(|cx| MultiBuffer::singleton(message_buffer.clone(), cx));
                let mut editor = Editor::new(
                    EditorMode::AutoHeight {
                        min_lines: 1,
                        max_lines: Some(6),
                    },
                    multi_buffer,
                    None,
                    window,
                    cx,
                );
                editor.set_show_gutter(false, cx);
                editor.set_show_wrap_guides(false, cx);
                editor.set_show_indent_guides(false, cx);
                editor.set_placeholder_text("Describe what you need…", window, cx);
                editor
            })
        };

        cx.new(|cx| Self {
            workspace: workspace.weak_handle(),
            fs,
            focus_handle: cx.focus_handle(),
            message_buffer,
            message_editor,
            thread: Thread::default(),
            markdown_by_item: std::collections::HashMap::default(),
            scroll_handle: ScrollHandle::new(),
            agent: None,
            session: None,
            connection_task: None,
            driver_task: None,
            turn_task: None,
            status: ConnectionState::Idle,
            pending_first_message: None,
        })
    }

    fn working_directory(&self, cx: &App) -> Option<PathBuf> {
        self.workspace
            .upgrade()?
            .read(cx)
            .worktrees(cx)
            .next()
            .map(|worktree| worktree.read(cx).abs_path().to_path_buf())
    }

    fn toggle(
        workspace: &mut Workspace,
        _: &Toggle,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        if !workspace.toggle_panel_focus::<AgentPanel>(window, cx) {
            workspace.close_panel::<AgentPanel>(window, cx);
        }
    }

    fn push_system_message(&mut self, content: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.thread.push_system_message(content.into().to_string());
        cx.notify();
    }

    // Connection lifecycle ------------------------------------------------

    fn start_agent(&mut self, profile: &'static AgentProfile, cx: &mut Context<Self>) {
        if !matches!(
            self.status,
            ConnectionState::Idle | ConnectionState::Failed { .. }
        ) {
            return;
        }
        let Some(working_directory) = self.working_directory(cx) else {
            self.push_system_message("Open a folder before starting an agent.", cx);
            return;
        };

        self.status = ConnectionState::Starting {
            label: profile.label.into(),
        };
        self.teardown_agent(cx);
        cx.notify();

        let config: AgentSpawnConfig = profile.spawn_config(Some(working_directory.clone()));
        let label: SharedString = profile.label.into();

        self.connection_task = Some(cx.spawn(async move |this, cx| {
            let (agent, mut events, driver) = match AcpAgent::spawn(config).await {
                Ok(agent_acp::SpawnedAgent {
                    agent,
                    events,
                    driver,
                }) => (agent, events, driver),
                Err(error) => {
                    Self::mark_failed(this, format!("{error}"), cx);
                    return;
                }
            };

            // Dropping a task cancels it, so the connection handle stays in
            // place and the driver gets its own slot.
            this.update(cx, |this, cx| {
                this.agent = Some(agent.clone());
                this.driver_task = Some(cx.background_spawn(driver));
                cx.notify();
            })
            .ok();

            if let Err(error) = agent.initialize(ClientCapabilities::default()).await {
                agent.terminate();
                Self::mark_failed(this, format!("Agent initialization failed: {error}"), cx);
                return;
            }

            let working_directory = working_directory.display().to_string();
            let Ok(session) = agent.new_session(&working_directory).await else {
                this.update(cx, |this, cx| {
                    this.agent = None;
                    this.status = ConnectionState::Failed {
                        error: "Failed to create a session.".into(),
                    };
                    cx.notify();
                })
                .ok();
                return;
            };

            let pending_message = this
                .update(cx, |this, cx| {
                    this.session = Some(session.clone());
                    this.thread
                        .set_session_shape(session.current_mode_id.clone());
                    this.status = ConnectionState::Running { label: label.clone() };
                    let pending = this.pending_first_message.take();
                    cx.notify();
                    pending
                })
                .ok()
                .flatten();

            if let Some(message) = pending_message {
                let blocks = vec![ContentBlock::text(message)];
                this.update(cx, |this, cx| {
                    this.status = ConnectionState::Working { label: label.clone() };
                    cx.notify();
                })
                .ok();
                let result = session.prompt(blocks).await;
                settle_turn(this.clone(), result, &label, cx);
            }

            while let Some(event) = events.next().await {
                this.update(cx, |this, cx| this.handle_event(event, cx)).ok();
            }

            this.update(cx, |this, cx| {
                this.agent = None;
                this.session = None;
                this.status = ConnectionState::Idle;
                this.push_system_message("Agent disconnected.", cx);
                cx.notify();
            })
            .ok();
        }));
    }

    fn mark_failed(this: WeakEntity<Self>, error: String, cx: &mut AsyncApp) {
        this.update(cx, |this, cx| {
            this.agent = None;
            this.session = None;
            this.status = ConnectionState::Failed {
                error: error.into(),
            };
            cx.notify();
        })
        .ok();
    }

    /// Disconnects the current agent process, if any. Dropping the agent arc
    /// runs its drop handler, which closes stdin and kills the process group.
    fn teardown_agent(&mut self, cx: &mut Context<Self>) {
        self.turn_task = None;
        self.connection_task = None;
        self.driver_task = None;
        if let Some(agent) = self.agent.take() {
            agent.terminate();
        }
        self.session = None;
        cx.notify();
    }

    fn handle_event(&mut self, event: AcpEvent, cx: &mut Context<Self>) {
        self.thread.handle_event(&event);
        match event {
            AcpEvent::SessionUpdate { update, .. } => {
                match update {
                    SessionUpdate::AgentMessageChunk { content } => {
                        if let Some(text) = content.text_content() {
                            let item_index =
                                self.thread.stream_agent_message_chunk(text.to_string());
                            self.stream_markdown(item_index, text, cx);
                        }
                    }
                    SessionUpdate::AgentThoughtChunk { content } => {
                        if let Some(text) = content.text_content() {
                            let item_index =
                                self.thread.stream_agent_thought_chunk(text.to_string());
                            self.stream_markdown(item_index, text, cx);
                        }
                    }
                    _ => {}
                }
                cx.notify();
            }
            AcpEvent::PermissionRequest { request_id, request } => {
                self.thread.queue_permission(request_id, request);
                cx.notify();
            }
            AcpEvent::Terminated => {}
        }
    }

    /// Keeps one markdown entity per streaming timeline item. The first chunk
    /// creates the entity seeded with everything the thread accumulated so
    /// far; every later chunk only has to be appended.
    fn stream_markdown(&mut self, item_index: Option<usize>, text: &str, cx: &mut Context<Self>) {
        let Some(item_index) = item_index else {
            return;
        };
        match self.markdown_by_item.get(&item_index) {
            Some(markdown) => {
                let markdown = markdown.clone();
                markdown.update(cx, |markdown, cx| markdown.append(text, cx));
            }
            None => {
                let full_text = match self.thread.timeline().get(item_index) {
                    Some(TimelineItem::AgentMessage { content })
                    | Some(TimelineItem::AgentThought { content }) => content.clone(),
                    _ => text.to_string(),
                };
                let markdown = cx.new(|cx| Markdown::new_text(full_text.into(), cx));
                self.markdown_by_item.insert(item_index, markdown);
            }
        }
    }

    fn turn(&mut self, blocks: Vec<ContentBlock>, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        self.status = ConnectionState::Working {
            label: self.active_label(),
        };
        cx.notify();

        let active_label = self.active_label();
        self.turn_task = Some(cx.spawn(async move |this, cx| {
            let result = session.prompt(blocks).await;
            settle_turn(this, result, &active_label, cx);
        }));
    }

    fn active_label(&self) -> SharedString {
        match &self.status {
            ConnectionState::Starting { label }
            | ConnectionState::Running { label }
            | ConnectionState::Working { label } => label.clone(),
            ConnectionState::Idle | ConnectionState::Failed { .. } => "agent".into(),
        }
    }

    // Actions -------------------------------------------------------------

    fn send_prompt(&mut self, _: &SendPrompt, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self.message_buffer.read(cx).text().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.message_buffer
            .update(cx, |message_buffer, cx| message_buffer.set_text("", cx));

        match &self.status {
            ConnectionState::Running { .. } | ConnectionState::Working { .. } => {
                self.thread.push_user_message(text.clone());
                self.turn(vec![ContentBlock::text(text)], cx);
            }
            ConnectionState::Starting { .. } => {
                self.thread.push_user_message(text.clone());
                self.pending_first_message = Some(text);
            }
            ConnectionState::Idle | ConnectionState::Failed { .. } => {
                self.thread.push_user_message(text.clone());
                self.pending_first_message = Some(text);
                self.start_agent(&BUILTIN_PROFILES[0], cx);
            }
        }
        cx.notify();
    }

    fn cancel_run(&mut self, _: &CancelRun, _: &mut Window, _cx: &mut Context<Self>) {
        if let Some(session) = &self.session {
            session.cancel();
        }
    }

    fn allow_once(&mut self, _: &AllowOnce, _: &mut Window, cx: &mut Context<Self>) {
        self.respond_to_permission("allow_once", "allow_always", cx);
    }

    fn allow_always(&mut self, _: &AllowAlways, _: &mut Window, cx: &mut Context<Self>) {
        self.respond_to_permission("allow_always", "allow_once", cx);
    }

    fn reject_once(&mut self, _: &RejectOnce, _: &mut Window, cx: &mut Context<Self>) {
        self.respond_to_permission("reject_once", "reject_always", cx);
    }

    /// Answers the newest pending permission with the first option matching
    /// the requested kind. A permission without options is cancelled so the
    /// agent does not stall waiting for an answer.
    fn respond_to_permission(
        &mut self,
        preferred_kind: &str,
        fallback_kind: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(PendingPermission { request_id, request }) =
            self.thread.pending_permissions().last()
        else {
            return;
        };
        let request_id = request_id.clone();
        let option = request
            .options
            .iter()
            .find(|option| option.kind.as_deref() == Some(preferred_kind))
            .or_else(|| {
                request
                    .options
                    .iter()
                    .find(|option| option.kind.as_deref() == Some(fallback_kind))
            })
            .or_else(|| request.options.first())
            .cloned();
        let outcome = match option {
            Some(option) => PermissionOutcome::Selected {
                option_id: option.id,
            },
            None => PermissionOutcome::Cancelled,
        };
        self.answer_permission(request_id, outcome, cx);
    }

    fn answer_permission(
        &mut self,
        request_id: MessageId,
        outcome: PermissionOutcome,
        cx: &mut Context<Self>,
    ) {
        if let Some(agent) = &self.agent {
            agent.acknowledge_permission(request_id.clone(), outcome);
        }
        self.thread.take_permission(&request_id);
        cx.notify();
    }

    fn stop(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.teardown_agent(cx);
        self.status = ConnectionState::Idle;
        cx.notify();
    }
}

/// Registers panel actions on every new workspace.
pub fn init(cx: &mut App) {
    cx.observe_new(
        |workspace: &mut Workspace, _window, _: &mut Context<Workspace>| {
            workspace.register_action(AgentPanel::toggle);
            workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
                workspace.toggle_panel_focus::<AgentPanel>(window, cx);
            });
        },
    )
    .detach();
}

pub async fn load(
    workspace: WeakEntity<Workspace>,
    mut cx: AsyncWindowContext,
) -> anyhow::Result<Entity<AgentPanel>> {
    let panel = workspace.update_in(&mut cx, |workspace, window, cx| {
        AgentPanel::new(workspace, window, cx)
    })?;
    Ok(panel)
}

fn settle_turn(
    this: WeakEntity<AgentPanel>,
    result: anyhow::Result<StopReason>,
    label: &SharedString,
    cx: &mut AsyncApp,
) {
    this.update(cx, |this, cx| {
        match result {
            Ok(_) => {
                this.status = ConnectionState::Running {
                    label: label.clone(),
                };
            }
            Err(error) => {
                // A failed turn usually leaves the session alive; the failure
                // is only reported instead of tearing down the connection.
                this.status = ConnectionState::Running {
                    label: label.clone(),
                };
                this.push_system_message(format!("Request failed: {error:#}"), cx);
            }
        }
        cx.notify();
    })
    .ok();
}

impl Focusable for AgentPanel {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<PanelEvent> for AgentPanel {}

impl Panel for AgentPanel {
    fn persistent_name() -> &'static str {
        "AgentPanel"
    }

    fn panel_key() -> &'static str {
        AGENT_PANEL_KEY
    }

    fn activation_focus_handle(&self, cx: &App) -> gpui::FocusHandle {
        if self.session.is_some() {
            self.message_editor.read(cx).focus_handle(cx)
        } else {
            self.focus_handle.clone()
        }
    }

    fn position(&self, _: &Window, cx: &App) -> DockPosition {
        AgentPanelSettings::get_global(cx).dock
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }

    fn set_position(&mut self, position: DockPosition, _: &mut Window, cx: &mut Context<Self>) {
        update_settings_file(self.fs.clone(), cx, move |settings, _| {
            settings.agent_panel.get_or_insert_default().dock = Some(position.into())
        });
    }

    fn default_size(&self, _: &Window, cx: &App) -> gpui::Pixels {
        AgentPanelSettings::get_global(cx).default_width
    }

    fn icon(&self, _: &Window, cx: &App) -> Option<IconName> {
        AgentPanelSettings::get_global(cx)
            .button
            .then_some(IconName::Sparkle)
    }

    fn icon_tooltip(&self, _: &Window, _: &App) -> Option<&'static str> {
        Some("Agent Panel")
    }

    fn toggle_action(&self) -> Box<dyn gpui::Action> {
        Box::new(ToggleFocus)
    }

    fn starts_open(&self, _: &Window, cx: &App) -> bool {
        AgentPanelSettings::get_global(cx).starts_open
    }

    fn activation_priority(&self) -> u32 {
        4
    }

    fn is_agent_panel(&self) -> bool {
        true
    }

    fn hide_button_setting(&self, _: &App) -> Option<HideStatusItem> {
        Some(HideStatusItem::new(|settings| {
            settings.agent_panel.get_or_insert_default().button = Some(false);
        }))
    }
}

impl Render for AgentPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .key_context("AgentPanel")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::send_prompt))
            .on_action(cx.listener(Self::cancel_run))
            .on_action(cx.listener(Self::allow_once))
            .on_action(cx.listener(Self::allow_always))
            .on_action(cx.listener(Self::reject_once))
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .child(self.render_status_bar(cx))
            .child(
                div()
                    .id("agent_thread")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(
                        v_flex()
                            .px_2()
                            .py_2()
                            .gap_2()
                            .children(self.render_timeline(window, cx)),
                    ),
            )
            .child(self.render_permissions(cx))
            .child(self.render_input(cx))
    }
}

impl AgentPanel {
    fn render_status_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.status.clone();
        let colors = cx.theme().colors();
        let mut bar = v_flex()
            .px_2()
            .py_1p5()
            .gap_1p5()
            .border_b_1()
            .border_color(colors.border);

        match &state {
            ConnectionState::Starting { label } => {
                bar = bar.child(
                    h_flex()
                        .gap_1p5()
                        .child(SpinnerLabel::new())
                        .child(
                            Label::new(format!("Starting {label}…"))
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                );
            }
            ConnectionState::Running { label } => {
                bar = bar.child(self.render_connected_row(label.clone(), false, cx));
            }
            ConnectionState::Working { label } => {
                bar = bar.child(self.render_connected_row(label.clone(), true, cx));
            }
            ConnectionState::Idle | ConnectionState::Failed { .. } => {
                let mut prompt = v_flex().gap_1();
                if let ConnectionState::Failed { error } = &state {
                    prompt = prompt.child(
                        Label::new(error.clone())
                            .size(LabelSize::Small)
                            .color(Color::Error),
                    );
                }
                prompt = prompt.child(h_flex().gap_1().flex_wrap().children(
                    BUILTIN_PROFILES.iter().map(|profile| {
                        Button::new(SharedString::from(profile.id), profile.label)
                            .label_size(LabelSize::Small)
                            .on_click(cx.listener({
                                move |this: &mut Self, _, _, cx| this.start_agent(profile, cx)
                            }))
                    }),
                ));
                bar = bar.child(prompt);
            }
        }
        bar
    }

    fn render_connected_row(
        &self,
        label: SharedString,
        working: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disconnect = IconButton::new("agent-disconnect", IconName::Stop)
            .shape(IconButtonShape::Square)
            .tooltip(Tooltip::text("Disconnect agent"))
            .on_click(cx.listener(|this, _, window, cx| this.stop(window, cx)));

        h_flex()
            .w_full()
            .justify_between()
            .child(
                h_flex()
                    .gap_1p5()
                    .when(working, |this| this.child(SpinnerLabel::new()))
                    .child(
                        Label::new(label)
                            .size(LabelSize::Small)
                            .color(if working { Color::Accent } else { Color::Muted }),
                    )
                    .when_some(
                        self.thread.current_mode_id().map(SharedString::from),
                        |this, mode| {
                            this.child(
                                Label::new(format!("· {mode}"))
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            )
                        },
                    ),
            )
            .child(disconnect)
            .into_any_element()
    }

    fn render_timeline(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        self.thread
            .timeline()
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                let elements: Vec<AnyElement> = match item {
                    TimelineItem::UserMessage { content } => {
                        vec![self.render_user_message(content, cx)]
                    }
                    TimelineItem::AgentMessage { .. } | TimelineItem::AgentThought { .. } => {
                        let Some(markdown) = self.markdown_by_item.get(&index) else {
                            return Vec::new();
                        };
                        let style = match item {
                            TimelineItem::AgentThought { .. } => {
                                thought_markdown_style(window, cx)
                            }
                            _ => message_markdown_style(window, cx),
                        };
                        let message = div()
                            .w_full()
                            .max_w_full()
                            .child(MarkdownElement::new(markdown.clone(), style));
                        match item {
                            TimelineItem::AgentThought { .. } => vec![
                                Label::new("Thought")
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted)
                                    .into_any_element(),
                                message.into_any_element(),
                            ],
                            _ => vec![message.into_any_element()],
                        }
                    }
                    TimelineItem::ToolCall { tool_call_id } => {
                        match self.thread.tool_call_state(tool_call_id) {
                            Some(state) => vec![self.render_tool_call(state)],
                            None => Vec::new(),
                        }
                    }
                    TimelineItem::Plan { entries } => vec![self.render_plan(entries, cx)],
                    TimelineItem::System { content } => {
                        vec![
                            Label::new(content.clone())
                                .size(LabelSize::XSmall)
                                .color(Color::Muted)
                                .into_any_element(),
                        ]
                    }
                };
                elements
            })
            .collect()
    }

    fn render_user_message(&self, content: &str, cx: &App) -> AnyElement {
        let colors = cx.theme().colors();
        v_flex()
            .gap_0p5()
            .child(
                Label::new("You")
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
            .child(
                div()
                    .w_full()
                    .rounded_md()
                    .bg(colors.surface_background)
                    .px_2()
                    .py_1p5()
                    .text_color(colors.editor_foreground)
                    .child(content.to_string()),
            )
            .into_any_element()
    }

    fn render_tool_call(&self, state: &crate::thread::ToolCallState) -> AnyElement {
        let (icon, status_color, status_text) = match state.status {
            agent_acp::ToolCallStatus::Pending => (
                Some(IconName::Clock),
                Color::Muted,
                SharedString::from("pending"),
            ),
            agent_acp::ToolCallStatus::InProgress => (
                None,
                Color::Accent,
                SharedString::from("running"),
            ),
            agent_acp::ToolCallStatus::Completed => (
                Some(IconName::Check),
                Color::Success,
                SharedString::from("done"),
            ),
            agent_acp::ToolCallStatus::Failed => (
                Some(IconName::Close),
                Color::Error,
                SharedString::from("failed"),
            ),
        };

        h_flex()
            .py_0p5()
            .gap_1p5()
            .overflow_hidden()
            .children(icon.map(|icon| {
                Icon::new(icon).size(IconSize::Small).color(status_color)
            }))
            .when(state.status == agent_acp::ToolCallStatus::InProgress, |this| {
                this.child(SpinnerLabel::new())
            })
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        Label::new(state.title.clone())
                            .size(LabelSize::Small)
                            .color(Color::Default),
                    ),
            )
            .child(
                Label::new(status_text)
                    .size(LabelSize::XSmall)
                    .color(status_color),
            )
            .into_any_element()
    }

    fn render_plan(&self, entries: &[agent_acp::PlanEntry], cx: &App) -> AnyElement {
        let colors = cx.theme().colors();
        v_flex()
            .w_full()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .px_2()
            .py_1p5()
            .gap_1()
            .child(
                Label::new("Plan")
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
            .children(entries.iter().map(|entry| {
                h_flex()
                    .gap_1p5()
                    .children(match entry.status {
                        agent_acp::PlanEntryStatus::Completed => Some(
                            Icon::new(IconName::Check)
                                .size(IconSize::Small)
                                .color(Color::Success),
                        ),
                        _ => Some(
                            Icon::new(IconName::Clock)
                                .size(IconSize::Small)
                                .color(Color::Muted),
                        ),
                    })
                    .child(
                        Label::new(entry.content.clone())
                            .size(LabelSize::Small)
                            .color(Color::Default),
                    )
            }))
            .into_any_element()
    }

    fn render_permissions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let pending: Vec<PendingPermission> = self.thread.pending_permissions().to_vec();

        v_flex()
            .flex_shrink_0()
            .pb_1()
            .gap_1()
            .children(pending.into_iter().rev().take(1).map(|pending| {
                let request_id = pending.request_id.clone();
                let title = pending
                    .request
                    .tool_call_id
                    .clone()
                    .map(|tool_call_id| format!("Permission for tool call {tool_call_id}"))
                    .unwrap_or_else(|| "Agent requests permission".to_string());

                v_flex()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.surface_background)
                    .px_2()
                    .py_1p5()
                    .gap_1()
                    .child(
                        Label::new(title)
                            .size(LabelSize::Small)
                            .color(Color::Default),
                    )
                    .child(h_flex().gap_1().flex_wrap().children(
                        pending.request.options.iter().map(|option| {
                            let request_id = request_id.clone();
                            let option_id = option.id.clone();
                            Button::new(
                                SharedString::from(format!("agent-option-{option_id}")),
                                SharedString::from(option.name.clone()),
                            )
                            .label_size(LabelSize::Small)
                            .color(Color::Accent)
                            .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                                this.answer_permission(
                                    request_id.clone(),
                                    PermissionOutcome::Selected {
                                        option_id: option_id.clone(),
                                    },
                                    cx,
                                )
                            }))
                        }),
                    ))
            }))
    }

    fn render_input(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let working = matches!(self.status, ConnectionState::Working { .. });
        let action_button = if working {
            IconButton::new("agent-cancel-run", IconName::Stop)
                .tooltip(Tooltip::text("Cancel turn"))
                .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                    this.cancel_run(&CancelRun, window, cx)
                }))
        } else {
            IconButton::new("agent-send", IconName::ArrowUp)
                .tooltip(Tooltip::text("Send"))
                .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                    this.send_prompt(&SendPrompt, window, cx)
                }))
        };

        h_flex()
            .flex_shrink_0()
            .items_end()
            .px_2()
            .py_2()
            .gap_1p5()
            .border_t_1()
            .border_color(colors.border)
            .child(
                div()
                    .id("agent-input")
                    .key_context("AgentInputEditor")
                    .flex_1()
                    .min_w_0()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.editor_background)
                    .w_full()
                    .px_1p5()
                    .py_0p5()
                    .max_h_24()
                    .overflow_y_scroll()
                    .child(self.message_editor.clone()),
            )
            .child(action_button)
    }
}

fn message_markdown_style(window: &Window, cx: &App) -> MarkdownStyle {
    let colors = cx.theme().colors();
    let mut base_text_style = window.text_style();
    base_text_style.refine(&TextStyleRefinement {
        color: Some(colors.editor_foreground),
        ..Default::default()
    });
    let mut style = MarkdownStyle::default();
    style.base_text_style = base_text_style;
    style
}

/// Thought chunks are rendered quieter than real output so progress stands
/// out from the answer.
fn thought_markdown_style(window: &Window, cx: &App) -> MarkdownStyle {
    let colors = cx.theme().colors();
    let mut style = message_markdown_style(window, cx);
    let mut muted_base = style.base_text_style;
    muted_base.refine(&TextStyleRefinement {
        color: Some(colors.text_muted),
        ..Default::default()
    });
    style.base_text_style = muted_base;
    style
}
