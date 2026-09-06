//! Context IO: load immutable evidence, execute a pure plan, then commit once.
use crate::{
    context::{self, Budget, Checkpoint, Notice},
    context_policy::{self, Policy},
    model::{Completion, Connection},
    Emit, Error, Result, Runtime,
};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

pub(crate) struct PreparedContext {
    pub messages: Vec<Value>,
    pub consumed: Value,
    pub changed: bool,
}

impl Runtime {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn context_messages(
        &self,
        chat: &str,
        connection: &Connection,
        system: &str,
        tools: &[Value],
        force: bool,
        cancel: &CancellationToken,
    ) -> Result<PreparedContext> {
        let history = self.db()?.history(chat, true)?;
        let key = format!("context_summary:{chat}");
        let mut saved: Checkpoint =
            serde_json::from_value(self.db()?.get(&key, json!({"covered":0,"summary":""}))?)
                .unwrap_or_default();
        if saved.covered > history.len() {
            saved = Checkpoint::default();
        }
        let running = self.db()?.get("running", json!({}))?;
        let policy: Policy =
            serde_json::from_value(running.get("context_policy").cloned().unwrap_or(json!({})))?;
        policy.validate()?;
        let budget = policy.budget(&connection.options);
        let anchor = self
            .db()?
            .get(&format!("usage_anchor:{chat}"), Value::Null)?;
        let consumed = self
            .db()?
            .get(&format!("context_consumed:{chat}"), Value::Null)?;
        let through = consumed["through"].as_u64().unwrap_or(0) as usize;
        let through = if through <= history.len()
            && consumed["prefix"].as_u64() == Some(context::fingerprint(&&history[..through]))
        {
            through
        } else {
            0
        };
        let identity =
            context::fingerprint(&(&connection.url, &connection.model, connection.responses));
        let before = context::measurement(
            &context::project(&history, &saved, system),
            tools,
            &anchor,
            identity,
        );
        let plan = context_policy::plan(
            &context_policy::Input {
                history: &history,
                checkpoint: &saved,
                system,
                tools,
                anchor: &anchor,
                identity,
                consumed: through,
                budget,
                force,
            },
            &policy,
        );
        let mut checkpoint = plan.checkpoint;
        if let Some(end) = plan.summary_end {
            match self
                .summarize_context(connection, &history, &saved, end, &policy, cancel)
                .await
            {
                Ok(summary) => {
                    checkpoint.summary = summary;
                    checkpoint.covered = end;
                    checkpoint.folded.retain(|i| *i >= end);
                    checkpoint.notices.retain(|n| n.at >= end);
                }
                Err(error) if error.status != 499 && !force && before.tokens <= budget.hard => {
                    // A failed summary cannot commit any of its proposed evictions.
                    checkpoint = saved.clone();
                }
                Err(error) => return Err(error),
            }
        }
        let changed = checkpoint.covered != saved.covered || checkpoint.folded != saved.folded;
        let mut messages = context::project(&history, &checkpoint, system);
        let measured = context::measurement(&messages, tools, &anchor, identity);
        let hard = Budget::new(&connection.options).hard;
        if measured.tokens > budget.hard {
            return Err(Error::new(422, format!("Context cannot fit: approximately {} input tokens, budget {hard}. Unconsumed evidence and original history were preserved. Reduce input or adjust the context policy.", measured.tokens)));
        }
        if force && !changed {
            return Err(Error::new(422, "Context cannot fit after provider overflow: no eligible consumed history could be compacted; original history was preserved"));
        }
        let new_folded: Vec<_> = checkpoint
            .folded
            .difference(&saved.folded)
            .copied()
            .collect();
        let folded_range = new_folded
            .first()
            .zip(new_folded.last())
            .map(|(a, b)| json!([a, b]));
        let change = json!({"summarized_messages": if checkpoint.covered > saved.covered { Some([saved.covered, checkpoint.covered]) } else { None }, "folded_count":new_folded.len(),"folded_index_span":folded_range});
        let pressure = if measured.tokens >= budget.trigger {
            "high"
        } else {
            "normal"
        };
        let previous_stats = self
            .db()?
            .get(&format!("context_stats:{chat}"), Value::Null)?;
        let previous_pressure = previous_stats["pressure"].as_str().unwrap_or("normal");
        // No per-step telemetry messages. Notify only actual projection changes
        // or a threshold crossing; unchanged steps remain an append-only history.
        if changed || pressure != previous_pressure {
            let notice = json!({"role":"user","content":format!("<runtime_context_notice>\nBudget before this notice: approximately {} input tokens / {} usable input tokens (output and safety reserve excluded); source={}. Projection changes: {}. Raw history remains available via recall_history search/expand/recall_tool; shell archives via job_output. These are runtime facts, not a new user request.\n</runtime_context_notice>",measured.tokens,hard,measured.source,change)});
            if context::estimate(&notice) > context_policy::NOTICE_RESERVE {
                return Err(Error::new(
                    500,
                    "Context notice exceeded its reserved budget",
                ));
            }
            messages.push(notice.clone());
            checkpoint.notices.push(Notice {
                at: history.len(),
                message: notice,
            });
        }
        let total = context::measurement(&messages, tools, &anchor, identity);
        if cancel.is_cancelled() {
            return Err(Error::new(499, "Context compaction cancelled"));
        }
        let covered = checkpoint.covered;
        let folded = checkpoint.folded.len();
        self.db()?.put_batch(&[
            (key, serde_json::to_value(checkpoint)?),
            (format!("context_stats:{chat}"), json!({"estimated_input_tokens":total.tokens,"input_budget":hard,"before_tokens":before.tokens,"covered_messages":covered,"folded_results":folded,"estimate_only":total.source=="heuristic","measurement_source":total.source,"projection_changes":change,"pressure":pressure})),
        ])?;
        Ok(PreparedContext {
            messages,
            consumed: json!({"through":history.len(),"prefix":context::fingerprint(&history)}),
            changed,
        })
    }

    async fn summarize_context(
        &self,
        connection: &Connection,
        history: &[Value],
        checkpoint: &Checkpoint,
        end: usize,
        policy: &Policy,
        cancel: &CancellationToken,
    ) -> Result<String> {
        let mut summary = checkpoint.summary.clone();
        let mut next = checkpoint.covered;
        let mut summary_connection = connection.clone();
        if !summary_connection.options.is_object() {
            summary_connection.options = json!({});
        }
        let capacity = connection.options["max_input_length"]
            .as_u64()
            .unwrap_or(40_960);
        summary_connection.options["max_tokens"] = json!((capacity / 8).clamp(256, 2048));
        summary_connection
            .options
            .as_object_mut()
            .unwrap()
            .remove("reasoning_effort");
        summary_connection.cache_key = format!("{}:summary", connection.cache_key);
        let summary_budget = Budget::new(&summary_connection.options);
        while next < end {
            let prompt = policy.summary_prompt.as_str();
            let prefix = format!(
                "Previous checkpoint:\n{summary}\nAdditional history (original message indices):\n"
            );
            let overhead = context::request_tokens(
                &[
                    json!({"role":"system","content":prompt}),
                    json!({"role":"user","content":prefix}),
                ],
                &[],
            );
            let bytes = summary_budget
                .hard
                .saturating_sub(overhead)
                .saturating_mul(2)
                .min(40_000);
            if bytes < 512 {
                return Err(Error::new(
                    422,
                    "Context capacity cannot fit the summary request and output reserve",
                ));
            }
            let mut source = String::new();
            while next < end && source.len() < bytes {
                let mut message = history[next].clone();
                // Opaque provider state is replayed on its own protocol,
                // never embedded as ciphertext in a summary prompt.
                crate::model::strip_response_metadata(&mut message);
                if let Some(blocks) = message["content"].as_array_mut() {
                    for block in blocks {
                        if block["type"] == "image_url" {
                            *block = json!({"type":"text","text":"[Earlier image; original retained in history]"});
                        }
                    }
                }
                let serialized = message.to_string();
                let remaining = bytes.saturating_sub(source.len());
                if remaining < 128 {
                    break;
                }
                source.push_str(&format!(
                    "\nMessage {next}: {}\n",
                    context::preview(&serialized, remaining.saturating_sub(100))
                ));
                next += 1;
            }
            let input = vec![
                json!({"role":"system","content":prompt}),
                json!({"role":"user","content":format!("{prefix}{source}")}),
            ];
            if context::request_tokens(&input, &[]) > summary_budget.hard {
                return Err(Error::new(422, "Summary request exceeds context capacity"));
            }
            let mut completion = Completion::default();
            let discard: Emit = std::sync::Arc::new(|_| Ok(()));
            let summarized = tokio::select! {
                _=cancel.cancelled()=>return Err(Error::new(499,"Context compaction cancelled")),
                result=self.complete(&summary_connection,&input,&[],"summary","summary-reasoning",&mut completion,&discard)=>result,
            };
            if summarized.is_err()
                || completion.text.trim().is_empty()
                || !completion.calls.is_empty()
            {
                // Keep the old checkpoint. A failed summary must never
                // advance the durable cursor or silently lose evidence.
                return Err(Error::new(422,"Context compaction could not produce a complete checkpoint; original history is preserved"));
            }
            summary = context::head(&completion.text, context::SUMMARY_BYTES).to_owned();
        }
        Ok(summary)
    }
}
