use serde::{Deserialize, Serialize};

/// A project's page in the shared Chromium the daemon runs: the page the Browser block shows
/// and the one the project's runs get with cedia's `--browser`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrowserPage {
    /// The browser's DevTools endpoint, `http://127.0.0.1:<port>`.
    pub endpoint: String,
    /// The page's DevTools WebSocket, `ws://127.0.0.1:<port>/devtools/page/<id>`.
    pub socket: String,
}
