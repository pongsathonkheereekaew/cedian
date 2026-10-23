use serde::{Deserialize, Serialize};

/// One MCP server under `mcpServers` in cedia's settings, as the Background tasks block lists
/// it. Only what says which server it is: never its arguments, environment or headers, which
/// can hold keys.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpServer {
    pub name: String,
    pub transport: McpTransport,
    /// cedia counts its tools as reads, not as commands.
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpTransport {
    /// A program cedia starts: its file name, without the directory.
    Command { program: String },
    /// A server cedia reaches over HTTP: its host.
    Url { host: String },
}

/// A server the person adds in Settings, as `cedia mcp add` takes it. The values of `env` and
/// `headers` go to cedia once and are never sent back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpAdd {
    pub target: McpTarget,
    /// `NAME=value` pairs: the program's environment, or the HTTP server's headers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairs: Vec<(String, String)>,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum McpTarget {
    Command { command: String, args: Vec<String> },
    Url { url: String },
}
