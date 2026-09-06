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
        let before = read_existing(&directory, Path::new(&name))?;
        Ok(Self {
            directory,
            name,
            before,
            content: content.as_bytes().to_vec(),
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
            self.directory.rename(&temp, &self.directory, &self.name)?;
            Ok(self.content.len())
        })();
        if result.is_err() {
            let _ = self.directory.remove_file(&temp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
