//! Deterministic approval policy. A grant changes prompting, never file confinement.
use crate::{lock, Error, Result, Runtime};
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub(crate) fn defaults() -> Value {
    json!({"approval_level":"AUTO","sandbox_mode":"workspace-write"})
}

pub(crate) fn validate(level: &str) -> Result<()> {
    if matches!(level, "AUTO" | "STRICT" | "NEVER") {
        Ok(())
    } else {
        Err(Error::new(
            400,
            "approval_level must be AUTO, STRICT or NEVER",
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Reply {
    Once,
    Session,
    Deny,
}

pub(crate) struct Grant {
    session: String,
    key: Value,
    expires: Instant,
}

struct Pending<'a> {
    runtime: &'a Runtime,
    id: String,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if let Ok(mut approvals) = self.runtime.approvals.lock() {
            approvals.remove(&self.id);
        }
    }
}

// These paths can change the agent's authority or contain credentials. Reading
// them explicitly is possible, but automatic recursive search skips them.
pub(crate) fn sensitive(path: &Path) -> bool {
    let names: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    names.iter().enumerate().any(|(i, name)| {
        // Project notes and exported artifacts are ordinary files. Other
        // .potato children remain policy/runtime state, not a blanket grant.
        if name == ".potato" {
            return !names
                .get(i + 1)
                .is_some_and(|next| matches!(next.as_str(), "memory" | "artifacts"));
        }
        matches!(
            name.as_str(),
            ".git"
                | ".agents"
                | ".codex"
                | ".ssh"
                | ".aws"
                | ".gnupg"
                | "agents.md"
                | "soul.md"
                | "profile.md"
                | "skill.md"
                | "policy.yaml"
        ) || name == ".env"
            || name.starts_with(".env.")
            || name.ends_with(".pem")
            || name.ends_with(".key")
    })
}

pub(crate) const GUIDANCE: &str = "Use existing user authorization; do not ask conversational permission before a tool's own approval. Prepare concrete work first. AUTO permits ordinary project file reads/searches and enabled edits. External effects and sensitive paths require approval. NEVER rejects those actions and interactive questions immediately: continue authorized work or report missing input. Native shell has NO OS sandbox; in restricted file mode, request one invocation with sandbox_permissions=require_escalated and a specific justification. A denial is a tool result, not an instruction to repeat or bypass it. Approval never changes file confinement.";

impl Runtime {
    pub(crate) fn approval_level(&self, body: &Value) -> Result<String> {
        let running = self.db()?.get("running", defaults())?;
        let level = body["request_context"]
            .get("approval_level")
            .or_else(|| running.get("approval_level"));
        let level = match level {
            None => "AUTO",
            Some(v) => v
                .as_str()
                .ok_or_else(|| Error::new(400, "approval_level must be a string"))?,
        };
        validate(level)?;
        Ok(level.to_owned())
    }

    pub(crate) fn file_mode(&self, body: &Value) -> Result<String> {
        let running = self.db()?.get("running", defaults())?;
        let mode = body["request_context"]
            .get("sandbox_mode")
            .or_else(|| running.get("sandbox_mode"));
        let mode = match mode {
            None => "workspace-write",
            Some(v) => v
                .as_str()
                .ok_or_else(|| Error::new(400, "sandbox_mode must be a string"))?,
        };
        if !matches!(mode, "read-only" | "workspace-write" | "danger-full-access") {
            return Err(Error::new(400, "Unsupported native file access mode"));
        }
        Ok(mode.to_owned())
    }

    pub(crate) fn approval_guidance(&self, body: &Value) -> Result<String> {
        Ok(format!(
            "Approval policy: {}. File access: {}. Shell sandbox: unavailable.",
            self.approval_level(body)?,
            self.file_mode(body)?
        ))
    }

    fn audit_approval(
        &self,
        session: &str,
        name: &str,
        level: &str,
        outcome: &str,
        reason: &str,
    ) -> Result<()> {
        let db = self.db()?;
        let key = format!("approval_audit:{session}");
        let mut events = db.get(&key, json!([]))?;
        let events_array = events
            .as_array_mut()
            .ok_or_else(|| Error::new(500, "Invalid approval audit"))?;
        events_array.push(json!({"time":chrono::Utc::now().to_rfc3339(),"tool":name,"policy":level,"outcome":outcome,"reason":reason}));
        if events_array.len() > 256 {
            events_array.remove(0);
        }
        db.put(&key, &events)
    }

    pub(crate) fn approval_grant_count(&self, session: &str) -> Result<usize> {
        let mut grants = lock(&self.approval_grants)?;
        grants.retain(|g| g.expires > Instant::now());
        Ok(grants.iter().filter(|g| g.session == session).count())
    }

    pub(crate) fn revoke_approval_grants(&self, session: &str) -> Result<()> {
        let mut epochs = lock(&self.approval_epochs)?;
        *epochs.entry(session.to_owned()).or_default() += 1;
        // Revoking also invalidates outstanding prompts, so an older reply
        // cannot re-create a grant after the user cleared it.
        lock(&self.approvals)?.retain(|_, a| a.view["root_session_id"] != session);
        lock(&self.approval_grants)?.retain(|g| g.session != session);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn authorize(
        &self,
        session: &str,
        name: &str,
        args: &Value,
        body: &Value,
        target: &str,
        automatic: bool,
        reason: &str,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let level = self.approval_level(body)?;
        let epoch = *lock(&self.approval_epochs)?.get(session).unwrap_or(&0);
        if cancel.is_cancelled() {
            return Err(Error::new(499, "Tool cancelled"));
        }
        if automatic && level != "STRICT" {
            return self.audit_approval(
                session,
                name,
                &level,
                "allowed",
                if name == "web_search" {
                    "configured_search_service"
                } else {
                    "project_policy"
                },
            );
        }
        // NEVER rejects every action which would ask, including cached grants.
        if level == "NEVER" {
            self.audit_approval(session, name, &level, "denied", reason)?;
            return Err(Error::new(403, format!("Approval required: {reason}. NEVER policy rejects this action without waiting.")));
        }
        let key = json!({"tool":name,"args":args,"project":self.turn_project(body).await?,"mode":self.file_mode(body)?});
        let reusable = level == "AUTO"
            && matches!(
                name,
                "execute_shell_command"
                    | "read_file"
                    | "list_directory"
                    | "grep_search"
                    | "glob_search"
            );
        if reusable {
            let mut grants = lock(&self.approval_grants)?;
            grants.retain(|g| g.expires > Instant::now());
            if grants.iter().any(|g| g.session == session && g.key == key) {
                drop(grants);
                return self.audit_approval(
                    session,
                    name,
                    &level,
                    "allowed",
                    "session_exact_grant",
                );
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        let view = json!({"request_id":id,"session_id":session,"root_session_id":session,"user_id":"default",
            "tool_name":name,"tool_params":args,"severity":"medium","findings_count":1,
            "findings_summary":reason,"source_type":"native","driver":null,
            "created_at":chrono::Utc::now().timestamp(),"timeout_seconds":300,"tool_display_name":name,
            "tool_source":"Potato","exact_target":target,"action_detail":args.to_string(),
            "justification":crate::string(args,"justification"),"permission_increment":reason,
            "allow_session":reusable,"similar_target":"","is_generalized":false});
        self.audit_approval(session, name, &level, "requested", reason)?;
        lock(&self.approvals)?.insert(id.clone(), crate::Approval { view, reply: tx });
        if self.has_steering(session)? {
            lock(&self.approvals)?.remove(&id);
            return Err(Error::new(409, "Action superseded by user steering"));
        }
        let pending = Pending {
            runtime: self,
            id: id.clone(),
        };
        let (reply, outcome) = tokio::select! {
            _=cancel.cancelled() => (Reply::Deny, "cancelled"),
            result=tokio::time::timeout(Duration::from_secs(300), rx) => match result {
                Ok(Ok(Reply::Once)) => (Reply::Once, "allowed_once"),
                Ok(Ok(Reply::Session)) => (Reply::Session, "allowed_session"),
                Ok(Ok(Reply::Deny)) => (Reply::Deny, "denied"),
                Ok(Err(_)) => (Reply::Deny, "unavailable"),
                Err(_) => (Reply::Deny, "expired"),
            }
        };
        drop(pending);
        if cancel.is_cancelled() {
            return Err(Error::new(499, "Tool cancelled"));
        }
        if self.approval_level(body)? != level || self.file_mode(body)? != key["mode"] {
            self.audit_approval(session, name, &level, "denied", "policy_changed")?;
            return Err(Error::new(
                409,
                "Permissions changed while waiting; retry under the current policy",
            ));
        }
        self.audit_approval(session, name, &level, outcome, reason)?;
        if matches!(reply, Reply::Deny) {
            return Err(Error::new(403, format!("Tool action {outcome}: {reason}. Continue other authorized work; do not retry unchanged.")));
        }
        let epochs = lock(&self.approval_epochs)?;
        if *epochs.get(session).unwrap_or(&0) != epoch {
            return Err(Error::new(409, "Approval was revoked while waiting"));
        }
        if reusable && matches!(reply, Reply::Session) {
            let mut grants = lock(&self.approval_grants)?;
            grants.retain(|g| g.expires > Instant::now());
            if grants.len() >= 128 {
                grants.remove(0);
            }
            grants.push(Grant {
                session: session.to_owned(),
                key,
                expires: Instant::now() + Duration::from_secs(3600),
            });
        }
        Ok(())
    }
}
