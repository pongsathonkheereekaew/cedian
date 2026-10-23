//! A thread written out as Markdown (phase C, export): the conversation as the agent pane's
//! คำตอบ mode reads it, each run of tool calls as a list, and each receipt as a line. Thinking
//! and sub-threads stay out; the file is for reading and sharing, and cedia's own files keep
//! the whole record.

use cedian_agent::Block;

use crate::{receipt, tool_card};

pub fn markdown(title: &str, id: &str, blocks: &[Block]) -> String {
    let mut out = format!("# {title}\n\n`{id}`\n");
    // Whether the last thing written was a tool line, so a run of calls stays one list.
    let mut in_tools = false;
    for block in blocks {
        let tool = matches!(block, Block::Tool(_));
        match block {
            Block::Session { model, role } => {
                out.push_str(&format!("\n---\n\n*session · {role} · {model}*\n"));
            }
            Block::User { text, images } => {
                out.push_str("\n**คุณ**\n\n");
                for image in images {
                    let name = image.name.as_deref().unwrap_or(&image.sha256);
                    out.push_str(&format!("> *รูป {name}*\n>\n"));
                }
                for line in text.lines() {
                    out.push_str(&format!("> {line}\n").replace("> \n", ">\n"));
                }
            }
            Block::Text { text, .. } => {
                if !text.trim().is_empty() {
                    out.push_str(&format!("\n{}\n", text.trim_end()));
                }
            }
            Block::Thinking { .. } => {}
            Block::Tool(call) => {
                if !in_tools {
                    out.push('\n');
                }
                let mut line = format!("- `{}` {}", call.name, tool_card::argument(call));
                if let Some(outcome) = tool_card::outcome(call) {
                    line.push_str(&format!(" · {outcome}"));
                }
                if call.failed() && call.exit_code.is_none() {
                    line.push_str(" · ล้มเหลว");
                }
                out.push_str(line.trim_end());
                out.push('\n');
            }
            Block::Result {
                subtype,
                turns,
                cost_usd,
                evidence,
                ..
            } => {
                let verified = if block.verified() {
                    "verified"
                } else {
                    "unverified"
                };
                let turns = match turns {
                    1 => "1 turn".to_owned(),
                    n => format!("{n} turns"),
                };
                out.push_str(&format!(
                    "\n> **{}** · {verified} · {turns} · ${cost_usd:.4}\n",
                    receipt::verdict(*subtype)
                ));
                if let Some(evidence) = evidence
                    && !evidence.changed.is_empty()
                {
                    out.push_str(&format!(
                        ">\n> ไฟล์ที่เปลี่ยน: {}\n",
                        evidence.changed.join(", ")
                    ));
                }
            }
        }
        in_tools = tool;
    }
    out
}

/// A file name for the export: the title's words, cut at spaces and ASCII punctuation, so a
/// Thai word keeps its tone marks; `thread` when it has none.
pub fn file_name(title: &str) -> String {
    let words: Vec<String> = title
        .split(|c: char| c.is_whitespace() || (c.is_ascii() && !c.is_ascii_alphanumeric()))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let mut name = words.join("-");
    if let Some((at, _)) = name.char_indices().nth(60) {
        name.truncate(at);
    }
    if name.is_empty() {
        name = "thread".into();
    }
    format!("{name}.md")
}

#[cfg(test)]
mod tests {
    use cedian_agent::Transcript;
    use cedian_protocol::{Line, fixtures_dir};

    use super::*;

    #[test]
    fn a_recorded_run_reads_as_markdown() {
        let stream = std::fs::read_to_string(fixtures_dir().join("edit-and-check/stream.jsonl"))
            .expect("stream");
        let mut transcript = Transcript::default();
        for text in stream.lines() {
            transcript.apply(&Line::parse(text).expect("line"));
        }
        let md = markdown("make f.txt say good", "t-1", transcript.blocks());
        assert!(md.starts_with("# make f.txt say good\n\n`t-1`\n"), "{md}");
        assert!(md.contains("**คุณ**\n\n> "), "{md}");
        assert!(md.contains("\n- `edit` "), "{md}");
        assert!(md.contains("> **เสร็จ** · verified"), "{md}");
        assert!(!md.contains("\n\n\n"), "{md}");
    }

    #[test]
    fn file_names_keep_letters_of_any_script() {
        assert_eq!(file_name("Fix the bug: login!"), "fix-the-bug-login.md");
        assert_eq!(file_name("แก้ บั๊ก/ด่วน"), "แก้-บั๊ก-ด่วน.md");
        assert_eq!(file_name("???"), "thread.md");
    }
}
