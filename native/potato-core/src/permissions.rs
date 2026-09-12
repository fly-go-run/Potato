//! User-owned read capabilities. These rules never grant write, shell or network access.
use crate::{lock, required, Error, Result, Runtime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Weak},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum ApprovalPolicy {
    #[default]
    #[serde(rename = "AUTO")]
    Auto,
    #[serde(rename = "STRICT")]
    Strict,
    #[serde(rename = "NEVER")]
    Never,
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum FileMode {
    #[serde(rename = "read-only")]
    ReadOnly,
    #[default]
    #[serde(rename = "workspace-write")]
    WorkspaceWrite,
    #[serde(rename = "danger-full-access", alias = "full-access")]
    FullAccess,
}
impl FileMode {
    pub const OPTIONS: [(Self, &'static str, &'static str); 3] = [
        (Self::ReadOnly, "read-only", "只读"),
        (Self::WorkspaceWrite, "workspace-write", "工作区读写"),
        (Self::FullAccess, "danger-full-access", "完全访问"),
    ];
    pub fn label(self) -> &'static str {
        Self::OPTIONS
            .into_iter()
            .find(|(mode, _, _)| *mode == self)
            .unwrap()
            .2
    }
}
impl ApprovalPolicy {
    pub const OPTIONS: [(Self, &'static str, &'static str); 3] = [
        (Self::Auto, "AUTO", "按规则确认"),
        (Self::Strict, "STRICT", "每次确认"),
        (Self::Never, "NEVER", "禁止需确认的操作"),
    ];
    pub fn label(self) -> &'static str {
        Self::OPTIONS
            .into_iter()
            .find(|(policy, _, _)| *policy == self)
            .unwrap()
            .2
    }
}
/// Approval routing is independent of file capabilities and prompting policy.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reviewer {
    User,
    #[default]
    Model,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct PermissionConfig {
    pub approval_level: ApprovalPolicy,
    pub sandbox_mode: FileMode,
    pub reviewer: Reviewer,
    pub reviewer_provider_id: String,
    pub reviewer_model: String,
}
impl PermissionConfig {
    pub fn validate(&self) -> Result<()> {
        if self.reviewer_provider_id.trim().is_empty() != self.reviewer_model.trim().is_empty() {
            return Err(Error::new(400,"Choose both reviewer provider and model, or leave both empty to follow the active model"));
        }
        if self.reviewer_provider_id.len() > 256 || self.reviewer_model.len() > 256 {
            return Err(Error::new(400, "Reviewer selection is too long"));
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Read,
    List,
    Search,
}
impl Operation {
    pub(crate) fn for_tool(name: &str) -> Option<Self> {
        match name {
            "read_file" => Some(Self::Read),
            "list_directory" => Some(Self::List),
            "grep_search" | "glob_search" => Some(Self::Search),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Deny,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DirectoryRule {
    pub id: String,
    pub kind: String,
    pub path: PathBuf,
    pub recursive: bool,
    pub operations: Vec<Operation>,
    pub decision: Decision,
    pub lifetime: String,
    pub created_by: String,
    pub session_id: Option<String>,
    identity: String,
}
fn identity(path: &Path) -> Result<String> {
    let m = std::fs::metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        Ok(format!("{:?}", m.created()?))
    }
}
#[derive(Clone, Debug)]
pub(crate) struct PathSnapshot {
    path: PathBuf,
    identities: Vec<(PathBuf, String)>,
}
impl PathSnapshot {
    pub(crate) fn capture(path: &Path) -> Result<Self> {
        let canonical = path.canonicalize()?;
        let identities = canonical
            .ancestors()
            .map(|p| Ok((p.to_owned(), identity(p)?)))
            .collect::<Result<_>>()?;
        Ok(Self {
            path: canonical,
            identities,
        })
    }
    pub(crate) fn verify(&self) -> Result<()> {
        if self.path.canonicalize()? != self.path
            || self
                .identities
                .iter()
                .any(|(path, id)| identity(path).as_ref().ok() != Some(id))
        {
            return Err(Error::new(
                409,
                "File or directory changed after permission review",
            ));
        }
        Ok(())
    }
}
#[derive(Default)]
pub(crate) struct State {
    pub version: u64,
    pub session_rules: Vec<DirectoryRule>,
    pub queues: HashMap<String, Weak<tokio::sync::Mutex<()>>>,
}
impl DirectoryRule {
    pub(crate) fn unchanged(&self) -> bool {
        self.path.canonicalize().is_ok_and(|p| p == self.path)
            && identity(&self.path).is_ok_and(|id| id == self.identity)
    }
    fn matches(&self, path: &Path, operation: Operation, session: &str) -> bool {
        (operation != Operation::Search || self.recursive)
            && self.operations.contains(&operation)
            && self.session_id.as_ref().is_none_or(|s| s == session)
            && (path == self.path
                || if self.recursive {
                    path.starts_with(&self.path)
                } else {
                    operation == Operation::Read && path.parent() == Some(self.path.as_path())
                })
            && self.path.canonicalize().is_ok_and(|p| p == self.path)
            && identity(&self.path).is_ok_and(|id| id == self.identity)
    }
}
impl Runtime {
    pub(crate) fn permission_version(&self) -> Result<u64> {
        Ok(lock(&self.permissions)?.version)
    }
    pub(crate) fn permission_queue(
        &self,
        session: &str,
        directory: &Path,
    ) -> Result<Arc<tokio::sync::Mutex<()>>> {
        let mut state = lock(&self.permissions)?;
        state.queues.retain(|_, q| q.strong_count() > 0);
        let key = format!("{session}:{}", directory.display());
        if let Some(queue) = state.queues.get(&key).and_then(Weak::upgrade) {
            return Ok(queue);
        }
        let queue = Arc::new(tokio::sync::Mutex::new(()));
        state.queues.insert(key, Arc::downgrade(&queue));
        Ok(queue)
    }
    pub(crate) fn persistent_rules(&self) -> Result<Vec<DirectoryRule>> {
        Ok(serde_json::from_value(
            self.db()?
                .get("permission_rules", json!({"schema_version":1,"rules":[]}))?["rules"]
                .clone(),
        )?)
    }
    pub(crate) fn directory_decision(
        &self,
        session: &str,
        name: &str,
        target: &str,
    ) -> Result<Option<(Decision, String)>> {
        let Some(operation) = Operation::for_tool(name) else {
            return Ok(None);
        };
        let canonical = Path::new(target).canonicalize()?;
        let path = canonical.as_path();
        let state = lock(&self.permissions)?;
        let persistent = self.persistent_rules()?;
        // Recursive searches cannot bypass a denial nested beneath their root.
        // A read denial also blocks search/list routes that could reveal the same data.
        if let Some(rule) = persistent
            .iter()
            .chain(state.session_rules.iter())
            .find(|r| {
                r.decision == Decision::Deny
                    && r.session_id.as_ref().is_none_or(|s| s == session)
                    && (r.operations.contains(&operation)
                        || r.operations.contains(&Operation::Read))
                    && ((path.starts_with(&r.path)
                        && (r.recursive
                            || path == r.path
                            || path.parent() == Some(r.path.as_path())))
                        || (operation != Operation::Read && r.path.starts_with(path)))
            })
        {
            return Ok(Some((Decision::Deny, rule.id.clone())));
        }
        let rules = persistent
            .iter()
            .chain(state.session_rules.iter())
            .filter(|r| r.matches(path, operation, session));
        let mut allow = None;
        for rule in rules {
            if rule.decision == Decision::Deny {
                return Ok(Some((Decision::Deny, rule.id.clone())));
            }
            if !crate::approval::sensitive(path) {
                allow = Some((Decision::Allow, rule.id.clone()));
            }
        }
        Ok(allow)
    }
    pub(crate) fn make_directory_rule(
        &self,
        body: &Value,
        session: Option<&str>,
    ) -> Result<DirectoryRule> {
        let raw = Path::new(required(body, "path")?);
        if !raw.is_absolute() {
            return Err(Error::new(400, "Directory must be an absolute path"));
        }
        let path = raw.canonicalize()?;
        if !path.is_dir()
            || crate::approval::sensitive(&path)
            || (path.starts_with(self.root.canonicalize()?)
                && !path.starts_with(self.root.canonicalize()?.join("workspace")))
        {
            return Err(Error::new(
                403,
                "Choose an ordinary directory; protected data cannot be granted",
            ));
        }
        let operations = match body.get("operations") {
            Some(v) => serde_json::from_value::<Vec<Operation>>(v.clone())?,
            None => vec![Operation::Read, Operation::List, Operation::Search],
        };
        if operations.is_empty() {
            return Err(Error::new(400, "Select at least one read operation"));
        }
        let decision = match body.get("decision") {
            Some(v) => serde_json::from_value(v.clone())?,
            None => Decision::Allow,
        };
        Ok(DirectoryRule {
            id: uuid::Uuid::new_v4().to_string(),
            kind: "directory".into(),
            identity: identity(&path)?,
            path,
            recursive: body["recursive"].as_bool().unwrap_or(true),
            operations,
            decision,
            lifetime: if session.is_some() {
                "session"
            } else {
                "persistent"
            }
            .into(),
            created_by: "user".into(),
            session_id: session.map(str::to_owned),
        })
    }
    pub(crate) fn save_directory_grant(
        &self,
        rule: DirectoryRule,
        expected_version: u64,
    ) -> Result<()> {
        let mut state = lock(&self.permissions)?;
        if state.version != expected_version {
            return Err(Error::new(409, "Permissions changed while waiting"));
        }
        // Adding a grant does not invalidate other same-scope requests. Revocation/edit does.
        if rule.session_id.is_some() {
            state.session_rules.push(rule)
        } else {
            let mut rules = self.persistent_rules()?;
            rules.push(rule);
            self.db()?.put(
                "permission_rules",
                &json!({"schema_version":1,"rules":rules}),
            )?;
        }
        Ok(())
    }
    pub(crate) fn permission_rules_api(&self, method: &str, body: &Value) -> Result<Value> {
        let mut state = lock(&self.permissions)?;
        let mut rules = self.persistent_rules()?;
        match method {
            "GET" => {}
            "POST" | "PUT" => {
                let mut rule = self.make_directory_rule(body, None)?;
                if method == "PUT" {
                    let id = required(body, "id")?;
                    let index = rules
                        .iter()
                        .position(|r| r.id == id)
                        .ok_or_else(|| Error::new(404, "Rule not found"))?;
                    rule.id = id.into();
                    rules[index] = rule;
                } else {
                    rules.push(rule)
                }
                self.db()?.put(
                    "permission_rules",
                    &json!({"schema_version":1,"rules":rules}),
                )?;
                state.version += 1;
                lock(&self.approvals)?.clear();
            }
            "DELETE" => {
                let id = required(body, "id")?;
                let before = rules.len() + state.session_rules.len();
                rules.retain(|r| r.id != id);
                state.session_rules.retain(|r| r.id != id);
                if before == rules.len() + state.session_rules.len() {
                    return Err(Error::new(404, "Rule not found"));
                }
                self.db()?.put(
                    "permission_rules",
                    &json!({"schema_version":1,"rules":rules}),
                )?;
                state.version += 1;
                lock(&self.approvals)?.clear();
            }
            _ => return Err(Error::new(405, "Unsupported permission operation")),
        }
        Ok(
            json!({"schema_version":1,"version":state.version,"rules":rules,"session_rules":state.session_rules}),
        )
    }
}
