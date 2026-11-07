use agent_acp::AgentSpawnConfig;
use std::path::PathBuf;

/// Best-effort preset for spawning a known ACP-speaking agent CLI. Presets
/// assume the required tooling is present on `PATH` and fail gracefully with
/// an error message in the panel when it is not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
}

impl AgentProfile {
    pub fn spawn_config(&self, working_directory: Option<PathBuf>) -> AgentSpawnConfig {
        AgentSpawnConfig {
            command: self.command.to_owned(),
            args: self.args.iter().map(|arg| (*arg).to_owned()).collect(),
            env: Vec::new(),
            working_directory,
        }
    }
}

pub const BUILTIN_PROFILES: &[AgentProfile] = &[
    AgentProfile {
        id: "claude",
        label: "Claude Code",
        command: "npx",
        args: &["-y", "@agentclientprotocol/claude-agent-acp"],
    },
    AgentProfile {
        id: "codex",
        label: "Codex",
        command: "npx",
        args: &["-y", "@agentclientprotocol/codex-acp"],
    },
    AgentProfile {
        id: "gemini",
        label: "Gemini CLI",
        command: "npx",
        args: &["-y", "@google/gemini-cli", "--acp"],
    },
    AgentProfile {
        id: "opencode",
        label: "OpenCode",
        command: "opencode",
        args: &["acp"],
    },
];
