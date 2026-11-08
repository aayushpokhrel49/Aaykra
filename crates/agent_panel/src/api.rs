use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_acp::{
    ContentBlock, ToolCall, ToolCallContent, ToolCallKind, ToolCallLocation, ToolCallStatus,
};
use fs::{Fs, RemoveOptions};
use futures::{AsyncReadExt as _, StreamExt as _};
use http_client::{AsyncBody, HttpClient, Method, Request};
use serde_json::{Value, json};
use util::process::Child;

/// How many request rounds (tool exchanges) one user message may take before
/// the loop stops, so a runaway model cannot burn the whole API key budget.
const MAX_TOOL_ROUNDS: usize = 10;
/// Upper bound on characters fed back to the model for a single file read.
const MAX_READ_CHARS: usize = 24_000;
/// File count cap for the built-in workspace scan when ripgrep is unavailable.
const MAX_SCAN_FILES: usize = 2500;
const MAX_SCAN_RESULTS: usize = 60;
/// How many persisted timeline rows are replayed as history for context.
pub const MAX_HISTORY_ITEMS: usize = 20;

/// Workspace context the file tools operate on. When `read_only` is set the
/// mutating tools are not offered, so plan mode can analyze without editing.
pub struct ToolHost {
    pub fs: Arc<dyn Fs>,
    pub root: PathBuf,
    pub read_only: bool,
}

/// Distinguishes the two callbacks the panel receives for every tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallPhase {
    /// The model asked for the tool and it is starting.
    Started,
    /// The tool finished (successfully or with an error).
    Finished,
}

/// Configuration for the user's own OpenAI-compatible endpoint.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// Builds one history message for the wire. `role` is `user` or `assistant`.
pub fn history_message(role: &str, content: impl Into<String>) -> Value {
    json!({ "role": role, "content": content.into() })
}

/// Events surfaced to the panel while an agent turn runs.
pub enum TurnEvent {
    /// A markdown delta for the running reply.
    Text(String),
    /// A tool call the model requested, with the phase it is in.
    ToolCall(ToolCall, ToolCallPhase),
}

/// Runs a full agentic turn against an OpenAI-compatible endpoint: streams
/// the model's reply, executes any function calls it asks for against real
/// workspace files, feeds the results back until the model settles on a
/// final message, and surfaces everything through `on_event`.
pub async fn run_agent_turn(
    http: Arc<dyn HttpClient>,
    config: ApiConfig,
    mut history: Vec<Value>,
    host: ToolHost,
    cancel: &AtomicBool,
    mut on_event: impl FnMut(TurnEvent),
) -> anyhow::Result<()> {
    let tool_definitions = tool_definitions(host.read_only);
    history.insert(
        0,
        json!({ "role": "system", "content": system_prompt(&host) }),
    );
    let mut tools_disabled = false;
    let mut round = 0;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        round += 1;
        if round > MAX_TOOL_ROUNDS {
            on_event(TurnEvent::Text(
                "\n\n[Stopped: this message used its tool-round budget. Ask to continue.]"
                    .to_string(),
            ));
            return Ok(());
        }
        let offered: Option<&[Value]> = if tools_disabled {
            None
        } else {
            Some(&tool_definitions)
        };
        let forwarded_text = &mut |delta: &str| on_event(TurnEvent::Text(delta.to_string()));
        let streamed = match stream_completion(
            &http,
            &config,
            &history,
            offered,
            cancel,
            forwarded_text,
        )
        .await
        {
            Ok(collector) => collector,
            Err(error) => {
                // Some OpenAI-compatible endpoints refuse function calling.
                // If the refusal is about tools, degrade to plain chat for
                // this turn instead of failing it outright.
                let error_text = format!("{error:#}");
                if !tools_disabled && round == 1 && error_text.to_lowercase().contains("tool") {
                    log::info!("Provider rejected tool calls; falling back to plain chat");
                    tools_disabled = true;
                    history.retain(|message| match message["role"].as_str() {
                        Some("assistant") => message.get("tool_calls").is_none(),
                        Some("tool") => false,
                        _ => true,
                    });
                    match stream_completion(&http, &config, &history, None, cancel, forwarded_text)
                        .await
                    {
                        Ok(collector) => collector,
                        Err(retry_error) => {
                            return Err(retry_error);
                        }
                    }
                } else {
                    return Err(error);
                }
            }
        };

        if streamed.tool_calls.is_empty() {
            return Ok(());
        }

        history.push(json!({
            "role": "assistant",
            "content": streamed.text,
            "tool_calls": streamed
                .tool_calls
                .iter()
                .filter(|tool_call| !tool_call.name.is_empty())
                .map(|tool_call| {
                    json!({
                        "id": tool_call.id,
                        "type": "function",
                        "function": {
                            "name": tool_call.name,
                            "arguments": tool_call.arguments,
                        },
                    })
                })
                .collect::<Vec<_>>(),
        }));

        for tool_call in streamed.tool_calls {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            if tool_call.name.is_empty() {
                // No name means a malformed stream entry; there is nothing to
                // run, and pushing an unpaired tool result would violate the
                // OpenAI invariant that every tool message answers an entry
                // of the assistant message above.
                continue;
            }
            let arguments: Value =
                serde_json::from_str(&tool_call.arguments).unwrap_or_else(|_| json!({}));
            let title = tool_call_title(&tool_call.name, &arguments);
            let kind = tool_call_kind(&tool_call.name);
            let requested_path = arguments["path"].as_str().unwrap_or(".");
            on_event(TurnEvent::ToolCall(
                ToolCall {
                    id: tool_call.id.clone(),
                    title: title.clone(),
                    kind: Some(kind),
                    status: ToolCallStatus::InProgress,
                    content: Vec::new(),
                    locations: vec![ToolCallLocation {
                        path: requested_path.to_string(),
                        line: None,
                    }],
                    raw_input: None,
                    raw_output: None,
                },
                ToolCallPhase::Started,
            ));

            let executed = execute_tool(&host, &tool_call.name, &arguments).await;
            match executed {
                Ok(executed) => {
                    let content = if let Some(change) = &executed.change {
                        vec![ToolCallContent::Diff {
                            path: change.path.clone(),
                            old_text: change.old.clone(),
                            new_text: Some(change.new.clone()),
                        }]
                    } else {
                        vec![ToolCallContent::Content {
                            content: ContentBlock::text(executed.display.clone()),
                        }]
                    };
                    let locations = executed.locations.clone();
                    on_event(TurnEvent::ToolCall(
                        ToolCall {
                            id: tool_call.id.clone(),
                            title,
                            kind: Some(kind),
                            status: ToolCallStatus::Completed,
                            content,
                            locations,
                            raw_input: None,
                            raw_output: None,
                        },
                        ToolCallPhase::Finished,
                    ));
                    history.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": executed.result,
                    }));
                }
                Err(error) => {
                    let message = format!("{error:#}");
                    on_event(TurnEvent::ToolCall(
                        ToolCall {
                            id: tool_call.id.clone(),
                            title,
                            kind: Some(kind),
                            status: ToolCallStatus::Failed,
                            content: vec![ToolCallContent::Content {
                                content: ContentBlock::text(format!("Failed: {message}")),
                            }],
                            locations: Vec::new(),
                            raw_input: None,
                            raw_output: None,
                        },
                        ToolCallPhase::Finished,
                    ));
                    history.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": format!("Tool error: {message}"),
                    }));
                }
            }
        }
    }
}

/// Streams a chat completion, forwarding content deltas to `on_text` and
/// accumulating any tool calls the model requests.
async fn stream_completion(
    http: &Arc<dyn HttpClient>,
    config: &ApiConfig,
    messages: &[Value],
    tools: Option<&[Value]>,
    cancel: &AtomicBool,
    on_text: &mut impl FnMut(&str),
) -> anyhow::Result<StreamCollector> {
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let mut body = json!({
        "model": config.model,
        "messages": messages,
        "stream": true,
    });
    if let Some(tools) = tools {
        body["tools"] = json!(tools);
        body["tool_choice"] = json!("auto");
    }

    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(&url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");
    if !config.api_key.is_empty() {
        builder = builder.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let response = http
        .send(builder.body(AsyncBody::from(body.to_string()))?)
        .await?;
    let status = response.status();
    if !status.is_success() {
        let mut error_text = String::new();
        response.into_body().read_to_string(&mut error_text).await?;
        anyhow::bail!("API returned {status}: {error_text}");
    }

    let mut body = response.into_body();
    let mut collector = StreamCollector::default();
    let mut buffered = Vec::new();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(collector);
        }
        let mut chunk = [0u8; 4096];
        let count = body.read(&mut chunk).await?;
        if count == 0 {
            break;
        }
        for &byte in &chunk[..count] {
            buffered.push(byte);
            if byte == b'\n' {
                collector.absorb_sse_line(&buffered, &mut *on_text)?;
                buffered.clear();
            }
        }
    }
    if !buffered.is_empty() {
        collector.absorb_sse_line(&buffered, &mut *on_text)?;
    }
    Ok(collector)
}

#[derive(Default)]
struct StreamCollector {
    text: String,
    tool_calls: Vec<PartialToolCall>,
}

#[derive(Clone, Default)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl StreamCollector {
    fn absorb_sse_line(
        &mut self,
        line: &[u8],
        on_text: &mut impl FnMut(&str),
    ) -> anyhow::Result<()> {
        let text = String::from_utf8_lossy(line);
        let text = text.trim();
        if !text.starts_with("data:") {
            return Ok(());
        }
        let data = text["data:".len()..].trim();
        if data.is_empty() || data == "[DONE]" {
            return Ok(());
        }
        let value: Value = serde_json::from_str(data)?;
        let choice = &value["choices"][0];
        if let Some(delta) = choice["delta"]["content"].as_str() {
            if !delta.is_empty() {
                self.text.push_str(delta);
                on_text(delta);
            }
        }
        if let Some(tool_calls) = choice["delta"]["tool_calls"].as_array() {
            for tool_call in tool_calls {
                let index = tool_call["index"].as_u64().unwrap_or(0) as usize;
                if index >= self.tool_calls.len() {
                    self.tool_calls
                        .resize(index + 1, PartialToolCall::default());
                }
                let partial = &mut self.tool_calls[index];
                if let Some(id) = tool_call["id"].as_str() {
                    partial.id = id.to_string();
                }
                if let Some(name) = tool_call["function"]["name"].as_str() {
                    partial.name.push_str(name);
                }
                if let Some(arguments) = tool_call["function"]["arguments"].as_str() {
                    partial.arguments.push_str(arguments);
                }
            }
        }
        // Providers that ignore `stream: true` still answer with a full
        // message; merge those the same way so the loop stays compatible.
        if let Some(message) = choice["message"]["content"].as_str() {
            if !message.is_empty() {
                self.text.push_str(message);
                on_text(message);
            }
        }
        if let Some(tool_calls) = choice["message"]["tool_calls"].as_array() {
            for (index, tool_call) in tool_calls.iter().enumerate() {
                if index >= self.tool_calls.len() {
                    self.tool_calls
                        .resize(index + 1, PartialToolCall::default());
                }
                let partial = &mut self.tool_calls[index];
                if let Some(id) = tool_call["id"].as_str() {
                    partial.id = id.to_string();
                }
                if let Some(name) = tool_call["function"]["name"].as_str() {
                    partial.name = name.to_string();
                }
                if let Some(arguments) = tool_call["function"]["arguments"].as_str() {
                    partial.arguments = arguments.to_string();
                }
            }
        }
        Ok(())
    }
}

/// Result of executing one tool.
struct ToolExecuted {
    /// Short text shown in the timeline row when there is no diff.
    display: String,
    /// Full text returned to the model as the tool result.
    result: String,
    /// File change to render as a diff in the timeline.
    change: Option<FileChange>,
    locations: Vec<ToolCallLocation>,
}

struct FileChange {
    path: String,
    old: Option<String>,
    new: String,
}

async fn execute_tool(
    host: &ToolHost,
    name: &str,
    arguments: &Value,
) -> anyhow::Result<ToolExecuted> {
    match name {
        "list_dir" => list_dir_tool(host, arguments).await,
        "read_file" => read_file_tool(host, arguments).await,
        "grep_search" => grep_search_tool(host, arguments).await,
        "write_file" if !host.read_only => write_file_tool(host, arguments).await,
        "edit_file" if !host.read_only => edit_file_tool(host, arguments).await,
        "delete_file" if !host.read_only => delete_file_tool(host, arguments).await,
        "write_file" | "edit_file" | "delete_file" => Err(anyhow::anyhow!(
            "{name} is unavailable in plan mode (read-only)"
        )),
        other => Err(anyhow::anyhow!("Unknown tool {other}")),
    }
}

async fn list_dir_tool(host: &ToolHost, arguments: &Value) -> anyhow::Result<ToolExecuted> {
    let relative = arguments["path"].as_str().unwrap_or("");
    let path = resolve_path(&host.root, relative)?;
    let display = relative_display(&host.root, &path);
    if !host.fs.is_dir(&path).await {
        anyhow::bail!("{} is not a directory", display);
    }
    let mut stream = host.fs.read_dir(&path).await?;
    let mut entries: Vec<String> = Vec::new();
    let mut truncated = false;
    while let Some(entry) = stream.next().await {
        if entries.len() >= 300 {
            truncated = true;
            break;
        }
        let entry = entry?;
        let is_dir = host
            .fs
            .metadata(&entry)
            .await
            .ok()
            .flatten()
            .map_or(false, |metadata| metadata.is_dir);
        let mut shown = relative_display(&host.root, &entry);
        if shown.is_empty() {
            shown = ".".to_string();
        }
        if is_dir {
            shown.push('/');
        }
        entries.push(shown);
    }
    entries.sort();
    let listing = entries.join("\n");
    let count = entries.len();
    Ok(ToolExecuted {
        display: format!("Listed {display}: {count} entries"),
        result: format!(
            "Contents of {display}:\n{listing}{}",
            if truncated {
                "\n... (listing truncated)"
            } else {
                ""
            }
        ),
        change: None,
        locations: vec![ToolCallLocation {
            path: display,
            line: None,
        }],
    })
}

async fn read_file_tool(host: &ToolHost, arguments: &Value) -> anyhow::Result<ToolExecuted> {
    let relative = file_argument(arguments, "path")?;
    let path = resolve_path(&host.root, relative)?;
    let display = relative_display(&host.root, &path);
    if let Some(metadata) = host.fs.metadata(&path).await.ok().flatten() {
        if metadata.is_dir {
            anyhow::bail!("{} is a directory; use list_dir", display);
        }
    }
    let content = host
        .fs
        .load(&path)
        .await
        .map_err(|error| anyhow::anyhow!("Could not read {display}: {error}"))?;
    let total_lines = content.lines().count().max(1);
    let start_line = arguments["start_line"].as_u64().unwrap_or(1).max(1) as usize;
    let end_line = arguments["end_line"].as_u64().unwrap_or(u64::MAX) as usize;
    let end_line = end_line.max(start_line).min(total_lines);
    let selected: Vec<&str> = content
        .lines()
        .skip(start_line - 1)
        .take(end_line - start_line + 1)
        .collect();
    let mut body = selected.join("\n");
    let truncated = body.len() > MAX_READ_CHARS;
    if truncated {
        let mut cut = MAX_READ_CHARS;
        while cut > 0 && !body.is_char_boundary(cut) {
            cut -= 1;
        }
        body.truncate(cut);
    }
    let shown_first = start_line;
    let shown_last = start_line + selected.len().saturating_sub(1);
    Ok(ToolExecuted {
        display: format!("Read {display} (lines {shown_first}-{shown_last} of {total_lines})"),
        result: format!(
            "File {display} (lines {shown_first}-{shown_last} of {total_lines} total):\n{body}{}",
            if truncated {
                "\n\u{2026} (truncated; request a smaller line range for the rest)"
            } else {
                ""
            }
        ),
        change: None,
        locations: vec![ToolCallLocation {
            path: display,
            line: Some(start_line as u64),
        }],
    })
}

async fn grep_search_tool(host: &ToolHost, arguments: &Value) -> anyhow::Result<ToolExecuted> {
    let query = arguments["query"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("grep_search requires a query string"))?;
    let ignore_case = arguments["ignore_case"].as_bool().unwrap_or(false);
    let relative = arguments["path"].as_str().unwrap_or("");
    let path = resolve_path(&host.root, relative)?;
    let display = relative_display(&host.root, &path);

    let result = match find_in_path("rg") {
        Some(rg_binary) => ripgrep_search(&rg_binary, query, &path, ignore_case).await,
        None => builtin_grep(host, query, &path, ignore_case).await,
    };
    let result = result?;
    let matches = result.lines().count();
    Ok(ToolExecuted {
        display: format!("Searched \u{201c}{query}\u{201d} in {display}: {matches} matches"),
        result,
        change: None,
        locations: vec![ToolCallLocation {
            path: display,
            line: None,
        }],
    })
}

async fn ripgrep_search(
    rg_binary: &Path,
    query: &str,
    path: &Path,
    ignore_case: bool,
) -> anyhow::Result<String> {
    let mut command = std::process::Command::new(rg_binary);
    command
        .arg("--color")
        .arg("never")
        .arg("--no-heading")
        .arg("--line-number")
        .arg("--max-columns")
        .arg("240")
        .arg("--max-count")
        .arg("40")
        .args(ignore_case.then_some("-i"))
        .args(["-e", query])
        .arg(path);
    let mut child = Child::spawn(command, Stdio::null(), Stdio::piped(), Stdio::null())?;
    let mut output = Vec::new();
    if let Some(stdout) = child.stdout().take() {
        let mut stdout = stdout;
        stdout.read_to_end(&mut output).await?;
    }
    let status = child
        .status()
        .await
        .map_err(|error| anyhow::anyhow!("ripgrep status unavailable: {error}"))?;
    let text = String::from_utf8_lossy(&output);
    let trimmed = trim_last(text.trim_end(), MAX_GREP_BYTES);
    match status.code().unwrap_or(-1) {
        0 if trimmed.is_empty() => Ok("No matches found.".to_string()),
        0 => Ok(trimmed),
        1 => Ok("No matches found.".to_string()),
        other => anyhow::bail!("ripgrep exited with status {other}: {trimmed}"),
    }
}

/// Substring search over the plain workspace files, used when ripgrep is not
/// installed so the agent still gets a way to find code.
async fn builtin_grep(
    host: &ToolHost,
    query: &str,
    directory: &Path,
    ignore_case: bool,
) -> anyhow::Result<String> {
    let needle = if ignore_case {
        query.to_lowercase()
    } else {
        query.to_string()
    };
    let case_insensitive = ignore_case;
    let mut pending = vec![directory.to_path_buf()];
    let mut matches: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    let mut limit_hit = false;

    while let Some(current) = pending.pop() {
        if matches.len() >= MAX_SCAN_RESULTS {
            break;
        }
        let mut stream = match host.fs.read_dir(&current).await {
            Ok(stream) => stream,
            Err(error) => {
                log::debug!("agent grep skipping {:?}: {}", current, error);
                continue;
            }
        };
        while let Some(entry) = stream.next().await {
            if matches.len() >= MAX_SCAN_RESULTS {
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    log::debug!("agent grep skipping entry: {}", error);
                    continue;
                }
            };
            let too_large = host
                .fs
                .metadata(&entry)
                .await
                .ok()
                .flatten()
                .map_or(false, |metadata| metadata.len >= 512_000);
            let is_dir = host
                .fs
                .metadata(&entry)
                .await
                .ok()
                .flatten()
                .map_or(false, |metadata| metadata.is_dir);
            if is_dir {
                let name = entry.file_name().to_string_lossy().into_owned();
                if SKIPPED_DIRECTORIES.contains(&name.as_str()) {
                    continue;
                }
                pending.push(entry);
                continue;
            }
            if too_large {
                continue;
            }
            scanned += 1;
            if scanned > MAX_SCAN_FILES {
                limit_hit = true;
                break;
            }
            let text = match host.fs.load(&entry).await {
                Ok(text) => text,
                Err(error) => {
                    log::debug!("agent grep skipping {:?}: {}", entry, error);
                    continue;
                }
            };
            if text.contains('\0') {
                continue;
            }
            let searchable = if case_insensitive {
                text.to_lowercase()
            } else {
                text
            };
            let shown_path = relative_display(&host.root, &entry);
            for (index, line) in searchable.lines().enumerate() {
                if !line.contains(&needle) {
                    continue;
                }
                let trimmed_line = line.trim().chars().take(200).collect::<String>();
                matches.push(format!("{shown_path}:{}: {trimmed_line}", index + 1));
                if matches.len() >= MAX_SCAN_RESULTS {
                    break;
                }
            }
        }
    }
    if matches.is_empty() {
        anyhow::bail!(
            "No matches for \u{201c}{query}\u{201d}{}",
            scan_note(limit_hit, scanned)
        );
    }
    Ok(format!(
        "{}{}",
        matches.join("\n"),
        scan_note(limit_hit, scanned)
    ))
}

fn scan_note(limit_hit: bool, scanned: usize) -> String {
    if limit_hit {
        format!(" (scan capped after {scanned} files; install ripgrep for full coverage)")
    } else {
        format!(" (scanned {scanned} files; install ripgrep for faster scanning)")
    }
}

const SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    ".cache",
    ".cargo",
    "zig-cache",
    ".tox",
    ".mypy_cache",
    "coverage",
];

const MAX_GREP_BYTES: usize = 32_000;

async fn write_file_tool(host: &ToolHost, arguments: &Value) -> anyhow::Result<ToolExecuted> {
    let relative = file_argument(arguments, "path")?;
    let content = arguments["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("write_file requires a content string"))?;
    let path = resolve_path(&host.root, relative)?;
    let display = relative_display(&host.root, &path);
    ensure_parent_directories(host, &path, &display).await?;
    let existed = host.fs.is_file(&path).await;
    // An unreadable old file (bad encoding, racing writers) must not block a
    // deliberate overwrite; the diff simply shows the new content only.
    let old = if existed {
        host.fs.load(&path).await.ok()
    } else {
        None
    };
    host.fs.write(&path, content.as_bytes()).await?;
    let verb = if existed { "Overwrote" } else { "Created" };
    Ok(ToolExecuted {
        display: format!("{verb} {display} ({} bytes)", content.len()),
        result: format!("Wrote {display} ({} bytes).", content.len()),
        change: Some(FileChange {
            path: display.clone(),
            old: old.as_deref().map(shorten_for_diff),
            new: Some(shorten_for_diff(content)),
        }),
        locations: vec![ToolCallLocation {
            path: display,
            line: None,
        }],
    })
}

/// Creates every missing ancestor directory below the workspace root.
async fn ensure_parent_directories(
    host: &ToolHost,
    path: &Path,
    display: &str,
) -> anyhow::Result<()> {
    let mut missing: Vec<PathBuf> = Vec::new();
    let mut current = path.parent();
    while let Some(ancestor) = current {
        if !ancestor.starts_with(&host.root) {
            break;
        }
        if !host.fs.is_dir(ancestor).await {
            missing.push(ancestor.to_path_buf());
        }
        current = ancestor.parent();
    }
    for ancestor in missing.iter().rev() {
        host.fs
            .create_dir(ancestor)
            .await
            .map_err(|error| anyhow::anyhow!("Could not create parent of {display}: {error}"))?;
    }
    Ok(())
}

/// Shortens a file body so a diff shown in the timeline stays readable.
const MAX_DIFF_CHARS: usize = 8_000;

async fn edit_file_tool(host: &ToolHost, arguments: &Value) -> anyhow::Result<ToolExecuted> {
    let relative = file_argument(arguments, "path")?;
    let old_string = arguments["old_string"]
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("edit_file needs a non-empty old_string; read the file first")
        })?;
    let new_string = arguments["new_string"].as_str().unwrap_or("");
    let replace_all = arguments["replace_all"].as_bool().unwrap_or(false);
    let path = resolve_path(&host.root, relative)?;
    let display = relative_display(&host.root, &path);
    let existing = host
        .fs
        .load(&path)
        .await
        .map_err(|error| anyhow::anyhow!("Could not read {display} to edit: {error}"))?;
    let occurrences = existing.matches(old_string).count();
    if occurrences == 0 {
        anyhow::bail!("old_string not found in {display}; use read_file and copy an exact snippet");
    }
    if occurrences > 1 && !replace_all {
        anyhow::bail!(
            "old_string matched {occurrences} times in {display}; add surrounding context so it is unique, or set replace_all: true"
        );
    }
    let replacement = if replace_all {
        existing.replace(old_string, new_string)
    } else {
        existing.replacen(old_string, new_string, 1)
    };
    let replaced = if replace_all { occurrences } else { 1 };
    let first_line = existing
        .find(old_string)
        .map(|offset| existing[..offset].lines().count() as u64 + 1);
    host.fs.write(&path, replacement.as_bytes()).await?;
    Ok(ToolExecuted {
        display: format!(
            "Edited {display} ({replaced} replacement{})",
            if replaced == 1 { "" } else { "s" }
        ),
        result: format!("Edited {display}: replaced {replaced} occurrence(s)."),
        change: Some(FileChange {
            path: display.clone(),
            old: Some(shorten_for_diff(&existing)),
            new: Some(shorten_for_diff(&replacement)),
        }),
        locations: vec![ToolCallLocation {
            path: display,
            line: first_line,
        }],
    })
}

async fn delete_file_tool(host: &ToolHost, arguments: &Value) -> anyhow::Result<ToolExecuted> {
    let relative = file_argument(arguments, "path")?;
    let path = resolve_path(&host.root, relative)?;
    let display = relative_display(&host.root, &path);
    if host.fs.is_dir(&path).await {
        anyhow::bail!(
            "{display} is a directory; deleting directories is not supported by this agent"
        );
    }
    if !host.fs.is_file(&path).await {
        anyhow::bail!("{display} does not exist");
    }
    host.fs.remove_file(&path, RemoveOptions::default()).await?;
    Ok(ToolExecuted {
        display: format!("Deleted {display}"),
        result: format!("Deleted {display}."),
        change: None,
        locations: vec![ToolCallLocation {
            path: display,
            line: None,
        }],
    })
}

fn system_prompt(host: &ToolHost) -> String {
    let mut prompt = format!(
        "You are a coding agent embedded in the Aaykra editor. The user's project is at \"{}\".\n\
Use the provided tools so the user sees exactly what you do and so your work lands in the real files:\n\
- Call list_dir and read_file (or grep_search) before changing anything, so edits match the current code.\n\
- Paths are relative to the project root.\n\
- Prefer edit_file with an exact old_string copied from the file; use write_file for new files or full rewrites; delete_file removes a file.\n\
- Keep tool results internal; your final visible reply is normal markdown for the user.\n\
- When you are done acting, reply with a short summary of what you did.",
        host.root.display()
    );
    if host.read_only {
        prompt.push_str(
            "\n\nPlan mode is active: editing tools are unavailable. Analyze the task with the \
             read-only tools, then reply with a precise step-by-step plan the user can approve.",
        );
    }
    prompt
}

fn tool_definitions(read_only: bool) -> Vec<Value> {
    let mut definitions = vec![
        json!({
            "type": "function",
            "function": {
                "name": "list_dir",
                "description": "List one level of a directory in the project.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path, relative to the project root. Default \".\" (the root).",
                        },
                    },
                    "required": [],
                },
            },
        }),
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a text file from the project.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path relative to the project root.",
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "First line to read (1-based). Optional.",
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "Last line to read, inclusive. Optional.",
                        },
                    },
                    "required": ["path"],
                },
            },
        }),
        json!({
            "type": "function",
            "function": {
                "name": "grep_search",
                "description": "Search file contents for a substring across the project.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Text to find." },
                        "path": {
                            "type": "string",
                            "description": "Directory to search in, relative to the root. Default: root.",
                        },
                        "ignore_case": { "type": "boolean", "description": "Match ignoring case. Default false." },
                    },
                    "required": ["query"],
                },
            },
        }),
    ];
    if !read_only {
        definitions.push(json!({
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Create a new file, or replace an entire file's content.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path relative to the project root." },
                        "content": { "type": "string", "description": "The complete new file content." },
                    },
                    "required": ["path", "content"],
                },
            },
        }));
        definitions.push(json!({
            "type": "function",
            "function": {
                "name": "edit_file",
                "description": "Replace an exact snippet in a file. old_string must match the current content.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path relative to the project root." },
                        "old_string": { "type": "string", "description": "Exact existing snippet to replace." },
                        "new_string": { "type": "string", "description": "Replacement snippet." },
                        "replace_all": {
                            "type": "boolean",
                            "description": "Replace every occurrence of old_string. Default false (exactly one match required).",
                        },
                    },
                    "required": ["path", "old_string", "new_string"],
                },
            },
        }));
        definitions.push(json!({
            "type": "function",
            "function": {
                "name": "delete_file",
                "description": "Delete a file from the project.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "File path relative to the project root." },
                    },
                    "required": ["path"],
                },
            },
        }));
    }
    definitions
}

fn tool_call_title(name: &str, arguments: &Value) -> String {
    let path = arguments["path"]
        .as_str()
        .filter(|path| !path.trim().is_empty())
        .unwrap_or(".");
    match name {
        "list_dir" => format!("List {path}"),
        "read_file" => format!("Read {path}"),
        "grep_search" => {
            let query = arguments["query"].as_str().unwrap_or("");
            format!("Search \u{201c}{query}\u{201d} in {path}")
        }
        "write_file" => format!("Write {path}"),
        "edit_file" => format!("Edit {path}"),
        "delete_file" => format!("Delete {path}"),
        other => format!("Call {other}"),
    }
}

fn tool_call_kind(name: &str) -> ToolCallKind {
    match name {
        "read_file" => ToolCallKind::Read,
        "list_dir" | "grep_search" => ToolCallKind::Search,
        "write_file" | "edit_file" => ToolCallKind::Edit,
        "delete_file" => ToolCallKind::Delete,
        _ => ToolCallKind::Other,
    }
}

/// Joins a model-provided path with the workspace root, refusing anything
/// that would traverse above it.
fn resolve_path(root: &Path, requested: &str) -> anyhow::Result<PathBuf> {
    let requested = requested.trim();
    if requested.is_empty() || requested == "." {
        return Ok(root.to_path_buf());
    }
    if requested.contains("..") {
        anyhow::bail!("Paths may not contain '..'; use a path relative to the project root");
    }
    let candidate = Path::new(requested);
    if candidate.is_absolute() {
        if candidate.starts_with(root) {
            return Ok(candidate.to_path_buf());
        }
        anyhow::bail!("Absolute paths must stay inside the project");
    }
    Ok(root.join(candidate))
}

/// Renders a path for display as a workspace-relative path.
fn relative_display(root: &Path, path: &Path) -> String {
    let shown = path
        .strip_prefix(root)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned());
    if shown.is_empty() {
        ".".to_string()
    } else {
        shown
    }
}

fn file_argument(arguments: &Value, name: &str) -> anyhow::Result<&str> {
    arguments[name]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing required string argument \u{201c}{name}\u{201d}"))
}

/// Locates an executable on PATH so tools degrade gracefully instead of
/// spawning blind commands.
fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path_variable = std::env::var_os("PATH")?;
    std::env::split_paths(&path_variable)
        .filter(|directory| !directory.as_os_str().is_empty())
        .map(|directory| directory.join(binary))
        .find(|candidate| candidate.is_file())
}

/// Truncates to the longest prefix that is still a valid UTF-8 boundary.
fn trim_last(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut cut = limit;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}\n(output truncated)", &text[..cut])
}

fn shorten_for_diff(text: &str) -> String {
    if text.len() <= MAX_DIFF_CHARS {
        text.to_string()
    } else {
        let mut cut = MAX_DIFF_CHARS;
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}\n\u{2026} (diff shortened)", &text[..cut])
    }
}

pub async fn fetch_models(
    http: Arc<dyn HttpClient>,
    config: &ApiConfig,
) -> anyhow::Result<Vec<String>> {
    let url = format!("{}/models", config.base_url.trim_end_matches('/'));
    let mut builder = Request::builder()
        .method(Method::GET)
        .uri(&url)
        .header("Accept", "application/json");
    if !config.api_key.is_empty() {
        builder = builder.header("Authorization", format!("Bearer {}", config.api_key));
    }
    let response = http.send(builder.body(AsyncBody::empty())?).await?;
    let status = response.status();
    if !status.is_success() {
        let mut error_text = String::new();
        response.into_body().read_to_string(&mut error_text).await?;
        anyhow::bail!("API returned {status}: {error_text}");
    }
    let mut text = String::new();
    response.into_body().read_to_string(&mut text).await?;
    let value: Value = serde_json::from_str(&text)?;
    Ok(value["data"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(|model| model["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}
