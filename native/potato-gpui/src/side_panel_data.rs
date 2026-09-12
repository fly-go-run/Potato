//! Read-only conversation/file projections. No tool execution or Git mutation.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    time::Duration,
};

pub const MAX_BYTES: usize = 2_000_000;
pub const MAX_LINES: usize = 1500;
const MAX_LINE_CHARS: usize = 4000;
fn display_line(text: &str) -> String {
    match text.char_indices().nth(MAX_LINE_CHARS) {
        Some((end, _)) => format!("{}… [长行已截断]", &text[..end]),
        None => text.into(),
    }
}
#[derive(Clone, Debug, Default)]
pub struct FileEntry {
    pub path: PathBuf,
    pub source: usize,
    pub edits: Vec<Edit>,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub binary: bool,
    pub additions: usize,
    pub deletions: usize,
}
#[derive(Clone, Debug)]
pub struct Edit {
    pub tool: String,
    pub before: String,
    pub after: String,
}
#[derive(Default)]
pub struct ConversationFiles {
    pub changes: Vec<FileEntry>,
    pub artifacts: Vec<FileEntry>,
}
#[derive(Clone, Debug)]
pub struct GitFiles {
    pub root: PathBuf,
    pub branch: String,
    pub entries: Vec<FileEntry>,
}
#[derive(Clone, Debug)]
pub struct Line {
    pub old: Option<usize>,
    pub new: Option<usize>,
    pub kind: char,
    pub text: String,
}
#[derive(Clone, Debug)]
pub struct DiffSection {
    pub label: String,
    pub lines: Vec<Line>,
}
#[derive(Clone, Debug)]
pub enum Preview {
    Text {
        text: String,
        markdown: bool,
        lines: Vec<CodeLine>,
        truncated: bool,
    },
    Image {
        bytes: Vec<u8>,
        extension: String,
    },
    External(String),
    Diff(Vec<DiffSection>),
}
#[derive(Clone, Debug)]
pub struct CodeLine {
    pub text: String,
    pub highlights: Vec<(std::ops::Range<usize>, u32)>,
}

pub fn resolve_path(raw: &str, project: &Path) -> Option<PathBuf> {
    let lower = raw.to_ascii_lowercase();
    let raw = if lower.starts_with("file:") || lower.starts_with("sandbox:") {
        let raw = raw.split_once(':')?.1.split(['?', '#']).next()?;
        let raw = if raw.starts_with("///") {
            &raw[2..]
        } else {
            raw
        };
        // Do not interpret a file URI authority as a local path.
        if raw.starts_with("//") {
            return None;
        }
        percent_encoding::percent_decode_str(raw)
            .decode_utf8()
            .ok()?
            .into_owned()
    } else {
        if raw.contains("://") || raw.starts_with('#') {
            return None;
        }
        raw.to_owned()
    };
    if raw.is_empty() || raw.contains('\0') {
        return None;
    }
    #[cfg(target_os = "windows")]
    let raw = if raw.starts_with('/')
        && raw.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic)
        && raw.as_bytes().get(2) == Some(&b':')
    {
        raw.trim_start_matches('/').to_owned()
    } else {
        raw
    };
    let path = PathBuf::from(raw);
    #[cfg(windows)]
    if matches!(
        path.components().next(),
        Some(std::path::Component::Prefix(_))
    ) && !path.is_absolute()
    {
        // C:report.txt depends on the process's per-drive working directory.
        return None;
    }
    if !path.is_absolute() && project.as_os_str().is_empty() {
        return None;
    }
    let path = if path.is_absolute() || (path.has_root() && !project.is_absolute()) {
        path
    } else {
        project.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Some(normalized)
}

/// Link syntax may encode spaces or include an anchor; tool-provided paths must
/// remain literal (a real filename may contain '#' or '%').
pub fn resolve_link(raw: &str, project: &Path) -> Option<PathBuf> {
    if raw.to_ascii_lowercase().starts_with("file:")
        || raw.to_ascii_lowercase().starts_with("sandbox:")
    {
        return resolve_path(raw, project);
    }
    let raw = raw.split(['?', '#']).next()?;
    let decoded = percent_encoding::percent_decode_str(raw)
        .decode_utf8()
        .ok()?;
    resolve_path(&decoded, project)
}

pub fn links(text: &str) -> Vec<String> {
    fn visit(node: &markdown::mdast::Node, out: &mut Vec<String>, defs: &BTreeMap<String, String>) {
        match node {
            markdown::mdast::Node::Link(link) => out.push(link.url.clone()),
            markdown::mdast::Node::LinkReference(link) => {
                if let Some(url) = defs.get(&link.identifier) {
                    out.push(url.clone());
                }
            }
            _ => {}
        }
        if let Some(children) = node.children() {
            for child in children {
                visit(child, out, defs);
            }
        }
    }
    fn definitions(node: &markdown::mdast::Node, out: &mut BTreeMap<String, String>) {
        if let markdown::mdast::Node::Definition(def) = node {
            out.insert(def.identifier.clone(), def.url.clone());
        }
        if let Some(children) = node.children() {
            for child in children {
                definitions(child, out);
            }
        }
    }
    let mut out = vec![];
    // The conversation already bounds individual messages; avoid parsing giant archives.
    if text.len() > MAX_BYTES {
        return out;
    }
    if let Ok(root) = markdown::to_mdast(text, &markdown::ParseOptions::default()) {
        let mut defs = BTreeMap::new();
        definitions(&root, &mut defs);
        visit(&root, &mut out, &defs);
    }
    out
}

pub fn collect(messages: &[Value], project: &Path) -> ConversationFiles {
    let mut changes: BTreeMap<PathBuf, FileEntry> = BTreeMap::new();
    let mut delivered: BTreeMap<PathBuf, FileEntry> = BTreeMap::new();
    for (index, call) in crate::chat::presentation(messages) {
        let data = &call["content"][0]["data"];
        let result = &call["tool_result"];
        let state = result["content"][0]["data"]["state"].as_str().unwrap_or("");
        if result.is_null()
            || result["status"] == "failed"
            || matches!(state, "error" | "failed" | "cancelled")
        {
            continue;
        }
        if result["status"] != "completed" && state != "success" {
            continue;
        }
        let output = result["content"][0]["data"]["output"]
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .unwrap_or(Value::Null);
        if output["is_error"] == true || output["success"] == false || output["error"].is_string() {
            continue;
        }
        let args = data["arguments"]
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .unwrap_or_else(|| data["arguments"].clone());
        let tool = data["name"].as_str().unwrap_or("");
        let raw = output["path"]
            .as_str()
            .or_else(|| output["file_path"].as_str())
            .or_else(|| args["file_path"].as_str())
            .or_else(|| args["path"].as_str());
        let Some(path) = raw.and_then(|p| resolve_path(p, project)) else {
            continue;
        };
        if tool == "send_file_to_user" {
            delivered.insert(
                path.clone(),
                FileEntry {
                    path,
                    source: index,
                    ..Default::default()
                },
            );
        } else if matches!(tool, "write_file" | "edit_file" | "append_file") {
            let entry = changes.entry(path.clone()).or_insert_with(|| FileEntry {
                path,
                source: index,
                ..Default::default()
            });
            entry.source = index;
            let before = args["old_text"].as_str().unwrap_or("").to_owned();
            let after = args[if tool == "edit_file" {
                "new_text"
            } else {
                "content"
            }]
            .as_str()
            .unwrap_or("")
            .to_owned();
            // This is an edit-fragment count, not a net working-tree diff.
            entry.additions += after.lines().count();
            entry.deletions += before.lines().count();
            entry.edits.push(Edit {
                tool: tool.into(),
                before,
                after,
            });
        }
    }
    for (source, message) in messages.iter().enumerate().filter(|(_, m)| {
        m["role"] == "assistant" && matches!(m["type"].as_str(), None | Some("message" | "result"))
    }) {
        let text = crate::view::message_text(message);
        for url in links(&text) {
            // HTTP links and images are not local delivered files. Plain absolute/relative
            // Markdown paths are supported too, including the native Office tool's links.
            if let Some(path) = resolve_link(&url, project) {
                if !url.starts_with("file:")
                    && !url.starts_with("sandbox:")
                    && !Path::new(&url).is_absolute()
                    && !changes.contains_key(&path)
                {
                    continue;
                }
                delivered.entry(path.clone()).or_insert(FileEntry {
                    path,
                    source,
                    ..Default::default()
                });
            }
        }
    }
    let mut artifacts: Vec<_> = delivered.into_values().collect();
    artifacts.sort_by_key(|e| std::cmp::Reverse(e.source));
    ConversationFiles {
        changes: changes.into_values().collect(),
        artifacts,
    }
}

/// Bound stdout and runtime, disable external diff/textconv hooks. Paths are argv,
/// never shell source. Git is only queried after an explicit panel open/refresh.
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .arg("--no-pager")
        .arg("-C")
        .arg(root)
        .args(["-c", "core.quotepath=false", "-c", "core.fsmonitor=false"])
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("无法运行 Git：{e}"))?;
    let stdout = child.stdout.take().ok_or("无法读取 Git 输出")?;
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut bytes = vec![];
        let result = stdout
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|e| e.to_string());
        let _ = tx.send(result);
    });
    let result = rx.recv_timeout(Duration::from_secs(8));
    let bytes = match result {
        Ok(Ok(bytes)) if bytes.len() <= MAX_BYTES => bytes,
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("Git 输出过大或查询超时，请在系统工具中查看".into());
        }
    };
    let _ = reader.join();
    // stdout closed normally; bound the remaining process lifetime too.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(bytes)
                } else {
                    Err("Git 查询失败：目录可能不是仓库或已不可访问".into())
                };
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Git 查询超时".into());
            }
        }
    }
}

pub fn git_files(project: &Path) -> Result<GitFiles, String> {
    if project.as_os_str().is_empty() {
        return Err("请先选择项目".into());
    }
    let root = String::from_utf8(git(project, &["rev-parse", "--show-toplevel"])?)
        .map_err(|_| "Git 路径不是 UTF-8")?;
    let root = PathBuf::from(root.trim_end_matches(['\r', '\n']));
    let branch = git(&root, &["rev-parse", "--abbrev-ref", "HEAD"])
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_else(|| "新仓库".into())
        .trim()
        .to_owned();
    let bytes = git(
        &root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--no-renames",
            "--untracked-files=all",
        ],
    )?;
    let mut entries = vec![];
    for record in bytes.split(|b| *b == 0).filter(|r| r.len() > 3) {
        let path = std::str::from_utf8(&record[3..])
            .map_err(|_| "存在非 UTF-8 文件名，请在系统工具中查看")?;
        entries.push(FileEntry {
            path: root.join(path),
            staged: !matches!(record[0], b' ' | b'?'),
            unstaged: record[1] != b' ',
            untracked: &record[..2] == b"??",
            ..Default::default()
        });
    }
    for cached in [false, true] {
        let mut args = vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--numstat",
            "-z",
        ];
        if cached {
            args.push("--cached");
        }
        let stats = git(&root, &args)?;
        for row in stats.split(|b| *b == 0) {
            let Ok(row) = std::str::from_utf8(row) else {
                continue;
            };
            let mut parts = row.splitn(3, '\t');
            let added = parts.next().unwrap_or("");
            let binary = added == "-";
            let add = added.parse::<usize>().unwrap_or(0);
            let del = parts
                .next()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            if let Some(path) = parts.next()
                && let Some(entry) = entries.iter_mut().find(|e| e.path == root.join(path))
            {
                entry.binary |= binary;
                entry.additions += add;
                entry.deletions += del;
            }
        }
    }
    Ok(GitFiles {
        root,
        branch,
        entries,
    })
}

pub fn unified_lines(text: &str) -> Vec<Line> {
    let (mut old, mut new) = (0, 0);
    let mut in_hunk = false;
    let mut lines = vec![];
    for text in text.lines() {
        if lines.len() >= MAX_LINES {
            lines.push(Line {
                old: None,
                new: None,
                kind: '@',
                text: "… 后续差异已截断，请在系统工具中查看".into(),
            });
            break;
        }
        let kind = text.chars().next().unwrap_or(' ');
        if text.starts_with("@@ ") {
            let mut parts = text.split_whitespace();
            parts.next();
            let start = |s: &str| {
                s.get(1..)
                    .unwrap_or("")
                    .split(',')
                    .next()
                    .unwrap_or("")
                    .parse()
                    .unwrap_or(0)
            };
            old = parts.next().map(start).unwrap_or(0);
            new = parts.next().map(start).unwrap_or(0);
            in_hunk = true;
            lines.push(Line {
                old: None,
                new: None,
                kind: '@',
                text: text.into(),
            });
        } else if in_hunk && matches!(kind, '+' | '-' | ' ') {
            lines.push(Line {
                old: (kind != '+').then_some(old),
                new: (kind != '-').then_some(new),
                kind,
                text: display_line(&text[1..]),
            });
            if kind != '+' {
                old += 1;
            }
            if kind != '-' {
                new += 1;
            }
        } else if text.starts_with("Binary files")
            || text.starts_with("new file mode")
            || text.starts_with("deleted file mode")
            || text.starts_with("old mode")
            || text.starts_with("new mode")
        {
            lines.push(Line {
                old: None,
                new: None,
                kind: '@',
                text: text.into(),
            });
        }
    }
    lines
}

pub fn read_diff(root: &Path, entry: &FileEntry) -> Result<Preview, String> {
    let relative = entry
        .path
        .strip_prefix(root)
        .map_err(|_| "文件不属于此仓库")?
        .to_str()
        .ok_or("无效路径")?;
    // The file may have been staged, reverted, or removed since the list loaded.
    let status = git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--no-renames",
            "--untracked-files=all",
            "--",
            relative,
        ],
    )?;
    let Some(record) = status.split(|b| *b == 0).find(|r| r.len() > 3) else {
        return Ok(Preview::Diff(vec![DiffSection {
            label: "工作区当前改动".into(),
            lines: vec![],
        }]));
    };
    if record[..2].contains(&b'U') || &record[..2] == b"AA" || &record[..2] == b"DD" {
        return Ok(Preview::External(
            "此文件存在合并冲突，请查看文件当前内容或使用系统 Git 工具处理".into(),
        ));
    }
    let entry = FileEntry {
        staged: !matches!(record[0], b' ' | b'?'),
        unstaged: record[1] != b' ',
        untracked: &record[..2] == b"??",
        ..entry.clone()
    };
    let mut sections = vec![];
    if entry.untracked {
        match read_preview(&entry.path, false)? {
            Preview::Text { text, .. } => {
                let mut lines: Vec<_> = text
                    .lines()
                    .take(MAX_LINES)
                    .enumerate()
                    .map(|(i, t)| Line {
                        old: None,
                        new: Some(i + 1),
                        kind: '+',
                        text: display_line(t),
                    })
                    .collect();
                if text.lines().count() > MAX_LINES {
                    lines.push(Line {
                        old: None,
                        new: None,
                        kind: '@',
                        text: "… 后续行已截断".into(),
                    });
                }
                sections.push(DiffSection {
                    label: "未跟踪文件".into(),
                    lines,
                });
            }
            _ => sections.push(DiffSection {
                label: "未跟踪文件".into(),
                lines: vec![Line {
                    old: None,
                    new: None,
                    kind: '@',
                    text: "二进制或大文件，请打开文件预览或使用系统应用".into(),
                }],
            }),
        }
    } else {
        for (cached, enabled) in [(false, entry.unstaged), (true, entry.staged)] {
            if !enabled {
                continue;
            }
            let mut args = vec![
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                "--no-color",
                "--unified=3",
            ];
            if cached {
                args.push("--cached");
            }
            args.extend(["--", relative]);
            let output = String::from_utf8_lossy(&git(root, &args)?).into_owned();
            sections.push(DiffSection {
                label: if cached { "已暂存" } else { "未暂存" }.into(),
                lines: unified_lines(&output),
            });
        }
    }
    Ok(Preview::Diff(sections))
}

pub fn edit_preview(entry: &FileEntry) -> Preview {
    let mut remaining = MAX_LINES;
    let mut sections = vec![];
    for (i, edit) in entry.edits.iter().enumerate() {
        if remaining == 0 {
            break;
        }
        let mut lines = vec![];
        for (kind, text) in [('-', &edit.before), ('+', &edit.after)] {
            for text in text.lines() {
                if remaining == 0 {
                    break;
                }
                remaining -= 1;
                lines.push(Line {
                    old: None,
                    new: None,
                    kind,
                    text: display_line(text),
                });
            }
        }
        if remaining == 0 {
            lines.push(Line {
                old: None,
                new: None,
                kind: '@',
                text: "… 达到预览上限".into(),
            });
        }
        let label = match edit.tool.as_str() {
            "write_file" => "写入内容（未记录原文件）",
            "append_file" => "追加内容",
            _ => "替换片段（不代表整个文件差异）",
        };
        sections.push(DiffSection {
            label: format!("{} · {label}", i + 1),
            lines,
        });
    }
    Preview::Diff(sections)
}

pub fn read_preview(path: &Path, dark: bool) -> Result<Preview, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("无法读取文件：{e}"))?;
    if !meta.is_file() {
        return Err("此路径不是普通文件".into());
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let image = matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif");
    let limit = if image { 20_000_000 } else { MAX_BYTES };
    if meta.len() > limit as u64 {
        return Ok(Preview::External("文件较大，请使用系统应用打开".into()));
    }
    let mut bytes = vec![];
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Ok(Preview::External("文件超出预览上限".into()));
    }
    if image {
        return Ok(Preview::Image { bytes, extension });
    }
    if bytes.contains(&0) {
        return Ok(Preview::External("此格式请使用系统应用打开".into()));
    }
    let Ok(text) = String::from_utf8(bytes) else {
        return Ok(Preview::External(
            "二进制文件或非 UTF-8 文本，请使用系统应用打开".into(),
        ));
    };
    let markdown = matches!(extension.as_str(), "md" | "markdown") && text.len() < 300_000;
    let truncated = text.lines().count() > MAX_LINES
        || text
            .lines()
            .any(|line| line.chars().count() > MAX_LINE_CHARS);
    let syntax = syntax_set();
    let language = syntax
        .find_syntax_by_extension(&extension)
        .unwrap_or_else(|| syntax.find_syntax_plain_text());
    let themes = themes();
    let theme = &themes.themes[if dark {
        "base16-ocean.dark"
    } else {
        "InspiredGitHub"
    }];
    let mut highlighter = syntect::easy::HighlightLines::new(language, theme);
    let highlight = text.len() <= 300_000;
    let lines = text
        .lines()
        .take(MAX_LINES)
        .map(|line| {
            let line = display_line(line);
            let mut offset = 0;
            let highlights = if highlight {
                highlighter
                    .highlight_line(&line, syntax)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(style, token)| {
                        let start = offset;
                        offset += token.len();
                        (
                            start..offset,
                            (u32::from(style.foreground.r) << 16)
                                | (u32::from(style.foreground.g) << 8)
                                | u32::from(style.foreground.b),
                        )
                    })
                    .collect()
            } else {
                vec![]
            };
            CodeLine {
                text: line,
                highlights,
            }
        })
        .collect();
    Ok(Preview::Text {
        text,
        markdown: markdown && !truncated,
        lines,
        truncated,
    })
}
fn syntax_set() -> &'static syntect::parsing::SyntaxSet {
    static SET: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();
    SET.get_or_init(syntect::parsing::SyntaxSet::load_defaults_nonewlines)
}
fn themes() -> &'static syntect::highlighting::ThemeSet {
    static SET: OnceLock<syntect::highlighting::ThemeSet> = OnceLock::new();
    SET.get_or_init(syntect::highlighting::ThemeSet::load_defaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("potato-panel-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn pair(id: &str, path: &str, ok: bool) -> Vec<Value> {
        vec![
            json!({"type":"function_call","content":[{"data":{"call_id":id,"name":"write_file","arguments":json!({"file_path":path,"content":"hello\nworld"}).to_string()}}]}),
            json!({"type":"function_call_output","status":if ok {"completed"} else {"failed"},"content":[{"data":{"call_id":id,"state":if ok {"success"} else {"error"}}}]}),
        ]
    }
    #[test]
    fn deliveries_exclude_failed_and_intermediate_files_and_deduplicate_links() {
        let temp = Temp::new();
        let mut messages = pair("one", "a.md", true);
        messages.extend(pair("two", "bad.md", false));
        assert_eq!(collect(&messages, &temp.0).changes.len(), 1);
        assert!(collect(&messages, &temp.0).artifacts.is_empty());
        messages.push(json!({"type":"message","role":"assistant","content":[{"type":"text","text":"[report](a.md) [again](a.md) ![image](image.png) [site](https://example.com)"}]}));
        let files = collect(&messages, &temp.0);
        assert_eq!(files.artifacts.len(), 1);
        assert_eq!(files.artifacts[0].path, temp.0.join("a.md"));
        assert_eq!(files.artifacts[0].source, 4);
    }
    #[test]
    fn file_links_handle_spaces_unicode_references_and_ignore_code_images() {
        let text = "[report](<sandbox:/tmp/报告 (1).docx>) [ref][a]\n\n[a]: file:///tmp/a%20b.txt\n\n![x](file:///tmp/image.png) ` [fake](file:///tmp/fake) `\n```\n[fake](file:///tmp/no)\n```";
        let urls = links(text);
        assert_eq!(urls.len(), 2);
        assert_eq!(
            resolve_path(&urls[0], Path::new("/tmp")).unwrap(),
            PathBuf::from("/tmp/报告 (1).docx")
        );
        assert_eq!(
            resolve_path(&urls[1], Path::new("/tmp")).unwrap(),
            PathBuf::from("/tmp/a b.txt")
        );
        assert!(resolve_path("file://host/path", Path::new("/tmp")).is_none());
        assert_eq!(
            resolve_link("/tmp/a%20%231.md#title", Path::new("/tmp")).unwrap(),
            PathBuf::from("/tmp/a #1.md")
        );
        assert_eq!(
            resolve_path("/tmp/a #1.md", Path::new("/tmp")).unwrap(),
            PathBuf::from("/tmp/a #1.md")
        );
        assert!(resolve_path("https://example.com/x", Path::new("/tmp")).is_none());
    }
    #[cfg(windows)]
    #[test]
    fn windows_links_preserve_drive_unicode_and_project_root() {
        let project = Path::new(r"C:\work\项目");
        for (raw, expected) in [
            (
                "file:///C:/work/%E6%8A%A5%E5%91%8A.txt",
                r"C:\work\报告.txt",
            ),
            (r"C:\work\a #1.txt", r"C:\work\a #1.txt"),
            ("/reports/a.txt", r"C:\reports\a.txt"),
            ("../a.txt", r"C:\work\a.txt"),
        ] {
            assert_eq!(resolve_path(raw, project).unwrap(), PathBuf::from(expected));
        }
        assert!(resolve_path("C:relative.txt", project).is_none());
        assert!(resolve_path("file://server/share/a.txt", project).is_none());
    }
    #[test]
    fn duplicate_call_ids_do_not_cross_user_turns() {
        let temp = Temp::new();
        let mut messages = pair("same", "a.txt", true);
        messages.pop();
        messages.push(json!({"role":"user","type":"message"}));
        messages.push(pair("same", "a.txt", true).pop().unwrap());
        assert!(collect(&messages, &temp.0).changes.is_empty());
    }
    #[test]
    fn git_diff_retains_old_new_numbers_and_does_not_confuse_content_headers() {
        let lines = unified_lines(
            "--- a/a\n+++ b/a\n@@ -3,2 +3,3 @@ fn\n context\n-old\n+new\n+++content\n",
        );
        assert_eq!((lines[1].old, lines[1].new), (Some(3), Some(3)));
        assert_eq!((lines[2].old, lines[2].new), (Some(4), None));
        assert_eq!((lines[4].old, lines[4].new), (None, Some(5)));
        assert_eq!(lines[4].text, "++content");
    }
    fn run(root: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
    }
    #[test]
    fn real_git_separates_staged_unstaged_untracked_and_deleted_files() {
        let temp = Temp::new();
        run(&temp.0, &["init", "-q"]);
        std::fs::write(temp.0.join("a space.txt"), "base\n").unwrap();
        std::fs::write(temp.0.join("deleted.txt"), "gone\n").unwrap();
        run(&temp.0, &["add", "."]);
        run(
            &temp.0,
            &[
                "-c",
                "user.name=Panel Test",
                "-c",
                "user.email=panel@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "base",
            ],
        );
        std::fs::write(temp.0.join("a space.txt"), "staged\n").unwrap();
        run(&temp.0, &["add", "a space.txt"]);
        std::fs::write(temp.0.join("a space.txt"), "working\n").unwrap();
        std::fs::write(temp.0.join("新文件.txt"), "new\n").unwrap();
        std::fs::remove_file(temp.0.join("deleted.txt")).unwrap();
        let files = git_files(&temp.0).unwrap();
        assert_eq!(files.entries.len(), 3);
        let entry = files
            .entries
            .iter()
            .find(|e| e.path.ends_with("a space.txt"))
            .unwrap();
        assert!(entry.staged && entry.unstaged);
        assert_eq!((entry.additions, entry.deletions), (2, 2));
        let Preview::Diff(sections) = read_diff(&files.root, entry).unwrap() else {
            panic!()
        };
        assert_eq!(sections.len(), 2);
        assert!(
            sections[0]
                .lines
                .iter()
                .any(|l| l.text == "working" && l.kind == '+')
        );
        assert!(
            sections[1]
                .lines
                .iter()
                .any(|l| l.text == "staged" && l.kind == '+')
        );
        let new = files.entries.iter().find(|e| e.untracked).unwrap();
        let Preview::Diff(sections) = read_diff(&files.root, new).unwrap() else {
            panic!()
        };
        assert_eq!(sections[0].lines[0].new, Some(1));
        let deleted = files
            .entries
            .iter()
            .find(|e| e.path.ends_with("deleted.txt"))
            .unwrap();
        assert!(matches!(
            read_diff(&files.root, deleted).unwrap(),
            Preview::Diff(_)
        ));
        run(&temp.0, &["add", "--", "a space.txt"]);
        let Preview::Diff(refreshed) = read_diff(&files.root, entry).unwrap() else {
            panic!()
        };
        assert_eq!(refreshed.len(), 1);
        assert_eq!(refreshed[0].label, "已暂存");
    }
    #[test]
    fn preview_bounds_large_binary_missing_and_utf8_files() {
        let temp = Temp::new();
        let file = temp.0.join("a.rs");
        std::fs::write(&file, "fn main() { println!(\"中文\"); }\n").unwrap();
        let Preview::Text { lines, .. } = read_preview(&file, false).unwrap() else {
            panic!()
        };
        assert!(!lines[0].highlights.is_empty());
        std::fs::write(&file, [0, 1, 2]).unwrap();
        assert!(matches!(
            read_preview(&file, false).unwrap(),
            Preview::External(_)
        ));
        std::fs::write(&file, vec![b'a'; MAX_BYTES + 1]).unwrap();
        assert!(matches!(
            read_preview(&file, false).unwrap(),
            Preview::External(_)
        ));
        std::fs::remove_file(&file).unwrap();
        assert!(read_preview(&file, false).is_err());
        assert!(git_files(&temp.0).is_err());
    }
    #[test]
    fn long_unicode_lines_are_bounded_without_splitting_characters() {
        let temp = Temp::new();
        let path = temp.0.join("long.txt");
        std::fs::write(&path, "中".repeat(5000)).unwrap();
        let Preview::Text {
            lines, truncated, ..
        } = read_preview(&path, false).unwrap()
        else {
            panic!()
        };
        assert!(truncated);
        assert!(lines[0].text.chars().count() < 4100);
        let diff = unified_lines(&format!("@@ -0,0 +1 @@\n+{}", "中".repeat(5000)));
        assert!(diff[1].text.chars().count() < 4100);
    }
    #[test]
    fn panel_width_preferences_persist_and_reject_invalid_sizes() {
        let temp = Temp::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let core = potato_core::Runtime::open(&temp.0).unwrap();
        runtime.block_on(async {
            core.request(
                "PUT",
                "/api/native/preferences",
                json!({"file_list_width":330,"file_detail_width":590}),
            )
            .await
            .unwrap();
            assert!(
                core.request(
                    "PUT",
                    "/api/native/preferences",
                    json!({"file_list_width":900})
                )
                .await
                .is_err()
            );
        });
        drop(core);
        let reopened = potato_core::Runtime::open(&temp.0).unwrap();
        let saved = runtime
            .block_on(reopened.request("GET", "/api/native/preferences", Value::Null))
            .unwrap();
        assert_eq!(saved["file_list_width"], 330);
        assert_eq!(saved["file_detail_width"], 590);
    }
}
