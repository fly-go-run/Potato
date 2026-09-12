//! Host-only exact-read fingerprints and bounded reviewer conversation forks.
use super::reviewer::{Assessment, Outcome, Risk};
use crate::{model::Connection, permissions::PathSnapshot, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::Metadata,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(crate) const TTL: Duration = Duration::from_secs(600);
pub(crate) const CAPACITY: usize = 128;
const MAX_FILE: u64 = 1_048_576;
pub(crate) fn hash(value: &impl serde::Serialize) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("serializable review data"))
    )
}
pub(crate) fn connection_id(connection: &Connection) -> String {
    hash(
        &json!({"url":connection.url,"key":connection.key,"model":connection.model,"responses":connection.responses,"options":connection.options}),
    )
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct FileState {
    pub path: PathBuf,
    metadata: String,
    content: String,
}
fn metadata_id(m: &Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        format!(
            "{}:{}:{}:{}:{}:{}:{}",
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec()
        )
    }
    #[cfg(not(unix))]
    {
        format!("{}:{:?}:{:?}", m.len(), m.modified(), m.created())
    }
}
impl FileState {
    pub(crate) fn unchanged_metadata(&self) -> bool {
        std::fs::symlink_metadata(&self.path)
            .is_ok_and(|m| m.is_file() && metadata_id(&m) == self.metadata)
            && self.path.canonicalize().is_ok_and(|p| p == self.path)
    }
    fn read(path: &Path) -> Option<Self> {
        let before = std::fs::symlink_metadata(path).ok()?;
        if !before.is_file() || before.len() > MAX_FILE {
            return None;
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000); /* FILE_FLAG_OPEN_REPARSE_POINT */
        }
        let file = options.open(path).ok()?;
        let opened = file.metadata().ok()?;
        if !opened.is_file()
            || opened.len() > MAX_FILE
            || metadata_id(&opened) != metadata_id(&before)
        {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(MAX_FILE + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 > MAX_FILE {
            return None;
        }
        let state = Self {
            path: path.to_owned(),
            metadata: metadata_id(&opened),
            content: format!("{:x}", Sha256::digest(bytes)),
        };
        state.unchanged_metadata().then_some(state)
    }
    pub(crate) async fn capture(path: &Path) -> Option<Self> {
        let path = path.to_owned();
        tokio::time::timeout(
            Duration::from_millis(250),
            tokio::task::spawn_blocking(move || Self::read(&path)),
        )
        .await
        .ok()?
        .ok()?
    }
}
#[derive(Clone)]
pub(crate) struct Allowed {
    pub session: String,
    pub key: String,
    pub authority: String,
    pub connection: String,
    pub version: u64,
    pub generation: u64,
    pub file: FileState,
    pub snapshot: PathSnapshot,
    pub assessment: Assessment,
    pub request_id: String,
    pub expires: Instant,
    pub expires_at: String,
}
impl Allowed {
    pub(crate) fn fresh(&self) -> bool {
        self.expires > Instant::now()
            && self.file.unchanged_metadata()
            && self.snapshot.verify().is_ok()
    }
}
#[derive(Clone)]
pub(crate) struct Trunk {
    pub base: String,
    pub revision: u64,
    pub turns: Vec<Value>,
}
pub(crate) fn cacheable(assessment: &Assessment) -> bool {
    assessment.outcome == Outcome::Allow && assessment.risk == Risk::Low
}

pub(crate) fn references(history: &[Value]) -> Vec<Value> {
    let mut budget = 6000usize;
    let mut references = Vec::new();
    for (index, frame) in history.iter().enumerate().rev().filter(|(_, f)| {
        f["role"] != "user" && matches!(f["role"].as_str(), Some("assistant" | "tool"))
    }) {
        if references.len() >= 8 || budget < 200 {
            break;
        }
        let text = frame["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| {
                if c["type"] == "text" {
                    c["text"].as_str().map(str::to_owned)
                } else if c["type"] == "data" {
                    Some(c["data"].to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            continue;
        }
        let limit = 1200.min(budget);
        let excerpt = crate::context::head(&text, limit);
        budget -= excerpt.len();
        references.push(json!({"id":format!("reference-{index}"),"role":frame["role"],"kind":frame["type"],"text":excerpt,"truncated":excerpt.len()<text.len(),"authority":"untrusted"}));
    }
    references.reverse();
    references
}
pub(crate) fn checked_user_budget(evidence: &[Value], answers: &[Value]) -> Result<()> {
    if evidence.len() + answers.len() > 64
        || serde_json::to_vec(&(evidence, answers))?.len() > 34_000
    {
        return Err(crate::Error::new(
            413,
            "User authorization history exceeds the independent review limit",
        ));
    }
    Ok(())
}
