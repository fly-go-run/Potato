use crate::{Error, Result, Runtime, required, string};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

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
    pub(crate) fn definitions(&self, images: bool) -> Result<Vec<Value>> {
        let cloud = self.cloud_memory_active()?.is_some();
        Ok(tool_registry::definitions(images).into_iter().filter(|d| {
            cloud || !matches!(d["function"]["name"].as_str(), Some("remember" | "forget_memory"))
        }).collect())
    }

    pub(crate) fn check_history_write(&self, project: &Path, target: &Path) -> Result<()> {
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
                let output = self
                    .jobs
                    .output(session, required(args, "job_id")?, args)
                    .await?;
                if !crate::jobs::active(&output) {
                    self.acknowledge_shell_followup(session,required(args,"job_id")?)?;
                }
                return Ok(output.to_string());
            }
            Some(Builtin::JobList) => return Ok(self.jobs.list(session)?.to_string()),
            Some(Builtin::JobKill) => {
                return Ok(self
                    .jobs
                    .cancel(session, required(args, "job_id")?)?
                    .to_string());
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
            let cloud = self.search_cloud_memory(&query)?;
            let args = args.clone();
            let cancel = cancel.clone();
            let mut result = tokio::task::spawn_blocking(move || {
                crate::memory::search_notes(&root, &query, &args, &cancel)
            })
            .await
            .map_err(|_| Error::new(500, "Memory search failed"))??;
            result["matches"].as_array_mut().unwrap().extend(cloud);
            return Ok(result.to_string());
        }
        if builtin == Some(Builtin::AskUser) {
            if self.approval_level(body)? == "NEVER" {
                return Err(Error::new(
                    403,
                    "Interactive questions are unavailable in NEVER mode; continue with available information or report what is missing",
                ));
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
        // Execution evidence belongs to the host. Model/MCP arguments must not
        // impersonate sandbox scope, prior failures or a background approval.
        if let Some(args) = args.as_object_mut() {
            for key in ["_execution", "_sandbox_failure", "_job_id"] {
                args.remove(key);
            }
        }
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
        if builtin == Some(Builtin::Shell) {
            return self.execute_shell(session, &args, body, cancel).await;
        }
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
            if !matches!(builtin, Some(Builtin::CreateOffice | Builtin::FillOffice))
                && target.is_absolute()
                && target.starts_with(&memory)
            {
                project = memory;
            }
            self.check_history_write(&project, &target)?;
            if builtin == Some(Builtin::FillOffice) {
                let source = PathBuf::from(required(&args, "template_path")?);
                self.check_history_write(&project, &source)?;
            }
            let args = args.clone();
            let operation = name.to_owned();
            Some(
                tokio::task::spawn_blocking(move || {
                    if operation == "fill_office_template" {
                        let source = PathBuf::from(required(&args, "template_path")?);
                        let format = required(&args, "format")?;
                        for path in [&source, &target] {
                            if path
                                .extension()
                                .and_then(|v| v.to_str())
                                .map(str::to_ascii_lowercase)
                                .as_deref()
                                != Some(format)
                            {
                                return Err(Error::new(
                                    400,
                                    "Template and output extension must match format",
                                ));
                            }
                        }
                        let bytes = crate::file_ops::read_office_template(&project, &source)?;
                        let bytes =
                            crate::office::template::fill(bytes, format, &args["replacements"])?;
                        crate::file_ops::PreparedWrite::prepare_artifact(&project, &target, bytes)
                    } else if operation == "create_office_file" {
                        let bytes = crate::office::generate(&target, &args)?;
                        crate::file_ops::PreparedWrite::prepare_artifact(&project, &target, bytes)
                    } else if operation == "write_file" {
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
        let cloud_approval = if access == Access::CloudMemory {
            let active = self.cloud_memory_active()?.ok_or_else(|| Error::new(401, "请先登录云端"))?;
            args["email"] = self.db()?.get("cloud_config", Value::Null)?["email"].clone();
            let row = if builtin == Some(Builtin::ForgetMemory) {
                let row = self.cached_cloud_memory(required(&args, "id")?)?;
                args["text"] = row["text"].clone();
                Some(row)
            } else {
                required(&args, "text")?;
                None
            };
            Some((active, row))
        } else { None };
        let target = if access == Access::CloudMemory {
            format!("{} / cloud personal memory{}", string(&args, "email"),
                if builtin == Some(Builtin::ForgetMemory) { format!(" / {}", required(&args, "id")?) } else { String::new() })
        } else if let Some((config, tool)) = &mcp {
            format!("{} / {}", required(config, "name")?, tool)
        } else if computer {
            self.computer_target(session, name, &args)?
        } else if builtin == Some(Builtin::WebSearch) {
            required(&args, "query")?.to_owned()
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
        let automatic = access != Access::CloudMemory
            && !computer
            && mcp.is_none()
            && !global_memory_write
            && ((matches!(access, Access::ReadPath | Access::WritePath) && in_project && ordinary)
                || builtin == Some(Builtin::WebSearch)
                || (builtin == Some(Builtin::MemoryWrite) && args["scope"] == "project"));
        let reason = if access == Access::CloudMemory {
            "Change persistent user-wide memory"
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
        let permission_version = self.permission_version()?;
        let review_generation = self.review_generation(session)?;
        let read_snapshot = if access == Access::ReadPath {
            Some(crate::permissions::PathSnapshot::capture(Path::new(
                &target,
            ))?)
        } else {
            None
        };
        self.authorize(
            session, name, &args, body, &target, automatic, reason, cancel,
        )
        .await?;
        if let Some(snapshot) = read_snapshot {
            snapshot.verify()?;
        }
        if self.permission_version()? != permission_version
            || self.review_generation(session)? != review_generation
            || cancel.is_cancelled()
        {
            return Err(Error::new(
                409,
                "Permissions changed or action cancelled before execution",
            ));
        }
        if self.has_steering(session)? {
            return Err(Error::new(
                409,
                "Action was not started: superseded by user steering",
            ));
        }
        if self.file_mode(body)? != initial_file_mode {
            return Err(Error::new(409, "File permissions changed before execution"));
        }
        if let Some((active, row)) = cloud_approval {
            if self.cloud_memory_active()? != Some(active) {
                return Err(Error::new(409, "云端账号已改变"));
            }
            if let Some(row) = row {
                if self.cached_cloud_memory(required(&args, "id")?)? != row {
                    return Err(Error::new(409, "记忆已在别处修改，请重试"));
                }
            }
            return match builtin {
                Some(Builtin::Remember) => Ok(self.cloud_remember(required(&args, "text")?).await?.to_string()),
                Some(Builtin::ForgetMemory) => Ok(self.cloud_forget(required(&args, "id")?).await?.to_string()),
                _ => unreachable!(),
            };
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
        if builtin == Some(Builtin::EditImage) {
            return tokio::select! {
                _ = cancel.cancelled() => Err(Error::new(499,"Image editing cancelled")),
                result = self.edit_image(required(&args,"prompt")?, body, image_count(&args)?) => result,
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
            if cancel.is_cancelled() {
                return Err(Error::new(499, "File write cancelled"));
            }
            let bytes = tokio::task::spawn_blocking(move || write.apply())
                .await
                .map_err(|_| Error::new(500, "File write failed"))??;
            if matches!(builtin, Some(Builtin::CreateOffice | Builtin::FillOffice)) {
                let warnings = if builtin == Some(Builtin::FillOffice) {
                    vec![
                        "Template text length changed; inspect layout in Office before delivery"
                            .to_owned(),
                    ]
                } else {
                    crate::office::layout_warnings(&args)
                };
                return Ok(json!({"written":true,"bytes":bytes,"path":resolved,"format":args["format"],"validation":"package checks and content-density heuristics only; not visually rendered","warnings":warnings,"formula_calculation":if args["format"]=="xlsx" {"explicit numeric formulas evaluated by IronCalc; cached results saved"} else {"not applicable"}}).to_string());
            }
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
            return self
                .generate_image(required(args, "prompt")?, image_count(args)?)
                .await;
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

    async fn generate_image(&self, prompt: &str, count: u64) -> Result<String> {
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
            .json(&json!({"model":connection.model,"prompt":prompt,"n":count}))
            .send()
            .await?;
        self.image_response(response).await
    }

    async fn edit_image(&self, prompt: &str, body: &Value, count: u64) -> Result<String> {
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
            .text("prompt", prompt.to_owned())
            .text("n", count.to_string());
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
        let items = value["data"]
            .as_array()
            .filter(|items| !items.is_empty())
            .ok_or_else(|| Error::new(502, "Image service returned no image"))?;
        let mut blocks = Vec::new();
        for item in items {
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
            blocks.push(json!({"type":"image","image_url":url}));
        }
        blocks.push(json!({"type":"text","text":format!("Generated {} image(s)", blocks.len())}));
        Ok(Value::Array(blocks).to_string())
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

fn image_count(args: &Value) -> Result<u64> {
    match args.get("n") {
        None => Ok(1),
        Some(value) => value
            .as_u64()
            .filter(|n| (1..=8).contains(n))
            .ok_or_else(|| Error::new(400, "Image count must be an integer between 1 and 8")),
    }
}

#[cfg(test)]
mod image_count_tests {
    use super::*;
    #[test]
    fn defaults_to_one_and_rejects_invalid_counts() {
        assert_eq!(image_count(&json!({})).unwrap(), 1);
        assert_eq!(image_count(&json!({"n": 8})).unwrap(), 8);
        for n in [
            json!(0),
            json!(9),
            json!(-1),
            json!(1.5),
            json!("2"),
            Value::Null,
        ] {
            assert!(image_count(&json!({"n": n})).is_err());
        }
    }
}
