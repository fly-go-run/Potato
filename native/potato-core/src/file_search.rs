//! Bounded native search with deterministic ordering and capability-scoped I/O.
use crate::{context::head, required, Error, Result};
use cap_std::{ambient_authority, fs::Dir};
use serde_json::{json, Value};
use std::{io::Read, path::Path};
use tokio_util::sync::CancellationToken;

pub(crate) fn search(
    root: &Path,
    private: &Path,
    workspace: &Path,
    name: &str,
    args: &Value,
    cancel: &CancellationToken,
) -> Result<String> {
    let pattern = required(args, "pattern")?;
    if pattern.len() > 4096 {
        return Err(Error::new(400, "Search pattern exceeds 4096 bytes"));
    }
    let glob = if name == "glob_search" {
        Some(
            globset::GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map_err(|_| Error::new(400, "Invalid glob pattern"))?
                .compile_matcher(),
        )
    } else {
        None
    };
    let expression = if args["literal"] == true {
        regex::escape(pattern)
    } else {
        pattern.to_owned()
    };
    let regex = if name == "grep_search" {
        Some(
            regex::RegexBuilder::new(&expression)
                .case_insensitive(args["case_sensitive"] == false)
                .size_limit(2_000_000)
                .build()
                .map_err(|_| Error::new(400, "Invalid or overly complex search expression"))?,
        )
    } else {
        None
    };
    let offset = args["offset"].as_u64().unwrap_or(0) as usize;
    let limit = args["limit"].as_u64().unwrap_or(50).clamp(1, 200) as usize;
    let root_dir = Dir::open_ambient_dir(root, ambient_authority())?;
    let mut stack = vec![(root_dir, std::path::PathBuf::new())];
    let mut found = Vec::new();
    let mut matched = 0usize;
    let mut visited = 0usize;
    let mut skipped = 0usize;
    let mut capped = false;
    'walk: while let Some((dir, relative)) = stack.pop() {
        if cancel.is_cancelled() {
            return Err(Error::new(499, "Search cancelled"));
        }
        let mut entries = dir.entries()?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|e| e.file_name());
        let mut subdirs = Vec::new();
        for entry in entries {
            if cancel.is_cancelled() {
                return Err(Error::new(499, "Search cancelled"));
            }
            visited += 1;
            if visited > 20_000 {
                capped = true;
                break 'walk;
            }
            let leaf = entry.file_name();
            let path = relative.join(&leaf);
            let absolute = root.join(&path);
            let kind = entry.file_type()?;
            // Traverse the container to discover public memory/artifacts, but
            // filter each child; .potato/config and nested secrets stay hidden.
            let memory_container = leaf.to_string_lossy().eq_ignore_ascii_case(".potato")
                && kind.is_dir()
                && !crate::approval::sensitive(&relative);
            if (crate::approval::sensitive(&path) && !memory_container)
                || (absolute.starts_with(private) && !absolute.starts_with(workspace))
            {
                skipped += 1;
                continue;
            }
            if kind.is_symlink() {
                skipped += 1;
                continue;
            }
            if kind.is_dir() {
                if !matches!(
                    leaf.to_str(),
                    Some(".git" | "node_modules" | "target" | ".venv")
                ) {
                    if let Ok(child) = dir.open_dir(&leaf) {
                        subdirs.push((child, path));
                    }
                }
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            if let Some(glob) = &glob {
                if glob.is_match(&path) {
                    if matched >= offset {
                        found.push(json!({"path":path}));
                    }
                    matched += 1;
                }
            } else if let Some(regex) = &regex {
                let Ok(file) = dir.open(&leaf) else {
                    skipped += 1;
                    continue;
                };
                let mut bytes = Vec::new();
                file.take(2_000_001).read_to_end(&mut bytes)?;
                if bytes.len() > 2_000_000 || bytes.contains(&0) {
                    skipped += 1;
                    continue;
                }
                let Ok(text) = std::str::from_utf8(&bytes) else {
                    skipped += 1;
                    continue;
                };
                for (line, text) in text.lines().enumerate() {
                    if let Some(hit) = regex.find(text) {
                        if matched >= offset {
                            let mut begin = hit.start().saturating_sub(120);
                            while !text.is_char_boundary(begin) {
                                begin -= 1;
                            }
                            found.push(json!({"path":path,"line":line+1,"text":head(&text[begin..],800),"excerpt":begin>0 || text.len()>800}));
                        }
                        matched += 1;
                        if found.len() > limit {
                            break;
                        }
                    }
                }
            }
            if found.len() > limit {
                break 'walk;
            }
        }
        stack.extend(subdirs.into_iter().rev());
    }
    let more = found.len() > limit;
    found.truncate(limit);
    Ok(json!({"root":root,"matches":found,"next_offset":if more{Some(offset+limit)}else{None},"scan_capped":capped,"visited":visited.min(20_000),"skipped_files":skipped,"notice":"Search skips sensitive paths (credentials and agent instructions), symlinks, .git/node_modules/target/.venv and binary/non-UTF8 files; content search skips files over 2 MB. Narrow the root when scan_capped is true."}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regex_glob_paging_and_private_paths() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a.rs"), "你好 one\n你好 two\nother").unwrap();
        std::fs::create_dir(root.path().join("private")).unwrap();
        std::fs::write(root.path().join("private/key"), "你好 secret").unwrap();
        let run = |name, args| -> Value {
            serde_json::from_str(
                &search(
                    root.path(),
                    &root.path().join("private"),
                    &root.path().join("workspace"),
                    name,
                    &args,
                    &CancellationToken::new(),
                )
                .unwrap(),
            )
            .unwrap()
        };
        let a = run("grep_search", json!({"pattern":"你好","limit":1}));
        assert_eq!(a["matches"][0]["line"], 1);
        assert_eq!(a["next_offset"], 1);
        let b = run(
            "grep_search",
            json!({"pattern":"你好","limit":1,"offset":1}),
        );
        assert_eq!(b["matches"][0]["line"], 2);
        assert!(b["next_offset"].is_null());
        assert_eq!(
            run("glob_search", json!({"pattern":"**/*.rs"}))["matches"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}
