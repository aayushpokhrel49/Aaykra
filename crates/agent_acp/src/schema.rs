use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u64 = 1;

pub const METHOD_INITIALIZE: &str = "initialize";
pub const METHOD_SESSION_NEW: &str = "session/new";
pub const METHOD_SESSION_LOAD: &str = "session/load";
pub const METHOD_SESSION_PROMPT: &str = "session/prompt";
pub const METHOD_SESSION_CANCEL: &str = "session/cancel";
pub const METHOD_SESSION_SET_MODE: &str = "session/set_mode";
pub const METHOD_SESSION_UPDATE: &str = "session/update";
pub const METHOD_REQUEST_PERMISSION: &str = "session/request_permission";
pub const METHOD_FS_READ_TEXT_FILE: &str = "fs/read_text_file";
pub const METHOD_FS_WRITE_TEXT_FILE: &str = "fs/write_text_file";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopReason(&'static str);

impl StopReason {
    pub const END_TURN: Self = Self("end_turn");
    pub const MAX_TOKENS: Self = Self("max_tokens");
    pub const REFUSAL: Self = Self("refusal");
    pub const CANCELLED: Self = Self("cancelled");

    pub fn from_str(string: &str) -> Option<Self> {
        match string {
            "end_turn" => Some(Self::END_TURN),
            "max_tokens" | "max_turn_requests" => Some(Self::MAX_TOKENS),
            "refusal" => Some(Self::REFUSAL),
            "cancelled" | "canceled" => Some(Self::CANCELLED),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

#[derive(Debug, PartialEq)]
pub enum PromptContent {
    Text(String),
    Image { data: String, mime_type: String },
}

impl PromptContent {
    pub fn into_blocks(content: Vec<PromptContent>) -> Vec<ContentBlock> {
        content
            .into_iter()
            .map(|content| match content {
                PromptContent::Text(text) => ContentBlock::text(text),
                PromptContent::Image { data, mime_type } => ContentBlock::Image { data, mime_type },
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ContentBlock {
    Text { text: String },
    Image { data: String, mime_type: String },
    ResourceLink { uri: String, name: String },
}

impl ContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn text_content(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }
}

impl Serialize for ContentBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        match self {
            Self::Text { text } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "text")?;
                map.serialize_entry("text", text)?;
                map.end()
            }
            Self::Image { data, mime_type } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "image")?;
                map.serialize_entry("data", data)?;
                map.serialize_entry("mimeType", mime_type)?;
                map.end()
            }
            Self::ResourceLink { uri, name } => {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "resource_link")?;
                map.serialize_entry("uri", uri)?;
                map.serialize_entry("name", name)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ContentBlock {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let block_type = value.get("type").and_then(Value::as_str);

        match block_type {
            Some("text") => Ok(ContentBlock::Text {
                text: text_or_log(&value, "text block"),
            }),
            Some("image") => Ok(ContentBlock::Image {
                data: value
                    .get("data")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                mime_type: value
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .unwrap_or("application/octet-stream")
                    .to_string(),
            }),
            Some("resource_link") => Ok(ContentBlock::ResourceLink {
                uri: value
                    .get("uri")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                name: value
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            }),
            _ => {
                log::debug!("agent sent an unsupported content block: {value}");
                Ok(ContentBlock::Text { text: "".into() })
            }
        }
    }
}

fn text_or_log(value: &Value, context: &str) -> String {
    let text = value
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if text.is_empty() {
        log::debug!("agent sent a {context} without text");
    }

    text
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u64,
    pub client_capabilities: ClientCapabilities,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fs: Option<FilesystemCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemCapabilities {
    pub read_text_file: bool,
    pub write_text_file: bool,
}

impl FilesystemCapabilities {
    pub fn enabled() -> Self {
        Self {
            read_text_file: true,
            write_text_file: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: u64,
    #[serde(default)]
    pub auth_methods: Vec<AuthMethod>,
    #[serde(default)]
    pub agent_capabilities: AgentCapabilities,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthMethod {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    #[serde(default)]
    pub load_session: bool,
    #[serde(default)]
    pub prompt_capabilities: PromptCapabilities,
    #[serde(default)]
    pub mcp_capabilities: McpCapabilities,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PromptCapabilities {
    #[serde(default)]
    pub embedded_context: bool,
    #[serde(default)]
    pub image: bool,
    #[serde(default)]
    pub audio: bool,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct McpCapabilities {
    #[serde(default)]
    pub http: bool,
    #[serde(default)]
    pub sse: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionModes {
    #[serde(default)]
    pub current: String,
    #[serde(default)]
    pub available: Vec<SessionMode>,
    #[serde(default)]
    pub models: Option<SessionModels>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionModels {
    #[serde(default)]
    pub current: String,
    #[serde(default)]
    pub available: Vec<SessionModel>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionModel {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionMode {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewResponse {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub modes: Option<SessionModes>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionParams {
    pub cwd: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoadSessionParams {
    pub cwd: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ToolCallKind {
    Read,
    Edit,
    Delete,
    Move,
    Search,
    Think,
    Fetch,
    Execute,
    Other,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallLocation {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub line: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallContent {
    Content { content: ContentBlock },
    Diff {
        #[serde(default)]
        path: String,
        #[serde(default)]
        old_text: Option<String>,
        #[serde(default)]
        new_text: Option<String>,
    },
    Terminal { terminal_id: String },
}

// Agent implementations have drifted around these enums across releases; missing or
// misspelled values must not silently drop tool calls, so they fall back to a
// displayable default instead.
impl<'de> Deserialize<'de> for ToolCallKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "read" => Ok(Self::Read),
            "edit" => Ok(Self::Edit),
            "delete" => Ok(Self::Delete),
            "move" => Ok(Self::Move),
            "search" => Ok(Self::Search),
            "think" => Ok(Self::Think),
            "fetch" => Ok(Self::Fetch),
            "execute" => Ok(Self::Execute),
            other => {
                if !other.is_empty() {
                    log::debug!("agent sent an unsupported tool call kind: {other:?}");
                }

                Ok(Self::Other)
            }
        }
    }
}

impl<'de> Deserialize<'de> for ToolCallStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "pending" => Ok(Self::Pending),
            "in_progress" => Ok(Self::InProgress),
            "completed" => Ok(Self::Completed),
            other => {
                log::debug!("agent sent an unsupported tool call status: {other:?}");
                Ok(Self::InProgress)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    #[serde(alias = "toolCallId")]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub kind: Option<ToolCallKind>,
    #[serde(default = "default_tool_call_status")]
    pub status: ToolCallStatus,
    #[serde(default)]
    pub content: Vec<ToolCallContent>,
    #[serde(default)]
    pub locations: Vec<ToolCallLocation>,
    #[serde(default)]
    pub raw_input: Option<Value>,
    #[serde(default)]
    pub raw_output: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallUpdate {
    #[serde(rename = "toolCallId", alias = "id")]
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub kind: Option<ToolCallKind>,
    #[serde(default)]
    pub status: Option<ToolCallStatus>,
    #[serde(default)]
    pub content: Option<Vec<ToolCallContent>>,
    #[serde(default)]
    pub locations: Option<Vec<ToolCallLocation>>,
    #[serde(default)]
    pub raw_input: Option<Value>,
    #[serde(default)]
    pub raw_output: Option<Value>,
}

fn default_tool_call_status() -> ToolCallStatus {
    ToolCallStatus::InProgress
}

#[derive(Debug, PartialEq)]
pub enum SessionUpdate {
    /// Agent is streaming the answer's text chunks into one message.
    AgentMessageChunk { content: ContentBlock },
    AgentThoughtChunk { content: ContentBlock },
    UserMessageChunk { content: ContentBlock },
    ToolCall(ToolCall),
    ToolCallUpdate(ToolCallUpdate),
    Plan { entries: Vec<PlanEntry> },
    AvailableCommands { available_commands: Vec<AvailableCommand> },
    CurrentMode { current_mode_id: String },
    Unknown { kind: String },
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanEntry {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub priority: PlanEntryPriority,
    #[serde(default)]
    pub status: PlanEntryStatus,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanEntryPriority {
    #[default]
    Primary,
    Middle,
    Low,
}

// Lenient: plan entries whose priority string the UI doesn't recognize must
// still be shown rather than dropped.
impl<'de> Deserialize<'de> for PlanEntryPriority {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "middle" => Ok(Self::Middle),
            "low" => Ok(Self::Low),
            "primary" | "high" => Ok(Self::Primary),
            other => {
                log::debug!("agent sent an unsupported plan priority: {other:?}");
                Ok(Self::Primary)
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanEntryStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableCommand {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub options: Vec<PermissionOption>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PermissionOutcome {
    #[serde(rename_all = "camelCase")]
    Selected { option_id: String },
    Cancelled,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PromptResponse {
    #[serde(default)]
    pub stop_reason: Option<String>,
}

pub fn parse_session_update(update: &Value) -> SessionUpdate {
    let kind = update
        .get("sessionUpdate")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match kind {
        "agent_message_chunk" => SessionUpdate::AgentMessageChunk {
            content: parse_chunk_content(update, "agent_message_chunk"),
        },
        "agent_thought_chunk" => SessionUpdate::AgentThoughtChunk {
            content: parse_chunk_content(update, "agent_thought_chunk"),
        },
        "user_message_chunk" => SessionUpdate::UserMessageChunk {
            content: parse_chunk_content(update, "user_message_chunk"),
        },
        "tool_call" => match ToolCall::deserialize(update) {
            Ok(tool_call) => SessionUpdate::ToolCall(tool_call),
            Err(error) => {
                log::warn!("failed to parse agent tool call: {error}");
                SessionUpdate::Unknown {
                    kind: kind.to_string(),
                }
            }
        },
        "tool_call_update" => match ToolCallUpdate::deserialize(update) {
            Ok(update) => SessionUpdate::ToolCallUpdate(update),
            Err(error) => {
                log::warn!("failed to parse agent tool call update: {error}");
                SessionUpdate::Unknown {
                    kind: kind.to_string(),
                }
            }
        },
        "plan" => {
            let entries = update
                .get("entries")
                .and_then(Value::as_array)
                .map(|array| {
                    array
                        .iter()
                        .filter_map(|entry| {
                            PlanEntry::deserialize(entry)
                                .inspect_err(|error| log::warn!("failed to parse plan entry: {error}"))
                                .ok()
                        })
                        .collect()
                })
                .unwrap_or_default();

            SessionUpdate::Plan { entries }
        }
        "available_commands_update" => {
            let available_commands = update
                .get("availableCommands")
                .and_then(Value::as_array)
                .map(|array| {
                    array
                        .iter()
                        .filter_map(|command| {
                            AvailableCommand::deserialize(command)
                                .inspect_err(|error| {
                                    log::warn!("failed to parse available command: {error}")
                                })
                                .ok()
                        })
                        .collect()
                })
                .unwrap_or_default();

            SessionUpdate::AvailableCommands { available_commands }
        }
        "current_mode_update" => SessionUpdate::CurrentMode {
            current_mode_id: update
                .get("currentModeId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        },
        _ => {
            log::debug!("agent sent an unknown session update kind: {kind}");
            SessionUpdate::Unknown {
                kind: kind.to_string(),
            }
        }
    }
}

fn parse_chunk_content(update: &Value, context: &str) -> ContentBlock {
    let empty = ContentBlock::text("");

    match update.get("content") {
        Some(Value::Array(blocks)) => {
            if blocks.len() > 1 {
                log::warn!("agent sent multiple blocks in {context}; using the first one");
            }

            blocks
                .first()
                .and_then(|block| ContentBlock::deserialize(block).ok())
                .unwrap_or(empty)
        }
        Some(block) => ContentBlock::deserialize(block).unwrap_or(empty),
        None => {
            log::debug!("agent sent {context} without content");
            empty
        }
    }
}
