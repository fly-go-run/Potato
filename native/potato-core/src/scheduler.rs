//! Persistent local scheduler. Claims are saved before execution; interrupted
//! runs are reported, never blindly replayed after restart.
use crate::{protocol, required, string, Error, Result, Runtime};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::{str::FromStr, sync::Arc, time::Duration};
use tokio::sync::mpsc;

fn timestamp(value: &Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.as_str()?)
        .ok()
        .map(|v| v.with_timezone(&Utc))
}

fn cron_schedule(value: &Value) -> Result<(cron::Schedule, chrono_tz::Tz)> {
    let expression = required(value, "cron")?;
    let mut fields: Vec<String> = expression.split_whitespace().map(str::to_owned).collect();
    if !(3..=5).contains(&fields.len()) {
        return Err(Error::new(400, "Cron requires 3–5 fields, without seconds"));
    }
    while fields.len() < 5 {
        fields.insert(0, "0".into());
    }
    // Expand numeric ranges before mapping Sunday to 1: 1-7 must remain
    // Monday through Sunday, and range steps must start at the original bound.
    let mut weekdays = Vec::new();
    for item in fields[4].split(',') {
        let (range, step) = item.split_once('/').unwrap_or((item, "1"));
        if let Some((start, end)) = range.split_once('-') {
            if let (Ok(start), Ok(end)) = (start.parse::<usize>(), end.parse::<usize>()) {
                let step = step
                    .parse::<usize>()
                    .map_err(|_| Error::new(400, "Invalid weekday step"))?;
                if start > end || end > 7 || step == 0 {
                    return Err(Error::new(400, "Invalid weekday range"));
                }
                weekdays.extend((start..=end).step_by(step).map(|day| day.to_string()));
                continue;
            }
        }
        weekdays.push(item.to_owned());
    }
    fields[4] = weekdays.join(",");
    // Python normalized crontab 0/7=Sunday; cron crate uses 1=Sunday.
    // Use names so the public contract is independent of library numbering.
    let names = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    let mut dow = String::new();
    let mut number = String::new();
    let mut step = false;
    for ch in fields[4].chars().chain(std::iter::once(',')) {
        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }
        if !number.is_empty() {
            if step {
                dow.push_str(&number);
            } else {
                let n = number
                    .parse::<usize>()
                    .map_err(|_| Error::new(400, "Invalid weekday"))?;
                dow.push_str(
                    names
                        .get(n)
                        .ok_or_else(|| Error::new(400, "Weekday must be 0–7"))?,
                );
            }
            number.clear();
        }
        dow.push(ch);
        if ch == '/' {
            step = true;
        } else if ch == ',' {
            step = false;
        }
    }
    fields[4] = dow.trim_end_matches(',').to_owned();
    let schedule = cron::Schedule::from_str(&format!("0 {} *", fields.join(" ")))
        .map_err(|_| Error::new(400, "Invalid cron expression"))?;
    let tz = value["timezone"]
        .as_str()
        .unwrap_or("UTC")
        .parse()
        .map_err(|_| Error::new(400, "Unknown IANA timezone"))?;
    Ok((schedule, tz))
}

fn next_time(schedule: &Value, after: DateTime<Utc>) -> Result<Option<DateTime<Utc>>> {
    match string(schedule, "type") {
        "once" => Ok(timestamp(&schedule["run_at"])
            .or_else(|| timestamp(&schedule["at"]))
            .filter(|t| *t > after)),
        "cron" => {
            let (cron, tz) = cron_schedule(schedule)?;
            Ok(cron
                .after(&after.with_timezone(&tz))
                .next()
                .map(|v| v.with_timezone(&Utc)))
        }
        _ => Err(Error::new(400, "Unknown schedule type")),
    }
}

fn validated(mut spec: Value, id: &str, now: DateTime<Utc>) -> Result<Value> {
    required(&spec, "name")?;
    let target = &spec["dispatch"]["target"];
    required(target, "session_id")?;
    if spec["dispatch"]["channel"] != "console" || spec["dispatch"]["type"] != "channel" {
        return Err(Error::new(
            400,
            "Desktop tasks must target a console conversation",
        ));
    }
    if target["user_id"].as_str().unwrap_or("default") != "default" {
        return Err(Error::new(400, "Unknown desktop user"));
    }
    match string(&spec, "task_type") {
        "text" => {
            required(&spec, "text")?;
            if spec["dispatch"]["silent"] == true {
                return Err(Error::new(400, "Text reminders cannot be silent"));
            }
        }
        "agent" => {
            let input = spec["request"]["input"]
                .as_array()
                .ok_or_else(|| Error::new(400, "Agent task requires input messages"))?;
            if input.len() != 1 || input[0]["role"] != "user" {
                return Err(Error::new(400, "Task requires one user message"));
            }
        }
        _ => return Err(Error::new(400, "Unknown task type")),
    }
    if spec["schedule"]["repeat_every_days"].is_number() {
        return Err(Error::new(
            400,
            "Use a recurring cron schedule for repeated tasks",
        ));
    }
    let next = next_time(&spec["schedule"], now)?;
    if next.is_none() {
        return Err(Error::new(400, "Schedule has no future execution"));
    }
    spec["id"] = json!(id);
    spec["enabled"] = json!(spec["enabled"].as_bool().unwrap_or(true));
    Ok(spec)
}

impl Runtime {
    pub(crate) fn cron_request(
        &self,
        method: &str,
        path: &str,
        body: &Value,
    ) -> Result<Option<Value>> {
        if path == "/api/cron/dispatch-targets" && method == "GET" {
            let items:Vec<_>=self.db()?.chats()?.iter().filter(|c|c["archived"]!=true)
                .map(|c|json!({"channel":"console","user_id":"default","session_id":c["session_id"]})).collect();
            return Ok(Some(json!({"channels":["console"],"items":items})));
        }
        if path != "/api/cron/jobs" && !path.starts_with("/api/cron/jobs/") {
            return Ok(None);
        }
        let db = self.db()?;
        let mut jobs = db.get("cron_jobs", json!({}))?;
        let now = Utc::now();
        if path == "/api/cron/jobs" {
            if method == "GET" {
                return Ok(Some(json!(jobs
                    .as_object()
                    .unwrap()
                    .values()
                    .map(|v| v["spec"].clone())
                    .collect::<Vec<_>>())));
            }
            if method != "POST" {
                return Err(Error::new(405, "Method not allowed"));
            }
            let id = uuid::Uuid::new_v4().to_string();
            let spec = validated(body.clone(), &id, now)?;
            jobs[&id] = json!({"spec":spec,"state":{"next_run_at":if spec["enabled"]==true {next_time(&spec["schedule"],now)?}else{None},"last_run_at":null,"last_status":null,"last_error":null},"history":[]});
            db.put("cron_jobs", &jobs)?;
            return Ok(Some(spec));
        }
        let rest = &path["/api/cron/jobs/".len()..];
        let (id, action) = rest.split_once('/').unwrap_or((rest, ""));
        let job = jobs
            .get_mut(id)
            .ok_or_else(|| Error::new(404, "Scheduled task not found"))?;
        let running = job["state"]["last_status"] == "running";
        let result = match (method, action) {
            ("GET", "") => job["spec"].clone(),
            ("GET", "state") => job["state"].clone(),
            ("GET", "history") => job["history"].clone(),
            ("PUT", "") if !running => {
                let spec = validated(body.clone(), id, now)?;
                job["spec"] = spec.clone();
                job["manual"] = json!(false);
                job["state"]["next_run_at"] = json!(if spec["enabled"] == true {
                    next_time(&spec["schedule"], now)?
                } else {
                    None
                });
                spec
            }
            ("DELETE", "") if !running => {
                jobs.as_object_mut().unwrap().remove(id);
                json!({"deleted":true})
            }
            ("POST", "pause" | "resume") => {
                let enabled = action == "resume";
                job["spec"]["enabled"] = json!(enabled);
                job["state"]["next_run_at"] = json!(if enabled {
                    next_time(&job["spec"]["schedule"], now)?
                } else {
                    None
                });
                json!({"paused":!enabled,"resumed":enabled})
            }
            ("POST", "run") if !running && job["manual"] != true => {
                job["manual"] = json!(true);
                json!({"started":true})
            }
            ("PUT" | "DELETE" | "POST", _) if running => {
                return Err(Error::new(409, "Scheduled task is running"))
            }
            ("POST", "run") => return Err(Error::new(409, "Task execution already queued")),
            _ => return Err(Error::new(405, "Method not allowed")),
        };
        db.put("cron_jobs", &jobs)?;
        Ok(Some(result))
    }

    /// The desktop host owns this future. Dropping it stops polling; no external
    /// daemon is installed and closed/asleep desktops cannot execute jobs.
    pub async fn serve_scheduler(self: Arc<Self>) {
        if let Err(error) = self.recover_jobs() {
            eprintln!("Native scheduler recovery: {error}");
            return;
        }
        let weak = Arc::downgrade(&self);
        drop(self);
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let Some(runtime) = weak.upgrade() else {
                return;
            };
            if let Err(error) = runtime.tick_outbox() {
                eprintln!("Native outbox: {error}");
            }
            if let Err(error) = runtime.tick_shell_followups() {
                eprintln!("Native shell recovery: {error}");
            }
            if let Err(error) = runtime.tick_jobs(Utc::now()).await {
                eprintln!("Native scheduler: {error}");
            }
        }
    }

    fn recover_jobs(&self) -> Result<()> {
        let db = self.db()?;
        let mut jobs = db.get("cron_jobs", json!({}))?;
        for job in jobs.as_object_mut().unwrap().values_mut() {
            if job["state"]["last_status"] == "running" {
                job["state"]["last_status"] = json!("error");
                job["state"]["last_error"] =
                    json!("Desktop closed before task completion; execution was not retried");
                if let Some(record) = job["history"].as_array_mut().and_then(|h| h.last_mut()) {
                    record["status"] = json!("error");
                    record["error"] = json!("Desktop interrupted");
                }
            }
        }
        db.put("cron_jobs", &jobs)
    }

    async fn tick_jobs(self: &Arc<Self>, now: DateTime<Utc>) -> Result<()> {
        let mut claimed = Vec::new();
        {
            let db = self.db()?;
            let mut jobs = db.get("cron_jobs", json!({}))?;
            let mut changed = false;
            for (id, job) in jobs.as_object_mut().unwrap() {
                if job["state"]["last_status"] == "running" {
                    continue;
                }
                let manual = job["manual"] == true;
                let due = timestamp(&job["state"]["next_run_at"]);
                if !(manual || job["spec"]["enabled"] == true && due.is_some_and(|v| v <= now)) {
                    continue;
                }
                changed = true;
                let grace = job["spec"]["runtime"]["misfire_grace_seconds"]
                    .as_i64()
                    .unwrap_or(600)
                    .clamp(0, 86400);
                let skipped = !manual && due.is_some_and(|v| (now - v).num_seconds() > grace);
                job["manual"] = json!(false);
                job["state"]["next_run_at"] = json!(if job["spec"]["enabled"] == true {
                    next_time(&job["spec"]["schedule"], now)?
                } else {
                    None
                });
                job["state"]["last_run_at"] = json!(now);
                job["state"]["last_status"] = json!(if skipped { "skipped" } else { "running" });
                job["state"]["last_error"] = Value::Null;
                let history = job["history"].as_array_mut().unwrap();
                history.push(json!({"run_at":now,"status":if skipped {"skipped"}else{"running"},"error":null,"trigger":if manual {"manual"}else{"scheduled"}}));
                if history.len() > 100 {
                    history.remove(0);
                }
                if !skipped {
                    claimed.push((id.clone(), job["spec"].clone()));
                }
            }
            if changed {
                db.put("cron_jobs", &jobs)?;
            }
        }
        for (id, spec) in claimed {
            let runtime = self.clone();
            tokio::spawn(async move {
                let result = runtime.execute_job(&spec).await;
                if let Err(e) = runtime.finish_job(&id, result) {
                    eprintln!("Native task result: {e}");
                }
                if spec["dispatch"]["silent"] != true {
                    runtime.notify_background(string(&spec["dispatch"]["target"], "session_id"));
                }
            });
        }
        Ok(())
    }

    fn finish_job(&self, id: &str, result: Result<()>) -> Result<()> {
        let db = self.db()?;
        let mut jobs = db.get("cron_jobs", json!({}))?;
        let Some(job) = jobs.get_mut(id) else {
            return Ok(());
        };
        let status = if result.is_ok() { "success" } else { "error" };
        let error = result.err().map(|e| e.message);
        job["state"]["last_status"] = json!(status);
        job["state"]["last_error"] = json!(error);
        if let Some(record) = job["history"].as_array_mut().and_then(|h| h.last_mut()) {
            record["status"] = json!(status);
            record["error"] = json!(error);
        }
        db.put("cron_jobs", &jobs)
    }

    async fn execute_job(self: &Arc<Self>, spec: &Value) -> Result<()> {
        let session = required(&spec["dispatch"]["target"], "session_id")?;
        if spec["task_type"] == "text" {
            let text = required(spec, "text")?;
            let id = uuid::Uuid::new_v4().to_string();
            let mut frame = protocol::message(
                &id,
                "message",
                "assistant",
                json!([protocol::text(&id, text, false)]),
                "completed",
            );
            frame["metadata"] = json!({"cron_job_id":spec["id"]});
            let mut db = self.db()?;
            let chat = db.ensure_chat(session, required(spec, "name")?)?;
            db.append(
                required(&chat, "id")?,
                &frame,
                Some(&json!({"role":"assistant","content":text})),
            )?;
            return Ok(());
        }
        let mut body = spec["request"].clone();
        // Background runs must not hang behind a permission card or question.
        if !body["request_context"].is_object() {
            body["request_context"] = json!({});
        }
        body["request_context"]["approval_level"] = json!("NEVER");
        body["session_id"] = json!(if spec["runtime"]["share_session"] == false {
            format!("cron-{}-{}", string(spec, "id"), uuid::Uuid::new_v4())
        } else {
            session.to_owned()
        });
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.start(
            id.clone(),
            body,
            Arc::new(move |frame| {
                if frame["object"] == "response" && frame["status"] != "in_progress" {
                    let _ = tx.send(frame);
                }
                Ok(())
            }),
        )?;
        if spec["dispatch"]["silent"] != true {
            self.notify_background(session);
        }
        let timeout = spec["runtime"]["timeout_seconds"]
            .as_u64()
            .unwrap_or(120)
            .clamp(1, 3600);
        match tokio::time::timeout(Duration::from_secs(timeout), rx.recv()).await {
            Ok(Some(frame)) if frame["status"] == "completed" => Ok(()),
            Ok(Some(frame)) => Err(Error::new(
                500,
                frame["error"]["message"]
                    .as_str()
                    .unwrap_or("Scheduled turn failed"),
            )),
            _ => {
                self.cancel(&id)?;
                Err(Error::new(408, "Scheduled task timed out"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reminder(run_at: DateTime<Utc>) -> Value {
        json!({"name":"Family reminder","enabled":true,"schedule":{"type":"once","run_at":run_at},
            "task_type":"text","text":"喝水","dispatch":{"type":"channel","channel":"console","target":{"user_id":"default","session_id":"family"}}})
    }

    #[test]
    fn timezone_weekday_and_dst_are_preserved() {
        let after = DateTime::parse_from_rfc3339("2026-09-06T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let monday = next_time(
            &json!({"type":"cron","cron":"0 9 * * 1","timezone":"Asia/Shanghai"}),
            after,
        )
        .unwrap()
        .unwrap();
        assert_eq!(monday.to_rfc3339(), "2026-09-07T01:00:00+00:00");
        let sunday = next_time(
            &json!({"type":"cron","cron":"0 9 * * 0","timezone":"Asia/Shanghai"}),
            after,
        )
        .unwrap()
        .unwrap();
        assert_eq!(sunday.to_rfc3339(), "2026-09-06T01:00:00+00:00");
        let before = DateTime::parse_from_rfc3339("2026-03-07T15:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next = next_time(
            &json!({"type":"cron","cron":"0 9 * * *","timezone":"America/New_York"}),
            before,
        )
        .unwrap()
        .unwrap();
        assert_eq!(next.to_rfc3339(), "2026-03-08T13:00:00+00:00");
    }

    #[test]
    fn numeric_weekday_ranges_preserve_sunday_and_steps() {
        let after = DateTime::parse_from_rfc3339("2026-09-06T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        for (expression, expected) in [
            ("0 9 * * 1-7", "2026-09-06T09:00:00+00:00"),
            ("0 9 * * 5-7", "2026-09-06T09:00:00+00:00"),
            ("0 9 * * 1-7/2", "2026-09-06T09:00:00+00:00"),
            ("0 9 * * 2-7/2", "2026-09-08T09:00:00+00:00"),
            ("0 9 * * 1-5", "2026-09-07T09:00:00+00:00"),
            ("0 9 * * 0-6", "2026-09-06T09:00:00+00:00"),
        ] {
            let next = next_time(
                &json!({"type":"cron","cron":expression,"timezone":"UTC"}),
                after,
            )
            .unwrap()
            .unwrap();
            assert_eq!(next.to_rfc3339(), expected, "{expression}");
        }
        for expression in ["0 9 * * 1-7/0", "0 9 * * 1-8", "0 9 * * 7-1"] {
            assert!(cron_schedule(&json!({"cron":expression})).is_err());
        }
    }

    #[tokio::test]
    async fn reminders_execute_once_and_survive_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(tmp.path()).unwrap();
        let due = Utc::now() + chrono::Duration::minutes(1);
        let spec = runtime
            .request("POST", "/api/cron/jobs", reminder(due))
            .await
            .unwrap();
        let id = required(&spec, "id").unwrap();
        runtime
            .tick_jobs(due + chrono::Duration::seconds(1))
            .await
            .unwrap();
        for _ in 0..30 {
            tokio::task::yield_now().await;
            if runtime
                .request("GET", &format!("/api/cron/jobs/{id}/state"), Value::Null)
                .await
                .unwrap()["last_status"]
                == "success"
            {
                break;
            }
        }
        assert_eq!(
            runtime
                .request("GET", &format!("/api/cron/jobs/{id}/state"), Value::Null)
                .await
                .unwrap()["last_status"],
            "success"
        );
        runtime
            .tick_jobs(due + chrono::Duration::seconds(2))
            .await
            .unwrap();
        drop(runtime);
        let runtime = Runtime::open(tmp.path()).unwrap();
        runtime.recover_jobs().unwrap();
        runtime
            .tick_jobs(due + chrono::Duration::hours(1))
            .await
            .unwrap();
        let chats = runtime
            .request("GET", "/api/chats", Value::Null)
            .await
            .unwrap();
        let chat_id = chats[0]["id"].as_str().unwrap();
        let history = runtime
            .request("GET", &format!("/api/chats/{chat_id}"), Value::Null)
            .await
            .unwrap();
        assert_eq!(history["messages"].as_array().unwrap().len(), 1);
        assert_eq!(history["messages"][0]["content"][0]["text"], "喝水");
        assert_eq!(
            runtime
                .request("GET", &format!("/api/cron/jobs/{id}/history"), Value::Null)
                .await
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn pause_manual_run_and_misfire_are_distinct() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(tmp.path()).unwrap();
        let due = Utc::now() + chrono::Duration::minutes(1);
        let spec = runtime
            .request("POST", "/api/cron/jobs", reminder(due))
            .await
            .unwrap();
        let path = format!("/api/cron/jobs/{}", required(&spec, "id").unwrap());
        runtime
            .request("POST", &format!("{path}/pause"), Value::Null)
            .await
            .unwrap();
        runtime.tick_jobs(due).await.unwrap();
        assert_eq!(
            runtime
                .request("GET", &format!("{path}/history"), Value::Null)
                .await
                .unwrap(),
            json!([])
        );
        runtime
            .request("POST", &format!("{path}/run"), Value::Null)
            .await
            .unwrap();
        assert_eq!(
            runtime
                .request("POST", &format!("{path}/run"), Value::Null)
                .await
                .unwrap_err()
                .status,
            409
        );
        runtime.tick_jobs(due).await.unwrap();
        for _ in 0..30 {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            runtime
                .request("GET", &format!("{path}/history"), Value::Null)
                .await
                .unwrap()[0]["trigger"],
            "manual"
        );
        let late = runtime
            .request("POST", "/api/cron/jobs", reminder(due))
            .await
            .unwrap();
        runtime
            .tick_jobs(due + chrono::Duration::hours(1))
            .await
            .unwrap();
        assert_eq!(
            runtime
                .request(
                    "GET",
                    &format!("/api/cron/jobs/{}/state", required(&late, "id").unwrap()),
                    Value::Null
                )
                .await
                .unwrap()["last_status"],
            "skipped"
        );
    }
}
