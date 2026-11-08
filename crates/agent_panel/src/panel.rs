use std::{path::PathBuf, sync::Arc};

use agent_acp::{
    AcpAgent, AcpEvent, AcpSession, AgentSpawnConfig, ClientCapabilities, ContentBlock, MessageId,
    PermissionOutcome, PlanEntry, SessionUpdate, StopReason,
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
use std::sync::atomic::{AtomicBool, Ordering};
use theme::ActiveTheme as _;
use ui::{IconButton, IconButtonShape, SpinnerLabel, Tooltip, prelude::*};
use workspace::{
    HideStatusItem, StatusItemView, Workspace,
    dock::{DockPosition, Panel, PanelEvent},
};

use crate::{
    agents::{ActiveAgent, AgentProfile, BUILTIN_PROFILES},
    api::{ApiConfig, fetch_models, stream_chat},
    session_store::{
        StoredSessionMeta, delete_session, load_body, load_index, new_session_id, now_unix_secs,
        save_session, sessions_dir,
    },
    settings::AgentPanelSettings,
    thread::{PendingPermission, PersistedItem, Thread, TimelineItem},
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
        /// Clears the conversation and disconnects the agent.
        ClearConversation,
        /// Toggles plan mode: analyze first, execute only after approval.
        TogglePlanMode,
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
    active_agent: ActiveAgent,
    show_agent_selector: bool,
    api_cancel: Arc<AtomicBool>,
    last_agent: Option<ActiveAgent>,
    sessions: Vec<StoredSessionMeta>,
    session_id: Option<String>,
    session_autosave: Option<Task<()>>,
    show_sessions: bool,
    api_models: Vec<String>,
    show_model_selector: bool,
    model_selection_task: Option<Task<()>>,
    plan_mode: bool,
    pending_plan: Option<String>,
    awaiting_approved_revision: bool,
    plan_entries: Vec<PlanEntry>,
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

        let panel = cx.new(|cx| Self {
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
            active_agent: ActiveAgent::Builtin(&BUILTIN_PROFILES[0]),
            show_agent_selector: false,
            api_cancel: Arc::new(AtomicBool::new(false)),
            last_agent: None,
            sessions: Vec::new(),
            session_id: None,
            session_autosave: None,
            show_sessions: false,
            api_models: Vec::new(),
            show_model_selector: false,
            model_selection_task: None,
            plan_mode: false,
            pending_plan: None,
            awaiting_approved_revision: false,
            plan_entries: Vec::new(),
        });

        panel
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
        self.last_agent = Some(ActiveAgent::Builtin(profile));
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
                    let message = if error.to_string().contains("No such file") {
                        format!(
                            "Could not start {label}: {error}\nMake sure `{}` is installed and on your PATH.",
                            profile.command
                        )
                    } else {
                        format!("Could not start {label}: {error}")
                    };
                    Self::mark_failed(this, message, cx);
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
                            self.scroll_handle.scroll_to_bottom();
                        }
                    }
                    SessionUpdate::AgentThoughtChunk { content } => {
                        if let Some(text) = content.text_content() {
                            let item_index =
                                self.thread.stream_agent_thought_chunk(text.to_string());
                            self.stream_markdown(item_index, text, cx);
                            self.scroll_handle.scroll_to_bottom();
                        }
                    }
                    SessionUpdate::Plan { entries } => {
                        self.plan_entries.extend(entries);
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
            ConnectionState::Idle | ConnectionState::Failed { .. } => self.active_agent.label(),
        }
    }

    /// Runs a turn against the user's own OpenAI-compatible endpoint. Deltas
    /// stream into the same timeline as ACP agent output.
    fn run_api_turn(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        let http = workspace.read(cx).app_state().client.http_client();
        let settings = AgentPanelSettings::get_global(cx);
        let config = ApiConfig {
            base_url: settings.api_url.clone(),
            api_key: settings.api_key.clone(),
            model: settings.model.clone(),
        };
        if config.base_url.is_empty() || config.api_key.is_empty() {
            self.push_system_message(
                "Custom API needs a base URL and API key. Set agent_panel.api_url and agent_panel.api_key in your settings.",
                cx,
            );
            return;
        }
        let label = self.active_label();
        self.status = ConnectionState::Working { label: label.clone() };
        self.last_agent = Some(ActiveAgent::Api);
        self.api_cancel.store(false, Ordering::Relaxed);
        let cancel = self.api_cancel.clone();
        self.turn_task = Some(cx.spawn(async move |this, cx| {
            let messages = vec![("user".to_string(), text)];
            let result = stream_chat(http, config, &messages, &cancel, |delta| {
                this.update(cx, |this, cx| {
                    if let Some(index) = this.thread.stream_agent_message_chunk(delta.to_string()) {
                        this.stream_markdown(Some(index), delta, cx);
                    }
                    this.scroll_handle.scroll_to_bottom();
                    cx.notify();
                })
                .ok();
            })
            .await;
            this.update(cx, |this, cx| {
                this.status = ConnectionState::Running { label: label.clone() };
                if let Err(error) = result {
                    this.push_system_message(format!("API request failed: {error:#}"), cx);
                }
                this.save_current_conversation(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    // Actions -------------------------------------------------------------

    fn send_prompt(&mut self, _: &SendPrompt, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self.message_buffer.read(cx).text().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.message_buffer
            .update(cx, |message_buffer, cx| message_buffer.set_text("", cx));

        let display = text.clone();
        let wire = self.build_wire_prompt(text);
        self.submit_text(display, wire, cx);
    }

    /// Builds the prompt actually sent to the agent. In plan mode the request
    /// is wrapped in an analyze-only directive; an edited plan is re-submitted
    /// as an approved revision to execute.
    fn build_wire_prompt(&mut self, text: String) -> String {
        if !self.plan_mode {
            return text;
        }
        if self.awaiting_approved_revision {
            self.awaiting_approved_revision = false;
            format!("Execute this approved plan exactly:\n{text}")
        } else {
            format!(
                "{text}\n\n[PLAN MODE] Analyze the codebase, identify the relevant files, and \
                 produce a detailed step-by-step plan (use the plan update when you support it). \
                 Do NOT modify files or run commands that change the system until the user \
                 approves the plan."
            )
        }
    }

    fn submit_text(&mut self, display: String, wire: String, cx: &mut Context<Self>) {
        self.thread.push_user_message(display);
        self.save_current_conversation(cx);
        cx.notify();

        match self.active_agent {
            ActiveAgent::Builtin(profile) => {
                match &self.status {
                    ConnectionState::Running { .. } | ConnectionState::Working { .. } => {
                        self.turn(vec![ContentBlock::text(wire)], cx);
                    }
                    ConnectionState::Starting { .. } => {
                        self.pending_first_message = Some(wire);
                    }
                    ConnectionState::Idle | ConnectionState::Failed { .. } => {
                        self.pending_first_message = Some(wire);
                        self.start_agent(profile, cx);
                    }
                }
            }
            ActiveAgent::Api => {
                self.run_api_turn(wire, cx);
            }
        }
    }

    fn toggle_plan_mode(&mut self, _: &TogglePlanMode, _window: &mut Window, cx: &mut Context<Self>) {
        self.plan_mode = !self.plan_mode;
        if !self.plan_mode {
            self.awaiting_approved_revision = false;
            self.pending_plan = None;
            self.plan_entries.clear();
        }
        self.show_agent_selector = false;
        self.show_model_selector = false;
        self.show_sessions = false;
        cx.notify();
    }

    fn approve_plan(&mut self, cx: &mut Context<Self>) {
        if let Some(plan) = self.pending_plan.take() {
            self.plan_mode = false;
            self.awaiting_approved_revision = false;
            let wire = format!("The user approved this plan. Execute it now, step by step:\n{plan}");
            self.submit_text("✓ Plan approved — executing.".into(), wire, cx);
        }
    }

    fn edit_plan(&mut self, cx: &mut Context<Self>) {
        if let Some(plan) = self.pending_plan.take() {
            self.message_buffer.update(cx, |buffer, cx| buffer.set_text(plan, cx));
            self.awaiting_approved_revision = true;
            self.pending_plan = None;
            cx.notify();
        }
    }

    fn reject_plan(&mut self, cx: &mut Context<Self>) {
        if self.pending_plan.take().is_some() {
            let wire = "The user rejected the proposed plan. Ask clarifying questions or propose a \
                        revised plan."
                .to_string();
            self.submit_text("✗ Plan rejected — asking for revision.".into(), wire, cx);
        }
    }

    /// If the just-finished turn ran in plan mode, freeze the captured plan
    /// into a reviewable summary and write the analysis file.
    fn finalize_plan_if_needed(&mut self, cx: &mut Context<Self>) {
        if !self.plan_mode || self.pending_plan.is_some() {
            return;
        }
        let mut content = String::new();
        if !self.plan_entries.is_empty() {
            for (index, entry) in self.plan_entries.iter().enumerate() {
                if index > 0 {
                    content.push('\n');
                }
                content.push_str(&entry.content);
            }
        } else if let Some(fallback) = self.thread.timeline().iter().rev().find_map(|item| {
            match item {
                TimelineItem::AgentMessage { content } | TimelineItem::AgentThought { content } => {
                    Some(content.clone())
                }
                _ => None,
            }
        }) {
            content = fallback;
        }
        if content.trim().is_empty() {
            return;
        }
        self.pending_plan = Some(content.clone());
        self.plan_entries.clear();
        self.write_analysis_file(&content, cx);
        cx.notify();
    }

    fn write_analysis_file(&self, content: &str, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        let Some(root) = workspace.read(cx).worktrees(cx).next().map(|worktree| {
            worktree.read(cx).abs_path().to_path_buf()
        }) else {
            return;
        };
        let fs = self.fs.clone();
        let dir = root.join(".aaykra");
        let path = dir.join("plan.md");
        let body = format!("# Plan\n\n{content}");
        cx.background_spawn(async move {
            if fs.create_dir(&dir).await.is_ok() {
                let _ = fs.write(&path, body.as_bytes()).await;
            }
        })
        .detach();
    }

    fn cancel_run(&mut self, _: &CancelRun, _: &mut Window, _cx: &mut Context<Self>) {
        if let Some(session) = &self.session {
            session.cancel();
        }
        self.api_cancel.store(true, Ordering::Relaxed);
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

    fn clear_conversation(&mut self, _: &ClearConversation, _: &mut Window, cx: &mut Context<Self>) {
        self.teardown_agent(cx);
        self.thread = Thread::default();
        self.markdown_by_item.clear();
        self.pending_first_message = None;
        self.status = ConnectionState::Idle;
        self.api_cancel.store(true, Ordering::Relaxed);
        self.session_id = None;
        self.session_autosave = None;
        self.show_sessions = false;
        cx.notify();
    }

    fn retry_last_agent(&mut self, cx: &mut Context<Self>) {
        match self.last_agent {
            Some(ActiveAgent::Builtin(profile)) => self.start_agent(profile, cx),
            Some(ActiveAgent::Api) | None => {
                self.status = ConnectionState::Idle;
                cx.notify();
            }
        }
    }

    // Session persistence ----------------------------------------------

    fn load_sessions(&mut self, cx: &mut Context<Self>) {
        let fs = self.fs.clone();
        let dir = sessions_dir();
        cx.spawn(async move |this, cx| {
            let sessions = load_index(fs, &dir).await.unwrap_or_default();
            this.update(cx, |this, cx| {
                this.sessions = sessions;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn restore_latest(&mut self, cx: &mut Context<Self>) {
        let latest = self.sessions.first().map(|meta| meta.id.clone());
        if let Some(id) = latest {
            self.restore_session(id, cx);
        }
    }

    fn restore_session(&mut self, id: String, cx: &mut Context<Self>) {
        let fs = self.fs.clone();
        let dir = sessions_dir();
        self.teardown_agent(cx);
        self.session_autosave = None;
        let task = cx.spawn(async move |this, cx| {
            let body = match load_body(fs.clone(), &dir, &id).await {
                Ok(body) => body,
                Err(error) => {
                    log::error!("failed to load agent session {id}: {error}");
                    return;
                }
            };
            let items: Vec<PersistedItem> = match serde_json::from_str(&body) {
                Ok(items) => items,
                Err(_) => return,
            };
            let meta = load_index(fs, &dir)
                .await
                .ok()
                .and_then(|metas| metas.into_iter().find(|entry| entry.id == id));
            this.update(cx, |this, cx| {
                this.thread = Thread::default();
                this.thread.import(items);
                this.session_id = Some(id);
                this.show_sessions = false;
                if let Some(meta) = meta {
                    this.active_agent = agent_from_id(&meta.agent_id);
                }
                this.rebuild_markdown(cx);
                this.status = ConnectionState::Idle;
                this.pending_first_message = None;
                cx.notify();
            })
            .ok();
        });
        self.connection_task = Some(task);
    }

    fn delete_session(&mut self, id: String, cx: &mut Context<Self>) {
        let fs = self.fs.clone();
        let dir = sessions_dir();
        cx.spawn(async move |this, cx| {
            if let Err(error) = delete_session(fs, &dir, &id).await {
                log::error!("failed to delete agent session {id}: {error}");
                return;
            }
            this.update(cx, |this, cx| {
                this.sessions.retain(|entry| entry.id != id);
                if this.session_id.as_deref() == Some(id.as_str()) {
                    this.session_id = None;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Persists the current conversation. Called after the conversation
    /// changes in a meaningful way (a turn was sent or finished).
    fn save_current_conversation(&mut self, cx: &mut Context<Self>) {
        if self.thread.timeline().is_empty() {
            return;
        }
        let id = match &self.session_id {
            Some(id) => id.clone(),
            None => {
                let id = new_session_id();
                self.session_id = Some(id.clone());
                id
            }
        };
        let fs = self.fs.clone();
        let dir = sessions_dir();
        let meta = StoredSessionMeta {
            id: id.clone(),
            title: self.thread.title(),
            agent_id: self.active_agent.id().to_string(),
            updated_at: now_unix_secs(),
        };
        let items = self.thread.export();
        let json = match serde_json::to_string(&items) {
            Ok(json) => json,
            Err(_) => return,
        };
        let task = cx.spawn(async move |this, cx| {
            if let Err(error) = save_session(fs, &dir, &meta, &json).await {
                log::error!("failed to save agent session: {error}");
                return;
            }
            this.update(cx, |this, cx| {
                if let Some(existing) = this.sessions.iter_mut().find(|entry| entry.id == meta.id) {
                    *existing = meta.clone();
                } else {
                    this.sessions.push(meta.clone());
                }
                this.sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                cx.notify();
            })
            .ok();
        });
        self.session_autosave = Some(task);
    }

    fn rebuild_markdown(&mut self, cx: &mut Context<Self>) {
        self.markdown_by_item.clear();
        for (index, item) in self.thread.timeline().iter().enumerate() {
            if let TimelineItem::AgentMessage { content } | TimelineItem::AgentThought { content } =
                item
            {
                let markdown = cx.new(|cx| Markdown::new_text(content.clone().into(), cx));
                self.markdown_by_item.insert(index, markdown);
            }
        }
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
    panel.update_in(&mut cx, |this, _window, cx| {
        this.load_sessions(cx);
        this.restore_latest(cx);
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
        this.finalize_plan_if_needed(cx);
        this.save_current_conversation(cx);
        cx.notify();
    })
    .ok();
}

fn agent_from_id(id: &str) -> ActiveAgent {
    if id == "api" {
        return ActiveAgent::Api;
    }
    BUILTIN_PROFILES
        .iter()
        .find(|profile| profile.id == id)
        .map(ActiveAgent::Builtin)
        .unwrap_or(ActiveAgent::Builtin(&BUILTIN_PROFILES[0]))
}

fn format_relative_time(secs: u64) -> String {
    let ago = now_unix_secs().saturating_sub(secs);
    if ago < 60 {
        "just now".to_string()
    } else if ago < 3600 {
        format!("{}m ago", ago / 60)
    } else if ago < 86400 {
        format!("{}h ago", ago / 3600)
    } else {
        format!("{}d ago", ago / 86400)
    }
}

impl Focusable for AgentPanel {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

/// Status-bar button that toggles the agent panel, placed alongside the
/// language / terminal indicators in the workspace footer.
pub struct AgentPanelStatusItem {
    workspace: WeakEntity<Workspace>,
}

impl AgentPanelStatusItem {
    pub fn new(workspace: WeakEntity<Workspace>) -> Self {
        Self { workspace }
    }
}

impl Render for AgentPanelStatusItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        IconButton::new("agent-panel-status", IconName::Sparkle)
            .tooltip(Tooltip::text("Agent Panel (Ctrl+Shift+A)"))
            .on_click(cx.listener(|_, _: &gpui::ClickEvent, window, cx| {
                window.dispatch_action(Box::new(ToggleFocus), cx);
            }))
    }
}

impl StatusItemView for AgentPanelStatusItem {
    fn set_active_pane_item(
        &mut self,
        _active_pane_item: Option<&dyn workspace::ItemHandle>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn hide_setting(&self, _cx: &App) -> Option<HideStatusItem> {
        Some(HideStatusItem::new(|settings| {
            settings.agent_panel.get_or_insert_default().button = Some(false);
        }))
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
        matches!(
            position,
            DockPosition::Left | DockPosition::Right | DockPosition::Bottom
        )
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
            .on_action(cx.listener(Self::clear_conversation))
            .on_action(cx.listener(Self::toggle_plan_mode))
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
                            .children(if self.thread.timeline().is_empty() {
                                vec![self.render_empty_state(cx)]
                            } else {
                                self.render_timeline(window, cx)
                            }),
                    ),
            )
            .child(self.render_permissions(cx))
            .child(self.render_plan_banner(cx))
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
            ConnectionState::Failed { error } => {
                bar = bar.child(
                    v_flex()
                        .gap_1()
                        .child(
                            Label::new(error.clone())
                                .size(LabelSize::Small)
                                .color(Color::Error),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new("agent-retry", "Retry")
                                        .label_size(LabelSize::Small)
                                        .color(Color::Accent)
                                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                                            this.retry_last_agent(cx)
                                        })),
                                )
                                .child(
                                    Button::new("agent-switch", "Switch agent")
                                        .label_size(LabelSize::Small)
                                        .on_click(cx.listener(
                                            |this, _: &gpui::ClickEvent, _, cx| {
                                                this.show_agent_selector = true;
                                                cx.notify();
                                            },
                                        )),
                                ),
                        ),
                );
            }
            ConnectionState::Idle => {}
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
                        vec![self.render_user_message(index, content, cx)]
                    }
                    TimelineItem::AgentMessage { .. } | TimelineItem::AgentThought { .. } => {
                        let Some(markdown) = self.markdown_by_item.get(&index) else {
                            return Vec::new();
                        };
                        let content = match item {
                            TimelineItem::AgentMessage { content }
                            | TimelineItem::AgentThought { content } => content.clone(),
                            _ => String::new(),
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
                        let message_container = v_flex()
                            .w_full()
                            .gap_0p5()
                            .child(
                                h_flex()
                                    .w_full()
                                    .justify_end()
                                    .child(self.render_copy_button(index, content)),
                            )
                            .child(message);
                        match item {
                            TimelineItem::AgentThought { .. } => vec![
                                Label::new("Thought")
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted)
                                    .into_any_element(),
                                message_container.into_any_element(),
                            ],
                            _ => vec![message_container.into_any_element()],
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

    fn render_user_message(&self, index: usize, content: &str, cx: &App) -> AnyElement {
        let colors = cx.theme().colors();
        v_flex()
            .gap_0p5()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        Label::new("You")
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    )
                    .child(self.render_copy_button(index, content.to_string())),
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

    fn render_copy_button(&self, index: usize, content: String) -> impl IntoElement {
        IconButton::new(format!("agent-copy-{index}"), IconName::Copy)
            .shape(IconButtonShape::Square)
            .tooltip(Tooltip::text("Copy"))
            .on_click(move |_, _window, cx: &mut App| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(content.clone()));
            })
    }

    fn render_empty_state(&self, cx: &App) -> AnyElement {
        let colors = cx.theme().colors();
        v_flex()
            .w_full()
            .h_full()
            .items_center()
            .justify_center()
            .gap_2()
            .child(
                Icon::new(IconName::Sparkle)
                    .size(IconSize::XLarge)
                    .color(Color::Muted),
            )
            .child(
                Label::new("Ask anything")
                    .size(LabelSize::Large)
                    .color(Color::Muted),
            )
            .child(
                Label::new("Pick an agent next to the input, then type your question.")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Label::new("Ctrl+Shift+A toggles this panel")
                    .size(LabelSize::XSmall)
                    .color(Color::Custom(colors.text_muted)),
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

    fn render_plan_banner(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let Some(plan) = &self.pending_plan else {
            return v_flex();
        };

        v_flex()
            .flex_shrink_0()
            .px_2()
            .gap_1()
            .child(
                v_flex()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.surface_background)
                    .px_2()
                    .py_1p5()
                    .gap_1()
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Icon::new(IconName::ListTree)
                                    .size(IconSize::Small)
                                    .color(Color::Accent),
                            )
                            .child(
                                Label::new("Plan ready — review before executing")
                                    .size(LabelSize::Small)
                                    .color(Color::Default),
                            ),
                    )
                    .child(
                        div()
                            .id("agent-plan-body")
                            .max_h_24()
                            .overflow_y_scroll()
                            .child(
                                Label::new(plan.clone())
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("plan-approve", "Approve")
                                    .label_size(LabelSize::Small)
                                    .color(Color::Accent)
                                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                                        this.approve_plan(cx)
                                    })),
                            )
                            .child(
                                Button::new("plan-edit", "Edit")
                                    .label_size(LabelSize::Small)
                                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                                        this.edit_plan(cx)
                                    })),
                            )
                            .child(
                                Button::new("plan-reject", "Reject")
                                    .label_size(LabelSize::Small)
                                    .color(Color::Error)
                                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                                        this.reject_plan(cx)
                                    })),
                            ),
                    ),
            )
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

    fn render_input(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
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

        let clear_button = IconButton::new("agent-clear", IconName::Trash)
            .tooltip(Tooltip::text("Clear conversation"))
            .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                this.clear_conversation(&ClearConversation, window, cx)
            }));

        let plan_icon = if self.plan_mode {
            IconName::BoltFilled
        } else {
            IconName::ListTree
        };
        let plan_mode_button = IconButton::new("agent-plan-mode", plan_icon)
            .tooltip(Tooltip::text(if self.plan_mode {
                "Planning mode ON (Ctrl+Alt+P) — approve plans before they run"
            } else {
                "Planning mode OFF (Ctrl+Alt+P)"
            }))
            .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                this.toggle_plan_mode(&TogglePlanMode, window, cx)
            }));

        let sessions_button = IconButton::new("agent-sessions", IconName::Clock)
            .tooltip(Tooltip::text("Sessions"))
            .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                this.show_sessions = !this.show_sessions;
                this.show_agent_selector = false;
                this.show_model_selector = false;
                cx.notify();
            }));

        let is_api = matches!(self.active_agent, ActiveAgent::Api);
        let model_button = is_api.then(|| {
            let current = AgentPanelSettings::get_global(cx).model.clone();
            Button::new(
                "agent-model-selector",
                if current.is_empty() {
                    SharedString::from("Model")
                } else {
                    SharedString::from(current)
                },
            )
            .label_size(LabelSize::Small)
            .tooltip(Tooltip::text("Choose model"))
            .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                this.show_model_selector = !this.show_model_selector;
                if this.show_model_selector && this.api_models.is_empty() {
                    this.load_api_models(cx);
                }
                this.show_agent_selector = false;
                cx.notify();
            }))
        });

        let selector_trigger = Button::new("agent-selector", self.active_agent.label())
            .label_size(LabelSize::Small)
            .tooltip(Tooltip::text("Choose agent"))
            .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                this.show_agent_selector = !this.show_agent_selector;
                this.show_sessions = false;
                cx.notify();
            }));

        v_flex()
            .flex_shrink_0()
            .px_2()
            .pb_2()
            .gap_1p5()
            .when(self.show_agent_selector, |this| {
                this.child(self.render_agent_selector(cx))
            })
            .when(self.show_sessions, |this| {
                this.child(self.render_sessions(cx))
            })
            .when(self.show_model_selector, |this| {
                this.child(self.render_model_selector(cx))
            })
            .child(
                h_flex()
                    .items_end()
                    .gap_1p5()
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
                    .child(clear_button)
                    .child(plan_mode_button)
                    .child(sessions_button)
                    .when_some(model_button, |this, button| this.child(button))
                    .child(selector_trigger)
                    .child(action_button),
            )
    }

    fn render_agent_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let hover_color = colors.element_hover;
        let mut agents: Vec<ActiveAgent> = BUILTIN_PROFILES
            .iter()
            .map(|profile| ActiveAgent::Builtin(profile))
            .collect();
        agents.push(ActiveAgent::Api);

        v_flex()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface_background)
            .overflow_hidden()
            .children(agents.into_iter().map(|agent| {
                let active = self.active_agent == agent;
                h_flex()
                    .id(format!("agent-select-{}", agent.id()))
                    .w_full()
                    .px_2()
                    .py_1()
                    .gap_1p5()
                    .hover(move |this| this.bg(hover_color))
                    .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                        let switching = this.active_agent != agent;
                        this.active_agent = agent;
                        this.show_agent_selector = false;
                        this.show_model_selector = false;
                        if switching
                            && !matches!(
                                this.status,
                                ConnectionState::Idle | ConnectionState::Failed { .. }
                            )
                        {
                            this.teardown_agent(cx);
                            this.status = ConnectionState::Idle;
                        }
                        cx.notify();
                    }))
                    .child(
                        Icon::new(IconName::Check)
                            .size(IconSize::Small)
                            .color(if active { Color::Accent } else { Color::Muted }),
                    )
                    .child(
                        Label::new(agent.label())
                            .size(LabelSize::Small)
                            .color(if active { Color::Default } else { Color::Muted }),
                    )
            }))
    }

    fn render_sessions(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let sessions = self.sessions.clone();

        v_flex()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface_background)
            .overflow_hidden()
            .child(
                h_flex()
                    .px_2()
                    .py_1()
                    .child(
                        Label::new("Sessions")
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    ),
            )
            .children(if sessions.is_empty() {
                vec![
                    h_flex()
                        .px_2()
                        .py_1()
                        .child(
                            Label::new("No saved sessions yet.")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        )
                        .into_any_element(),
                ]
            } else {
                sessions
                    .into_iter()
                    .map(|meta| self.render_session_row(meta, cx))
                    .collect()
            })
    }

    fn load_api_models(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        let http = workspace.read(cx).app_state().client.http_client();
        let settings = AgentPanelSettings::get_global(cx);
        let config = ApiConfig {
            base_url: settings.api_url.clone(),
            api_key: settings.api_key.clone(),
            model: settings.model.clone(),
        };
        if config.base_url.is_empty() {
            return;
        }
        let task = cx.spawn(async move |this, cx| {
            match fetch_models(http, &config).await {
                Ok(models) => {
                    this.update(cx, |this, cx| {
                        this.api_models = models;
                        cx.notify();
                    })
                    .ok();
                }
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.push_system_message(format!("Could not load models: {error:#}"), cx);
                    })
                    .ok();
                }
            }
        });
        self.model_selection_task = Some(task);
    }

    fn render_model_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let hover_color = colors.element_hover;
        let current = AgentPanelSettings::get_global(cx).model;
        let fs = self.fs.clone();

        v_flex()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.surface_background)
            .overflow_hidden()
            .children(if self.api_models.is_empty() {
                vec![
                    h_flex()
                        .px_2()
                        .py_1()
                        .child(
                            Label::new("No models loaded. Check your API URL and key.")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        )
                        .into_any_element(),
                ]
            } else {
                self.api_models
                    .iter()
                    .map(|model| {
                        let active = *model == current;
                        let model = model.clone();
                        let fs = fs.clone();
                        h_flex()
                            .id(format!("agent-model-{model}"))
                            .w_full()
                            .px_2()
                            .py_1()
                            .gap_1p5()
                            .hover(move |this| this.bg(hover_color))
                            .on_click(cx.listener(
                                move |this: &mut Self, _, _, cx| {
                                    update_settings_file(fs, cx, move |settings, _| {
                                        settings.agent_panel.get_or_insert_default().model =
                                            Some(model.clone());
                                    });
                                    this.show_model_selector = false;
                                    cx.notify();
                                },
                            ))
                            .child(
                                Icon::new(IconName::Check)
                                    .size(IconSize::Small)
                                    .color(if active {
                                        Color::Accent
                                    } else {
                                        Color::Muted
                                    }),
                            )
                            .child(
                                Label::new(model)
                                    .size(LabelSize::Small)
                                    .color(if active {
                                        Color::Default
                                    } else {
                                        Color::Muted
                                    }),
                            )
                            .into_any_element()
                    })
                    .collect()
            })
    }

    fn render_session_row(
        &mut self,
        meta: StoredSessionMeta,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let hover_color = colors.element_hover;
        let id = meta.id.clone();
        let open_id = id.clone();
        let open_row = h_flex()
            .id(format!("agent-session-{open_id}"))
            .flex_1()
            .min_w_0()
            .px_2()
            .py_1()
            .gap_1p5()
            .hover(move |this| this.bg(hover_color))
            .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                this.restore_session(open_id.clone(), cx);
            }))
            .child(
                v_flex()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        Label::new(meta.title.clone())
                            .size(LabelSize::Small)
                            .color(Color::Default),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Label::new(meta.agent_id.clone())
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted),
                            )
                            .child(
                                Label::new(format_relative_time(meta.updated_at))
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted),
                            ),
                    ),
            );

        let delete_button = IconButton::new(format!("agent-session-delete-{id}"), IconName::Trash)
            .shape(IconButtonShape::Square)
            .tooltip(Tooltip::text("Delete session"))
            .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                this.delete_session(id.clone(), cx);
            }));

        h_flex()
            .w_full()
            .items_center()
            .child(open_row)
            .child(delete_button)
            .into_any_element()
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
