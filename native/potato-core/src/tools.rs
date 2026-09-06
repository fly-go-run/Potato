use crate::{lock, required, string, Approval, Error, Result, Runtime};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};
use tokio::{io::AsyncReadExt, sync::oneshot};
use tokio_util::sync::CancellationToken;

pub(crate) fn definitions(images: bool) -> Vec<Value> {
    let mut tools = vec![
        json!({"type":"function","function":{"name":"web_search","description":"Search the web using the user's configured search service. Results include source URLs and untrusted webpage content.","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"execute_shell_command","description":"Execute a shell command with the computer account's permissions after exact approval. Requires danger-full-access mode; this is not an OS sandbox. Uses the conversation project as working directory. Cancellation and timeout terminate the process group. No background execution.","parameters":{"type":"object","properties":{"command":{"type":"string"},"timeout":{"type":"integer","minimum":1,"maximum":3600}},"required":["command"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"read_skill","description":"Read an enabled skill's instructions or referenced Markdown document. Paths are relative to that skill.","parameters":{"type":"object","properties":{"name":{"type":"string"},"path":{"type":"string"}},"required":["name"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"write_file","description":"Create or replace a UTF-8 file in the conversation project after exact approval. Requires workspace-write mode, an existing parent directory and at most 1 MB of content. Refuses files changed during approval and paths outside the project.","parameters":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"memory_search","description":"Search saved memory notes using all query terms. Use before answering questions about previously saved preferences or facts.","parameters":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"memory_write","description":"Save a non-sensitive memory note after approval. The relative Markdown path may contain folders; content replaces that note. Never store credentials.","parameters":{"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"create_scheduled_task","description":"Create a text reminder or future agent task in this conversation after user approval. Runs while Potato is open and the computer is awake. Use an explicit timezone for cron or an ISO timestamp with offset for once.",
            "parameters":{"type":"object","properties":{"name":{"type":"string"},"prompt":{"type":"string"},"task_type":{"type":"string","enum":["text","agent"]},"schedule":{"type":"object","properties":{"type":{"type":"string","enum":["once","cron"]},"run_at":{"type":"string"},"cron":{"type":"string"},"timezone":{"type":"string"}},"required":["type"],"additionalProperties":false}},"required":["name","prompt","task_type","schedule"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"request_user_input","description":"Ask the user a question and wait for their explicit answer. Supports single choice, multiple choice or free text. Use when required information is missing.",
            "parameters":{"type":"object","properties":{"title":{"type":"string"},"options":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"label":{"type":"string"}},"required":["id","label"],"additionalProperties":false}},"multiple":{"type":"boolean"}},"required":["title"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"read_file","description":"Read a UTF-8 file after the user approves its absolute path (maximum 1 MB).",
            "parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"list_directory","description":"List directory entries after the user approves its absolute path.",
            "parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}}),
    ];
    if images {
        tools.push(json!({"type":"function","function":{"name":"edit_image","description":"Edit images attached to the current user message using the configured image service. Describe the requested change; image bytes are supplied automatically.","parameters":{"type":"object","properties":{"prompt":{"type":"string"}},"required":["prompt"],"additionalProperties":false}}}));
        tools.push(json!({"type":"function","function":{"name":"generate_image_gpt","description":"Generate an image from a prompt using the configured image service.",
        "parameters":{"type":"object","properties":{"prompt":{"type":"string"}},"required":["prompt"],"additionalProperties":false}}}));
    }
    tools
}

impl Runtime {
    pub(crate) async fn execute_tool(
        &self,
        session: &str,
        name: &str,
        args: &Value,
        body: &Value,
        cancel: &CancellationToken,
        emit: &crate::Emit,
    ) -> Result<String> {
        if name == "read_skill" {
            return self.read_skill(args);
        }
        if name == "memory_search" {
            return Ok(self.search_memory(required(args, "query")?)?.to_string());
        }
        if name == "request_user_input" {
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
        if mcp.is_none()
            && !computer
            && !matches!(
                name,
                "read_file"
                    | "list_directory"
                    | "generate_image_gpt"
                    | "edit_image"
                    | "create_scheduled_task"
                    | "memory_write"
                    | "write_file"
                    | "execute_shell_command"
                    | "web_search"
            )
        {
            return Err(Error::new(400, "Unknown native tool"));
        }
        let mut args = args.clone();
        let shell_project = if name == "execute_shell_command" {
            let running = self
                .db()?
                .get("running", json!({"sandbox_mode":"read-only"}))?;
            let mode = body["request_context"]["sandbox_mode"]
                .as_str()
                .unwrap_or_else(|| string(&running, "sandbox_mode"));
            if mode != "danger-full-access" {
                return Err(Error::new(
                    403,
                    "Shell execution requires full-access mode and exact approval",
                ));
            }
            let project = self.turn_project(body).await?;
            args["cwd"] = json!(project.to_string_lossy());
            Some(project)
        } else {
            None
        };
        let prepared_write = if name == "write_file" {
            let running = self
                .db()?
                .get("running", json!({"sandbox_mode":"read-only"}))?;
            let mode = body["request_context"]["sandbox_mode"]
                .as_str()
                .unwrap_or_else(|| string(&running, "sandbox_mode"));
            if !matches!(mode, "workspace-write" | "danger-full-access") {
                return Err(Error::new(
                    403,
                    "Enable project writing before editing files",
                ));
            }
            let project = self.turn_project(body).await?;
            let target = PathBuf::from(required(&args, "path")?);
            let content = args["content"]
                .as_str()
                .ok_or_else(|| Error::new(400, "File content is required"))?
                .to_owned();
            Some(
                tokio::task::spawn_blocking(move || {
                    crate::file_ops::PreparedWrite::prepare(&project, &target, &content)
                })
                .await
                .map_err(|_| Error::new(500, "File preparation failed"))??,
            )
        } else {
            None
        };
        let target = if let Some((config, tool)) = &mcp {
            format!("{} / {}", required(config, "name")?, tool)
        } else if computer {
            self.computer_target(session, name, &args)?
        } else if name == "web_search" {
            required(&args, "query")?.to_owned()
        } else if shell_project.is_some() {
            format!(
                "Unsandboxed command in {}: {}",
                string(&args, "cwd"),
                required(&args, "command")?
            )
        } else if matches!(name, "memory_write" | "write_file") {
            required(&args, "path")?.to_owned()
        } else if matches!(
            name,
            "generate_image_gpt" | "edit_image" | "create_scheduled_task"
        ) {
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
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        let view = json!({"request_id":id,"session_id":session,"root_session_id":session,"user_id":"default",
            "tool_name":name,"tool_params":args,"severity":"medium","findings_count":1,
            "findings_summary":"Approve this exact action once","source_type":"native","driver":null,
            "created_at":chrono::Utc::now().timestamp(),"timeout_seconds":300,"tool_display_name":name,
            "tool_source":"Potato","exact_target":target,"similar_target":"","is_generalized":false});
        lock(&self.approvals)?.insert(id.clone(), Approval { view, reply: tx });
        let approved = tokio::select! {
            _=cancel.cancelled()=>false,
            result=tokio::time::timeout(Duration::from_secs(300),rx)=>matches!(result,Ok(Ok(true))),
        };
        lock(&self.approvals)?.remove(&id);
        if !approved || cancel.is_cancelled() {
            return Err(Error::new(403, "Tool action denied, cancelled, or expired"));
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
        if let Some(project) = shell_project {
            return crate::processes::execute(
                required(&args, "command")?,
                &project,
                args["timeout"].as_u64().unwrap_or(60),
                cancel,
            )
            .await
            .map(|result| result.to_string());
        }
        if name == "edit_image" {
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
            let bytes = tokio::task::spawn_blocking(move || write.apply())
                .await
                .map_err(|_| Error::new(500, "File write failed"))??;
            return Ok(json!({"written":true,"bytes":bytes,"path":args["path"]}).to_string());
        }
        if name == "memory_write" {
            let path = format!("/api/workspace/memory/{}", required(&args, "path")?);
            return Ok(self
                .document_request("PUT", &path, &args)?
                .unwrap()
                .to_string());
        }
        if name == "create_scheduled_task" {
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
            result=self.perform_tool(name,&args)=>result,
        }
    }

    async fn perform_tool(&self, name: &str, args: &Value) -> Result<String> {
        if name == "web_search" {
            return self.web_search(required(args, "query")?).await;
        }
        if name == "generate_image_gpt" {
            return self.generate_image(required(args, "prompt")?).await;
        }
        let path = PathBuf::from(required(args, "path")?);
        if tokio::fs::canonicalize(&path).await? != path {
            return Err(Error::new(409, "Tool target changed after approval"));
        }
        match name {
            "read_file" => {
                let file = tokio::fs::File::open(path).await?;
                if !file.metadata().await?.is_file() {
                    return Err(Error::new(400, "Target must be a regular file"));
                }
                let mut bytes = Vec::new();
                file.take(1_000_001).read_to_end(&mut bytes).await?;
                if bytes.len() > 1_000_000 {
                    return Err(Error::new(413, "File exceeds the 1 MB tool limit"));
                }
                String::from_utf8(bytes).map_err(|_| Error::new(400, "File is not UTF-8 text"))
            }
            "list_directory" => {
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
