use crate::{lock, required, string, Error, Result, Runtime};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

pub(crate) use crate::tool_registry::definitions;
use crate::tool_registry::{self, Access, Builtin};

// Resolve existing ancestors too, so a new file beneath a redirected directory
// receives the same protection as an existing file.
fn resolve_write_path(path: &Path) -> Result<PathBuf> {
    match path.canonicalize() {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| Error::new(400, "Invalid write path"))?;
            let name = path
                .file_name()
                .ok_or_else(|| Error::new(400, "Invalid write path"))?;
            Ok(resolve_write_path(parent)?.join(name))
        }
        Err(error) => Err(error.into()),
    }
}

impl Runtime {
    fn check_history_write(&self, project: &Path, target: &Path) -> Result<()> {
        let history = self.root.canonicalize()?.join("workspace/history");
        let path = if target.is_absolute() {
            target.to_owned()
        } else {
            project.join(target)
        };
        if path.starts_with(&history)
            || resolve_write_path(&path)?.starts_with(&resolve_write_path(&history)?)
        {
            return Err(Error::new(
                403,
                "Runtime history is read-only for model file tools",
            ));
        }
        Ok(())
    }

    pub(crate) async fn execute_tool(
        &self,
        session: &str,
        name: &str,
        args: &Value,
        body: &Value,
        cancel: &CancellationToken,
        emit: &crate::Emit,
    ) -> Result<String> {
        self.approval_level(body)?;
        let initial_file_mode = self.file_mode(body)?;
        if cancel.is_cancelled() {
            return Err(Error::new(499, "Tool cancelled"));
        }
        let spec = tool_registry::lookup(name);
        let builtin = spec.as_ref().map(|s| s.kind);
        let access = spec.as_ref().map(|s| s.access).unwrap_or(Access::None);
        match builtin {
            Some(Builtin::JobOutput) => {
                return Ok(self
                    .jobs
                    .output(session, required(args, "job_id")?, args)
                    .await?
                    .to_string())
            }
            Some(Builtin::JobList) => return Ok(self.jobs.list(session)?.to_string()),
            Some(Builtin::JobKill) => {
                return Ok(self
                    .jobs
                    .cancel(session, required(args, "job_id")?)?
                    .to_string())
            }
            _ => {}
        }
        if builtin == Some(Builtin::Recall) {
            return Ok(self.db()?.recall(session, args)?.to_string());
        }
        if builtin == Some(Builtin::Usage) {
            let db = self.db()?;
            let chat = db
                .chats()?
                .into_iter()
                .find(|c| c["session_id"] == session)
                .ok_or_else(|| Error::new(404, "Chat not found"))?;
            let id = required(&chat, "id")?;
            return Ok(json!({"usage":db.get(&format!("usage:{id}"),Value::Null)?,"totals":db.get(&format!("usage_totals:{id}"),Value::Null)?,"context":db.get(&format!("context_stats:{id}"),Value::Null)?}).to_string());
        }
        if builtin == Some(Builtin::ReadSkill) {
            return self.read_skill(args);
        }
        if builtin == Some(Builtin::MemorySearch) {
            let root = match args["scope"].as_str().unwrap_or("global") {
                "global" => self.memory_root()?,
                "project" => self.project_memory_root(&self.turn_project(body).await?, false)?,
                _ => return Err(Error::new(400, "scope must be global or project")),
            };
            let query = required(args, "query")?.to_owned();
            let args = args.clone();
            let cancel = cancel.clone();
            return tokio::task::spawn_blocking(move || {
                crate::memory::search_notes(&root, &query, &args, &cancel).map(|v| v.to_string())
            })
            .await
            .map_err(|_| Error::new(500, "Memory search failed"))?;
        }
        if builtin == Some(Builtin::AskUser) {
            if self.approval_level(body)? == "NEVER" {
                return Err(Error::new(403, "Interactive questions are unavailable in NEVER mode; continue with available information or report what is missing"));
            }
            let answer = self.ask_user(session, args, cancel).await?;
            let question: Value = serde_json::from_str(&answer)?;
            let mut labels = Vec::new();
            if let Some(selected) = question["answer"]["selected"].as_array() {
                for id in selected {
                    if let Some(option) = question["options"]
                        .as_array()
                        .and_then(|options| options.iter().find(|o| o["id"] == *id))
                    {
                        labels.push(string(option, "label"));
                    }
                }
            }
            let title = string(&question, "title");
            let text = if question["status"] == "skipped" {
                "已跳过".into()
            } else {
                format!(
                    "{}{}{}",
                    labels.join("、"),
                    if labels.is_empty() { "" } else { "\n" },
                    string(&question["answer"], "text")
                )
            };
            let id = uuid::Uuid::new_v4().to_string();
            let mut frame = crate::protocol::message(
                &id,
                "message",
                "user",
                json!([crate::protocol::text(&id, &text, false)]),
                "completed",
            );
            frame["metadata"] =
                json!({"question_request_id":question["request_id"],"question_title":title});
            let mut db = self.db()?;
            let chat = db.ensure_chat(session, title)?;
            db.append(required(&chat, "id")?, &frame, None)?;
            drop(db);
            emit(frame)?;
            return Ok(answer);
        }
        let mcp = if name.starts_with("mcp_") {
            Some(self.mcp_target(name)?)
        } else {
            None
        };
        let computer = crate::computer::definitions()
            .iter()
            .any(|tool| tool["function"]["name"] == name);
        if mcp.is_none() && !computer && spec.is_none() {
            return Err(Error::new(400, "Unknown native tool"));
        }
        let mut args = args.clone();
        if access == Access::WritePath || builtin == Some(Builtin::ReadFile) {
            if let Some(path) = args.get("file_path").cloned() {
                args["path"] = path;
            }
        }
        if builtin == Some(Builtin::WebSearch) && args["query"].is_null() {
            args["query"] = args["search_term"].clone();
        }
        if access == Access::ReadPath {
            if args["path"].is_null() {
                args["path"] = json!(".");
            }
            let path = PathBuf::from(required(&args, "path")?);
            if !path.is_absolute() {
                args["path"] = json!(self.turn_project(body).await?.join(path));
            }
        }
        let shell_project = if builtin == Some(Builtin::Shell) {
            let mode = self.file_mode(body)?;
            let escalation = match args["sandbox_permissions"]
                .as_str()
                .unwrap_or("use_default")
            {
                "use_default" => false,
                "require_escalated" => {
                    required(&args, "justification")?;
                    true
                }
                _ => return Err(Error::new(400, "Unsupported sandbox_permissions")),
            };
            if mode != "danger-full-access" && !escalation {
                return Err(Error::new(403, "Native shell has no OS sandbox. Retry with sandbox_permissions=require_escalated and justification to request this unsandboxed action once, or use project file tools."));
            }
            let mut project = self.turn_project(body).await?;
            if let Some(cwd) = args["cwd"].as_str() {
                let path = PathBuf::from(cwd);
                project = tokio::fs::canonicalize(if path.is_absolute() {
                    path
                } else {
                    project.join(path)
                })
                .await?;
                self.check_public_path(&project).await?;
                if !project.is_dir() {
                    return Err(Error::new(400, "cwd must be a directory"));
                }
            }
            self.check_history_write(&project, &project)?;
            args["cwd"] = json!(project.to_string_lossy());
            Some(project)
        } else {
            None
        };
        let prepared_write = if access == Access::WritePath {
            let mode = self.file_mode(body)?;
            if !matches!(mode.as_str(), "workspace-write" | "danger-full-access") {
                return Err(Error::new(
                    403,
                    "Enable project writing before editing files",
                ));
            }
            let mut project = self.turn_project(body).await?;
            let target = PathBuf::from(required(&args, "path")?);
            let memory = self.memory_root()?;
            if target.is_absolute() && target.starts_with(&memory) {
                project = memory;
            }
            self.check_history_write(&project, &target)?;
            let args = args.clone();
            let operation = name.to_owned();
            Some(
                tokio::task::spawn_blocking(move || {
                    if operation == "write_file" {
                        crate::file_ops::PreparedWrite::prepare(
                            &project,
                            &target,
                            args["content"]
                                .as_str()
                                .ok_or_else(|| Error::new(400, "content is required"))?,
                        )
                    } else {
                        crate::file_ops::PreparedWrite::prepare_change(
                            &project, &target, &operation, &args,
                        )
                    }
                })
                .await
                .map_err(|_| Error::new(500, "File preparation failed"))??,
            )
        } else {
            None
        };
        let prepared_memory = if builtin == Some(Builtin::MemoryWrite) {
            if args["scope"] == "project" {
                let mode = self.file_mode(body)?;
                if !matches!(mode.as_str(), "workspace-write" | "danger-full-access") {
                    return Err(Error::new(
                        403,
                        "Enable project writing before saving project memory",
                    ));
                }
            }
            let root = match args["scope"].as_str().unwrap_or("global") {
                "global" => self.memory_root()?,
                "project" => {
                    let project = self.turn_project(body).await?;
                    self.check_history_write(&project, Path::new(".potato/memory"))?;
                    self.project_memory_root(&project, true)?
                }
                _ => return Err(Error::new(400, "scope must be global or project")),
            };
            let path = required(&args, "path")?;
            let content = args["content"]
                .as_str()
                .ok_or_else(|| Error::new(400, "content is required"))?;
            self.check_history_write(&root, Path::new(path))?;
            let write =
                crate::memory::prepare_note(&root, path, content, args.get("expected_content"))?;
            Some((write, root.join(path)))
        } else {
            None
        };
        let target = if let Some((config, tool)) = &mcp {
            format!("{} / {}", required(config, "name")?, tool)
        } else if computer {
            self.computer_target(session, name, &args)?
        } else if builtin == Some(Builtin::WebSearch) {
            required(&args, "query")?.to_owned()
        } else if shell_project.is_some() {
            format!(
                "Unsandboxed command in {}: {}",
                string(&args, "cwd"),
                required(&args, "command")?
            )
        } else if let Some((_, path)) = &prepared_memory {
            path.display().to_string()
        } else if matches!(access, Access::MemoryWrite | Access::WritePath) {
            required(&args, "path")?.to_owned()
        } else if access == Access::Prompt {
            required(&args, "prompt")?.to_owned()
        } else {
            let path = PathBuf::from(required(&args, "path")?);
            if !path.is_absolute() {
                return Err(Error::new(400, "Tool requires an absolute path"));
            }
            let canonical = tokio::fs::canonicalize(&path).await?;
            // Credentials and the native database are never model-readable.
            self.check_public_path(&canonical).await?;
            args["path"] = json!(canonical.to_string_lossy());
            canonical.to_string_lossy().to_string()
        };
        let project = self.turn_project(body).await?;
        let path = PathBuf::from(string(&args, "path"));
        let resolved = if path.is_absolute() {
            path.clone()
        } else {
            project.join(&path)
        };
        let in_project = resolved.starts_with(&project);
        let ordinary =
            !crate::approval::sensitive(resolved.strip_prefix(&project).unwrap_or(&resolved));
        // User-wide memory keeps the same approval boundary even when the
        // chosen project contains the runtime workspace.
        let global_memory_write =
            access == Access::WritePath && resolved.starts_with(self.memory_root()?);
        let automatic = !computer
            && mcp.is_none()
            && !global_memory_write
            && ((matches!(access, Access::ReadPath | Access::WritePath) && in_project && ordinary)
                || builtin == Some(Builtin::WebSearch)
                || (builtin == Some(Builtin::MemoryWrite) && args["scope"] == "project"));
        let reason = if shell_project.is_some() {
            "Run this command with the computer account's permissions, without an OS sandbox"
        } else if computer || mcp.is_some() {
            "Interact with an external application or service"
        } else if access == Access::ReadPath && !in_project {
            "Read outside the conversation project"
        } else if global_memory_write {
            "Change persistent user-wide memory"
        } else if !ordinary {
            "Access sensitive data or persistent agent instructions"
        } else if prepared_memory.is_some() {
            "Change persistent user-wide memory"
        } else {
            "Approve this concrete action"
        };
        if self.file_mode(body)? != initial_file_mode {
            return Err(Error::new(
                409,
                "File permissions changed during tool preparation",
            ));
        }
        self.authorize(
            session, name, &args, body, &target, automatic, reason, cancel,
        )
        .await?;
        if self.has_steering(session)? {
            return Err(Error::new(
                409,
                "Action was not started: superseded by user steering",
            ));
        }
        if self.file_mode(body)? != initial_file_mode {
            return Err(Error::new(409, "File permissions changed before execution"));
        }
        if computer {
            let result = tokio::select! {
                _=cancel.cancelled()=>None,
                result=self.computer_tool(session,name,&args)=>Some(result),
            };
            return match result {
                Some(result) => result,
                None => {
                    self.cancel_computer().await;
                    Err(Error::new(499, "Computer operation cancelled"))
                }
            };
        }
        if matches!(builtin, Some(Builtin::Grep) | Some(Builtin::Glob)) {
            let path = PathBuf::from(required(&args, "path")?);
            if tokio::fs::canonicalize(&path).await? != path {
                return Err(Error::new(409, "Search target changed after approval"));
            }
            let private = tokio::fs::canonicalize(&self.root).await?;
            let workspace = self.workspace_dir().await?;
            let args = args.clone();
            let name = name.to_owned();
            let cancel = cancel.clone();
            return tokio::task::spawn_blocking(move || {
                crate::file_search::search(&path, &private, &workspace, &name, &args, &cancel)
            })
            .await
            .map_err(|_| Error::new(500, "Search failed"))?;
        }
        if let Some(project) = shell_project {
            if tokio::fs::canonicalize(&project).await? != project {
                return Err(Error::new(
                    409,
                    "Command working directory changed after approval",
                ));
            }
            self.check_public_path(&project).await?;
            self.check_history_write(&project, &project)?;
            let background = args["run_in_background"] == true;
            let job_cancel = if background {
                CancellationToken::new()
            } else {
                cancel.child_token()
            };
            let listener = self.background_emit.clone();
            let notify_session = session.to_owned();
            let notify = std::sync::Arc::new(move || {
                let emit = lock(&listener).ok().and_then(|e| e.clone());
                if let Some(emit) = emit {
                    let _ = emit(json!({"session_id":notify_session}));
                }
            });
            let id = self.jobs.start(
                session,
                required(&args, "command")?.to_owned(),
                project,
                args["timeout"].as_u64().unwrap_or(60),
                job_cancel,
                notify,
            )?;
            if background {
                return Ok(json!({"job_id":id,"status":"running","notice":"Use job_output for status and paged output; job_kill stops the process group."}).to_string());
            }
            let state = self.jobs.wait(session, &id, cancel).await?;
            return Ok(state.to_string());
        }
        if builtin == Some(Builtin::EditImage) {
            return tokio::select! {
                _ = cancel.cancelled() => Err(Error::new(499,"Image editing cancelled")),
                result = self.edit_image(required(&args,"prompt")?, body) => result,
            };
        }
        if let Some((config, tool)) = mcp {
            // Re-check enablement/configuration after approval; never silently
            // send approved arguments to a newly configured endpoint.
            let (current, _) = self.mcp_target(name)?;
            if current != config {
                return Err(Error::new(409, "MCP settings changed after approval"));
            }
            return tokio::select! {
                _=cancel.cancelled()=>Err(Error::new(499,"MCP call cancelled")),
                result=self.mcp_call(&config,Some((&tool,&args)))=>result.map(|value|value.to_string()),
            };
        }
        if let Some(write) = prepared_write {
            self.check_history_write(&project, &path)?;
            let bytes = tokio::task::spawn_blocking(move || write.apply())
                .await
                .map_err(|_| Error::new(500, "File write failed"))??;
            return Ok(json!({"written":true,"bytes":bytes,"path":args["path"]}).to_string());
        }
        if let Some((write, path)) = prepared_memory {
            self.check_history_write(&project, &path)?;
            let bytes = tokio::task::spawn_blocking(move || write.apply())
                .await
                .map_err(|_| Error::new(500, "Memory write failed"))??;
            return Ok(json!({"written":true,"path":path,"bytes":bytes}).to_string());
        }
        if builtin == Some(Builtin::Schedule) {
            let prompt = required(&args, "prompt")?;
            let spec = json!({"name":required(&args,"name")?,"enabled":true,"schedule":args["schedule"],
                "task_type":required(&args,"task_type")?,"text":prompt,
                "request":{"input":[{"role":"user","content":[{"type":"text","text":prompt}]}]},
                "dispatch":{"type":"channel","channel":"console","target":{"session_id":session,"user_id":"default"}}});
            return Ok(self
                .cron_request("POST", "/api/cron/jobs", &spec)?
                .unwrap()
                .to_string());
        }
        tokio::select! {
            _=cancel.cancelled()=>Err(Error::new(499,"Tool cancelled")),
            result=self.perform_tool(builtin,&args)=>result,
        }
    }

    async fn perform_tool(&self, builtin: Option<Builtin>, args: &Value) -> Result<String> {
        if builtin == Some(Builtin::WebSearch) {
            return self.web_search(required(args, "query")?).await;
        }
        if builtin == Some(Builtin::GenerateImage) {
            return self.generate_image(required(args, "prompt")?).await;
        }
        let path = PathBuf::from(required(args, "path")?);
        if tokio::fs::canonicalize(&path).await? != path {
            return Err(Error::new(409, "Tool target changed after approval"));
        }
        match builtin {
            Some(Builtin::ReadFile) => {
                let args = args.clone();
                tokio::task::spawn_blocking(move || crate::file_ops::read_range(&path, &args))
                    .await
                    .map_err(|_| Error::new(500, "File read failed"))?
            }
            Some(Builtin::ListDirectory) => {
                let mut dir = tokio::fs::read_dir(path).await?;
                let mut names = Vec::new();
                while let Some(entry) = dir.next_entry().await? {
                    names.push(entry.file_name().to_string_lossy().to_string());
                    if names.len() >= 1000 {
                        break;
                    }
                }
                names.sort();
                Ok(json!({"entries":names,"limit":1000}).to_string())
            }
            _ => Err(Error::new(400, "Unknown native tool")),
        }
    }

    async fn generate_image(&self, prompt: &str) -> Result<String> {
        if self.db()?.get("image_plugin_installed", json!(true))? != true {
            return Err(Error::new(403, "Image plugin is disabled"));
        }
        let settings = self.db()?.get("media", json!({}))?;
        let connection = self.provider_connection(
            string(&settings, "image_provider_id"),
            string(&settings, "image_model"),
        )?;
        let response = self
            .client
            .post(format!("{}/images/generations", connection.url))
            .bearer_auth(connection.key)
            .json(&json!({"model":connection.model,"prompt":prompt,"n":1}))
            .send()
            .await?;
        self.image_response(response).await
    }

    async fn edit_image(&self, prompt: &str, body: &Value) -> Result<String> {
        if self.db()?.get("image_plugin_installed", json!(true))? != true {
            return Err(Error::new(403, "Image plugin is disabled"));
        }
        let settings = self.db()?.get("media", json!({}))?;
        let connection = self.provider_connection(
            string(&settings, "image_provider_id"),
            string(&settings, "image_model"),
        )?;
        let mut form = reqwest::multipart::Form::new()
            .text("model", connection.model)
            .text("prompt", prompt.to_owned());
        let mut count = 0;
        let mut total = 0;
        for message in body["input"].as_array().into_iter().flatten() {
            if message["role"] != "user" {
                continue;
            }
            for block in message["content"].as_array().into_iter().flatten() {
                if block["type"] != "image" {
                    continue;
                }
                let url = block["image_url"].as_str().unwrap_or("");
                let (header, data) = url
                    .split_once(",")
                    .ok_or_else(|| Error::new(400, "Attach an image directly to edit it"))?;
                let (mime, extension) = match header {
                    "data:image/png;base64" => ("image/png", "png"),
                    "data:image/jpeg;base64" => ("image/jpeg", "jpg"),
                    "data:image/webp;base64" => ("image/webp", "webp"),
                    _ => return Err(Error::new(400, "Image editing supports PNG, JPEG and WebP")),
                };
                if data.len() > 28_000_000 {
                    return Err(Error::new(413, "Image is too large"));
                }
                let bytes = STANDARD
                    .decode(data)
                    .map_err(|_| Error::new(400, "Invalid image encoding"))?;
                total += bytes.len();
                count += 1;
                if total > 20_000_000 || count > 8 {
                    return Err(Error::new(413, "Too many or too large images"));
                }
                form = form.part(
                    "image[]",
                    reqwest::multipart::Part::bytes(bytes)
                        .mime_str(mime)?
                        .file_name(format!("image-{count}.{extension}")),
                );
            }
        }
        if count == 0 {
            return Err(Error::new(
                400,
                "Attach an image to the current message before editing",
            ));
        }
        let response = self
            .client
            .post(format!("{}/images/edits", connection.url))
            .bearer_auth(connection.key)
            .multipart(form)
            .send()
            .await?;
        self.image_response(response).await
    }

    async fn image_response(&self, response: reqwest::Response) -> Result<String> {
        if !response.status().is_success() {
            return Err(Error::new(
                502,
                format!("Image service returned HTTP {}", response.status().as_u16()),
            ));
        }
        let value: Value = response.json().await?;
        let item = value["data"]
            .as_array()
            .and_then(|d| d.first())
            .ok_or_else(|| Error::new(502, "Image service returned no image"))?;
        let url = if let Some(encoded) = item["b64_json"].as_str() {
            if encoded.len() > 28_000_000 {
                return Err(Error::new(413, "Generated image is too large"));
            }
            STANDARD
                .decode(encoded)
                .map_err(|_| Error::new(502, "Invalid image encoding"))?;
            format!("data:image/png;base64,{encoded}")
        } else {
            let url = required(item, "url")?;
            if !url.starts_with("https://") {
                return Err(Error::new(502, "Image URL must use HTTPS"));
            }
            use futures_util::StreamExt;
            let response = self.client.get(url).send().await?;
            if !response.status().is_success() {
                return Err(Error::new(502, "Could not download generated image"));
            }
            let mime = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("image/png")
                .split(';')
                .next()
                .unwrap_or("image/png")
                .to_owned();
            if !matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/webp") {
                return Err(Error::new(
                    502,
                    "Generated image has unsupported media type",
                ));
            }
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                if bytes.len() + chunk.len() > 20_000_000 {
                    return Err(Error::new(413, "Generated image is too large"));
                }
                bytes.extend_from_slice(&chunk);
            }
            format!("data:{mime};base64,{}", STANDARD.encode(bytes))
        };
        // Existing frontend renders rich tool-output image blocks.
        Ok(
            json!([{"type":"image","image_url":url},{"type":"text","text":"Image generated"}])
                .to_string(),
        )
    }

    /// Used by the desktop IPC upload adapter; WAV is produced by the existing
    /// browser recorder, so no ffmpeg, Python, or local ASR model is needed.
    pub async fn transcribe(
        &self,
        filename: String,
        mime: String,
        bytes: Vec<u8>,
    ) -> Result<Value> {
        if self.speech_type()? != "whisper_api" {
            return Err(Error::new(
                400,
                "Recorded-audio transcription is disabled; use the selected speech service",
            ));
        }
        if bytes.is_empty() || bytes.len() > 20_000_000 {
            return Err(Error::new(413, "Audio must be between 1 byte and 20 MB"));
        }
        let settings = self.db()?.get("media", json!({}))?;
        let connection = self.provider_connection(
            string(&settings, "speech_provider_id"),
            string(&settings, "speech_model"),
        )?;
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(filename)
            .mime_str(&mime)?;
        let form = reqwest::multipart::Form::new()
            .text("model", connection.model)
            .part("file", part);
        let response = self
            .client
            .post(format!("{}/audio/transcriptions", connection.url))
            .bearer_auth(connection.key)
            .multipart(form)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(Error::new(
                502,
                format!(
                    "Speech service returned HTTP {}",
                    response.status().as_u16()
                ),
            ));
        }
        let value: Value = response.json().await?;
        Ok(json!({"text":required(&value,"text")?}))
    }
}

#[cfg(test)]
mod history_write_tests {
    use super::*;

    async fn call(runtime: &Runtime, name: &str, args: Value, body: &Value) -> Result<String> {
        let emit: crate::Emit = std::sync::Arc::new(|_| Ok(()));
        runtime
            .execute_tool(
                "history-guard",
                name,
                &args,
                body,
                &CancellationToken::new(),
                &emit,
            )
            .await
    }

    #[tokio::test]
    async fn model_file_routing_keeps_runtime_history_read_only() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(&temp.path().join("runtime")).unwrap();
        let workspace = runtime.workspace_dir().await.unwrap();
        let history = workspace.join("history");
        std::fs::create_dir_all(&history).unwrap();
        let archive = history.join("record.md");
        std::fs::write(&archive, "original archive").unwrap();
        let external = temp.path().join("project");
        std::fs::create_dir_all(external.join("history")).unwrap();
        let default =
            json!({"request_context":{"sandbox_mode":"workspace-write","approval_level":"NEVER"}});
        let ancestor = json!({"request_context":{"sandbox_mode":"workspace-write","approval_level":"NEVER","potato.coding_project_dir":temp.path()}});
        let outside = json!({"request_context":{"sandbox_mode":"workspace-write","approval_level":"NEVER","potato.coding_project_dir":external}});
        for body in [&default, &ancestor, &outside] {
            for (name, args) in [
                ("write_file", json!({"path":archive,"content":"changed"})),
                ("append_file", json!({"path":archive,"content":"changed"})),
                (
                    "edit_file",
                    json!({"file_path":archive,"old_text":"original","new_text":"changed"}),
                ),
                (
                    "write_file",
                    json!({"path":history.join("new.md"),"content":"changed"}),
                ),
            ] {
                let error = call(&runtime, name, args, body).await.unwrap_err();
                assert_eq!(error.status, 403);
                assert!(error.message.contains("history"));
            }
        }
        for body in [&default, &ancestor] {
            let read = call(&runtime, "read_file", json!({"path":archive}), body)
                .await
                .unwrap();
            assert!(read.contains("original archive"));
        }
        let inside = json!({"request_context":{"sandbox_mode":"workspace-write","approval_level":"NEVER","potato.coding_project_dir":history}});
        let error = call(
            &runtime,
            "memory_write",
            json!({"scope":"project","path":"note.md","content":"changed"}),
            &inside,
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, 403);
        assert!(error.message.contains("history"));
        assert!(!history.join(".potato").exists());
        call(
            &runtime,
            "write_file",
            json!({"path":"notes.md","content":"notes"}),
            &default,
        )
        .await
        .unwrap();
        call(
            &runtime,
            "write_file",
            json!({"path":"history/notes.md","content":"project history"}),
            &outside,
        )
        .await
        .unwrap();
        call(
            &runtime,
            "memory_write",
            json!({"scope":"project","path":"note.md","content":"memory"}),
            &outside,
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(archive).unwrap(),
            "original archive"
        );
        assert!(!history.join("new.md").exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn history_guard_resolves_aliases_and_new_descendants() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(&temp.path().join("runtime")).unwrap();
        let workspace = runtime.workspace_dir().await.unwrap();
        let history = workspace.join("history");
        std::fs::create_dir_all(&history).unwrap();
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&history, &alias).unwrap();
        let body = json!({"request_context":{"sandbox_mode":"workspace-write","approval_level":"NEVER","potato.coding_project_dir":temp.path()}});
        let error = call(
            &runtime,
            "write_file",
            json!({"path":alias.join("nested/new.md"),"content":"changed"}),
            &body,
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, 403);
        assert!(error.message.contains("history"));
        let body = json!({"request_context":{"sandbox_mode":"workspace-write","approval_level":"NEVER","potato.coding_project_dir":alias}});
        let error = call(
            &runtime,
            "memory_write",
            json!({"scope":"project","path":"note.md","content":"changed"}),
            &body,
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, 403);
        assert!(!history.join(".potato").exists());
    }
}
