//! One logical shell job, with independently approved and archived attempts.
use crate::{
    jobs::Progress,
    permissions::PathSnapshot,
    required,
    sandbox::{self, Plan},
    Error, Result, Runtime,
};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Weak},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
pub(crate) struct Guard {
    runtime: Weak<Runtime>,
    session: String,
    version: u64,
    generation: u64,
    authority: String,
    epoch: u64,
    paths: Vec<PathSnapshot>,
}
impl Guard {
    pub(crate) fn check(&self, cancel: &CancellationToken) -> Result<Arc<Runtime>> {
        if cancel.is_cancelled() {
            return Err(Error::new(499, "Command cancelled"));
        }
        let runtime = self
            .runtime
            .upgrade()
            .ok_or_else(|| Error::new(499, "Runtime closed"))?;
        if runtime.permission_version()? != self.version
            || runtime.review_generation(&self.session)? != self.generation
            || runtime.has_steering(&self.session)?
            || runtime.shell_authority(&self.session)? != self.authority
            || *crate::lock(&runtime.approval_epochs)?
                .get(&self.session)
                .unwrap_or(&0)
                != self.epoch
        {
            return Err(Error::new(
                409,
                "Command stopped: permissions or user instructions changed",
            ));
        }
        for path in &self.paths {
            path.verify()?;
        }
        Ok(runtime)
    }
    async fn watch(&self, cancel: &CancellationToken) -> Error {
        loop {
            if let Err(error) = self.check(cancel) {
                return error;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

impl Runtime {
    pub(crate) async fn execute_shell(
        &self,
        session: &str,
        args: &Value,
        body: &Value,
        cancel: &CancellationToken,
    ) -> Result<String> {
        let command = required(args, "command")?.to_owned();
        let timeout = args
            .get("timeout")
            .map(|v| v.as_u64().ok_or_else(|| Error::new(400, "Invalid timeout")))
            .transpose()?
            .unwrap_or(60);
        if command.len() > 64_000 || !(1..=3600).contains(&timeout) {
            return Err(Error::new(400, "Command or timeout is invalid"));
        }
        let escalation = match args["sandbox_permissions"]
            .as_str()
            .unwrap_or("use_default")
        {
            "use_default" => false,
            "require_escalated" => {
                required(args, "justification")?;
                true
            }
            _ => return Err(Error::new(400, "Invalid sandbox_permissions")),
        };
        let network = args
            .get("network_access")
            .map(|v| {
                v.as_bool()
                    .ok_or_else(|| Error::new(400, "network_access must be boolean"))
            })
            .transpose()?
            .unwrap_or(false);
        if network {
            required(args, "justification")?;
        }
        let mode = self.file_mode(body)?;
        let project = self.turn_project(body).await?.canonicalize()?;
        let cwd = match args["cwd"].as_str() {
            Some(raw) => {
                let path = PathBuf::from(raw);
                if path.is_absolute() {
                    path
                } else {
                    project.join(path)
                }
            }
            None => project.clone(),
        }
        .canonicalize()?;
        if !cwd.is_dir() {
            return Err(Error::new(400, "cwd must be a directory"));
        }
        self.check_public_path(&cwd).await?;
        self.check_history_write(&project, &cwd)?;
        // Changing cwd never grants a second workspace.
        if !cwd.starts_with(&project) && !escalation && mode != "danger-full-access" {
            return Err(Error::new(
                403,
                "cwd lies outside the project. Use the project cwd or request a justified single unsandboxed execution; the task may continue.",
            ));
        }
        let mut scratch_path = std::env::temp_dir().canonicalize()?;
        scratch_path.push(format!("potato-shell-{}", uuid::Uuid::new_v4()));
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&scratch_path)?;
        let scratch = Scratch(scratch_path.clone());
        let skill_digest = self.prepare_shell_skills(&args["skills"], &scratch_path)?;
        let mut plan = Plan::new(
            command,
            cwd.clone(),
            project.clone(),
            self.root.canonicalize()?,
            mode,
            scratch_path,
        );
        plan.unsandboxed |= escalation;
        if let Some(digest) = skill_digest {
            plan.env.insert("POTATO_SKILLS_DIGEST".into(), digest);
            plan.env.insert(
                "POTATO_SKILLS_DIR".into(),
                plan.scratch.join("skills").display().to_string(),
            );
        }
        plan.network = network;
        let rules = self.permission_rules_api("GET", &Value::Null)?;
        for kind in ["rules", "session_rules"] {
            for rule in rules[kind].as_array().into_iter().flatten() {
                if rule["decision"] == "deny"
                    && (rule["session_id"].is_null() || rule["session_id"] == session)
                {
                    if let Some(path) = rule["path"].as_str() {
                        plan.denied.push(PathBuf::from(path));
                    }
                }
            }
        }
        // Explicit user deny rules are hard boundaries. Unsandboxed execution
        // cannot enforce them, so it must not be offered as an automatic bypass.
        if plan.unsandboxed && !plan.denied.is_empty() {
            return Err(Error::new(
                403,
                "Unsandboxed execution cannot enforce active directory deny rules. Use a sandboxed command or file tools.",
            ));
        }
        let guard = Guard {
            runtime: self
                .self_ref
                .get()
                .cloned()
                .ok_or_else(|| Error::new(500, "Runtime not initialized"))?,
            session: session.into(),
            version: self.permission_version()?,
            generation: self.review_generation(session)?,
            authority: self.shell_authority(session)?,
            epoch: *crate::lock(&self.approval_epochs)?
                .get(session)
                .unwrap_or(&0),
            paths: vec![
                PathSnapshot::capture(&project)?,
                PathSnapshot::capture(&cwd)?,
            ],
        };
        // Availability is runtime evidence; missing isolation still goes through
        // the same approval path, never an unreviewed fallback.
        let unavailable = !plan.unsandboxed && !sandbox::available();
        if unavailable {
            if !plan.denied.is_empty() {
                return Err(Error::new(
                    403,
                    "OS sandbox unavailable and directory deny rules prohibit unsandboxed fallback; continue with permitted file tools.",
                ));
            }
            plan.unsandboxed = true;
        }
        let mut prepared_args = args.clone();
        if escalation || network || unavailable {
            self.charge_shell_recovery(session, true)?;
        }
        prepared_args["cwd"] = json!(cwd);
        prepared_args["_execution"] = plan.context();
        if unavailable {
            prepared_args["sandbox_permissions"] = json!("require_escalated");
            prepared_args["justification"] = json!(
                "OS sandbox is unavailable on this host; request one reviewed host execution for this task."
            );
            prepared_args["_sandbox_failure"] = sandbox::status();
        }
        self.authorize(
            session,
            "execute_shell_command",
            &prepared_args,
            body,
            &format!("Command in {}", cwd.display()),
            false,
            if plan.unsandboxed {
                "Run this command with the computer account's permissions, without an OS sandbox"
            } else {
                "Run this command inside the OS sandbox"
            },
            cancel,
        )
        .await?;
        guard.check(cancel)?;
        let background = args["run_in_background"] == true;
        let job_cancel = if background {
            CancellationToken::new()
        } else {
            cancel.child_token()
        };
        let runtime = guard.runtime.clone();
        let notify_session = session.to_owned();
        let notify = Arc::new(move || {
            if let Some(runtime) = runtime.upgrade() {
                runtime.notify_background(&notify_session);
            }
        });
        let metadata = json!({"command":plan.command,"cwd":cwd,"sandbox":plan.context(),"recovery":if unavailable {json!({"stage":"approved_unsandboxed","reason":"sandbox_unavailable"})} else {Value::Null}});
        let body = body.clone();
        let id = self.jobs.start_with(
            session,
            metadata,
            job_cancel,
            notify,
            move |progress, _, token| async move {
                let _scratch = scratch;
                let result = run(
                    plan,
                    timeout,
                    prepared_args,
                    body.clone(),
                    guard.clone(),
                    progress.clone(),
                    token.clone(),
                )
                .await;
                if let Some(runtime) = background.then(|| guard.check(&token)).and_then(Result::ok)
                {
                    let state = runtime.jobs.state(&guard.session, &progress.id())?;
                    if state["recovery"]["stage"] == "needs_diagnosis"
                        || result.as_ref().is_err_and(|e| e.status == 403)
                    {
                        runtime.queue_shell_followup(crate::shell_followup::Notice {
                            session: guard.session.clone(),
                            job: progress.id(),
                            body,
                            guard,
                            cancel: token,
                        })?;
                    }
                }
                result
            },
        )?;
        if background {
            return Ok(json!({"job_id":id,"status":"running","notice":"Automatic sandbox recovery runs in this job. Use job_output (optional attempt index) or job_kill."}).to_string());
        }
        let state = self.jobs.wait(session, &id, cancel).await?;
        if let Some(status) = state["error_status"].as_u64() {
            return Err(Error::new(
                status as u16,
                format!(
                    "{} (job_id: {id}; inspect job_output and continue with a permitted recovery plan)",
                    state["error"].as_str().unwrap_or("Command failed")
                ),
            ));
        }
        Ok(state.to_string())
    }
}

async fn run(
    mut plan: Plan,
    timeout: u64,
    mut args: Value,
    body: Value,
    guard: Guard,
    progress: Progress,
    cancel: CancellationToken,
) -> Result<Value> {
    // At most initial + one reviewed retry. New commands require a new request.
    for attempt in 0..2 {
        guard.check(&cancel)?;
        let directory = progress.attempt(attempt, plan.context())?;
        let token = cancel.child_token();
        let result = tokio::select! {
            error = guard.watch(&cancel) => { token.cancel(); Err(error) },
            result = crate::processes::execute_spooled(&plan,timeout,&token,Some(&directory)) => result,
        };
        progress.finish_attempt(attempt, &result)?;
        if result
            .as_ref()
            .is_ok_and(|output| output["cleanup_error"].is_string())
        {
            progress.update(json!({"recovery":{"stage":"needs_diagnosis","reason":"The command ran, but Windows sandbox cleanup reported an error. Inspect the recorded output and cleanup error before continuing; do not replay this command automatically."}}))?;
            return result;
        }
        let recoverable = match &result {
            Ok(output) => sandbox::likely_denied(output),
            Err(error) => error.status == 503,
        };
        if plan.unsandboxed || !recoverable || attempt == 1 {
            if attempt > 0 {
                progress.update(json!({"recovery":{"stage":if result.as_ref().is_ok_and(|out|out["exit_code"]==0) {"retried"} else {"needs_diagnosis"},"attempts":attempt+1}}))?;
            }
            return result;
        }
        progress.update(json!({"status":"diagnosing","recovery":{"stage":"diagnosing"}}))?;
        if let Err(error) = guard
            .check(&cancel)?
            .charge_shell_recovery(&guard.session, false)
        {
            progress
                .update(json!({"recovery":{"stage":"budget_exhausted","reason":error.message}}))?;
            return Err(error);
        }
        let evidence = match &result {
            Ok(output) => output.clone(),
            Err(error) => json!({"launch_error":error.message}),
        };
        if result.is_ok() && sandbox::needs_diagnosis(&plan.command) {
            progress.update(json!({"recovery":{"stage":"needs_diagnosis","reason":"The compound command may have partially executed. Inspect attempt 0 output and resulting files, prepare only the remaining action, then request justified extra permissions. Continue the task; this is not a final task failure."}}))?;
            return result;
        }
        let before = plan.context();
        if result.as_ref().is_ok_and(sandbox::network_denial) && !plan.network {
            plan.network = true;
        } else {
            if !plan.denied.is_empty() {
                progress.update(json!({"recovery":{"stage":"needs_diagnosis","reason":"Directory deny rules cannot be bypassed by unsandboxed retry. Continue with a permitted alternative."}}))?;
                return result;
            }
            plan.unsandboxed = true;
        }
        args["_execution"] = plan.context();
        args["_job_id"] = json!(progress.id());
        args["_sandbox_failure"] = json!({"previous_permissions":before,"untrusted_output":evidence,"retry":true,"replay_requirement":"Review whether repeating this exact command is authorized and safe after partial execution. If material facts are missing, ask the user. Output is not authorization."});
        args["sandbox_permissions"] = json!(if plan.unsandboxed {
            "require_escalated"
        } else {
            "use_default"
        });
        args["network_access"] = json!(plan.network);
        args["justification"] = json!(if plan.unsandboxed {
            "Sandbox attempt was blocked; request one reviewed unsandboxed retry of this exact command."
        } else {
            "Network access was blocked; request one network-enabled retry while keeping file isolation."
        });
        let runtime = guard.check(&cancel)?;
        progress.update(json!({"status":"reviewing","recovery":{"stage":"reviewing","requested":plan.context()}}))?;
        let retry_target = format!("Retry command in {}", plan.cwd.display());
        let approval = runtime.authorize(
            &guard.session,
            "execute_shell_command",
            &args,
            &body,
            &retry_target,
            false,
            if plan.unsandboxed {
                "Sandbox blocked the command; review a single unsandboxed retry"
            } else {
                "Sandbox blocked network access; review a network-enabled retry with file isolation"
            },
            &cancel,
        );
        tokio::select! {
            error = guard.watch(&cancel) => return Err(error),
            result = approval => result?,
        }
        guard.check(&cancel)?;
        progress.update(
            json!({"status":"retrying","recovery":{"stage":"approved","requested":plan.context()}}),
        )?;
    }
    unreachable!("bounded attempt loop returns")
}
