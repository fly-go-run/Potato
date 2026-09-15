//! Email-authenticated model service. Credentials are separate from remote control.
use crate::{Error, Result, Runtime, lock, model::Connection, required, string};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::time::Duration;

pub(crate) const PROVIDER: &str = "potato-cloud";
const SERVICE: &str = "https://potato-remote.recodex.top/";
pub(crate) fn service_url(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value).map_err(|_| Error::new(400, "云端服务地址无效"))?;
    let fixture = cfg!(debug_assertions)
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost"));
    if (!fixture && value != SERVICE)
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Error::new(400, "云端登录必须使用 Potato 服务"));
    }
    Ok(url)
}
pub(crate) fn alive(config: &Value) -> bool {
    config["expires"].as_i64().unwrap_or(0) > chrono::Utc::now().timestamp_millis()
        && config["session_token"].is_string()
}

impl Runtime {
    pub(crate) fn cloud_settings(&self) -> Result<Value> {
        let db = self.db()?;
        let config = db.get("cloud_config", Value::Null)?;
        let pending = db.get("cloud_login", Value::Null)?;
        let login =
            if pending["expires"].as_i64().unwrap_or(0) > chrono::Utc::now().timestamp_millis() {
                json!({"verification_url":pending["verification_url"],"code":pending["code"]})
            } else {
                Value::Null
            };
        Ok(
            json!({"signed_in":alive(&config),"email":config["email"],"expires":config["expires"],"login":login,
            "model_count":config["models"].as_array().map_or(0, Vec::len),"error":config["error"],"has_session":config["session_token"].is_string(),"local_config_error":db.get("legacy_auto_models_error",Value::Null)?}),
        )
    }
    pub(crate) fn cancel_cloud_login(&self) -> Result<Value> {
        let mut generation = lock(&self.cloud_generation)?;
        *generation += 1;
        self.db()?.put("cloud_login", &Value::Null)?;
        drop(generation);
        self.cloud_settings()
    }
    pub(crate) async fn begin_cloud_login(&self, body: Value) -> Result<Value> {
        let relay = service_url(body["relay"].as_str().unwrap_or(SERVICE))?;
        if self.db()?.get("cloud_config", Value::Null)?["session_token"].is_string() {
            return Err(Error::new(409, "请先退出当前云端账号，再重新登录"));
        }
        let generation = {
            let mut generation = lock(&self.cloud_generation)?;
            *generation += 1;
            self.db()?.put("cloud_login", &Value::Null)?;
            *generation
        };
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let login = self.cloud_http(&relay, "v1/cloud/auth/start", "", Some(json!({"role":"cloud","name":format!("Potato · {}",std::env::consts::OS),"client_token":token}))).await?;
        let id = required(&login, "id")?;
        uuid::Uuid::parse_str(id).map_err(|_| Error::new(502, "登录编号无效"))?;
        let url = reqwest::Url::parse(required(&login, "verification_url")?)
            .map_err(|_| Error::new(502, "登录地址无效"))?;
        if url.origin() != relay.origin()
            || url.path() != "/v1/cloud/auth/authorize"
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query_pairs().count() != 1
            || url
                .query_pairs()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.into_owned())
                .as_deref()
                != Some(id)
        {
            return Err(Error::new(502, "登录页面不属于云端服务"));
        }
        let expires = login["expires"]
            .as_i64()
            .filter(|n| *n > chrono::Utc::now().timestamp_millis())
            .ok_or_else(|| Error::new(502, "登录请求已过期"))?;
        let code = required(&login, "code")?;
        if code.len() != 8 || !code.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(Error::new(502, "登录确认码无效"));
        }
        let guard = lock(&self.cloud_generation)?;
        if *guard != generation {
            return Err(Error::new(409, "登录已取消"));
        }
        let db = self.db()?;
        db.put("cloud_login", &json!({"id":id,"relay":relay.as_str(),"verification_url":url.as_str(),"code":code,"expires":expires,"client_token":db.seal(&token)?}))?;
        drop(db);
        drop(guard);
        self.cloud_settings()
    }
    pub(crate) async fn poll_cloud_login(&self) -> Result<Value> {
        let (login, generation) = {
            let guard = lock(&self.cloud_generation)?;
            (self.db()?.get("cloud_login", Value::Null)?, *guard)
        };
        if login.is_null() {
            return self.cloud_settings();
        }
        if login["expires"].as_i64().unwrap_or(0) <= chrono::Utc::now().timestamp_millis() {
            self.cancel_cloud_login()?;
            return Err(Error::new(401, "登录已过期，请重新开始"));
        }
        let relay = service_url(required(&login, "relay")?)?;
        let token = self.db()?.unseal(required(&login, "client_token")?)?;
        let reply = self
            .cloud_http(
                &relay,
                "v1/cloud/auth/poll",
                "",
                Some(json!({"id":login["id"],"client_token":token})),
            )
            .await?;
        if reply["status"] == "pending" {
            return self.cloud_settings();
        }
        let owner = required(&reply, "owner")?;
        let email = required(&reply, "email")?;
        let expires = reply["expires"]
            .as_i64()
            .filter(|n| *n > chrono::Utc::now().timestamp_millis())
            .ok_or_else(|| Error::new(502, "登录会话已过期"))?;
        if reply["status"] != "authorized"
            || owner.len() != 64
            || !owner.bytes().all(|b| b.is_ascii_hexdigit())
            || email.len() > 320
            || !email.contains('@')
        {
            return Err(Error::new(502, "登录服务返回无效身份"));
        }
        let session = format!("{}.{}.{}", owner, required(&login, "id")?, token);
        {
            let mut guard = lock(&self.cloud_generation)?;
            if *guard != generation {
                return Err(Error::new(409, "登录已取消"));
            }
            let db = self.db()?;
            db.put("cloud_config", &json!({"relay":relay.as_str(),"email":email,"expires":expires,"session_token":db.seal(&session)?,"models":[]}))?;
            db.put("cloud_login", &Value::Null)?;
            *guard += 1;
        }
        self.refresh_cloud_models().await
    }
    pub(crate) async fn refresh_cloud_models(&self) -> Result<Value> {
        let (mut config, generation) = {
            let guard = lock(&self.cloud_generation)?;
            (self.db()?.get("cloud_config", Value::Null)?, *guard)
        };
        if !alive(&config) {
            return Err(Error::new(401, "请使用邮箱重新登录云端模型"));
        }
        let relay = service_url(required(&config, "relay")?)?;
        let token = self.db()?.unseal(required(&config, "session_token")?)?;
        let response = self
            .cloud_http(&relay, "v1/models", &token, None)
            .await
            .and_then(validate_catalog);
        let guard = lock(&self.cloud_generation)?;
        if *guard != generation {
            return Err(Error::new(409, "云端账号已改变"));
        }
        let db = self.db()?;
        match response {
            Ok((models, default_model)) => {
                config["models"] = json!(models);
                config["default_model"] = json!(default_model);
                config["error"] = Value::Null;
            }
            Err(error) => {
                config["error"] = json!(error.message);
                if matches!(error.status, 401 | 403) {
                    config["models"] = json!([]);
                }
            }
        }
        db.put("cloud_config", &config)?;
        drop(db);
        drop(guard);
        self.ensure_model_selection()?;
        self.cloud_settings()
    }
    pub(crate) async fn logout_cloud(&self) -> Result<Value> {
        // Invalidate in-flight login/catalog work before the network round trip.
        let (config, generation) = {
            let mut guard = lock(&self.cloud_generation)?;
            *guard += 1;
            self.db()?.put("cloud_login", &Value::Null)?;
            (self.db()?.get("cloud_config", Value::Null)?, *guard)
        };
        if let Some(sealed) = config["session_token"].as_str() {
            let relay = service_url(required(&config, "relay")?)?;
            let token = self.db()?.unseal(sealed)?;
            if let Err(error) = self
                .cloud_http(&relay, "v1/cloud/account/logout", &token, Some(json!({})))
                .await
            {
                if !matches!(error.status, 401 | 403) {
                    return Err(error);
                }
            }
        }
        let guard = lock(&self.cloud_generation)?;
        if *guard != generation {
            return Err(Error::new(409, "云端账号已改变"));
        }
        let db = self.db()?;
        db.put("cloud_config", &Value::Null)?;
        db.put(
            &format!("cloud_memory_cache:{}", crate::cloud_memory::owner_key(&config)),
            &Value::Null,
        )?;
        db.put("cloud_preferences", &json!({}))?;
        if db.get("active", Value::Null)?["provider_id"] == PROVIDER {
            db.put("active", &Value::Null)?;
        }
        if db.get("active_manual", Value::Null)?["provider_id"] == PROVIDER {
            db.put("active_manual", &Value::Null)?;
        }
        drop(db);
        drop(guard);
        self.cloud_settings()
    }
    pub(crate) fn cloud_provider(&self) -> Result<Option<Value>> {
        let db = self.db()?;
        let config = db.get("cloud_config", Value::Null)?;
        if !alive(&config) {
            return Ok(None);
        }
        let mut models = config["models"].as_array().cloned().unwrap_or_default();
        let preferences = db.get("cloud_preferences", json!({}))?;
        for model in &mut models {
            let id = string(model, "id").to_owned();
            model["reasoning_effort"] = preferences[&id].clone();
            model["reasoning_effort"] = crate::reasoning::effective_effort(&Value::Null, model)
                .map_or(Value::Null, |s| json!(s));
        }
        Ok(Some(
            json!({"id":PROVIDER,"name":"云端模型","managed":true,"is_custom":false,"is_local":false,"freeze_url":true,"require_api_key":false,"api_key":"********","models":models,"extra_models":[],"chat_model":"OpenAIChatModel"}),
        ))
    }
    pub(crate) fn cloud_connection(&self, model: &str) -> Result<Connection> {
        let provider = self
            .cloud_provider()?
            .ok_or_else(|| Error::new(401, "请使用邮箱登录云端模型"))?;
        let options = provider["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == model)
            .cloned()
            .ok_or_else(|| Error::new(400, "云端模型已改变，请刷新模型列表"))?;
        let config = self.db()?.get("cloud_config", Value::Null)?;
        let relay = service_url(required(&config, "relay")?)?;
        Ok(Connection {
            cache_key: String::new(),
            url: relay.join("v1/desktop").unwrap().to_string(),
            key: self.db()?.unseal(required(&config, "session_token")?)?,
            model: model.to_owned(),
            responses: false,
            options,
        })
    }
    pub(crate) fn cloud_model_preference(&self, model: &str, body: &Value) -> Result<Value> {
        if !body
            .as_object()
            .is_some_and(|o| o.len() == 1 && o.contains_key("reasoning_effort"))
        {
            return Err(Error::new(403, "云端模型配置由服务端管理"));
        }
        let provider = self
            .cloud_provider()?
            .ok_or_else(|| Error::new(401, "请先登录云端模型"))?;
        let entry = provider["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == model)
            .ok_or_else(|| Error::new(404, "云端模型不存在"))?;
        let effort = &body["reasoning_effort"];
        if !effort.is_null()
            && !crate::reasoning::effort_options(&provider, entry)
                .iter()
                .any(|s| effort == s)
        {
            return Err(Error::new(400, "此模型不支持该思考档位"));
        }
        let db = self.db()?;
        let mut preferences = db.get("cloud_preferences", json!({}))?;
        preferences[model] = effort.clone();
        db.put("cloud_preferences", &preferences)?;
        drop(db);
        Ok(self.cloud_provider()?.unwrap_or(Value::Null))
    }
    pub(crate) async fn cloud_http(
        &self,
        relay: &reqwest::Url,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let url = relay
            .join(path)
            .map_err(|_| Error::new(400, "云端地址无效"))?;
        let request = if let Some(body) = body {
            self.client.post(url).json(&body)
        } else {
            self.client.get(url)
        };
        let response = request
            .bearer_auth(token)
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|_| Error::new(502, "无法连接云端服务，请检查网络后重试"))?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(Error::new(
                status,
                match status {
                    401 => "登录已失效，请重新登录",
                    403 => "此邮箱尚未获准使用模型，请联系邀请你的人",
                    429 => "请求较多，请稍后重试",
                    _ => "云端服务暂不可用，请稍后重试",
                },
            ));
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            bytes.extend_from_slice(&chunk.map_err(|_| Error::new(502, "云端连接中断，请重试"))?);
            if bytes.len() > 512 * 1024 {
                return Err(Error::new(502, "云端响应过大"));
            }
        }
        serde_json::from_slice(&bytes).map_err(|_| Error::new(502, "云端返回格式无效"))
    }
}

fn validate_catalog(value: Value) -> Result<(Vec<Value>, String)> {
    let fail = || Error::new(502, "云端模型目录无效");
    let rows = value["data"]
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= 500)
        .ok_or_else(fail)?;
    let mut models = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for row in rows {
        let id = row["id"]
            .as_str()
            .filter(|id| {
                id.len() <= 240
                    && id
                        .split_once('/')
                        .is_some_and(|(p, m)| !p.is_empty() && !m.is_empty())
            })
            .ok_or_else(fail)?;
        if !ids.insert(id) {
            return Err(fail());
        }
        let mut model = json!({"id":id,"name":row["name"].as_str().unwrap_or(id),"thinking_param_style":"effort"});
        if let Some(options) = row["reasoning_effort_options"].as_array() {
            if options.len() > 32
                || !options
                    .iter()
                    .all(|o| o.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 32))
            {
                return Err(fail());
            }
            model["reasoning_effort_options"] = json!(options);
        } else {
            model["reasoning_effort_options"] = json!([]);
        }
        for field in [
            "supports_image",
            "supports_multimodal",
            "max_input_length",
            "max_tokens",
        ] {
            if let Some(value) = row.get(field) {
                model[field] = value.clone();
            }
        }
        models.push(model);
    }
    let default = value["default_model"]
        .as_str()
        .filter(|id| ids.contains(id))
        .ok_or_else(fail)?
        .to_owned();
    Ok((models, default))
}
