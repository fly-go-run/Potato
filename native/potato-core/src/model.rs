use crate::{
    api::{validate_protocol, validate_url},
    protocol, required, string, Emit, Error, Result, Runtime,
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(crate) struct Connection {
    pub cache_key: String,
    pub url: String,
    pub key: String,
    pub model: String,
    pub responses: bool,
    pub options: Value,
}
#[derive(Default)]
pub(crate) struct Completion {
    pub(crate) text: String,
    pub(crate) reasoning: String,
    pub(crate) response_output: BTreeMap<usize, Value>,
    pub(crate) calls: BTreeMap<usize, Value>,
    pub(crate) finished: bool,
    pub(crate) usage: Option<Value>,
    // Public commentary/final items keep separate UI identities. The model wire
    // still contains the complete, ordered Responses output.
    phased_messages: BTreeMap<usize, Value>,
    unphased_text: String,
    reasoning_clock: Option<protocol::ActivityClock>,
    reasoning_finished: Option<Value>,
}

impl Runtime {
    pub(crate) fn connection(&self) -> Result<Connection> {
        let active = self.ensure_model_selection()?;
        self.provider_connection(string(&active, "provider_id"), string(&active, "model"))
    }
    pub(crate) fn provider_connection(&self, id: &str, model: &str) -> Result<Connection> {
        if id == crate::cloud::PROVIDER { return self.cloud_connection(model); }
        if model.trim().is_empty() {
            return Err(Error::new(400, "Select a model in Settings first"));
        }
        let provider = self
            .providers()?
            .into_iter()
            .find(|p| p["id"] == id)
            .ok_or_else(|| Error::new(400, "Configure a provider in Settings first"))?;
        let url = required(&provider, "base_url")?
            .trim_end_matches('/')
            .to_owned();
        validate_url(&url)?;
        validate_protocol(string(&provider, "chat_model"))?;
        let key = match self.model_env_key_at(None, id, model) {
            Some(key) => key,
            None => self.db()?.unseal(string(&provider, "api_key"))?,
        };
        if key.trim().is_empty() || key == "********" {
            return Err(Error::new(400, "Configure the provider API key first"));
        }
        let mut options = ["extra_models", "models"].iter()
            .flat_map(|key| provider[*key].as_array().into_iter().flatten())
            .find(|m| m["id"] == model).cloned().unwrap_or(json!({}));
        options["reasoning_effort"] = crate::reasoning::effective_effort(&provider, &options).map_or(Value::Null, |v| json!(v));
        Ok(Connection {
            cache_key: String::new(),
            options,
            url,
            key,
            model: model.to_owned(),
            responses: provider["chat_model"] == "OpenAIResponseModel",
        })
    }

    // Explicit turn identity and cancellation inputs keep the protocol boundary visible.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_turn(
        &self,
        chat: &str,
        session: &str,
        request: &str,
        body: &Value,
        mut connection: Connection,
        cancel: &CancellationToken,
        emit: &Emit,
    ) -> Result<()> {
        emit(protocol::response(request, session, "in_progress"))?;
        connection.cache_key = format!("potato:{chat}");
        let project = self.turn_project(body).await?;
        let history_guidance = self.db()?.bind_project(chat, &project)?;
        let system = format!(
            "{}\n{}\n{}\n{}\n{}",
            self.system_prompt()?,
            self.skill_instructions()?,
            crate::memory::GUIDANCE,
            crate::approval::GUIDANCE,
            crate::prompts::SCHEDULING
        );
        let memory_snapshot = self.memory_guidance(&project)?;
        let memory_fingerprint = json!(crate::context::fingerprint(&memory_snapshot));
        let memory_key = format!("memory_context_fingerprint:{chat}");
        let memory_changed = self.db()?.get(&memory_key, Value::Null)? != memory_fingerprint;
        let runtime_context = format!(
            "<runtime_context>\n{}\nConversation project directory: {}\n{}\n{}\n{}\n</runtime_context>",
            crate::prompts::time_context(), project.display(), history_guidance, if memory_changed { memory_snapshot.as_str() } else { "Memory locations and index previews unchanged; retrieve current files on demand." }, self.approval_guidance(body)?);
        self.db()?.attach_runtime_context(chat, &runtime_context)?;
        self.db()?.put(&memory_key, &memory_fingerprint)?;
        let media = self.db()?.get("media", json!({}))?;
        let image_ready = self.db()?.get("image_plugin_installed", json!(true))? == true
            && self
                .provider_connection(
                    string(&media, "image_provider_id"),
                    string(&media, "image_model"),
                )
                .is_ok();
        let mut definitions = crate::tools::definitions(image_ready);
        definitions.extend(self.mcp_definitions()?);
        if self.db()?.get("computer_enabled", json!(false))? == true {
            definitions.extend(crate::computer::definitions());
        }
        definitions
            .sort_by(|a, b| string(&a["function"], "name").cmp(string(&b["function"], "name")));
        let max_iters = self.db()?.get("running", json!({}))?["max_iters"]
            .as_u64()
            .unwrap_or(100)
            .clamp(1, 1000);
        for _ in 0..max_iters {
            if cancel.is_cancelled() {
                return Err(Error::new(499, "Turn cancelled"));
            }
            self.deliver_steering(chat, emit)?;
            let mut prepared = self
                .context_messages(chat, &connection, &system, &definitions, false, cancel)
                .await?;
            let mut messages = prepared.messages.clone();
            let id = uuid::Uuid::new_v4().to_string();
            let reasoning_id = uuid::Uuid::new_v4().to_string();
            let clock = protocol::ActivityClock::default();
            let mut initial = protocol::message(
                &id,
                "message",
                "assistant",
                json!([]),
                "in_progress",
            );
            initial["metadata"] = clock.metadata("running");
            emit(initial)?;
            let mut completion = Completion::default();
            let mut result = tokio::select! {
                _=cancel.cancelled()=>Err(Error::new(499,"Turn cancelled")),
                result=self.complete(&connection,&messages,&definitions,&id,&reasoning_id,&mut completion,emit)=>result,
            };
            if result.as_ref().is_err_and(|e| e.status == 413)
                && completion.text.is_empty()
                && completion.calls.is_empty()
                && completion.reasoning.is_empty()
            {
                let smaller = self
                    .context_messages(chat, &connection, &system, &definitions, true, cancel)
                    .await?;
                if smaller.changed {
                    prepared = smaller;
                    messages = prepared.messages.clone();
                    completion = Completion::default();
                    result = tokio::select! {
                        _=cancel.cancelled()=>Err(Error::new(499,"Turn cancelled")),
                        result=self.complete(&connection,&messages,&definitions,&id,&reasoning_id,&mut completion,emit)=>result,
                    };
                }
            }
            if let Some(usage) = &completion.usage {
                let mut db = self.db()?;
                let mut totals = db.get(&format!("usage_totals:{chat}"), json!({}))?;
                for field in ["input_tokens", "output_tokens", "cached_input_tokens"] {
                    if let Some(n) = usage[field].as_u64() {
                        totals[field] =
                            json!(totals[field].as_u64().unwrap_or(0).saturating_add(n));
                    }
                }
                totals["reported_requests"] =
                    json!(totals["reported_requests"].as_u64().unwrap_or(0) + 1);
                if let (Some(input), Some(_)) = (
                    usage["input_tokens"].as_u64(),
                    usage["cached_input_tokens"].as_u64(),
                ) {
                    totals["cache_observed_input_tokens"] =
                        json!(totals["cache_observed_input_tokens"].as_u64().unwrap_or(0) + input);
                    totals["cache_observed_requests"] =
                        json!(totals["cache_observed_requests"].as_u64().unwrap_or(0) + 1);
                }
                totals["scope"] = json!("conversation model requests; summaries excluded");
                db.put_batch(&[
                    (format!("usage:{chat}"), usage.clone()),
                    (format!("usage_totals:{chat}"), totals),
                ])?;
                if result.is_ok() && usage["input_tokens"].is_u64() {
                    db.put(&format!("usage_anchor:{chat}"),&json!({"identity":crate::context::fingerprint(&(&connection.url,&connection.model,connection.responses)),"messages":messages.len(),"prefix":crate::context::fingerprint(&messages),"tools":crate::context::fingerprint(&definitions),"input_tokens":usage["input_tokens"]}))?;
                }
            }
            if result.is_ok() {
                self.db()?
                    .put(&format!("context_consumed:{chat}"), &prepared.consumed)?;
            }
            let status = if cancel.is_cancelled() {
                "cancelled"
            } else if result.is_err() {
                "failed"
            } else {
                "completed"
            };
            let mut frame = protocol::message(
                &id,
                "message",
                "assistant",
                json!([protocol::text(&id, if completion.phased_messages.is_empty() { &completion.text } else { &completion.unphased_text }, false)]),
                status,
            );
            frame["metadata"] = clock.metadata(status);
            let mut wire = json!({"role":"assistant","content":completion.text});
            if result.is_ok() && !completion.response_output.is_empty() {
                // BTreeMap merges all output kinds by their provider output_index.
                wire["_responses_output"] =
                    json!(completion.response_output.values().collect::<Vec<_>>());
            }
            if !completion.reasoning.is_empty() {
                wire["reasoning_content"] = json!(completion.reasoning);
                let mut reasoning = protocol::message(
                    &reasoning_id,
                    "reasoning",
                    "assistant",
                    json!([protocol::text(&reasoning_id, &completion.reasoning, false)]),
                    status,
                );
                reasoning["metadata"] = completion.reasoning_finished.clone()
                    .or_else(|| completion.reasoning_clock.as_ref().map(|c| c.metadata(status)))
                    .unwrap_or(Value::Null);
                self.db()?.append(chat, &reasoning, None)?;
                emit(reasoning)?;
            }
            let calls: Vec<Value> = completion.calls.into_values().collect();
            if result.is_ok() && !calls.is_empty() {
                wire["tool_calls"] = json!(calls);
            }
            self.db()?.append(chat, &frame, Some(&wire))?;
            emit(frame)?;
            for mut message in completion.phased_messages.into_values() {
                message["status"] = json!(status);
                message["metadata"] = clock.metadata(status);
                self.db()?.append(chat, &message, None)?;
                emit(message)?;
            }
            result?;

            if calls.is_empty() {
                if self.finish_unless_steered(chat, session)? {
                    return Ok(());
                }
                continue;
            }
            self.execute_calls(chat, session, &calls, &definitions, body, cancel, emit)
                .await?;
        }
        Err(Error::new(
            422,
            format!("Tool turn limit reached ({max_iters} model calls)"),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn complete(
        &self,
        connection: &Connection,
        messages: &[Value],
        tools: &[Value],
        id: &str,
        reasoning_id: &str,
        completion: &mut Completion,
        emit: &Emit,
    ) -> Result<()> {
        let mut payload = if connection.responses {
            json!({"model":connection.model,"input":responses_input(messages),"stream":true,"store":false,"include":["reasoning.encrypted_content"],
                "tools":tools.iter().map(|t|{let mut f=t["function"].clone();f["type"]=json!("function");f["strict"]=json!(false);f}).collect::<Vec<_>>()})
        } else {
            let messages: Vec<Value> = messages
                .iter()
                .map(|m| {
                    let mut m = m.clone();
                    strip_response_metadata(&mut m);
                    m
                })
                .collect();
            json!({"model":connection.model,"messages":messages,"stream":true,"tools":tools})
        };
        if connection.responses && !connection.cache_key.is_empty() {
            payload["prompt_cache_key"] = json!(connection.cache_key);
        }
        if !connection.responses {
            payload["stream_options"] = json!({"include_usage":true});
        }
        {
            let max = connection.options["max_tokens"]
                .as_u64()
                .filter(|n| *n > 0)
                .unwrap_or(4096);
            payload[if connection.responses {
                "max_output_tokens"
            } else {
                "max_tokens"
            }] = json!(max);
        }
        if let Some(effort) = connection.options["reasoning_effort"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            if connection.responses {
                payload["reasoning"] = json!({"effort":effort});
            } else {
                payload["reasoning_effort"] = json!(effort);
            }
        }
        let endpoint = if connection.responses {
            "responses"
        } else {
            "chat/completions"
        };
        let response = self
            .client
            .post(format!("{}/{endpoint}", connection.url))
            .bearer_auth(&connection.key)
            .json(&payload)
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            // Inspect a bounded error body only to classify overflow. Never echo
            // provider text, which may contain credentials or prompt contents.
            let mut body = Vec::new();
            let mut chunks = response.bytes_stream();
            while let Some(chunk) = chunks.next().await {
                let chunk = chunk?;
                let left = 16_384usize.saturating_sub(body.len());
                body.extend_from_slice(&chunk[..chunk.len().min(left)]);
                if body.len() >= 16_384 {
                    break;
                }
            }
            let detail = String::from_utf8_lossy(&body).to_lowercase();
            let overflow = matches!(status, 400 | 413 | 422)
                && [
                    "context_length_exceeded",
                    "context_window_exceeded",
                    "maximum context length",
                    "exceeds the context",
                    "context window",
                ]
                .iter()
                .any(|s| detail.contains(s));
            return Err(Error::new(
                if overflow { 413 } else { 502 },
                format!(
                    "Model returned HTTP {status}{}",
                    if overflow { " (context overflow)" } else { "" }
                ),
            ));
        }
        let mut stream = response.bytes_stream();
        let mut decoder = protocol::SseDecoder::default();
        let mut received_bytes = 0u64;
        while let Some(bytes) = stream.next().await {
            let bytes = bytes?;
            received_bytes = received_bytes.saturating_add(bytes.len() as u64);
            if connection.options["response_byte_limit"].as_u64().is_some_and(|limit|received_bytes>limit) {
                return Err(Error::new(502,"Model response exceeded its byte limit"));
            }
            for event in decoder.push(&bytes)? {
                if event == "[DONE]" {
                    if !completion.finished {
                        return Err(Error::new(
                            502,
                            "Model stream ended without a finish reason",
                        ));
                    }
                    return Ok(());
                }
                let value: Value = serde_json::from_str(&event)
                    .map_err(|_| Error::new(502, "Invalid model stream JSON"))?;
                if let Some(usage) = value
                    .get("usage")
                    .or_else(|| value["response"].get("usage"))
                    .filter(|u| u.is_object())
                {
                    completion.usage = Some(normalize_usage(usage));
                }
                if value.get("error").is_some()
                    || matches!(
                        string(&value, "type"),
                        "error" | "response.failed" | "response.incomplete"
                    )
                {
                    return Err(Error::new(
                        502,
                        "Model reported a failed or incomplete response",
                    ));
                }
                if connection.responses {
                    match string(&value, "type") {
                        "response.output_item.added" => {
                            let item = &value["item"];
                            if item["type"] == "message" {
                                if let Some(phase) = item["phase"].as_str() {
                                    let index = value["output_index"].as_u64().unwrap_or(0) as usize;
                                    let message_id = format!("{id}-output-{index}");
                                    let mut frame = protocol::message(&message_id, "message", "assistant", json!([]), "in_progress");
                                    frame["phase"] = json!(phase);
                                    completion.phased_messages.insert(index, frame.clone());
                                    emit(frame)?;
                                }
                            }
                        }
                        "response.output_text.delta" => {
                            // Keep text positions even for compatible providers that
                            // omit output_item.done for messages.
                            let index = value["output_index"].as_u64().unwrap_or(0) as usize;
                            let item = completion.response_output.entry(index).or_insert_with(||
                                json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":"","annotations":[]}]}));
                            if let Some(item_id) = value["item_id"].as_str() {
                                item["id"] = json!(item_id);
                            }
                            let text = format!(
                                "{}{}",
                                string(&item["content"][0], "text"),
                                string(&value, "delta")
                            );
                            item["content"][0]["text"] = json!(text);
                            let delta_id = if let Some(frame) = completion.phased_messages.get_mut(&index) {
                                let message_id = string(frame, "id").to_owned();
                                frame["content"] = json!([protocol::text(&message_id, &text, false)]);
                                message_id
                            } else {
                                completion.unphased_text.push_str(string(&value, "delta"));
                                id.to_owned()
                            };
                            append_text(
                                completion,
                                string(&value, "delta"),
                                false,
                                &delta_id,
                                reasoning_id,
                                emit,
                            )?;
                        }
                        "response.reasoning_summary_text.delta" => append_text(
                            completion,
                            string(&value, "delta"),
                            true,
                            id,
                            reasoning_id,
                            emit,
                        )?,
                        "response.output_item.done" => {
                            let item = &value["item"];
                            let index = value["output_index"].as_u64().unwrap_or(0) as usize;
                            if let Some(frame) = completion.phased_messages.get_mut(&index) {
                                frame["status"] = json!("completed");
                                emit(frame.clone())?;
                            }
                            if matches!(
                                string(item, "type"),
                                "reasoning" | "message" | "function_call"
                            ) {
                                completion.response_output.insert(index, item.clone());
                            }
                            if item["type"] == "function_call" {
                                completion.calls.insert(index,
                                    json!({"id":item["call_id"],"type":"function","function":{"name":item["name"],"arguments":item["arguments"]}}));
                            }
                        }
                        "response.completed" => {
                            completion.finished = true;
                            return Ok(());
                        }
                        _ => {}
                    }
                } else if let Some(choice) = value["choices"].as_array().and_then(|c| c.first()) {
                    let delta = &choice["delta"];
                    append_text(
                        completion,
                        string(delta, "content"),
                        false,
                        id,
                        reasoning_id,
                        emit,
                    )?;
                    append_text(
                        completion,
                        string(delta, "reasoning_content"),
                        true,
                        id,
                        reasoning_id,
                        emit,
                    )?;
                    if let Some(calls) = delta["tool_calls"].as_array() {
                        for c in calls {
                            let index = c["index"].as_u64().unwrap_or(0) as usize;
                            if index > 64 {
                                return Err(Error::new(502, "Too many tool calls"));
                            }
                            let call=completion.calls.entry(index).or_insert_with(||json!({"id":"","type":"function","function":{"name":"","arguments":""}}));
                            if let Some(id) = c["id"].as_str() {
                                call["id"] = json!(id);
                            }
                            for key in ["name", "arguments"] {
                                let merged = format!(
                                    "{}{}",
                                    string(&call["function"], key),
                                    string(&c["function"], key)
                                );
                                if merged.len() > 1_000_000 {
                                    return Err(Error::new(502, "Tool arguments too large"));
                                }
                                call["function"][key] = json!(merged);
                            }
                        }
                    }
                    if let Some(reason) = choice["finish_reason"].as_str() {
                        if !matches!(reason, "stop" | "tool_calls") {
                            return Err(Error::new(502, format!("Model stopped early: {reason}")));
                        }
                        completion.finished = true;
                    }
                }
            }
        }
        if completion.finished {
            Ok(())
        } else {
            Err(Error::new(
                502,
                "Model stream disconnected before completion",
            ))
        }
    }
}

fn append_text(
    completion: &mut Completion,
    text: &str,
    reasoning: bool,
    id: &str,
    reasoning_id: &str,
    emit: &Emit,
) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    if !reasoning && !completion.reasoning.is_empty() && completion.reasoning_finished.is_none() {
        let metadata = completion.reasoning_clock.as_ref().map(|c| c.metadata("completed")).unwrap_or(Value::Null);
        let mut frame = protocol::message(reasoning_id, "reasoning", "assistant",
            json!([protocol::text(reasoning_id, &completion.reasoning, false)]), "completed");
        frame["metadata"] = metadata.clone();
        completion.reasoning_finished = Some(metadata);
        emit(frame)?;
    }
    if reasoning && completion.reasoning_clock.is_none() {
        completion.reasoning_clock = Some(protocol::ActivityClock::default());
    }
    let restarting = reasoning && completion.reasoning_finished.take().is_some();
    let target = if reasoning {
        &mut completion.reasoning
    } else {
        &mut completion.text
    };
    if target.len() + text.len() > 4_000_000 {
        return Err(Error::new(502, "Model output too large"));
    }
    if reasoning && (target.is_empty() || restarting) {
        let mut frame = protocol::message(
            reasoning_id,
            "reasoning",
            "assistant",
            json!([]),
            "in_progress",
        );
        frame["metadata"] = completion.reasoning_clock.as_ref().map(|c| c.metadata("running")).unwrap_or(Value::Null);
        emit(frame)?;
    }
    target.push_str(text);
    emit(protocol::text(
        if reasoning { reasoning_id } else { id },
        text,
        true,
    ))
}

pub(crate) fn strip_response_metadata(message: &mut Value) {
    if let Some(object) = message.as_object_mut() {
        object.remove("_responses_reasoning");
        object.remove("_responses_output");
    }
}

fn responses_input(messages: &[Value]) -> Vec<Value> {
    let mut input = Vec::new();
    for m in messages {
        if m["role"] == "assistant" {
            if let Some(items) = m["_responses_output"].as_array() {
                input.extend(items.iter().cloned());
                continue;
            }
            // Old persisted wires lost output positions. Only the single-output
            // case has an unambiguous reasoning association.
            if let Some(items) = m["_responses_reasoning"].as_array() {
                let outputs = usize::from(m["content"].as_str().is_some_and(|s| !s.is_empty()))
                    + m["tool_calls"].as_array().map_or(0, Vec::len);
                if items.len() == 1 && outputs == 1 && items[0]["type"] == "reasoning" {
                    input.push(items[0].clone());
                }
            }
        }
        if m["role"] == "tool" {
            input.push(json!({"type":"function_call_output","call_id":m["tool_call_id"],"output":m["content"]}));
            continue;
        }
        let content = if let Some(blocks) = m["content"].as_array() {
            json!(blocks
                .iter()
                .map(|b| if b["type"] == "image_url" {
                    json!({"type":"input_image","image_url":b["image_url"]["url"]})
                } else {
                    json!({"type":"input_text","text":b["text"]})
                })
                .collect::<Vec<_>>())
        } else {
            m["content"].clone()
        };
        if content.as_str() != Some("") {
            input.push(json!({"role":m["role"],"content":content}));
        }
        if let Some(calls) = m["tool_calls"].as_array() {
            for c in calls {
                input.push(json!({"type":"function_call","call_id":c["id"],"name":c["function"]["name"],"arguments":c["function"]["arguments"]}));
            }
        }
    }
    input
}

fn normalize_usage(usage: &Value) -> Value {
    json!({
        "input_tokens":usage.get("input_tokens").or_else(||usage.get("prompt_tokens")),
        "output_tokens":usage.get("output_tokens").or_else(||usage.get("completion_tokens")),
        "cached_input_tokens":usage["input_tokens_details"].get("cached_tokens")
            .or_else(||usage["prompt_tokens_details"].get("cached_tokens"))
            .or_else(||usage.get("prompt_cache_hit_tokens")),
        "source":"provider", "scope":"last_model_request"
    })
}

#[cfg(test)]
mod replay_tests {
    use super::*;

    #[test]
    fn reasoning_finishes_on_first_body_delta_and_can_resume() {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = events.clone();
        let emit: Emit = std::sync::Arc::new(move |v| { sink.lock().unwrap().push(v); Ok(()) });
        let mut completion = Completion::default();
        append_text(&mut completion, "summary", true, "m", "r", &emit).unwrap();
        append_text(&mut completion, "answer", false, "m", "r", &emit).unwrap();
        append_text(&mut completion, " continues", false, "m", "r", &emit).unwrap();
        {
            let events = events.lock().unwrap();
            let completed: Vec<_> = events.iter().filter(|e| e["id"] == "r" && e["status"] == "completed").collect();
            assert_eq!(completed.len(), 1);
            assert_eq!(completed[0]["metadata"]["activity"]["state"], "completed");
            assert!(completed[0]["metadata"]["activity"]["elapsed_ms"].is_u64());
        }
        append_text(&mut completion, " resumed", true, "m", "r", &emit).unwrap();
        assert!(completion.reasoning_finished.is_none());
        let events = events.lock().unwrap();
        assert_eq!(events.iter().filter(|e| e["id"] == "r" && e["status"] == "in_progress").count(), 2);
    }

    #[tokio::test]
    async fn every_turn_keeps_history_locations_when_memory_preview_is_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(temp.path()).unwrap();
        let chat = runtime
            .db()
            .unwrap()
            .ensure_chat("history-context", "test")
            .unwrap();
        let id = required(&chat, "id").unwrap();
        let project = runtime.workspace_dir().await.unwrap();
        let guidance = runtime.db().unwrap().bind_project(id, &project).unwrap();
        assert!(!guidance.is_empty());
        let cancel = CancellationToken::new();
        cancel.cancel(); // Exercise context assembly without contacting a provider.
        let emit: Emit = std::sync::Arc::new(|_| Ok(()));
        let mut first = Vec::new();
        for turn in 0..2 {
            runtime.db().unwrap().append(id, &json!({"id":format!("user-{turn}"),"type":"message","role":"user","content":[]}), Some(&json!({"role":"user","content":format!("turn {turn}")}))).unwrap();
            let result = runtime
                .run_turn(
                    id,
                    "history-context",
                    "request",
                    &json!({}),
                    Connection {
                        cache_key: String::new(),
                        url: String::new(),
                        key: String::new(),
                        model: "test".into(),
                        responses: false,
                        options: json!({}),
                    },
                    &cancel,
                    &emit,
                )
                .await;
            assert_eq!(result.unwrap_err().status, 499);
            let history = runtime.db().unwrap().history(id, true).unwrap();
            let latest = history.last().unwrap().to_string();
            assert!(latest.contains(&guidance.replace('\n', "\\n")));
            if turn == 0 {
                assert!(!latest.contains("Memory locations and index previews unchanged"));
                first = history;
            } else {
                assert!(latest.contains("Memory locations and index previews unchanged"));
                assert_eq!(&history[..first.len()], first.as_slice());
            }
        }
    }

    #[test]
    fn legacy_replay_requires_an_unambiguous_reasoning_successor() {
        let reasoning = json!({"type":"reasoning","id":"rs_old","encrypted_content":"old"});
        let call = json!({"id":"a","function":{"name":"probe","arguments":"{}"}});
        let mut wire = json!({"role":"assistant","content":"","tool_calls":[call],"_responses_reasoning":[reasoning]});
        assert_eq!(responses_input(&[wire.clone()])[0], reasoning);
        wire["content"] = json!("text whose original position is unknown");
        assert!(responses_input(&[wire.clone()])
            .iter()
            .all(|i| i["type"] != "reasoning"));
        wire["content"] = json!("");
        wire["tool_calls"] = json!([]);
        assert!(responses_input(&[wire]).is_empty());
    }

    // Explicit opt-in: sends only synthetic probe data to the configured service.
    // Credentials are imported into a temporary encrypted store, never printed.
    #[tokio::test]
    #[ignore = "requires POTATO_LIVE_LEGACY_DIR, POTATO_LIVE_SECRET_DIR and POTATO_LIVE_MODEL"]
    async fn live_responses_replay() {
        let working = std::env::var("POTATO_LIVE_LEGACY_DIR").unwrap();
        let secret = std::env::var("POTATO_LIVE_SECRET_DIR").unwrap();
        let model = std::env::var("POTATO_LIVE_MODEL").unwrap();
        let root = tempfile::tempdir().unwrap();
        let runtime = Runtime::open(root.path()).unwrap();
        runtime
            .import_legacy_settings(
                std::path::Path::new(&working),
                std::path::Path::new(&secret),
            )
            .unwrap();
        let mut connection = runtime.provider_connection("sub2api", &model).unwrap();
        connection.responses = true;
        connection.options = json!({"max_tokens":2048,"reasoning_effort":"medium"});
        let definitions = vec![
            json!({"type":"function","function":{"name":"probe","description":"Returns a synthetic protocol validation token.","parameters":{"type":"object","properties":{},"additionalProperties":false}}}),
        ];
        let mut messages = vec![
            json!({"role":"user","content":"This is a protocol validation test. Call probe exactly once now. After receiving its result, call probe exactly once again. After the second result, reply with VERIFIED. Do not skip either call."}),
        ];
        let emit: Emit = std::sync::Arc::new(|_| Ok(()));
        let mut reasoning_items = 0;
        for step in 0..3 {
            let mut completion = Completion::default();
            tokio::time::timeout(
                std::time::Duration::from_secs(60),
                runtime.complete(
                    &connection,
                    &messages,
                    &definitions,
                    "probe-text",
                    "probe-reasoning",
                    &mut completion,
                    &emit,
                ),
            )
            .await
            .expect("live Responses request timed out")
            .expect("live Responses request failed");
            reasoning_items += completion
                .response_output
                .values()
                .filter(|v| {
                    v["type"] == "reasoning"
                        && v["encrypted_content"]
                            .as_str()
                            .is_some_and(|s| !s.is_empty())
                })
                .count();
            let calls: Vec<_> = completion.calls.into_values().collect();
            if step < 2 {
                assert_eq!(calls.len(), 1, "expected one synthetic tool call per step");
                assert_eq!(calls[0]["function"]["name"], "probe");
            } else {
                assert!(calls.is_empty(), "expected a final message");
                assert!(completion.text.contains("VERIFIED"));
            }
            let mut wire = json!({"role":"assistant","content":completion.text,"_responses_output":completion.response_output.into_values().collect::<Vec<_>>()});
            if !calls.is_empty() {
                wire["tool_calls"] = json!(calls);
            }
            messages.push(wire);
            for call in calls {
                messages.push(json!({"role":"tool","tool_call_id":call["id"],"content":format!("Synthetic result {}", step + 1)}));
            }
            messages = crate::context::repair(&messages);
        }
        assert!(
            reasoning_items > 0,
            "service did not return encrypted reasoning; replay was not validated"
        );
        println!("Live Responses: 3 requests, 2 synthetic tool calls, {reasoning_items} encrypted reasoning items; replay accepted.");
    }
}
