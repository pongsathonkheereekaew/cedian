//! What the Background tasks block lists beside the threads (Interface, Phase C): each
//! project's scheduled runs, as `cedia schedules` folds them, and the MCP servers in cedia's
//! settings. cedia keeps the schedules and says what is due; it runs nothing at a time
//! itself, so the daemon wakes each schedule of a project a window shows: `cedia schedules
//! fire <id>`, then the prompt on its thread, as a run or, when one is working, a message.

use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    path::{Path, PathBuf},
};

use cedian_index::project_key;
use cedian_protocol::{Capability, Schedule};
use cedian_wire::{McpServer, McpTransport, Notice};
use serde_json::Value;

use super::{ClientId, Daemon, Shared, lock};
use crate::{cedia::Flags, desktop_settings::settings_path};

/// A project's schedules as last listed, and the file's state they were listed at.
#[derive(Default)]
pub struct Schedules(HashMap<String, Listed>);

struct Listed {
    print: u64,
    list: Vec<Schedule>,
}

impl Schedules {
    fn list(&self, slug: &str) -> Option<&Vec<Schedule>> {
        self.0.get(slug).map(|l| &l.list)
    }
}

pub fn schedules_path(home: &Path, slug: &str) -> PathBuf {
    home.join("projects").join(slug).join("schedules.jsonl")
}

/// The file's size and time in one number; a missing file has its own.
fn print(path: &Path) -> u64 {
    let meta = std::fs::metadata(path)
        .ok()
        .map(|m| (m.len(), m.modified().ok()));
    let mut hasher = DefaultHasher::new();
    meta.hash(&mut hasher);
    hasher.finish()
}

/// The schedules due at `now`, as cedia decides it: on, with their next time passed. Times are
/// cedia's `toISOString`, which sort as text.
fn any_due(list: &[Schedule], now: &str) -> bool {
    list.iter()
        .any(|s| s.enabled && s.next.as_deref().is_some_and(|next| next <= now))
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// The servers under `mcpServers` in cedia's settings, by name. A file cedian cannot read
/// lists none: cedia reports its own settings errors.
pub fn mcp_servers(settings: &Path) -> Vec<McpServer> {
    let Some(Value::Object(servers)) = std::fs::read_to_string(settings)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|mut file| file.get_mut("mcpServers").map(Value::take))
    else {
        return Vec::new();
    };
    let mut list: Vec<McpServer> = servers
        .into_iter()
        .filter_map(|(name, server)| {
            let transport = match (server.get("command"), server.get("url")) {
                (Some(Value::String(command)), None) => McpTransport::Command {
                    program: Path::new(command)
                        .file_name()
                        .map_or_else(|| command.clone(), |n| n.to_string_lossy().into_owned()),
                },
                (None, Some(Value::String(url))) => McpTransport::Url { host: host_of(url) },
                _ => return None,
            };
            Some(McpServer {
                name,
                transport,
                read_only: server.get("readOnly") == Some(&Value::Bool(true)),
            })
        })
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

/// `https://user:key@host:8080/path?q` as `host:8080`: never what could carry a key.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host)
        .to_owned()
}

impl Daemon {
    fn can_schedule(&self) -> bool {
        self.version
            .as_ref()
            .is_ok_and(|v| v.capabilities.contains(&Capability::Schedules))
    }

    fn background_notice(&self, cwd: &Path, slug: &str) -> Notice {
        Notice::Background {
            cwd: cwd.to_owned(),
            schedules: self
                .can_schedule()
                .then(|| self.schedules.list(slug).cloned().unwrap_or_default()),
            mcp: mcp_servers(&settings_path(&self.home)),
        }
    }

    /// Sends the project's background to each window that shows it.
    fn tell_background(&mut self, slug: &str) {
        let to: Vec<(ClientId, PathBuf)> = self
            .clients
            .iter()
            .flat_map(|(id, client)| {
                client
                    .background
                    .iter()
                    .filter(|(_, key)| key.as_str() == slug)
                    .map(|(cwd, _)| (*id, cwd.clone()))
            })
            .collect();
        for (id, cwd) in to {
            let notice = self.background_notice(&cwd, slug);
            self.send(id, notice);
        }
    }

    /// Lists the project's schedules again and tells the windows that show them.
    fn relist(shared: &Shared, cwd: &Path) {
        let slug = project_key(cwd);
        let (cedia, path) = {
            let daemon = lock(shared);
            (daemon.cedia.clone(), schedules_path(&daemon.home, &slug))
        };
        // Taken before listing, so a change made while cedia reads is seen at the next poll.
        let at = print(&path);
        let list = match cedia.schedules(cwd) {
            Ok(list) => list,
            Err(error) => {
                log::warn!("{error}");
                return;
            }
        };
        let mut daemon = lock(shared);
        daemon
            .schedules
            .0
            .insert(slug.clone(), Listed { print: at, list });
        daemon.tell_background(&slug);
    }

    pub(super) fn background(shared: &Shared, client: ClientId, cwd: PathBuf) {
        let slug = project_key(&cwd);
        let listed = {
            let mut daemon = lock(shared);
            if let Some(c) = daemon.clients.get_mut(&client) {
                c.background.insert(cwd.clone(), slug.clone());
            }
            !daemon.can_schedule() || daemon.schedules.list(&slug).is_some()
        };
        if listed {
            let mut daemon = lock(shared);
            let notice = daemon.background_notice(&cwd, &slug);
            daemon.send(client, notice);
        } else {
            Self::relist(shared, &cwd);
        }
    }

    pub(super) fn set_schedule(shared: &Shared, client: ClientId, cwd: &Path, id: &str, on: bool) {
        let cedia = {
            let daemon = lock(shared);
            if !daemon.can_schedule() {
                return;
            }
            daemon.cedia.clone()
        };
        let verb = if on { "resume" } else { "pause" };
        match cedia.schedule(cwd, verb, Some(id)) {
            Ok(_) => Self::relist(shared, cwd),
            Err(error) => lock(shared).error(client, error),
        }
    }

    /// Each poll: lists again the schedules of each project a window shows whose file moved,
    /// and wakes those that are due.
    pub fn wake_schedules(shared: &Shared) {
        let (stale, due) = {
            let daemon = lock(shared);
            if !daemon.can_schedule() {
                return;
            }
            let mut projects: Vec<(PathBuf, String)> = daemon
                .clients
                .values()
                .flat_map(|c| {
                    c.watching
                        .iter()
                        .map(|(cwd, slug)| (cwd.clone(), slug.clone()))
                })
                .collect();
            projects.sort();
            projects.dedup_by(|a, b| a.1 == b.1);
            let now = now();
            let mut stale = Vec::new();
            let mut due = Vec::new();
            for (cwd, slug) in projects {
                let path = schedules_path(&daemon.home, &slug);
                match daemon.schedules.0.get(&slug) {
                    Some(listed) if listed.print == print(&path) => {
                        if any_due(&listed.list, &now) {
                            due.push(cwd);
                        }
                    }
                    _ => stale.push(cwd),
                }
            }
            (stale, due)
        };
        for cwd in stale {
            Self::relist(shared, &cwd);
        }
        for cwd in due {
            Self::fire_due(shared, &cwd);
        }
    }

    /// Starts each schedule cedia says is due in the project: marks it fired, then runs its
    /// prompt on its thread.
    fn fire_due(shared: &Shared, cwd: &Path) {
        let cedia = lock(shared).cedia.clone();
        let due = match cedia.schedule(cwd, "due", None) {
            Ok(due) => due,
            Err(error) => return log::warn!("{error}"),
        };
        for schedule in due {
            if let Err(error) = cedia.schedule(cwd, "fire", Some(&schedule.id)) {
                log::warn!("{error}");
                continue;
            }
            log::info!("schedule {} wakes {}", schedule.id, schedule.thread);
            let running = lock(shared).running(&schedule.thread);
            if running {
                if let Err(error) = cedia.send(cwd, &schedule.thread, &schedule.prompt) {
                    log::warn!("{error:#}");
                }
            } else {
                Self::prompt(
                    shared,
                    None,
                    cwd.to_owned(),
                    cwd.to_owned(),
                    &schedule.prompt,
                    Some(schedule.thread),
                    Flags::default(),
                );
            }
        }
        Self::relist(shared, cwd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule(enabled: bool, next: Option<&str>) -> Schedule {
        serde_json::from_value(serde_json::json!({
            "id": "s-1", "prompt": "p", "thread": "t-1", "enabled": enabled,
            "next": next, "createdAt": "2026-10-22T00:00:00.000Z"
        }))
        .expect("schedule")
    }

    #[test]
    fn only_a_schedule_that_is_on_and_past_its_time_is_due() {
        let now = "2026-10-22T10:00:00.000Z";
        assert!(any_due(
            &[schedule(true, Some("2026-10-22T09:59:59.000Z"))],
            now
        ));
        assert!(any_due(&[schedule(true, Some(now))], now));
        assert!(!any_due(
            &[schedule(true, Some("2026-10-22T10:00:01.000Z"))],
            now
        ));
        assert!(!any_due(
            &[schedule(false, Some("2026-10-22T09:00:00.000Z"))],
            now
        ));
        // A one-off that ran has no next time.
        assert!(!any_due(&[schedule(true, None)], now));
    }

    #[test]
    fn a_server_is_named_by_its_program_or_host_and_never_its_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"mcpServers": {
                "zeta": {"url": "https://me:secret@mcp.example.com:8443/v1?token=x"},
                "github": {"command": "/opt/bin/github-mcp", "args": ["--key", "k"], "readOnly": true},
                "broken": {"args": []}
            }}"#,
        )
        .expect("write");
        assert_eq!(
            mcp_servers(&path),
            vec![
                McpServer {
                    name: "github".into(),
                    transport: McpTransport::Command {
                        program: "github-mcp".into()
                    },
                    read_only: true,
                },
                McpServer {
                    name: "zeta".into(),
                    transport: McpTransport::Url {
                        host: "mcp.example.com:8443".into()
                    },
                    read_only: false,
                },
            ]
        );
        assert!(mcp_servers(&dir.path().join("missing.json")).is_empty());
    }
}
