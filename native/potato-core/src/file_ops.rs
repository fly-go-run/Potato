use crate::{Error, Result};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Component, Path},
};

pub(crate) struct PreparedWrite {
    directory: Dir,
    name: OsString,
    before: Option<Vec<u8>>,
    content: Vec<u8>,
    create_only: bool,
}

/// Template input is limited to ordinary, non-symlink files in the project.
pub(crate) fn read_office_template(project: &Path, target: &Path) -> Result<Vec<u8>> {
    let relative = if target.is_absolute() {
        target
            .strip_prefix(project)
            .map_err(|_| Error::new(403, "Template must be inside the project"))?
    } else {
        target
    };
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
        || crate::approval::sensitive(relative)
    {
        return Err(Error::new(403, "Template must be an ordinary project file"));
    }
    let mut path = project.to_path_buf();
    for component in relative.components() {
        path.push(component.as_os_str());
        if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(Error::new(403, "Template cannot traverse symlinks"));
        }
    }
    let root = Dir::open_ambient_dir(project, ambient_authority())?;
    let file = root.open(relative)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > 20_000_000 {
        return Err(Error::new(
            413,
            "Template must be a regular file up to 20 MB",
        ));
    }
    let mut bytes = Vec::new();
    file.take(20_000_001).read_to_end(&mut bytes)?;
    if bytes.len() > 20_000_000 {
        return Err(Error::new(413, "Template exceeds 20 MB"));
    }
    Ok(bytes)
}

fn read_existing(dir: &Dir, name: &Path) -> Result<Option<Vec<u8>>> {
    match dir.open(name) {
        Ok(file) => {
            if !file.metadata()?.is_file() {
                return Err(Error::new(400, "Write target must be a regular file"));
            }
            let mut bytes = Vec::new();
            file.take(1_000_001).read_to_end(&mut bytes)?;
            if bytes.len() > 1_000_000 {
                return Err(Error::new(413, "File exceeds 1 MB edit limit"));
            }
            Ok(Some(bytes))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

impl PreparedWrite {
    /// Office artifacts are binary and create-only. Reuse the same confined
    /// directory handle and concurrent-change checks as ordinary file writes.
    pub(crate) fn prepare_artifact(
        project: &Path,
        target: &Path,
        content: Vec<u8>,
    ) -> Result<Self> {
        if content.len() > 20_000_000 {
            return Err(Error::new(413, "Office artifact exceeds 20 MB"));
        }
        let mut prepared = Self::prepare(project, target, "")?;
        if prepared.before.is_some() {
            return Err(Error::new(
                409,
                "Office output already exists; choose a new file name",
            ));
        }
        prepared.content = content;
        prepared.create_only = true;
        Ok(prepared)
    }
    pub(crate) fn expect_content(self, expected: Option<&serde_json::Value>) -> Result<Self> {
        if let Some(expected) = expected {
            let before = match &self.before {
                Some(bytes) => serde_json::json!(std::str::from_utf8(bytes)
                    .map_err(|_| Error::new(400, "File is not UTF-8 text"))?),
                None => serde_json::Value::Null,
            };
            if before != *expected {
                return Err(Error::new(409, "File changed; read it again before saving"));
            }
        }
        Ok(self)
    }
    pub(crate) fn prepare_change(
        project: &Path,
        target: &Path,
        operation: &str,
        args: &serde_json::Value,
    ) -> Result<Self> {
        let mut prepared = Self::prepare(project, target, "")?;
        let before = std::str::from_utf8(prepared.before.as_deref().unwrap_or_default())
            .map_err(|_| Error::new(400, "File is not UTF-8 text"))?;
        let after = match operation {
            "append_file" => format!(
                "{before}{}",
                args["content"]
                    .as_str()
                    .ok_or_else(|| Error::new(400, "content is required"))?
            ),
            "edit_file" => {
                if prepared.before.is_none() {
                    return Err(Error::new(404, "Edit target does not exist"));
                }
                let old = crate::required(args, "old_text")?;
                let new = args["new_text"]
                    .as_str()
                    .ok_or_else(|| Error::new(400, "new_text is required"))?;
                let count = before.matches(old).count();
                if count == 0 {
                    return Err(Error::new(
                        409,
                        "old_text was not found; read the current file before editing",
                    ));
                }
                if count > 1 && args["replace_all"] != true {
                    return Err(Error::new(409,"old_text matches multiple locations; include more context or set replace_all"));
                }
                before.replace(old, new)
            }
            _ => return Err(Error::new(400, "Unknown file change operation")),
        };
        if after.len() > 1_000_000 {
            return Err(Error::new(413, "File exceeds 1 MB edit limit"));
        }
        prepared.content = after.into_bytes();
        Ok(prepared)
    }
    pub(crate) fn prepare(project: &Path, target: &Path, content: &str) -> Result<Self> {
        if content.len() > 1_000_000 {
            return Err(Error::new(413, "File exceeds 1 MB edit limit"));
        }
        let relative = if target.is_absolute() {
            target.strip_prefix(project).map_err(|_| {
                Error::new(403, "File writes are limited to the conversation project")
            })?
        } else {
            target
        };
        if relative.as_os_str().is_empty()
            || relative.components().any(|c| {
                !matches!(c, Component::Normal(_))
                    || c.as_os_str().to_string_lossy().eq_ignore_ascii_case(".git")
            })
        {
            return Err(Error::new(
                400,
                "Expected a project file outside Git metadata",
            ));
        }
        let root = Dir::open_ambient_dir(project, ambient_authority())?;
        let directory = root.open_dir(
            relative
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )?;
        let name = relative
            .file_name()
            .ok_or_else(|| Error::new(400, "File name required"))?
            .to_owned();
        if directory
            .symlink_metadata(&name)
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(Error::new(403, "Cannot replace a symbolic link"));
        }
        // Reject redirected parent directories as well as leaf symlinks. This
        // prevents an ordinary-looking project path from targeting .git or a
        // policy directory through an in-project symlink.
        let mut parent = project.to_path_buf();
        for component in relative.components() {
            parent.push(component.as_os_str());
            if std::fs::symlink_metadata(&parent).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(Error::new(
                    403,
                    "File writes cannot traverse symbolic links",
                ));
            }
        }
        let before = read_existing(&directory, Path::new(&name))?;
        Ok(Self {
            directory,
            name,
            before,
            content: content.as_bytes().to_vec(),
            create_only: false,
        })
    }

    pub(crate) fn apply(self) -> Result<usize> {
        if self
            .directory
            .symlink_metadata(&self.name)
            .is_ok_and(|m| m.file_type().is_symlink())
            || read_existing(&self.directory, Path::new(&self.name))? != self.before
        {
            return Err(Error::new(
                409,
                "File changed while awaiting approval; read it again before editing",
            ));
        }
        let temp = format!(".potato-edit-{}", uuid::Uuid::new_v4());
        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            let mut file = self.directory.open_with(&temp, &options)?;
            if let Ok(metadata) = self.directory.metadata(&self.name) {
                file.set_permissions(metadata.permissions())?;
            }
            file.write_all(&self.content)?;
            file.sync_all()?;
            drop(file);
            if self.create_only {
                // Atomic no-clobber publication, including files created after
                // the optimistic check above. Both names share one directory.
                self.directory
                    .hard_link(&temp, &self.directory, &self.name)
                    .map_err(|e| {
                        if e.kind() == std::io::ErrorKind::AlreadyExists {
                            Error::new(409, "Office output already exists; choose a new file name")
                        } else {
                            e.into()
                        }
                    })?;
                let _ = self.directory.remove_file(&temp);
            } else {
                self.directory.rename(&temp, &self.directory, &self.name)?;
            }
            Ok(self.content.len())
        })();
        if result.is_err() {
            let _ = self.directory.remove_file(&temp);
        }
        result
    }
}

pub(crate) fn read_range(path: &Path, args: &serde_json::Value) -> Result<String> {
    use serde_json::json;
    let start = args["start_line"].as_u64().unwrap_or(1) as usize;
    let end = args["end_line"]
        .as_u64()
        .map(|n| n as usize)
        .unwrap_or(start.saturating_add(399));
    if start == 0 || end < start || end - start >= 10_000 {
        return Err(Error::new(
            400,
            "Use a 1-based line range containing at most 10000 lines",
        ));
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::new(400, "Target must be a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(16_000_001).read_to_end(&mut bytes)?;
    if bytes.len() > 16_000_000 {
        return Err(Error::new(
            413,
            "Text file exceeds the 16 MB read limit; narrow it with a search or shell command",
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| Error::new(400, "File is not UTF-8 text"))?;
    if text.contains('\0') {
        return Err(Error::new(400, "File contains binary data"));
    }
    let lines: Vec<_> = text.lines().collect();
    if start > lines.len() + 1 {
        return Err(Error::new(
            400,
            format!("start_line is beyond the {} lines in the file", lines.len()),
        ));
    }
    let actual_end = end.min(lines.len());
    let content = lines
        .iter()
        .enumerate()
        .skip(start - 1)
        .take(end - start + 1)
        .map(|(i, line)| format!("{}: {line}", i + 1))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(json!({"path":path,"start_line":start,"end_line":actual_end,"total_lines":lines.len(),"content":content,"next_start_line":if actual_end<lines.len(){Some(actual_end+1)}else{None}}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_artifacts_preserve_concurrent_files_and_support_large_outputs() {
        let root = tempfile::tempdir().unwrap();
        let target = Path::new("result.xlsx");
        let prepared =
            PreparedWrite::prepare_artifact(root.path(), target, vec![0; 2_000_000]).unwrap();
        std::fs::write(root.path().join(target), b"user content").unwrap();
        assert_eq!(prepared.apply().unwrap_err().status, 409);
        assert_eq!(
            std::fs::read(root.path().join(target)).unwrap(),
            b"user content"
        );
        PreparedWrite::prepare_artifact(root.path(), Path::new("large.xlsx"), vec![0; 2_000_000])
            .unwrap()
            .apply()
            .unwrap();
        assert_eq!(
            std::fs::metadata(root.path().join("large.xlsx"))
                .unwrap()
                .len(),
            2_000_000
        );
    }
    #[test]
    fn writes_atomically_and_rejects_changed_or_outside_targets() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path();
        std::fs::write(project.join("notes.md"), "before").unwrap();
        let edit = PreparedWrite::prepare(project, Path::new("notes.md"), "after").unwrap();
        std::fs::write(project.join("notes.md"), "user edited").unwrap();
        assert_eq!(edit.apply().unwrap_err().status, 409);
        assert_eq!(
            std::fs::read_to_string(project.join("notes.md")).unwrap(),
            "user edited"
        );
        assert!(PreparedWrite::prepare(project, Path::new("../outside.md"), "bad").is_err());
        assert!(PreparedWrite::prepare(project, Path::new(".git/config"), "bad").is_err());
        PreparedWrite::prepare(project, Path::new("new.md"), "你好")
            .unwrap()
            .apply()
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(project.join("new.md")).unwrap(),
            "你好"
        );
        assert!(!std::fs::read_dir(project).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".potato-edit")));
    }
    #[cfg(unix)]
    #[test]
    fn symlink_parent_cannot_escape_the_project() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
        assert!(PreparedWrite::prepare(root.path(), Path::new("escape/file"), "bad").is_err());
        assert!(!outside.path().join("file").exists());
    }
}

#[cfg(test)]
mod change_tests {
    use super::*;
    use serde_json::{json, Value};
    #[test]
    fn edits_are_unambiguous_and_preserve_concurrent_changes() {
        let root = tempfile::tempdir().unwrap();
        let path = Path::new("a.txt");
        std::fs::write(root.path().join(path), "same\nsame\n中文").unwrap();
        assert!(PreparedWrite::prepare_change(
            root.path(),
            path,
            "edit_file",
            &json!({"old_text":"same","new_text":"new"})
        )
        .is_err());
        PreparedWrite::prepare_change(
            root.path(),
            path,
            "edit_file",
            &json!({"old_text":"same","new_text":"new","replace_all":true}),
        )
        .unwrap()
        .apply()
        .unwrap();
        let pending = PreparedWrite::prepare_change(
            root.path(),
            path,
            "append_file",
            &json!({"content":" tail"}),
        )
        .unwrap();
        std::fs::write(root.path().join(path), "concurrent edit").unwrap();
        assert_eq!(pending.apply().unwrap_err().status, 409);
        assert_eq!(
            std::fs::read_to_string(root.path().join(path)).unwrap(),
            "concurrent edit"
        );
    }
    #[test]
    fn line_ranges_are_numbered_and_page_without_gaps() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a.txt");
        std::fs::write(&path, "一\n二\n三\n").unwrap();
        let first: Value = serde_json::from_str(
            &read_range(&path, &json!({"start_line":1,"end_line":2})).unwrap(),
        )
        .unwrap();
        assert_eq!(first["content"], "1: 一\n2: 二");
        assert_eq!(first["next_start_line"], 3);
        let last: Value =
            serde_json::from_str(&read_range(&path, &json!({"start_line":3})).unwrap()).unwrap();
        assert_eq!(last["content"], "3: 三");
        assert!(last["next_start_line"].is_null());
        assert!(read_range(&path, &json!({"start_line":0})).is_err());
    }
}
