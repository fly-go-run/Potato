//! Account-scoped cache of cloud personal memory; the cloud remains authoritative.
use crate::{cloud, lock, required, string, Error, Result, Runtime};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

pub(crate) fn owner_key(config: &Value) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!(
            "{}\n{}",
            string(config, "relay"),
            string(config, "email")
        ))
    )
}
fn memories(status: &Value) -> Result<Vec<Value>> {
    Ok(status["memories"]
        .as_array()
        .ok_or_else(|| Error::new(502, "云端记忆返回格式无效"))?
        .iter()
        .filter(|m| m["forgotten"] != true)
        .cloned()
        .collect())
}
impl Runtime {
    pub(crate) fn cloud_memory_active(&self) -> Result<Option<(reqwest::Url, String, String)>> {
        let db = self.db()?;
        let config = db.get("cloud_config", Value::Null)?;
        if !cloud::alive(&config) {
            return Ok(None);
        }
        Ok(Some((
            cloud::service_url(required(&config, "relay")?)?,
            db.unseal(required(&config, "session_token")?)?,
            owner_key(&config),
        )))
    }
    pub(crate) fn cloud_memory_cache(&self) -> Result<Value> {
        let Some((_, _, owner)) = self.cloud_memory_active()? else {
            return Ok(Value::Null);
        };
        self.db()?
            .get(&format!("cloud_memory_cache:{owner}"), Value::Null)
    }
    pub(crate) async fn refresh_cloud_memory(&self, force: bool) -> Result<()> {
        let (active, generation) = {
            let guard = lock(&self.cloud_generation)?;
            (self.cloud_memory_active()?, *guard)
        };
        let Some((relay, token, owner)) = active else {
            return Ok(());
        };
        let key = format!("cloud_memory_cache:{owner}");
        let cache = self.db()?.get(&key, Value::Null)?;
        if !force
            && cache["fetched_at"]
                .as_i64()
                .is_some_and(|at| chrono::Utc::now().timestamp_millis() - at < 300_000)
        {
            return Ok(());
        }
        let rows = memories(
            &self
                .cloud_http(&relay, "v1/recall/status", &token, None)
                .await?,
        )?;
        let guard = lock(&self.cloud_generation)?;
        if *guard != generation || self.cloud_memory_active()? != Some((relay, token, owner)) {
            return Err(Error::new(409, "云端账号已改变"));
        }
        self.db()?.put(
            &key,
            &json!({"memories":rows,"fetched_at":chrono::Utc::now().timestamp_millis()}),
        )
    }
    pub(crate) fn cloud_memory_guidance(&self) -> Result<Option<String>> {
        if self.cloud_memory_active()?.is_none() {
            return Ok(None);
        }
        let cache = self.cloud_memory_cache()?;
        let Some(at) = cache["fetched_at"].as_i64() else {
            return Ok(Some("Personal memory (cloud): not loaded yet.".into()));
        };
        let mut rows = memories(&cache)?;
        rows.sort_by(|a, b| string(b, "updated").cmp(string(a, "updated")));
        let synced = chrono::DateTime::from_timestamp_millis(at)
            .ok_or_else(|| Error::new(502, "云端记忆缓存时间无效"))?
            .to_rfc3339();
        let mut out = format!(
            "Personal memory (cloud, {} items, synced {}{}):",
            rows.len(),
            synced,
            if chrono::Utc::now().timestamp_millis() - at > 1_800_000 {
                ", offline"
            } else {
                ""
            }
        );
        for (i, row) in rows.iter().enumerate() {
            let line = format!(
                "\n- [{}] {}",
                string(row, "id").chars().take(8).collect::<String>(),
                string(row, "text")
                    .chars()
                    .take(120)
                    .collect::<String>()
                    .replace(['\n', '\r'], " ")
            );
            let remainder = rows.len() - i - 1;
            let reserve = if remainder > 0 {
                format!("\n- … {remainder} more; use memory_search").len()
            } else {
                0
            };
            if out.len() + line.len() + reserve > 4000 {
                out.push_str(&format!("\n- … {} more; use memory_search", rows.len() - i));
                break;
            }
            out.push_str(&line);
        }
        Ok(Some(out))
    }
    pub(crate) fn search_cloud_memory(&self, query: &str) -> Result<Vec<Value>> {
        if self.cloud_memory_active()?.is_none() {
            return Ok(Vec::new());
        }
        let normalized = query.nfkc().collect::<String>().to_lowercase();
        let terms: Vec<_> = normalized.split_whitespace().collect();
        let cache = self.cloud_memory_cache()?;
        Ok(cache["memories"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| {
                let text = string(m, "text").nfkc().collect::<String>().to_lowercase();
                m["forgotten"] != true && terms.iter().all(|term| text.contains(term))
            })
            .take(30)
            .map(|m| json!({"scope":"cloud","id":m["id"],"text":m["text"],"updated":m["updated"]}))
            .collect())
    }
    pub(crate) async fn cloud_remember(&self, text: &str) -> Result<Value> {
        let (relay, token, _) = self
            .cloud_memory_active()?
            .ok_or_else(|| Error::new(401, "请先登录云端"))?;
        let id = uuid::Uuid::new_v4().to_string();
        if let Err(error) = self
            .cloud_http(
                &relay,
                "v1/recall/memory",
                &token,
                Some(json!({"id":id,"text":text,"base":null,"forget":false})),
            )
            .await
        {
            if error.status < 500 {
                return Err(error);
            }
            let status = self
                .cloud_http(&relay, "v1/recall/status", &token, None)
                .await
                .and_then(|s| {
                    s["memories"]
                        .as_array()
                        .cloned()
                        .ok_or_else(|| Error::new(502, "云端记忆返回格式无效"))
                })
                .map_err(|_| Error::new(502, "云端记忆暂不可用，未确认是否已保存"))?;
            if !status.iter().any(|m| m["id"] == id) {
                return Err(error);
            }
        }
        let _ = self.refresh_cloud_memory(true).await;
        Ok(json!({"id":id,"saved":true}))
    }
    pub(crate) fn cached_cloud_memory(&self, id: &str) -> Result<Value> {
        self.cloud_memory_cache()?["memories"]
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|m| m["id"] == id && m["forgotten"] != true)
            })
            .cloned()
            .ok_or_else(|| Error::new(404, "记忆不存在或已被删除"))
    }
    pub(crate) async fn cloud_forget(&self, id: &str) -> Result<Value> {
        let (relay, token, _) = self
            .cloud_memory_active()?
            .ok_or_else(|| Error::new(401, "请先登录云端"))?;
        let row = self.cached_cloud_memory(id)?;
        if let Err(error) = self
            .cloud_http(
                &relay,
                "v1/recall/memory",
                &token,
                Some(json!({"id":id,"text":"","base":row["revision"],"forget":true})),
            )
            .await
        {
            if error.status == 409 {
                let _ = self.refresh_cloud_memory(true).await;
                return Err(Error::new(409, "记忆已在别处修改，请重试"));
            }
            let status = self
                .cloud_http(&relay, "v1/recall/status", &token, None)
                .await
                .and_then(|s| memories(&s))
                .map_err(|_| Error::new(502, "云端记忆暂不可用，未确认是否已保存"))?;
            if status.iter().any(|m| m["id"] == id) {
                return Err(error);
            }
        }
        let _ = self.refresh_cloud_memory(true).await;
        Ok(json!({"id":id,"forgotten":true}))
    }
}
