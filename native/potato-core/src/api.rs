use crate::{lock, required, string, Error, Result, Runtime};
use serde_json::{json, Value};

pub(crate) fn default_providers() -> Value {
    json!([
        provider(
            "deepseek",
            "DeepSeek",
            "https://api.deepseek.com",
            "OpenAIChatModel",
            json!([{"id":"deepseek-chat","name":"DeepSeek Chat","max_input_length":128_000},{"id":"deepseek-reasoner","name":"DeepSeek Reasoner","max_input_length":128_000}])
        ),
        provider("sub2api", "sub2api", "", "OpenAIResponseModel", json!([]))
    ])
}
fn provider(id: &str, name: &str, url: &str, protocol: &str, models: Value) -> Value {
    json!({"id":id,"name":name,"base_url":url,"chat_model":protocol,"models":models,
        "extra_models":[],"api_key":"","api_key_prefix":"","api_key_prefixes":[],
        "is_local":false,"freeze_url":false,"require_api_key":true,"is_custom":true})
}

impl Runtime {
    pub async fn request(&self, method: &str, path: &str, body: Value) -> Result<Value> {
        if !path.starts_with("/api/") {
            return Err(Error::new(400, "Only local API paths are accepted"));
        }
        let url = reqwest::Url::parse(&format!("http://native{path}"))
            .map_err(|_| Error::new(400, "Invalid API path"))?;
        let path = percent_encoding::percent_decode_str(url.path())
            .decode_utf8()
            .map_err(|_| Error::new(400, "Invalid path encoding"))?;
        let query = |key: &str| {
            url.query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default()
        };
        if let Some(result) = self.document_request(method, &path, &body)? {
            return Ok(result);
        }
        if let Some(result) = self.skill_request(method, &path, &body)? {
            return Ok(result);
        }
        if let Some(result) = self.cron_request(method, &path, &body)? {
            return Ok(result);
        }
        if let Some(result) = self.mcp_request(method, &path, &body).await? {
            return Ok(result);
        }
        if let Some(result) = self.project_request(method, &path, &body, &url).await? {
            return Ok(result);
        }
        match (method, path.as_ref()) {
            ("GET", "/api/native/remote") => return self.remote_settings(),
            ("POST", "/api/native/remote") => return self.configure_remote(body).await,
            ("POST", "/api/native/remote/login/start") => return self.begin_remote_login(body).await,
            ("POST", "/api/native/remote/login/poll") => return self.poll_remote_login().await,
            ("POST", "/api/native/remote/login/cancel") => return self.cancel_remote_login(),
            ("POST", "/api/native/remote/logout") => return self.logout_remote().await,
            ("POST", "/api/agent/outbox") => return self.outbox_request(&body),
            ("POST", "/api/agent/steer") => return self.steer(&body),
            ("GET", "/api/native/preferences")=>return self.db()?.get("native_preferences",json!({"dark":false,"collapsed":false,"width":1080,"height":760,"selected":""})),
            ("PUT", "/api/native/preferences")=>{
                let mut saved=self.db()?.get("native_preferences",json!({} ))?;
                for key in ["dark","collapsed","follow_system","remember_window"]{if let Some(value)=body.get(key){if !value.is_boolean(){return Err(Error::new(400,"Invalid preference"));}saved[key]=value.clone();}}
                for (key,min,max) in [("width",760.,4000.),("height",540.,2400.),("file_list_width",240.,720.),("file_detail_width",240.,720.)]{if let Some(value)=body.get(key){let n=value.as_f64().ok_or_else(||Error::new(400,"Invalid window size"))?;if !n.is_finite()||n<min||n>max{return Err(Error::new(400,"Invalid window size"));}saved[key]=value.clone();}}
                for key in ["x","y"] {if let Some(value)=body.get(key) {let n=value.as_f64().ok_or_else(||Error::new(400,"Invalid window position"))?;if !n.is_finite() || n.abs()>100_000. {return Err(Error::new(400,"Invalid window position"));}saved[key]=value.clone();}}
                if let Some(value)=body.get("selected"){let id=value.as_str().ok_or_else(||Error::new(400,"Invalid selected conversation"))?;if id.len()>200{return Err(Error::new(400,"Invalid selected conversation"));}saved["selected"]=value.clone();}
                self.db()?.put("native_preferences",&saved)?;return Ok(saved);
            },
            ("GET", "/api/native/legacy-settings") => return self.legacy_settings_status(),
            ("POST", "/api/native/legacy-settings") => return self.import_legacy_settings(std::path::Path::new(required(&body,"working_dir")?),std::path::Path::new(required(&body,"secret_dir")?)),
            ("POST", "/api/console/upload") => return tokio::task::spawn_blocking(move||crate::attachments::upload(body)).await.map_err(|_|Error::new(500,"Document extraction failed"))?,
            ("GET", "/api/workspace/web-search-backend") => return self.search_settings(),
            ("PUT", "/api/workspace/web-search-backend") => return self.save_search_settings(&body),
            ("GET", "/api/plugins") => {
                let installed=self.db()?.get("image_plugin_installed",json!(true))?==true;
                let media=self.db()?.get("media",json!({}))?;
                let loaded=installed && self.provider_connection(string(&media,"image_provider_id"),string(&media,"image_model")).is_ok();
                return Ok(if installed {json!([{"id":"gpt-image2","name":"图片生成","description":"通过已配置的图片供应商生成图片","version":"1.0.0","source":"builtin","enabled":true,"loaded":loaded,"plugin_type":"tool","tool_count":1}])}else{json!([])});
            }
            ("GET", "/api/plugins/catalog") => return Ok(json!({"updated_at":null,"error":null,"plugins":[{"id":"gpt-image2","plugin_id":"gpt-image2","name":"图片生成","description":"Rust 内置图片工具，使用设置中的图片供应商","version":"1.0.0","install_url":"builtin:gpt-image2","installed":self.db()?.get("image_plugin_installed",json!(true))?,"upgrade_available":false}]})),
            ("POST", "/api/plugins/install") if body["source"]=="builtin:gpt-image2" => {
                self.db()?.put("image_plugin_installed",&json!(true))?;
                return Ok(json!({"id":"gpt-image2","name":"图片生成","version":"1.0.0","source":"builtin","enabled":true,"tool_count":1}));
            }
            ("DELETE", "/api/plugins/gpt-image2") => {
                self.db()?.put("image_plugin_installed",&json!(false))?;
                return Ok(json!({"deleted":true}));
            }
            ("GET", "/api/workspace/download") => {
                let runs=lock(&self.runs)?;
                if !runs.is_empty(){return Err(Error::new(409,"Stop active turns before exporting workspace"));}
                return self.export_workspace();
            }
            ("GET", "/api/questions") => return Ok(json!({"questions":self.db()?.questions(&query("session_id"))?})),
            ("GET", "/api/native/doubao-settings") => {
                let mut value=self.db()?.get("doubao",json!({"api_key":"","app_id":"","resource_id":"volc.seedasr.sauc.duration","enabled":false}))?;
                value["api_key"]=json!(if string(&value,"api_key").is_empty(){""}else{"********"});
                return Ok(value);
            }
            ("PUT", "/api/native/doubao-settings") => {
                let mut value=self.db()?.get("doubao",json!({"api_key":"","app_id":"","resource_id":"volc.seedasr.sauc.duration","enabled":false}))?;
                if let Some(key)=body["api_key"].as_str().filter(|k|*k!="********"){value["api_key"]=json!(self.db()?.seal(key)?);}
                for field in ["app_id","resource_id"]{if let Some(v)=body[field].as_str(){value[field]=json!(v);}}
                if let Some(enabled)=body["enabled"].as_bool(){value["enabled"]=json!(enabled);self.db()?.put("speech_type",&json!(if enabled {"doubao_asr"}else{"disabled"}))?;}
                self.db()?.put("doubao",&value)?;
                value["api_key"]=json!(if string(&value,"api_key").is_empty(){""}else{"********"});return Ok(value);
            }
            ("POST", "/api/native/import-history") => {
                let runs=lock(&self.runs)?;
                if !runs.is_empty(){return Err(Error::new(409,"Stop active turns before importing history"));}
                return self.db()?.import_history(&body);
            }
            ("GET", "/api/auth/status") => return Ok(json!({"enabled":false,"has_users":false})),
            ("GET", "/api/version") => return Ok(json!({"version":env!("CARGO_PKG_VERSION"),"runtime":"rust"})),
            ("GET", "/api/healthz") => return Ok(json!({"status":"ok","startup_state":"ready","runtime":"rust",
                "uptime_seconds":self.started_at.elapsed().as_secs(),"agents_loaded":["default"],
                "startup_ms":self.db()?.get("last_startup_ms",json!(null))?})),
            ("GET", "/api/workspace/language") => return Ok(json!({"agent_id":"default","language":self.db()?.get("language",json!("zh"))?})),
            ("PUT", "/api/workspace/language") => {
                let language=required(&body,"language")?;
                if !matches!(language,"zh"|"en"|"id"|"ru") { return Err(Error::new(400,"Unsupported workspace language")); }
                self.db()?.put("language",&json!(language))?;
                return Ok(json!({"agent_id":"default","language":language}));
            }
            ("GET" | "POST" | "PUT" | "DELETE", "/api/permissions/rules") => return self.permission_rules_api(method, &body),
            ("GET", "/api/workspace/running-config") => {
                let mut value=self.db()?.get("running",crate::approval::defaults())?;
                let config: crate::permissions::PermissionConfig=serde_json::from_value(value.clone())?;
                config.validate()?;
                value["sandbox_mode"]=serde_json::to_value(config.sandbox_mode)?;
                value["reviewer"]=serde_json::to_value(config.reviewer)?;
                value["reviewer_provider_id"]=json!(config.reviewer_provider_id);
                value["reviewer_model"]=json!(config.reviewer_model);
                value["directory_rule_count"]=json!(self.persistent_rules()?.len());
                value["shell_sandbox"]=crate::sandbox::status();
                return Ok(value);
            },
            ("PUT", "/api/workspace/running-config") => {
                // File capability and prompting policy are independent.
                let mut value = self.db()?.get("running",crate::approval::defaults())?;
                if let Some(limit)=body.get("max_iters") {
                    let limit=limit.as_u64().filter(|n|(1..=1000).contains(n)).ok_or_else(||Error::new(400,"max_iters must be between 1 and 1000"))?;
                    value["max_iters"]=json!(limit);
                }
                if let Some(limit)=body.get("max_parallel_reads") {
                    let limit=limit.as_u64().filter(|n|(1..=16).contains(n)).ok_or_else(||Error::new(400,"max_parallel_reads must be between 1 and 16"))?;
                    value["max_parallel_reads"]=json!(limit);
                }
                if let Some(config) = body.get("context_policy") {
                    let policy: crate::context_policy::Policy = serde_json::from_value(config.clone())?;
                    policy.validate()?;
                    value["context_policy"] = serde_json::to_value(policy)?;
                }
                if let Some(level)=body.get("approval_level") {let level=level.as_str().ok_or_else(||Error::new(400,"approval_level must be a string"))?;crate::approval::validate(level)?;value["approval_level"]=json!(level);}
                if let Some(mode)=body.get("sandbox_mode") {
                    let mode=mode.as_str().ok_or_else(||Error::new(400,"sandbox_mode must be a string"))?;
                    let mode: crate::permissions::FileMode=serde_json::from_value(json!(mode)).map_err(|_|Error::new(400,"Unsupported native file access mode"))?;
                    value["sandbox_mode"]=serde_json::to_value(mode)?;
                }
                if let Some(reviewer)=body.get("reviewer") {value["reviewer"]=reviewer.clone();}
                for field in ["reviewer_provider_id","reviewer_model"] {if let Some(selected)=body.get(field) {let selected=selected.as_str().ok_or_else(||Error::new(400,"Reviewer selection must be a string"))?;value[field]=json!(selected.trim());}}
                let config: crate::permissions::PermissionConfig=serde_json::from_value(value.clone()).map_err(|_|Error::new(400,"Invalid permission configuration"))?;
                config.validate()?;
                if ["reviewer", "reviewer_provider_id", "reviewer_model"].iter().any(|key| body.get(*key).is_some()) {
                    self.validate_reviewer_connection(&config)?;
                }
                value["reviewer"]=serde_json::to_value(config.reviewer)?;
                let mut permissions=lock(&self.permissions)?;
                self.db()?.put("running", &value)?;
                if body.get("approval_level").is_some() || body.get("sandbox_mode").is_some() || body.get("reviewer").is_some() || body.get("reviewer_provider_id").is_some() || body.get("reviewer_model").is_some() {
                    permissions.version+=1;
                    lock(&self.approvals)?.clear();
                }
                return Ok(value);
            }
            ("GET", "/api/config/security/sandbox") => {
                let mut status = crate::sandbox::status();
                let mode = self.file_mode(&json!({}))?;
                status["enabled"] = json!(mode != "danger-full-access");
                status["effective"] = json!(mode != "danger-full-access" && status["available"] == true);
                status["mode"] = json!(mode);
                status["scope"] = json!("shell commands and descendants; MCP/computer use have separate permissions");
                return Ok(status);
            },
            ("GET", "/api/computer-use") => return self.computer_status().await,
            ("POST", "/api/computer-use/check") => return self.check_computer_permissions().await,
            ("PUT", "/api/computer-use") => {
                if body["always_allowed_apps"].as_array().is_some_and(|a|!a.is_empty()){return Err(Error::new(400,"Native computer actions require exact approval"));}
                if let Some(enabled)=body["enabled"].as_bool(){
                    if enabled && self.computer_status().await?["driver_available"] != true {return Err(Error::new(400,"Native computer driver is not bundled"));}
                    self.db()?.put("computer_enabled",&json!(enabled))?;
                    if !enabled {self.cancel_computer().await;}
                }
                return self.computer_status().await;
            }
            ("GET", "/api/settings/upload-limit") => return Ok(json!({"upload_max_size_mb":crate::attachments::MAX_BYTES / 1_000_000})),
            ("GET", "/api/chats") => {
                let mut chats = self.db()?.chats()?;
                let runs = lock(&self.runs)?;
                for chat in &mut chats { chat["status"] = json!(if runs.contains_key(string(chat,"session_id")) {"running"} else {"idle"}); }
                let archived=query("archived")=="true";
                chats.retain(|c| (c["archived"]==true)==archived);
                chats.sort_by_key(|c|std::cmp::Reverse(c["pinned"]==true));
                let search=query("q").trim().to_lowercase();
                if !search.is_empty(){let db=self.db()?;let mut matches=Vec::new();for chat in chats {
                    if string(&chat,"name").to_lowercase().contains(&search) || db.history(string(&chat,"id"),false).is_ok_and(|messages|messages.iter().any(|m|m["content"].to_string().to_lowercase().contains(&search))){matches.push(chat);}
                }chats=matches;}
                return Ok(json!(chats));
            }
            ("POST", "/api/console/chat/stop") => {
                let chat = self.db()?.chat(&query("chat_id"))?;
                let mut request = json!({"session_id":chat["session_id"],"action":"stop"});
                if let Some(expected) = body.get("expected_run_id") { request["expected_run_id"] = expected.clone(); }
                let result = self.outbox_request(&request)?;
                return Ok(json!({"stopped":result["stopped"]}));
            }
            ("GET", "/api/approval/list" | "/api/console/push-messages") => {
                let approvals = lock(&self.approvals)?;
                let pending: Vec<_> = approvals.values().filter(|a| a.view["root_session_id"] == query("session_id")).map(|a|a.view.clone()).collect();
                drop(approvals);
                let reviews=self.review_status(&query("session_id"))?;
                return Ok(json!({"shell_jobs":self.jobs.list(&query("session_id"))?["jobs"],"messages":[],"pending_approvals":pending,"session_grants":self.approval_grant_count(&query("session_id"))?,"active_reviews":reviews["active_reviews"],"recent_reviews":reviews["recent_reviews"],"review_cache":reviews["review_cache"]}));
            }
            ("POST", "/api/approval/approve" | "/api/approval/deny") => {
                let mut approvals = lock(&self.approvals)?;
                let id = required(&body,"request_id")?;
                let approval = approvals.get(id).ok_or_else(|| Error::new(404,"Approval expired"))?;
                if approval.view["root_session_id"] != body["session_id"] || approval.view["user_id"] != body["user_id"] {
                    return Err(Error::new(403,"Approval belongs to another session"));
                }
                if approval.reply.is_closed() || approval.view["created_at"].as_i64().unwrap_or(0) + 300 <= chrono::Utc::now().timestamp() {
                    approvals.remove(id);
                    return Err(Error::new(409,"Approval expired or turn cancelled"));
                }
                let scope = body["scope"].as_str().unwrap_or("exact");
                if !matches!(scope,"exact"|"session"|"session_directory"|"persistent_directory") || (scope=="session" && approval.view["allow_session"]!=true) || (scope.ends_with("_directory") && approval.view["allow_directory"]!=true) {
                    return Err(Error::new(400,"Approval scope is not available for this action"));
                }
                let reply = if path.ends_with("/deny") {crate::approval::Reply::Deny} else if scope.ends_with("_directory") {
                    let rule=self.make_directory_rule(&json!({"path":required(&body,"directory")?,"recursive":body["recursive"].as_bool().unwrap_or(true)}), if scope=="session_directory" {Some(required(&body,"session_id")?)} else {None})?;
                    if !rule.recursive && matches!(required(&approval.view,"tool_name")?,"grep_search"|"glob_search") {return Err(Error::new(400,"Recursive search requires a recursive directory grant"));}
                    let target=std::path::Path::new(required(&approval.view,"exact_target")?);
                    if !target.starts_with(&rule.path) || (!rule.recursive && target!=rule.path && target.parent()!=Some(rule.path.as_path())) {return Err(Error::new(400,"Selected directory does not cover this action"));}
                    crate::approval::Reply::Directory(Box::new(rule))
                } else if scope=="session" {crate::approval::Reply::Session} else {crate::approval::Reply::Once};
                let approval = approvals.remove(id).unwrap();
                approval.reply.send(reply).map_err(|_| Error::new(409,"Turn no longer waiting for approval"))?;
                return Ok(json!({"success":true,"request_id":id,"message":"Decision recorded","tool_name":approval.view["tool_name"]}));
            }
            ("DELETE", "/api/approval/review-cache") => {
                if required(&body,"user_id")?!="default"{return Err(Error::new(403,"Unknown user"));}
                return self.clear_review_cache(required(&body,"session_id")?);
            }
            ("POST", "/api/approval/revoke-session") => {
                if required(&body,"user_id")? != "default" {return Err(Error::new(403,"Unknown user"));}
                self.revoke_approval_grants(required(&body,"session_id")?)?;
                return Ok(json!({"success":true}));
            }
            ("GET", "/api/approval/audit") => return self.db()?.get(&format!("approval_audit:{}",query("session_id")),json!([])),
            ("GET", "/api/native/cloud") => return self.cloud_settings(),
            ("POST", "/api/native/cloud/login/start") => return self.begin_cloud_login(body).await,
            ("POST", "/api/native/cloud/login/poll") => return self.poll_cloud_login().await,
            ("POST", "/api/native/cloud/login/cancel") => return self.cancel_cloud_login(),
            ("POST", "/api/native/cloud/refresh") => return self.refresh_cloud_models().await,
            ("POST", "/api/native/cloud/logout") => return self.logout_cloud().await,
            ("GET", "/api/models/active") => return self.active_model(),
            ("PUT", "/api/models/active") => {
                let provider_id = required(&body,"provider_id")?;
                let model = required(&body,"model")?;
                if provider_id == crate::cloud::PROVIDER { self.cloud_connection(model)?; }
                let providers = self.providers()?;
                if !providers.iter().any(|p| p["id"] == provider_id) { return Err(Error::new(404,"Provider not found")); }
                let selection = json!({"provider_id":provider_id,"model":model});
                self.db()?.put_batch(&[("active".into(),selection.clone()),("active_manual".into(),selection)])?;
                return self.active_model();
            }
            ("GET", "/api/models") => return Ok(json!(self.public_providers()?)),
            ("POST", "/api/models/custom-providers") => {
                let id = required(&body,"id")?;
                if id == crate::cloud::PROVIDER { return Err(Error::new(403,"云端服务商由系统管理")); }
                let mut providers = self.providers()?;
                if providers.iter().any(|p|p["id"] == id) { return Err(Error::new(409,"Provider already exists")); }
                let protocol = body["chat_model"].as_str().unwrap_or("OpenAIChatModel");
                validate_protocol(protocol)?;
                let url = string(&body,"default_base_url");
                if !url.is_empty() { validate_url(url)?; }
                let mut p = provider(id, required(&body,"name")?, url, protocol, json!([]));
                if let Some(key) = body["api_key"].as_str() { p["api_key"] = json!(self.db()?.seal(key)?); }
                providers.push(p.clone()); self.save_providers(providers)?; p["api_key"] = json!(if string(&p,"api_key").is_empty() {""} else {"********"}); return Ok(p);
            }
            ("GET", "/api/native/media-settings") => return self.db()?.get("media", json!({"speech_provider_id":"","speech_model":"whisper-1","image_provider_id":"","image_model":"gpt-image-2"})),
            ("PUT", "/api/native/media-settings") => {
                for key in ["speech_provider_id","speech_model","image_provider_id","image_model"] {
                    if !body[key].is_string() { return Err(Error::new(400,format!("{key} must be a string"))); }
                }
                self.db()?.put("media", &body)?; return Ok(body);
            }
            ("GET", "/api/workspace/transcription-provider-type") => return Ok(json!({"transcription_provider_type":self.speech_type()?})),
            ("PUT", "/api/workspace/transcription-provider-type") => {
                let kind=required(&body,"transcription_provider_type")?;
                if !matches!(kind,"disabled"|"whisper_api"|"doubao_asr") { return Err(Error::new(501,"This speech provider is not available in Rust")); }
                let db=self.db()?;
                let mut doubao=db.get("doubao",json!({}))?;
                doubao["enabled"]=json!(kind=="doubao_asr");
                db.put("doubao",&doubao)?;
                db.put("speech_type",&json!(kind))?;
                return Ok(json!({"transcription_provider_type":kind}));
            }
            ("GET", "/api/workspace/speech-status") => {
                let doubao=self.db()?.get("doubao",json!({}))?;
                let credentials=!string(&doubao,"api_key").is_empty();
                let kind=self.speech_type()?;
                let media = self.db()?.get("media",json!({}))?;
                let ready = match kind.as_str() {
                    "doubao_asr"=>credentials,
                    "whisper_api"=>self.provider_connection(string(&media,"speech_provider_id"),string(&media,"speech_model")).is_ok(),
                    _=>false,
                };
                return Ok(json!({"transcription_provider_type":kind,"doubao_credentials_configured":credentials,"ffmpeg_available":true,"ready":ready}));
            }
            _ => {}
        }
        if let Some(id) = path
            .strip_prefix("/api/questions/")
            .and_then(|p| p.strip_suffix("/answer"))
        {
            if method != "POST" {
                return Err(Error::new(405, "Method not allowed"));
            }
            return self.answer_question(id, &body);
        }
        if path == "/api/native/history-health" && method == "GET" {
            return self.db()?.history_health();
        }
        if let Some(id) = path.strip_prefix("/api/chats/") {
            if let Some(id) = id.strip_suffix("/archive") {
                if method != "GET" {
                    return Err(Error::new(405, "Method not allowed"));
                }
                return self.db()?.archive_location(id);
            }
            if let Some(id) = id.strip_suffix("/context-stats") {
                if method != "GET" {
                    return Err(Error::new(405, "Method not allowed"));
                }
                let db = self.db()?;
                db.chat(id)?;
                return Ok(
                    json!({"context":db.get(&format!("context_stats:{id}"),Value::Null)?,"usage":db.get(&format!("usage:{id}"),Value::Null)?,"totals":db.get(&format!("usage_totals:{id}"),Value::Null)?}),
                );
            }
            let mut spec = self.db()?.chat(id)?;
            let running = lock(&self.runs)?.contains_key(string(&spec, "session_id"));
            return match method {
                "GET" => Ok(
                    json!({"messages":self.db()?.history(id,false)?,"status":if running {"running"} else {"idle"}}),
                ),
                "PUT" => {
                    if let Some(name) = body["name"].as_str() {
                        if name.trim().is_empty() || name.len() > 500 {
                            return Err(Error::new(
                                400,
                                "Conversation name must contain 1–500 bytes",
                            ));
                        }
                        spec["name"] = json!(name.trim());
                    }
                    if let Some(pinned) = body["pinned"].as_bool() {
                        spec["pinned"] = json!(pinned);
                    }
                    if let Some(archived) = body["archived"].as_bool() {
                        if running {
                            return Err(Error::new(409, "Stop the conversation before archiving"));
                        }
                        spec["archived"] = json!(archived);
                        spec["archived_at"] = if archived {
                            json!(chrono::Utc::now().to_rfc3339())
                        } else {
                            Value::Null
                        };
                    }
                    spec["updated_at"] = json!(chrono::Utc::now().to_rfc3339());
                    self.db()?.save_chat(&spec)?;
                    Ok(spec)
                }
                "DELETE" if !running => {
                    let deleted = self.db()?.delete_chat(id)?;
                    self.revoke_approval_grants(string(&spec, "session_id"))?;
                    Ok(json!({"deleted":deleted}))
                }
                "DELETE" => Err(Error::new(
                    409,
                    "Stop the running turn before deleting this chat",
                )),
                _ => Err(Error::new(405, "Method not allowed")),
            };
        }
        if let Some(rest) = path.strip_prefix("/api/models/potato-cloud/") {
            if method == "PUT" {
                if let Some(model) = rest.strip_prefix("models/").and_then(|s|s.strip_suffix("/config")) { return self.cloud_model_preference(model, &body); }
            }
            return Err(Error::new(403,"云端模型由服务端管理，请使用邮箱登录或刷新模型列表"));
        }
        if let Some(rest) = path.strip_prefix("/api/models/") {
            let mut providers = self.providers()?;
            if let Some(id) = rest
                .strip_prefix("custom-providers/")
                .filter(|_| method == "DELETE")
            {
                let target = providers
                    .iter()
                    .find(|p| p["id"] == id)
                    .ok_or_else(|| Error::new(404, "Provider not found"))?;
                if target["is_custom"] != true || matches!(id, "deepseek" | "sub2api") {
                    return Err(Error::new(400, "Built-in providers cannot be deleted"));
                }
                providers.retain(|p| p["id"] != id);
                self.save_providers(providers)?;
                let active = self.db()?.get("active", Value::Null)?;
                if active["provider_id"] == id {
                    self.db()?.put("active", &Value::Null)?;
                }
                return Ok(json!(self.public_providers()?));
            }
            let (id, action) = rest
                .split_once('/')
                .ok_or_else(|| Error::new(404, "Unknown model endpoint"))?;
            let p = providers
                .iter_mut()
                .find(|p| p["id"] == id)
                .ok_or_else(|| Error::new(404, "Provider not found"))?;
            match (method, action) {
                ("PUT", "config") => {
                    if let Some(key) = body["api_key"].as_str().filter(|k| *k != "********") {
                        p["api_key"] = json!(self.db()?.seal(key)?);
                    }
                    if let Some(url) = body["base_url"].as_str() {
                        if p["freeze_url"] == true && p["base_url"] != url {
                            return Err(Error::new(400, "Provider URL is managed by the system"));
                        }
                        validate_url(url)?;
                        p["base_url"] = json!(url);
                    }
                    if let Some(protocol) = body["chat_model"].as_str() {
                        validate_protocol(protocol)?;
                        p["chat_model"] = json!(protocol);
                    }
                }
                ("POST", "models") => {
                    let id = required(&body, "id")?;
                    let models = p["extra_models"].as_array_mut().unwrap();
                    if !models.iter().any(|m| m["id"] == id) {
                        models.push(json!({"id":id,"name":body["name"].as_str().unwrap_or(id)}));
                    }
                }
                ("PUT", action) if action.starts_with("models/") => {
                    let model_id = action[7..].strip_suffix("/config").unwrap_or(&action[7..]);
                    let key = if p["extra_models"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|m| m["id"] == model_id)
                    {
                        "extra_models"
                    } else {
                        "models"
                    };
                    let provider_capabilities = p.clone();
                    let model = p[key]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|m| m["id"] == model_id)
                        .ok_or_else(|| Error::new(404, "Model not found"))?;
                    for field in [
                        "name",
                        "supports_multimodal",
                        "supports_image",
                        "supports_video",
                        "is_free",
                        "max_tokens",
                        "max_input_length",
                        "reasoning_effort",
                        "reasoning_effort_options",
                        "thinking_param_style",
                    ] {
                        if let Some(value) = body.get(field) {
                            if matches!(field, "max_tokens" | "max_input_length")
                                && !value.is_null()
                                && !value.as_u64().is_some_and(|n| n > 0 && n <= 10_000_000)
                            {
                                return Err(Error::new(
                                    400,
                                    "Model token limits must be positive integers",
                                ));
                            }
                            if field == "reasoning_effort" && !value.is_null() && !value.is_string()
                            {
                                return Err(Error::new(400, "Reasoning effort must be text"));
                            }
                            if field == "reasoning_effort_options"
                                && !value.is_null()
                                && !value.as_array().is_some_and(|a| {
                                    a.iter()
                                        .all(|v| v.as_str().is_some_and(|s| !s.trim().is_empty()))
                                })
                            {
                                return Err(Error::new(
                                    400,
                                    "Reasoning options must be an array of nonempty strings",
                                ));
                            }
                            model[field] = value.clone();
                        }
                    }
                    if body
                        .get("reasoning_effort")
                        .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                        && (model["reasoning_effort_options"].is_array()
                            || provider_capabilities["reasoning_effort_options"].is_array())
                        && crate::reasoning::effective_effort(&provider_capabilities, model)
                            .is_none()
                    {
                        return Err(Error::new(
                            400,
                            "This model does not support the selected reasoning effort",
                        ));
                    }
                    if body.get("reasoning_effort_options").is_some()
                        && crate::reasoning::effective_effort(&provider_capabilities, model)
                            .is_none()
                    {
                        model["reasoning_effort"] = Value::Null;
                    }
                    if let (Some(input), Some(output)) = (
                        model["max_input_length"].as_u64(),
                        model["max_tokens"].as_u64(),
                    ) {
                        if output >= input {
                            return Err(Error::new(
                                400,
                                "Output token limit must be smaller than context capacity",
                            ));
                        }
                    }
                }
                ("DELETE", action) if action.starts_with("models/") => {
                    let id = &action[7..];
                    if !["models", "extra_models"].iter().any(|key| {
                        p[*key]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|m| m["id"] == id)
                    }) {
                        return Err(Error::new(404, "Model not found"));
                    }
                    for key in ["models", "extra_models"] {
                        p[key].as_array_mut().unwrap().retain(|m| m["id"] != id);
                    }
                }
                ("POST", "discover" | "test") => {
                    let mut config = p.clone();
                    if let Some(url) = body["base_url"].as_str() {
                        config["base_url"] = json!(url);
                    }
                    let key =
                        if let Some(key) = body["api_key"].as_str().filter(|k| *k != "********") {
                            key.to_owned()
                        } else {
                            self.db()?.unseal(string(p, "api_key"))?
                        };
                    validate_url(string(&config, "base_url"))?;
                    let response = self
                        .client
                        .get(format!(
                            "{}/models",
                            string(&config, "base_url").trim_end_matches('/')
                        ))
                        .bearer_auth(key)
                        .send()
                        .await?;
                    if !response.status().is_success() {
                        return Err(Error::new(
                            502,
                            format!(
                                "Model discovery returned HTTP {}",
                                response.status().as_u16()
                            ),
                        ));
                    }
                    let value: Value = response.json().await?;
                    let models: Vec<_> = value["data"]
                        .as_array()
                        .ok_or_else(|| Error::new(502, "Invalid model list"))?
                        .iter()
                        .filter_map(crate::reasoning::discovered_model)
                        .collect();
                    if action == "test" {
                        return Ok(json!({"success":true,"message":"Connected"}));
                    }
                    let count = models.len();
                    let mut merged = p["extra_models"].as_array().cloned().unwrap_or_default();
                    for model in &models {
                        if let Some(existing) = merged
                            .iter_mut()
                            .find(|existing| existing["id"] == model["id"])
                        {
                            for field in ["reasoning_effort_options", "thinking_param_style"] {
                                if existing.get(field).is_none_or(Value::is_null) {
                                    if let Some(value) = model.get(field) {
                                        existing[field] = value.clone();
                                    }
                                }
                            }
                        } else {
                            merged.push(model.clone());
                        }
                    }
                    p["extra_models"] = json!(merged);
                    let result = json!({"success":true,"models":models,"message":"Models discovered","added_count":count});
                    self.save_providers(providers)?;
                    return Ok(result);
                }
                _ => return Err(Error::new(501, "This model setting has not been migrated")),
            }
            let mut result = p.clone();
            result["api_key"] = json!(if string(p, "api_key").is_empty() {
                ""
            } else {
                "********"
            });
            self.save_providers(providers)?;
            if method == "DELETE" && action.starts_with("models/") {
                let active = self.db()?.get("active", Value::Null)?;
                if active["provider_id"] == id && active["model"] == action[7..] {
                    self.db()?.put("active", &Value::Null)?;
                }
            }
            return Ok(result);
        }
        Err(Error::new(
            501,
            format!("This feature has not been migrated to Rust: {method} {path}"),
        ))
    }

    pub(crate) fn providers(&self) -> Result<Vec<Value>> {
        let mut providers = self
            .db()?
            .get("providers", default_providers())?
            .as_array()
            .ok_or_else(|| Error::new(500, "Invalid provider store"))?
            .clone();
        // Older stores predate model capacities. Fill only absent built-in
        // limits, including an extra_models override, without replacing user settings.
        for provider in &mut providers {
            if provider["id"] == "deepseek" {
                for key in ["models", "extra_models"] {
                    if let Some(models) = provider[key].as_array_mut() {
                        for model in models {
                            if matches!(string(model, "id"), "deepseek-chat" | "deepseek-reasoner")
                                && model["max_input_length"].is_null()
                            {
                                model["max_input_length"] = json!(128_000);
                            }
                        }
                    }
                }
            }
        }
        providers.retain(|p| p["id"] != crate::cloud::PROVIDER);
        if let Some(cloud) = self.cloud_provider()? { providers.insert(0, cloud); }
        Ok(providers)
    }
    pub(crate) fn speech_type(&self) -> Result<String> {
        let db = self.db()?;
        if db.get("doubao", json!({}))?["enabled"] == true {
            return Ok("doubao_asr".into());
        }
        let saved = db.get("speech_type", Value::Null)?;
        if let Some(kind) = saved.as_str() {
            return Ok(kind.to_owned());
        }
        let media = db.get("media", json!({}))?;
        Ok(if string(&media, "speech_provider_id").is_empty() {
            "disabled"
        } else {
            "whisper_api"
        }
        .into())
    }
    fn save_providers(&self, mut providers: Vec<Value>) -> Result<()> {
        providers.retain(|p| p["id"] != crate::cloud::PROVIDER);
        self.db()?.put("providers", &json!(providers))
    }
    fn public_providers(&self) -> Result<Vec<Value>> {
        let mut providers = self.providers()?;
        for p in &mut providers {
            if matches!(string(p, "id"), "deepseek" | "sub2api") {
                p["is_custom"] = json!(false);
            }
            let env_configured = self.model_env_key_at(None, string(p,"id"), "").is_some();
            p["api_key"] = json!(if string(p, "api_key").is_empty() && !env_configured {
                ""
            } else {
                "********"
            });
        }
        Ok(providers)
    }
    fn active_model(&self) -> Result<Value> {
        Ok(
            json!({"active_llm":self.ensure_model_selection()?,"effective_max_input_length":null}),
        )
    }
}

pub(crate) fn validate_protocol(protocol: &str) -> Result<()> {
    if matches!(protocol, "OpenAIChatModel" | "OpenAIResponseModel") {
        Ok(())
    } else {
        Err(Error::new(501, "This model protocol has not been migrated"))
    }
}
pub(crate) fn validate_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|_| Error::new(400, "A valid provider base URL is required"))?;
    if !matches!(parsed.scheme(), "https" | "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(Error::new(
            400,
            "Provider URL must be HTTP(S), without credentials, query or fragment",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod capacity_tests {
    use super::*;

    #[test]
    fn builtin_capacity_reaches_connections_and_preserves_saved_overrides() {
        let root = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(root.path()).unwrap();
        let mut providers = default_providers();
        for model in providers[0]["models"].as_array().unwrap() {
            assert_eq!(model["max_input_length"], 128_000);
            assert!(crate::context::Budget::new(model).trigger > 90_000);
        }
        // Simulate a pre-upgrade store and an explicitly customized capacity.
        providers[0]["models"][0]
            .as_object_mut()
            .unwrap()
            .remove("max_input_length");
        providers[0]["models"][1]["max_input_length"] = json!(64_000);
        providers[0]["extra_models"] = json!([
            {"id":"deepseek-chat","max_tokens":2048},
            {"id":"custom-model","name":"Custom"}
        ]);
        providers[0]["api_key"] = json!(runtime.db().unwrap().seal("fixture-key").unwrap());
        runtime.db().unwrap().put("providers", &providers).unwrap();
        let migrated = runtime.providers().unwrap();
        assert_eq!(migrated[0]["models"][0]["max_input_length"], 128_000);
        assert_eq!(migrated[0]["models"][1]["max_input_length"], 64_000);
        assert_eq!(migrated[0]["extra_models"][0]["max_input_length"], 128_000);
        assert!(migrated[0]["extra_models"][1]["max_input_length"].is_null());
        let connection = runtime
            .provider_connection("deepseek", "deepseek-chat")
            .unwrap();
        assert_eq!(connection.options["max_input_length"], 128_000);
        assert_eq!(connection.options["max_tokens"], 2048);
        assert_eq!(
            runtime.db().unwrap().get("providers", Value::Null).unwrap(),
            providers
        );
    }
}
