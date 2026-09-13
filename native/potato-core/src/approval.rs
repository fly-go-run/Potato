//! Deterministic approval policy. A grant changes prompting, never file confinement.
use crate::{Error, Result, Runtime, lock};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub(crate) fn defaults() -> Value {
    json!({"approval_level":"AUTO","sandbox_mode":"workspace-write","reviewer":"model"})
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

#[derive(Clone, Debug)]
pub(crate) enum Reply {
    Once,
    Session,
    Deny,
    Directory(Box<crate::permissions::DirectoryRule>),
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

pub(crate) const GUIDANCE: &str = "Use existing user authorization; do not ask conversational permission before a tool's own approval. Prepare authorized concrete work first. Shell execution reports its actual OS sandbox and permissions. Sandboxed failures enter automatic recovery and independent approval; an approved retry continues in the same job. If a result requests diagnosis, inspect partial effects, prepare the remaining action and continue the task; a tool failure alone is not a final task failure. require_escalated requests one unsandboxed execution; network_access requests networking while retaining file isolation. Respect explicit denials and runtime restrictions; continue only with an authorized, materially safer alternative. Approval never silently changes session defaults.";

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
        let mode: crate::permissions::FileMode = serde_json::from_value(json!(mode))
            .map_err(|_| Error::new(400, "Unsupported native file access mode"))?;
        Ok(serde_json::to_value(mode)?.as_str().unwrap().to_owned())
    }

    pub(crate) fn approval_guidance(&self, body: &Value) -> Result<String> {
        let level = self.approval_level(body)?;
        let mode = self.file_mode(body)?;
        let policy = match level.as_str() {
            "AUTO" => {
                "Ordinary project reads/searches, enabled project edits and configured web search normally run without prompting. Other effects, sensitive paths and global memory writes pass through approval unless an applicable grant exists. Runtime deny rules still apply."
            }
            "STRICT" => {
                "Every action routed through the approval gate requires confirmation, including ordinary project reads and edits; reusable grants do not skip it. Call the needed tool without adding a separate conversational permission question."
            }
            "NEVER" => {
                "Actions that would need approval and interactive questions are rejected immediately, including actions with cached grants. Continue permitted project work or report missing input; do not request escalation or wait for an unavailable approval."
            }
            _ => unreachable!("approval_level validates the policy"),
        };
        let files = match mode.as_str() {
            "read-only" => {
                "Project file and project memory writes are disabled. Global memory_write still follows its separate approval gate. Use read/search tools for permitted inspection."
            }
            "workspace-write" => {
                "File tools can write within the project and supported memory locations, subject to approval and protected paths."
            }
            "danger-full-access" => {
                "Native shell can be invoked subject to approval. File tools retain their project/memory confinement and protected-path checks."
            }
            _ => unreachable!("file_mode validates the mode"),
        };
        let sandbox = crate::sandbox::status();
        let shell = if mode == "danger-full-access" {
            "Shell runs without OS file isolation, subject to approval."
        } else if sandbox["available"] == true {
            "Shell runs in an OS sandbox and enters automatic recovery on sandbox denial. Use network_access=true with justification for networking within file isolation, or require_escalated with justification for a single unsandboxed action. Inspect job recovery and partial effects if diagnosis is requested, then continue with a concrete recovery plan."
        } else {
            "OS sandbox is unavailable. The runtime routes necessary host execution through approval; no unreviewed fallback occurs. Prefer file tools when sufficient."
        };
        let reviewer = match level.as_str() {
            "NEVER" => "none; operations requiring approval are blocked",
            "STRICT" => "user; every operation is reviewed manually",
            _ if self.reviewer_config()?.reviewer == crate::permissions::Reviewer::Model => {
                "independent model auto-review; submit the concrete tool directly, without asking the user for approval first. The host will ask the user only if the review needs authorization or cannot complete"
            }
            _ => "user; submit the concrete tool and let the host present any required approval",
        };
        Ok(format!(
            "Approval policy: {}. Approval reviewer: {}. File access: {}. Shell sandbox: {sandbox}.\n{policy}\n{files}\n{shell}",
            level, reviewer, mode
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn audit_approval(
        &self,
        session: &str,
        name: &str,
        level: &str,
        outcome: &str,
        reason: &str,
        context: Value,
    ) -> Result<()> {
        let db = self.db()?;
        let key = format!("approval_audit:{session}");
        let mut events = db.get(&key, json!([]))?;
        let events_array = events
            .as_array_mut()
            .ok_or_else(|| Error::new(500, "Invalid approval audit"))?;
        let mut event = context;
        event["time"] = json!(chrono::Utc::now().to_rfc3339());
        event["tool"] = json!(name);
        event["policy"] = json!(level);
        event["outcome"] = json!(outcome);
        event["reason"] = json!(reason);
        event["source"] = json!(if reason.starts_with("directory_rule:") {
            "directory_rule"
        } else if reason == "session_exact_grant" {
            "session_exact_grant"
        } else if outcome.starts_with("allowed_") {
            "user"
        } else {
            "policy"
        });
        event["rule_id"] = reason
            .strip_prefix("directory_rule:")
            .map_or(Value::Null, |id| json!(id));
        events_array.push(event);
        if events_array.len() > 256 {
            events_array.remove(0);
        }
        db.put(&key, &events)
    }

    pub(crate) fn approval_grant_count(&self, session: &str) -> Result<usize> {
        let directory_count = lock(&self.permissions)?
            .session_rules
            .iter()
            .filter(|r| r.session_id.as_deref() == Some(session))
            .count();
        let mut grants = lock(&self.approval_grants)?;
        grants.retain(|g| g.expires > Instant::now());
        Ok(directory_count + grants.iter().filter(|g| g.session == session).count())
    }

    pub(crate) fn revoke_approval_grants(&self, session: &str) -> Result<()> {
        let mut permissions = lock(&self.permissions)?;
        permissions.version += 1;
        permissions
            .session_rules
            .retain(|r| r.session_id.as_deref() != Some(session));
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
        let directory = crate::permissions::Operation::for_tool(name).and_then(|_| {
            let path = Path::new(target);
            if sensitive(path) {
                None
            } else if path.is_dir() {
                Some(path.to_owned())
            } else {
                path.parent().map(Path::to_owned)
            }
        });
        let snapshot_path = if crate::permissions::Operation::for_tool(name).is_some() {
            Some(Path::new(target))
        } else if name == "execute_shell_command" {
            Some(Path::new(crate::string(args, "cwd")))
        } else {
            None
        };
        let target_snapshot = snapshot_path
            .map(crate::permissions::PathSnapshot::capture)
            .transpose()?;
        let version = self.permission_version()?;
        let review_generation = self.review_generation(session)?;
        let action_id = uuid::Uuid::new_v4().to_string();
        let started = Instant::now();
        let audit = |session: &str, name: &str, level: &str, outcome: &str, reason: &str| {
            self.audit_approval(session,name,level,outcome,reason,json!({"action_id":action_id,"target":target,"permission_version":version,"elapsed_ms":started.elapsed().as_millis(),"reviewer":"user","model":null,"tokens":null}))
        };
        let queue = directory
            .as_ref()
            .filter(|_| !automatic && self.approval_level(body).is_ok_and(|level| level == "AUTO"))
            .map(|p| self.permission_queue(session, p))
            .transpose()?;
        let _queue_guard = if let Some(queue) = queue.as_ref() {
            Some(tokio::select! {
                _=cancel.cancelled()=>return Err(Error::new(499,"Tool cancelled")),
                guard=queue.lock()=>guard,
            })
        } else {
            None
        };
        if self.permission_version()? != version
            || self.review_generation(session)? != review_generation
            || self.has_steering(session)?
        {
            return Err(Error::new(
                409,
                "Permissions or user instructions changed while waiting",
            ));
        }
        let level = self.approval_level(body)?;
        let rule = self.directory_decision(session, name, target)?;
        if let Some((crate::permissions::Decision::Deny, ref id)) = rule {
            audit(
                session,
                name,
                &level,
                "denied",
                &format!("directory_rule:{id}"),
            )?;
            return Err(Error::new(403, "Access denied by a directory rule"));
        }
        let epoch = *lock(&self.approval_epochs)?.get(session).unwrap_or(&0);
        if cancel.is_cancelled() {
            return Err(Error::new(499, "Tool cancelled"));
        }
        if automatic && level != "STRICT" {
            return audit(
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
            audit(session, name, &level, "denied", reason)?;
            return Err(Error::new(
                403,
                format!(
                    "Approval required: {reason}. NEVER policy rejects this action without waiting."
                ),
            ));
        }
        if level == "AUTO" {
            if let Some((crate::permissions::Decision::Allow, id)) = rule {
                return audit(
                    session,
                    name,
                    &level,
                    "allowed",
                    &format!("directory_rule:{id}"),
                );
            }
        }
        let key = json!({"tool":name,"args":args,"project":self.turn_project(body).await?,"mode":self.file_mode(body)?});
        let reusable = level == "AUTO"
            && matches!(
                name,
                "read_file"
                    | "list_directory"
                    | "grep_search"
                    | "glob_search"
            );
        if reusable {
            let mut grants = lock(&self.approval_grants)?;
            grants.retain(|g| g.expires > Instant::now());
            if grants.iter().any(|g| g.session == session && g.key == key) {
                drop(grants);
                return audit(session, name, &level, "allowed", "session_exact_grant");
            }
        }
        let mut review_context = Value::Null;
        if level == "AUTO"
            && self.reviewer_config()?.reviewer == crate::permissions::Reviewer::Model
        {
            let reviewed = self
                .review_action(
                    session,
                    name,
                    args,
                    target,
                    &key["project"],
                    &key["mode"],
                    reason,
                    &action_id,
                    version,
                    target_snapshot.as_ref(),
                    cancel,
                )
                .await?;
            if cancel.is_cancelled()
                || self.permission_version()? != version
                || self.review_generation(session)? != review_generation
                || self.has_steering(session)?
            {
                return Err(Error::new(
                    409,
                    "Model approval became stale before execution",
                ));
            }
            if let Some(snapshot) = target_snapshot.as_ref() {
                snapshot.verify()?;
            }
            if let Some(assessment) = reviewed.assessment.as_ref() {
                match assessment.outcome {
                    crate::reviewer::Outcome::Allow => return Ok(()),
                    crate::reviewer::Outcome::Deny if reviewed.event["circuit_breaker"] == true => {
                        return Err(Error::new(
                            crate::reviewer::CIRCUIT_BREAKER,
                            "助手重复申请同一个已拒绝动作，已停止本轮执行。请补充授权或调整任务后再继续。",
                        ));
                    }
                    crate::reviewer::Outcome::Deny => {
                        return Err(Error::new(
                            403,
                            format!(
                                "Model review denied this action: {}. Do not retry unchanged; obtain new user authorization or change the action.",
                                assessment.rationale
                            ),
                        ));
                    }
                    crate::reviewer::Outcome::AskUser => {}
                }
            }
            review_context = json!({"outcome":reviewed.event["outcome"],"rationale":reviewed.event["rationale"],"failure":reviewed.failure});
        }
        let id = action_id.clone();
        let (tx, rx) = oneshot::channel();
        let mut view = json!({"request_id":id,"session_id":session,"root_session_id":session,"user_id":"default",
            "background_job_id":args["_job_id"],"tool_name":name,"tool_params":args,"severity":"medium","findings_count":1,
            "findings_summary":reason,"source_type":"native","driver":null,
            "created_at":chrono::Utc::now().timestamp(),"timeout_seconds":300,"tool_display_name":name,
            "tool_source":"Potato","exact_target":target,"action_detail":args.to_string(),
            "justification":crate::string(args,"justification"),"permission_increment":reason,
            "allow_session":reusable,"allow_directory":directory.is_some() && level=="AUTO", "suggested_directory":directory,"directory_recursive":true,"operations":["read","list","search"],"permission_version":version,"similar_target":"","is_generalized":false});
        if !review_context.is_null() {
            view["review_outcome"] = review_context["outcome"].clone();
            view["review_rationale"] = review_context["rationale"].clone();
            view["review_failure"] = review_context["failure"].clone();
        }
        audit(session, name, &level, "requested", reason)?;
        lock(&self.approvals)?.insert(id.clone(), crate::Approval { view, reply: tx });
        if self.permission_version()? != version
            || self.review_generation(session)? != review_generation
            || self.has_steering(session)?
        {
            lock(&self.approvals)?.remove(&id);
            return Err(Error::new(
                409,
                "Permissions or user instructions changed while creating approval",
            ));
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
                Ok(Ok(Reply::Directory(rule))) => (Reply::Directory(rule), "allowed_directory"),
                Ok(Ok(Reply::Deny)) => (Reply::Deny, "denied"),
                Ok(Err(_)) => (Reply::Deny, "unavailable"),
                Err(_) => (Reply::Deny, "expired"),
            }
        };
        drop(pending);
        if cancel.is_cancelled() {
            return Err(Error::new(499, "Tool cancelled"));
        }
        if self.permission_version()? != version
            || self.review_generation(session)? != review_generation
            || self.has_steering(session)?
            || self.approval_level(body)? != level
            || self.file_mode(body)? != key["mode"]
        {
            audit(session, name, &level, "denied", "policy_changed")?;
            return Err(Error::new(
                409,
                "Permissions changed while waiting; retry under the current policy",
            ));
        }
        if matches!(reply, Reply::Deny) {
            audit(session, name, &level, outcome, reason)?;
            return Err(Error::new(
                403,
                format!(
                    "Tool action {outcome}: {reason}. Continue other authorized work; do not retry unchanged."
                ),
            ));
        }
        if let Some(snapshot) = target_snapshot {
            snapshot.verify()?;
        }
        if let Reply::Directory(rule) = reply {
            if directory.is_none() || level != "AUTO" {
                return Err(Error::new(403, "Directory grant unavailable"));
            }
            // Recheck the exact target and the selected root before committing an explicit user rule.
            let current = self
                .make_directory_rule(&serde_json::to_value(&rule)?, rule.session_id.as_deref())?;
            if !rule.unchanged() || current.path != rule.path {
                return Err(Error::new(409, "Directory changed while waiting"));
            }
            let rule_id = rule.id.clone();
            self.save_directory_grant(*rule, version)?;
            return audit(
                session,
                name,
                &level,
                outcome,
                &format!("directory_rule:{rule_id}"),
            );
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
        audit(session, name, &level, outcome, reason)
    }
}
