//! Bounded, capability-scoped transaction journals. Callers serialize access.
use crate::{Error, Result};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;

const LOG_LIMIT: usize = 256 * 1024 * 1024;
const RECORD_LIMIT: usize = 32 * 1024 * 1024;
const TRANSCRIPT: &str = "transcript.jsonl";

pub(crate) struct Archive {
    pub(crate) root: PathBuf,
    directory: Dir,
    // Only the most recently used session is retained, bounded by LOG_LIMIT.
    cache: Mutex<Option<Cached>>,
    #[cfg(test)]
    loads: std::sync::atomic::AtomicUsize,
}

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<cap_std::time::SystemTime>,
    created: Option<cap_std::time::SystemTime>,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64, i64, i64),
}
fn stamp(dir: &Dir, name: &str) -> Result<Option<Stamp>> {
    if !check_entry(dir, name, false)? {
        return Ok(None);
    }
    let meta = dir.metadata(name)?;
    #[cfg(unix)]
    use cap_std::fs::MetadataExt;
    Ok(Some(Stamp {
        len: meta.len(),
        modified: meta.modified().ok(),
        created: meta.created().ok(),
        #[cfg(unix)]
        identity: (
            meta.dev(),
            meta.ino(),
            meta.mtime(),
            meta.mtime_nsec(),
            meta.ctime(),
            meta.ctime_nsec(),
        ),
    }))
}
struct Cached {
    id: String,
    records: Arc<Vec<Value>>,
    size: usize,
    hydrated_total: usize,
    log: Option<Stamp>,
    artifacts: Vec<(String, Stamp)>,
}
impl Cached {
    fn valid(&self, dir: &Dir) -> Result<bool> {
        // Conservatively reload on platforms without Unix inode/change-time stamps.
        if !cfg!(unix) || stamp(dir, TRANSCRIPT)? != self.log {
            return Ok(false);
        }
        if !self.artifacts.is_empty() {
            if !check_entry(dir, "artifacts", true)? {
                return Ok(false);
            }
            let artifacts = dir.open_dir("artifacts")?;
            for (name, expected) in &self.artifacts {
                if stamp(&artifacts, name)?.as_ref() != Some(expected) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

fn invalid(message: &str) -> Error {
    Error::new(400, message)
}
fn validate_id(id: &str) -> Result<()> {
    let uuid = Uuid::parse_str(id).map_err(|_| invalid("Invalid session UUID"))?;
    if uuid.to_string() != id {
        return Err(invalid("Session UUID must use canonical lowercase form"));
    }
    Ok(())
}
fn check_entry(dir: &Dir, name: &str, directory: bool) -> Result<bool> {
    match dir.symlink_metadata(name) {
        Ok(meta)
            if !meta.file_type().is_symlink()
                && (if directory {
                    meta.is_dir()
                } else {
                    meta.is_file()
                }) =>
        {
            Ok(true)
        }
        Ok(_) => Err(invalid(
            "Archive entries must not be symlinks or special files",
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
fn child_dir(dir: &Dir, name: &str) -> Result<Dir> {
    if !check_entry(dir, name, true)? {
        match dir.create_dir(name) {
            Ok(()) => sync_dir(dir)?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                check_entry(dir, name, true)?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Ok(dir.open_dir(name)?)
}
fn sync_dir(_dir: &Dir) -> Result<()> {
    // Windows directory handles do not support FlushFileBuffers (GENERIC_WRITE
    // is required). Keep flushing every written file, but directory fsync is a
    // Unix durability barrier; attempting it on Windows prevents first startup.
    #[cfg(unix)]
    _dir.try_clone()?.into_std_file().sync_all()?;
    Ok(())
}
fn encode(record: &Value) -> Result<Vec<u8>> {
    // Bound serialization itself, including escaping expansion.
    struct Bounded(Vec<u8>);
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > RECORD_LIMIT.saturating_sub(self.0.len()) {
                return Err(std::io::Error::other("Transcript record exceeds 32 MiB"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut bytes = Bounded(Vec::new());
    serde_json::to_writer(&mut bytes, record)?;
    bytes.0.push(b'\n');
    Ok(bytes.0)
}

// The lock has a stable name independent of transcript publication/replacement.
fn lock(dir: &Dir) -> Result<std::fs::File> {
    check_entry(dir, "journal.lock", false)?;
    let mut create = OpenOptions::new();
    create.read(true).write(true).create_new(true);
    let mut existing = OpenOptions::new();
    existing.read(true).write(true);
    let file = match dir.open_with("journal.lock", &create) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            check_entry(dir, "journal.lock", false)?;
            dir.open_with("journal.lock", &existing)?
        }
        Err(e) => return Err(e.into()),
    }
    .into_std();
    file.lock()?;
    Ok(file)
}

fn pack(dir: &Dir, record: &Value) -> Result<Value> {
    let mut value = record.clone();
    let mut refs = Vec::new();
    fn visit(dir: &Dir, value: &mut Value, pointer: &str, refs: &mut Vec<Value>) -> Result<()> {
        match value {
            Value::String(text) if text.len() > 16 * 1024 => {
                let artifacts = child_dir(dir, "artifacts")?;
                let name = format!("{}.txt", Uuid::new_v4());
                let temp = format!("{}.tmp", Uuid::new_v4());
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                let mut file = artifacts.open_with(&temp, &options)?;
                file.write_all(text.as_bytes())?;
                file.sync_all()?;
                artifacts.rename(&temp, &artifacts, &name)?;
                sync_dir(&artifacts)?;
                refs.push(
                    serde_json::json!({"pointer": pointer, "path": format!("artifacts/{name}")}),
                );
                *text = format!(
                    "{}… [full text in artifacts/{name}]",
                    text.chars().take(256).collect::<String>()
                );
            }
            Value::Array(values) => {
                for (index, value) in values.iter_mut().enumerate() {
                    visit(dir, value, &format!("{pointer}/{index}"), refs)?;
                }
            }
            Value::Object(values) => {
                for (key, value) in values.iter_mut() {
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    visit(dir, value, &format!("{pointer}/{escaped}"), refs)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    visit(dir, &mut value, "", &mut refs)?;
    Ok(serde_json::json!({"format":"potato-transcript-v1", "record":value, "text_refs":refs}))
}

fn unpack(dir: &Dir, mut envelope: Value, stamps: &mut Vec<(String, Stamp)>) -> Result<Value> {
    if envelope["format"] != "potato-transcript-v1" {
        return Err(invalid("Unknown transcript format"));
    }
    let mut record = envelope
        .get_mut("record")
        .map(Value::take)
        .ok_or_else(|| invalid("Missing transcript record"))?;
    let refs = envelope["text_refs"]
        .as_array()
        .ok_or_else(|| invalid("Missing transcript text references"))?;
    let mut seen = std::collections::HashSet::new();
    let mut hydrated_bytes = 0usize;
    for reference in refs {
        let pointer = reference["pointer"]
            .as_str()
            .ok_or_else(|| invalid("Invalid text pointer"))?;
        if !seen.insert(pointer) {
            return Err(invalid("Duplicate text pointer"));
        }
        let path = reference["path"]
            .as_str()
            .ok_or_else(|| invalid("Invalid text reference"))?;
        let name = path
            .strip_prefix("artifacts/")
            .ok_or_else(|| invalid("Invalid artifact path"))?;
        let id = name
            .strip_suffix(".txt")
            .ok_or_else(|| invalid("Invalid artifact extension"))?;
        validate_id(id)?;
        if !check_entry(dir, "artifacts", true)? {
            return Err(invalid("Missing artifacts directory"));
        }
        let artifacts = dir.open_dir("artifacts")?;
        if !check_entry(&artifacts, name, false)? {
            return Err(invalid("Missing transcript artifact"));
        }
        let before =
            stamp(&artifacts, name)?.ok_or_else(|| invalid("Missing transcript artifact"))?;
        let file = artifacts.open(name)?;
        if !file.metadata()?.is_file() || file.metadata()?.len() > RECORD_LIMIT as u64 {
            return Err(invalid("Invalid or oversized transcript artifact"));
        }
        let mut bytes = Vec::new();
        file.take((RECORD_LIMIT.saturating_sub(hydrated_bytes) + 1) as u64)
            .read_to_end(&mut bytes)?;
        hydrated_bytes += bytes.len();
        if hydrated_bytes > RECORD_LIMIT {
            return Err(Error::new(413, "Hydrated transcript record exceeds 32 MiB"));
        }
        let target = record
            .pointer_mut(pointer)
            .ok_or_else(|| invalid("Transcript text pointer does not exist"))?;
        if !target.is_string() {
            return Err(invalid("Transcript text pointer must select a string"));
        }
        stamps.push((name.to_owned(), before));
        *target = Value::String(
            String::from_utf8(bytes).map_err(|_| invalid("Transcript artifact is not UTF-8"))?,
        );
    }
    Ok(record)
}

impl Archive {
    pub(crate) fn open(root: &Path) -> Result<Self> {
        let root = if root.is_absolute() {
            root.to_path_buf()
        } else {
            std::env::current_dir()?.join(root)
        };
        if root
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(invalid("Archive root must not contain parent traversal"));
        }
        let parent = root
            .parent()
            .ok_or_else(|| invalid("Archive needs a dedicated directory"))?;
        // The workspace exists before this archive is opened. Refuse redirection
        // in every ancestor, then retain the capability instead of reopening paths.
        for ancestor in parent.ancestors() {
            if std::fs::symlink_metadata(ancestor)?
                .file_type()
                .is_symlink()
            {
                return Err(invalid("Archive root ancestors must not be symlinks"));
            }
        }
        let name = root
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| invalid("Invalid archive root"))?;
        let parent = Dir::open_ambient_dir(parent, ambient_authority())?;
        let directory = child_dir(&parent, name)?;
        child_dir(&directory, "sessions")?;
        child_dir(&directory, "deleted")?;
        Ok(Self {
            root,
            directory,
            cache: Mutex::new(None),
            #[cfg(test)]
            loads: std::sync::atomic::AtomicUsize::new(0),
        })
    }
    pub(crate) fn directory(&self) -> Result<Dir> {
        Ok(self.directory.try_clone()?)
    }
    fn session(&self, id: &str) -> Result<Dir> {
        validate_id(id)?;
        if self.is_discarded(id)? {
            return Err(Error::new(404, "Chat deleted"));
        }
        let sessions = child_dir(&self.directory, "sessions")?;
        child_dir(&sessions, id)
    }
    pub(crate) fn session_dir(&self, id: &str) -> Result<PathBuf> {
        self.session(id)?;
        Ok(self.root.join("sessions").join(id))
    }
    pub(crate) fn path(&self, id: &str) -> Result<PathBuf> {
        Ok(self.session_dir(id)?.join(TRANSCRIPT))
    }
    /// Complete malformed lines always fail. A torn final line is saved durably
    /// before truncation, so a subsequent append cannot concatenate onto it.
    fn envelopes(dir: &Dir) -> Result<(Vec<Value>, usize, Option<Stamp>)> {
        if !check_entry(dir, TRANSCRIPT, false)? {
            return Ok((Vec::new(), 0, None));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        let mut file = dir.open_with(TRANSCRIPT, &options)?;
        if !file.metadata()?.is_file() {
            return Err(invalid("Transcript must be a regular file"));
        }
        if file.metadata()?.len() > LOG_LIMIT as u64 {
            return Err(Error::new(413, "Transcript exceeds 256 MiB"));
        }
        let before = stamp(dir, TRANSCRIPT)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(LOG_LIMIT as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > LOG_LIMIT {
            return Err(Error::new(413, "Transcript exceeds 256 MiB"));
        }
        if stamp(dir, TRANSCRIPT)? != before {
            return Err(Error::new(409, "Transcript changed while being read"));
        }
        let end = bytes
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map_or(0, |n| n + 1);
        let mut records = Vec::new();
        for (index, line) in bytes[..end]
            .split(|byte| *byte == b'\n')
            .take(bytes[..end].iter().filter(|byte| **byte == b'\n').count())
            .enumerate()
        {
            if line.len() > RECORD_LIMIT {
                return Err(Error::new(413, "Transcript record exceeds 32 MiB"));
            }
            let envelope = serde_json::from_slice(line).map_err(|e| {
                Error::new(500, format!("Corrupt transcript line {}: {e}", index + 1))
            })?;
            records.push(envelope);
        }
        if end < bytes.len() {
            if bytes.len() - end > RECORD_LIMIT {
                return Err(Error::new(413, "Transcript tail exceeds 32 MiB"));
            }
            let recovery = format!("recovery-tail-{}.bin", Uuid::new_v4());
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            let mut saved = dir.open_with(&recovery, &options)?;
            saved.write_all(&bytes[end..])?;
            saved.sync_all()?;
            sync_dir(dir)?;
            file.set_len(end as u64)?;
            file.sync_all()?;
        }
        let log = if end < bytes.len() {
            stamp(dir, TRANSCRIPT)?
        } else {
            before
        };
        Ok((records, end, log))
    }
    fn cached(&self, dir: &Dir, id: &str, cache: &mut Option<Cached>) -> Result<()> {
        if cache.as_ref().is_some_and(|cached| cached.id == id)
            && cache.as_ref().unwrap().valid(dir)?
        {
            return Ok(());
        }
        *cache = None;
        #[cfg(test)]
        self.loads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let (envelopes, size, log) = Self::envelopes(dir)?;
        let mut records = Vec::new();
        let mut artifacts = Vec::new();
        let mut hydrated_total = 0usize;
        for envelope in envelopes {
            let record = unpack(dir, envelope, &mut artifacts)?;
            hydrated_total += encode(&record)?.len();
            if hydrated_total > LOG_LIMIT {
                return Err(Error::new(413, "Hydrated transcript exceeds 256 MiB"));
            }
            records.push(record);
        }
        *cache = Some(Cached {
            id: id.to_owned(),
            records: Arc::new(records),
            size,
            hydrated_total,
            log,
            artifacts,
        });
        Ok(())
    }
    pub(crate) fn read_shared(&self, id: &str) -> Result<Arc<Vec<Value>>> {
        let dir = self.session(id)?;
        let _lock = lock(&dir)?;
        if self.is_discarded(id)? {
            return Err(Error::new(404, "Chat deleted"));
        }
        let mut cache = self.cache.lock().unwrap();
        self.cached(&dir, id, &mut cache)?;
        Ok(Arc::clone(&cache.as_ref().unwrap().records))
    }
    #[cfg(test)]
    pub(crate) fn read(&self, id: &str) -> Result<Vec<Value>> {
        Ok(self.read_shared(id)?.as_ref().clone())
    }
    /// Read catalog events without opening message text artifacts. Legacy large
    /// session metadata is hydrated selectively; previews are never authoritative.
    pub(crate) fn catalog(&self, id: &str) -> Result<Vec<Value>> {
        validate_id(id)?;
        if self.is_discarded(id)? {
            return Ok(vec![serde_json::json!({"type":"deleted"})]);
        }
        let dir = self.session(id)?;
        let _lock = lock(&dir)?;
        if self.is_discarded(id)? {
            return Ok(vec![serde_json::json!({"type":"deleted"})]);
        }
        let mut result = Vec::new();
        for envelope in Self::envelopes(&dir)?.0 {
            if envelope["format"] != "potato-transcript-v1" {
                return Err(invalid("Unknown transcript format"));
            }
            let record = &envelope["record"];
            if record["version"] != 1 {
                return Err(Error::new(409, "Unsupported transcript event version"));
            }
            let events = record["events"]
                .as_array()
                .ok_or_else(|| invalid("Invalid transcript events"))?;
            let refs = envelope["text_refs"]
                .as_array()
                .ok_or_else(|| invalid("Missing transcript text references"))?;
            let mut pointers = std::collections::HashSet::new();
            for reference in refs {
                let pointer = reference["pointer"]
                    .as_str()
                    .ok_or_else(|| invalid("Invalid text pointer"))?;
                if !pointers.insert(pointer)
                    || !record.pointer(pointer).is_some_and(Value::is_string)
                {
                    return Err(invalid("Invalid or duplicate text pointer"));
                }
                let path = reference["path"]
                    .as_str()
                    .ok_or_else(|| invalid("Invalid text reference"))?;
                let artifact_id = path
                    .strip_prefix("artifacts/")
                    .and_then(|name| name.strip_suffix(".txt"))
                    .ok_or_else(|| invalid("Invalid artifact path"))?;
                validate_id(artifact_id)?;
            }
            let mut selected = Vec::new();
            for (index, event) in events.iter().enumerate() {
                match event["type"].as_str() {
                    Some("session" | "deleted") => selected.push(index),
                    Some("append" | "replace_frame" | "replace_wire") => {}
                    _ => return Err(Error::new(409, "Unknown transcript event type")),
                }
            }
            if selected.is_empty() {
                continue;
            }
            let metadata_refs: Vec<Value> = refs
                .iter()
                .filter(|reference| {
                    let pointer = reference["pointer"].as_str().unwrap();
                    selected
                        .iter()
                        .any(|index| pointer.starts_with(&format!("/events/{index}/")))
                })
                .cloned()
                .collect();
            let mut metadata_envelope = envelope;
            metadata_envelope["text_refs"] = Value::Array(metadata_refs);
            let hydrated = unpack(&dir, metadata_envelope, &mut Vec::new())?;
            // Bound each hydrated metadata event, not the cumulative conversation.
            for index in selected {
                let event = &hydrated["events"][index];
                encode(event)?;
                result.push(event.clone());
            }
        }
        Ok(result)
    }
    pub(crate) fn append(&self, id: &str, record: &Value) -> Result<()> {
        let dir = self.session(id)?;
        let _lock = lock(&dir)?;
        if self.is_discarded(id)? {
            return Err(Error::new(404, "Chat deleted"));
        }
        if !check_entry(&dir, TRANSCRIPT, false)? {
            return Err(Error::new(500, "Conversation transcript is missing"));
        }
        let mut cache = self.cache.lock().unwrap();
        self.cached(&dir, id, &mut cache)?;
        let cached = cache.as_mut().unwrap();
        let hydrated_total = cached.hydrated_total + encode(record)?.len();
        if hydrated_total > LOG_LIMIT {
            return Err(Error::new(413, "Hydrated transcript exceeds 256 MiB"));
        }
        let envelope = pack(&dir, record)?;
        let bytes = encode(&envelope)?;
        if bytes.len() > LOG_LIMIT.saturating_sub(cached.size) {
            return Err(Error::new(413, "Transcript exceeds 256 MiB"));
        }
        // Stamp new sidecars without rereading or serializing prior content.
        let mut new_stamps = Vec::new();
        for reference in envelope["text_refs"].as_array().unwrap() {
            let name = reference["path"]
                .as_str()
                .unwrap()
                .strip_prefix("artifacts/")
                .unwrap();
            let artifacts = dir.open_dir("artifacts")?;
            new_stamps.push((
                name.to_owned(),
                stamp(&artifacts, name)?.ok_or_else(|| invalid("Missing transcript artifact"))?,
            ));
        }
        let mut options = OpenOptions::new();
        options.append(true);
        let mut file = dir.open_with(TRANSCRIPT, &options)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        let log = stamp(&dir, TRANSCRIPT)?;
        Arc::make_mut(&mut cached.records).push(record.clone());
        cached.size += bytes.len();
        cached.hydrated_total = hydrated_total;
        cached.artifacts.extend(new_stamps);
        cached.log = log;
        Ok(())
    }
    pub(crate) fn initialize(&self, id: &str, records: &[Value]) -> Result<()> {
        let dir = self.session(id)?;
        let _lock = lock(&dir)?;
        if self.is_discarded(id)? {
            return Err(Error::new(404, "Chat deleted"));
        }
        if check_entry(&dir, TRANSCRIPT, false)? {
            let mut cache = self.cache.lock().unwrap();
            self.cached(&dir, id, &mut cache)?;
            return if cache.as_ref().unwrap().records.as_ref() == records {
                Ok(())
            } else {
                Err(Error::new(
                    409,
                    "Transcript already exists with different records",
                ))
            };
        }
        let temp = format!("initialize-{}.tmp", Uuid::new_v4());
        let result = (|| -> Result<()> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            let mut file = dir.open_with(&temp, &options)?;
            let mut total = 0;
            let mut hydrated_total = 0;
            for record in records {
                hydrated_total += encode(record)?.len();
                if hydrated_total > LOG_LIMIT {
                    return Err(Error::new(413, "Hydrated transcript exceeds 256 MiB"));
                }
                let bytes = encode(&pack(&dir, record)?)?;
                if bytes.len() > LOG_LIMIT - total {
                    return Err(Error::new(413, "Transcript exceeds 256 MiB"));
                }
                file.write_all(&bytes)?;
                total += bytes.len();
            }
            file.sync_all()?;
            // Hard-link publication is atomic and cannot overwrite an existing log.
            dir.hard_link(&temp, &dir, TRANSCRIPT)?;
            sync_dir(&dir)
        })();
        let cleanup = dir.remove_file(&temp);
        result?;
        cleanup?;
        sync_dir(&dir)
    }
    fn is_discarded(&self, id: &str) -> Result<bool> {
        let deleted = child_dir(&self.directory, "deleted")?;
        check_entry(&deleted, id, false)
    }
    /// Publish deletion independently of journal health or remaining capacity.
    /// The marker survives until the caller commits its catalog deletion and
    /// calls finish_discard, making interrupted removal recoverable.
    pub(crate) fn discard(&self, id: &str) -> Result<()> {
        validate_id(id)?;
        let sessions = child_dir(&self.directory, "sessions")?;
        let session = if check_entry(&sessions, id, true)? {
            Some(sessions.open_dir(id)?)
        } else {
            None
        };
        let _lock = session.as_ref().map(lock).transpose()?;
        let deleted = child_dir(&self.directory, "deleted")?;
        if !check_entry(&deleted, id, false)? {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            match deleted.open_with(id, &options) {
                Ok(file) => file.sync_all()?,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    check_entry(&deleted, id, false)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
        sync_dir(&deleted)?;
        *self.cache.lock().unwrap() = None;
        if session.is_some() {
            sessions.remove_dir_all(id)?;
            sync_dir(&sessions)?;
        }
        Ok(())
    }
    /// Called only after the caller has durably removed the catalog entry.
    pub(crate) fn finish_discard(&self, id: &str) -> Result<()> {
        validate_id(id)?;
        if !self.is_discarded(id)? {
            return Ok(());
        }
        self.remove(id)?;
        let deleted = child_dir(&self.directory, "deleted")?;
        deleted.remove_file(id)?;
        sync_dir(&deleted)
    }
    pub(crate) fn remove(&self, id: &str) -> Result<()> {
        validate_id(id)?;
        let sessions = child_dir(&self.directory, "sessions")?;
        if check_entry(&sessions, id, true)? {
            let dir = sessions.open_dir(id)?;
            let _lock = lock(&dir)?;
            sessions.remove_dir_all(id)?;
            sync_dir(&sessions)?;
        }
        Ok(())
    }
    pub(crate) fn ids(&self) -> Result<Vec<String>> {
        let sessions = child_dir(&self.directory, "sessions")?;
        let mut ids = Vec::new();
        for entry in sessions.entries()? {
            let entry = entry?;
            if let Some(id) = entry.file_name().to_str() {
                if validate_id(id).is_ok() {
                    // Include malformed session entries so catalog can report a
                    // per-session error instead of blocking unrelated sessions.
                    let published = (|| -> Result<bool> {
                        if !check_entry(&sessions, id, true)? {
                            return Ok(false);
                        }
                        let session = sessions.open_dir(id)?;
                        check_entry(&session, TRANSCRIPT, false)
                    })();
                    if !matches!(published, Ok(false)) {
                        ids.push(id.to_owned());
                    }
                }
            }
        }
        let deleted = child_dir(&self.directory, "deleted")?;
        for entry in deleted.entries()? {
            let entry = entry?;
            if let Some(id) = entry.file_name().to_str() {
                if validate_id(id).is_ok() {
                    ids.push(id.to_owned());
                }
            }
        }
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn archive() -> (tempfile::TempDir, Archive, String) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("archive");
        let archive = Archive::open(&root).unwrap();
        (temp, archive, Uuid::new_v4().to_string())
    }
    #[test]
    fn exact_roundtrip_and_initialization() {
        let (_temp, archive, id) = archive();
        let records = vec![
            json!({"version":1,"events":["你好 🥔",{"$potato_text_ref":"artifacts/foo.txt"}],"large":"a".repeat(20000)}),
        ];
        archive.initialize(&id, &records).unwrap();
        archive.initialize(&id, &records).unwrap();
        assert!(archive.initialize(&id, &[json!(false)]).is_err());
        archive.append(&id, &json!([null, true, -3, 1.25])).unwrap();
        assert_eq!(
            archive.read(&id).unwrap(),
            vec![records[0].clone(), json!([null, true, -3, 1.25])]
        );
        assert_eq!(archive.ids().unwrap(), vec![id.clone()]);
        archive.remove(&id).unwrap();
        assert!(archive.ids().unwrap().is_empty());
    }
    #[test]
    fn torn_tail_is_preserved_before_append() {
        let (_temp, archive, id) = archive();
        let path = archive.path(&id).unwrap();
        let tail = b"{\"broken\":\"\xff";
        archive.initialize(&id, &[]).unwrap();
        archive.append(&id, &json!({"ok":1})).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.extend_from_slice(tail);
        std::fs::write(&path, bytes).unwrap();
        archive.append(&id, &json!({"ok":2})).unwrap();
        assert_eq!(
            archive.read(&id).unwrap(),
            vec![json!({"ok":1}), json!({"ok":2})]
        );
        let recovery = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|e| e == "bin"))
            .unwrap();
        assert_eq!(std::fs::read(recovery).unwrap(), tail);
    }
    #[test]
    fn complete_corruption_is_not_discarded() {
        let (_temp, archive, id) = archive();
        let path = archive.path(&id).unwrap();
        archive.initialize(&id, &[]).unwrap();
        archive.append(&id, &json!({"ok":1})).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.extend_from_slice(b"not-json\nunfinished");
        std::fs::write(&path, &bytes).unwrap();
        assert!(archive.read(&id).is_err());
        assert!(archive.append(&id, &json!(1)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    #[test]
    fn artifacts_roundtrip_escape_keys_and_validate_confinement() {
        let (_temp, archive, id) = archive();
        let long = "土豆🙂".repeat(5000);
        let record = json!({"a/~b":[long], "format":"potato-transcript-v1", "record":{"$potato_text_ref":"artifacts/fake.txt"}, "text_refs":[]});
        archive.initialize(&id, &[]).unwrap();
        archive.append(&id, &record).unwrap();
        assert_eq!(archive.read(&id).unwrap(), vec![record]);
        let path = archive.path(&id).unwrap();
        let mut stored: Value =
            serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
        assert_eq!(stored["text_refs"][0]["pointer"], "/a~1~0b/0");
        assert!(std::fs::metadata(&path).unwrap().len() < 4096);
        let relative = stored["text_refs"][0]["path"].as_str().unwrap();
        assert_eq!(
            std::fs::read_to_string(path.parent().unwrap().join(relative)).unwrap(),
            long
        );
        stored["text_refs"][0]["path"] = json!("artifacts/../../outside.txt");
        std::fs::write(&path, encode(&stored).unwrap()).unwrap();
        assert!(archive.read(&id).is_err());
    }
    #[test]
    fn independently_opened_archives_serialize_appends() {
        let (_temp, archive, id) = archive();
        archive.initialize(&id, &[]).unwrap();
        let mut threads = Vec::new();
        for worker in 0..4 {
            let root = archive.root.clone();
            let id = id.clone();
            threads.push(std::thread::spawn(move || {
                let archive = Archive::open(&root).unwrap();
                for count in 0..8 {
                    archive.append(&id, &json!([worker, count])).unwrap();
                }
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(archive.read(&id).unwrap().len(), 32);
    }
    #[test]
    fn oversized_record_rejected_without_a_journal() {
        let (_temp, archive, id) = archive();
        archive.initialize(&id, &[]).unwrap();
        assert!(archive
            .append(&id, &json!("x".repeat(RECORD_LIMIT)))
            .is_err());
        assert_eq!(
            std::fs::metadata(archive.path(&id).unwrap()).unwrap().len(),
            0
        );
        let file = std::fs::File::create(archive.path(&id).unwrap()).unwrap();
        file.set_len(LOG_LIMIT as u64 + 1).unwrap();
        assert!(archive.read(&id).is_err());
        assert_eq!(file.metadata().unwrap().len(), LOG_LIMIT as u64 + 1);
    }
    #[test]
    fn unpublished_directories_are_ignored_and_append_requires_publication() {
        let (_temp, archive, id) = archive();
        let session = archive.session_dir(&id).unwrap();
        std::fs::write(session.join("initialize-interrupted.tmp"), b"incomplete").unwrap();
        assert!(archive.ids().unwrap().is_empty());
        assert!(archive.append(&id, &json!("must not recreate")).is_err());
        assert!(!archive.path(&id).unwrap().exists());
        archive.initialize(&id, &[json!("published")]).unwrap();
        assert_eq!(archive.ids().unwrap(), vec![id.clone()]);
        std::fs::remove_file(archive.path(&id).unwrap()).unwrap();
        assert!(archive.append(&id, &json!("must not recreate")).is_err());
        assert!(!archive.path(&id).unwrap().exists());
    }
    #[cfg(unix)]
    #[test]
    fn cached_appends_and_shared_reads_do_not_reload_prior_content() {
        let (_temp, archive, id) = archive();
        archive
            .initialize(&id, &[json!({"large":"x".repeat(20000)})])
            .unwrap();
        archive.read_shared(&id).unwrap();
        let before = archive.loads.load(std::sync::atomic::Ordering::Relaxed);
        for n in 0..4 {
            archive.append(&id, &json!({"n": n})).unwrap();
            assert_eq!(archive.read_shared(&id).unwrap().len(), n + 2);
        }
        assert_eq!(
            archive.loads.load(std::sync::atomic::Ordering::Relaxed),
            before
        );
        let first = archive.read_shared(&id).unwrap();
        let second = archive.read_shared(&id).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        let other = Uuid::new_v4().to_string();
        archive.initialize(&other, &[]).unwrap();
        archive.read_shared(&other).unwrap();
        assert_eq!(archive.cache.lock().unwrap().as_ref().unwrap().id, other);
    }
    #[test]
    fn cache_invalidates_same_size_log_edits_and_replacements() {
        let (_temp, archive, id) = archive();
        archive.initialize(&id, &[json!({"v":"aaa"})]).unwrap();
        archive.read_shared(&id).unwrap();
        let path = archive.path(&id).unwrap();
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        let old = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, old.replace("aaa", "bbb")).unwrap();
        // A preserved mtime must not conceal an in-place edit on Unix.
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(before)
            .unwrap();
        assert_eq!(archive.read(&id).unwrap(), vec![json!({"v":"bbb"})]);
        let replacement = path.with_extension("replacement");
        std::fs::write(&replacement, old.replace("aaa", "ccc")).unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        archive.append(&id, &json!(4)).unwrap();
        assert_eq!(
            archive.read(&id).unwrap(),
            vec![json!({"v":"ccc"}), json!(4)]
        );
    }
    #[test]
    fn cache_invalidates_artifact_edits_and_deletion() {
        let (_temp, archive, id) = archive();
        archive
            .initialize(&id, &[json!({"v":"a".repeat(20000)})])
            .unwrap();
        archive.read_shared(&id).unwrap();
        let path = archive.path(&id).unwrap();
        let envelope: Value =
            serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
        let artifact = path
            .parent()
            .unwrap()
            .join(envelope["text_refs"][0]["path"].as_str().unwrap());
        let before = std::fs::metadata(&artifact).unwrap().modified().unwrap();
        std::fs::write(&artifact, "b".repeat(20000)).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&artifact)
            .unwrap()
            .set_modified(before)
            .unwrap();
        assert_eq!(archive.read(&id).unwrap()[0]["v"], "b".repeat(20000));
        std::fs::remove_file(&artifact).unwrap();
        assert!(archive.read_shared(&id).is_err());
        assert!(archive.append(&id, &json!(1)).is_err());
    }
    #[test]
    fn catalog_skips_message_artifacts_and_hydrates_legacy_metadata() {
        let (_temp, archive, id) = archive();
        let metadata = json!({"type":"session", "spec":{"id":id,"name":"n".repeat(20000)}});
        archive.initialize(&id, &[json!({"version":1,"events":[metadata, {"type":"append","frame":{"text":"x".repeat(20000)}}]})]).unwrap();
        let path = archive.path(&id).unwrap();
        let envelope: Value =
            serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
        for reference in envelope["text_refs"].as_array().unwrap() {
            if reference["pointer"]
                .as_str()
                .unwrap()
                .starts_with("/events/1/")
            {
                std::fs::remove_file(
                    path.parent()
                        .unwrap()
                        .join(reference["path"].as_str().unwrap()),
                )
                .unwrap();
            }
        }
        assert!(archive.read(&id).is_err());
        assert_eq!(archive.catalog(&id).unwrap(), vec![metadata]);
    }
    #[test]
    fn discard_corrupt_log_is_durable_until_catalog_commit() {
        let (_temp, archive, id) = archive();
        archive.initialize(&id, &[]).unwrap();
        let path = archive.path(&id).unwrap();
        std::fs::write(&path, b"bad complete line\n").unwrap();
        archive.discard(&id).unwrap();
        let reopened = Archive::open(&archive.root).unwrap();
        assert_eq!(reopened.ids().unwrap(), vec![id.clone()]);
        assert_eq!(
            reopened.catalog(&id).unwrap(),
            vec![json!({"type":"deleted"})]
        );
        assert!(reopened.read(&id).is_err());
        assert!(reopened.initialize(&id, &[]).is_err());
        // Simulate an interrupted cleanup leaving an old directory behind.
        std::fs::create_dir(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"bad complete line\n").unwrap();
        assert_eq!(
            reopened.catalog(&id).unwrap(),
            vec![json!({"type":"deleted"})]
        );
        reopened.finish_discard(&id).unwrap();
        assert!(reopened.ids().unwrap().is_empty());
        reopened
            .initialize(&id, &[json!("explicit reimport")])
            .unwrap();
        assert_eq!(
            reopened.read(&id).unwrap(),
            vec![json!("explicit reimport")]
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected() {
        use std::os::unix::fs::symlink;
        let (temp, archive, id) = archive();
        let outside = temp.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, archive.root.join("sessions").join(&id)).unwrap();
        assert!(archive.append(&id, &json!(1)).is_err());
        assert_eq!(archive.ids().unwrap(), vec![id.clone()]);
        assert!(archive.catalog(&id).is_err());
        assert!(!outside.join(TRANSCRIPT).exists());
        std::fs::remove_file(archive.root.join("sessions").join(&id)).unwrap();
        let path = archive.path(&id).unwrap();
        std::fs::write(outside.join("secret"), b"{}\n").unwrap();
        symlink(outside.join("secret"), &path).unwrap();
        assert!(archive.read(&id).is_err());
        let redirect = temp.path().join("redirect");
        symlink(&outside, &redirect).unwrap();
        assert!(Archive::open(&redirect).is_err());
    }
}
