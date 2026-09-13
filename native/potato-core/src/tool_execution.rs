//! Bounded read concurrency with ordered evidence and sequential action barriers.
use crate::{protocol, required, Emit, Error, Result, Runtime};
use futures_util::{stream, FutureExt, StreamExt};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

impl Runtime {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn execute_calls(
        &self,
        chat: &str,
        session: &str,
        calls: &[Value],
        definitions: &[Value],
        body: &Value,
        cancel: &CancellationToken,
        emit: &Emit,
    ) -> Result<()> {
        let concurrency = self.db()?.get("running", json!({}))?["max_parallel_reads"]
            .as_u64()
            .unwrap_or(4)
            .clamp(1, 16) as usize;
        let execution_cancel = cancel.child_token();
        let cancel = &execution_cancel;
        let mut blocked = false;
        let mut start = 0;
        while start < calls.len() {
            let parallel = |call: &Value| {
                crate::tool_registry::parallel(call["function"]["name"].as_str().unwrap_or(""))
            };
            let end = if parallel(&calls[start]) {
                start + calls[start..].iter().take_while(|c| parallel(c)).count()
            } else {
                start + 1
            };
            let futures: Vec<_> = calls[start..end].iter().map(|call| async move {
                let call_id = required(call,"id")?;
                let name = required(&call["function"],"name")?;
                let arguments = required(&call["function"],"arguments")?;
                let id = uuid::Uuid::new_v4().to_string();
                let clock = protocol::ActivityClock::default();
                let mut frame = protocol::message(&id,"function_call","assistant",json!([protocol::data(&id,json!({"call_id":call_id,"name":name,"arguments":arguments}))]),"completed");
                frame["metadata"] = clock.metadata("running");
                self.db()?.append(chat,&frame,None)?;
                emit(frame.clone())?;
                let result = if cancel.is_cancelled() {
                    Err(Error::new(499,"Tool was not started: turn cancelled"))
                } else if self.has_steering(session)? {
                    Err(Error::new(409,"Tool was not started: superseded by queued user steering"))
                } else if !definitions.iter().any(|d| d["function"]["name"].as_str() == Some(name)) {
                    Err(Error::new(400,"Tool was not advertised for this request; use an available tool"))
                } else {
                    match serde_json::from_str::<Value>(arguments) {
                        Ok(args) if args.is_object() => self.execute_tool(session,name,&args,body,cancel,emit).await,
                        _ => Err(Error::new(400,"Tool arguments must be a valid JSON object; correct the arguments and retry")),
                    }
                };
                let state = execution_state(&result);
                let timing = clock.metadata(state);
                // Report completion immediately, even when ordered output is
                // waiting for an earlier parallel read. Persist timing on the
                // result below, keeping model evidence in its original order.
                frame["metadata"] = timing.clone();
                emit(frame)?;
                Ok::<_,Error>((call_id,name,result,timing))
            }.boxed()).collect();
            let mut results = stream::iter(futures).buffered(concurrency);
            while let Some(result) = results.next().await {
                let (call_id, name, result, timing) = result?;
                if result
                    .as_ref()
                    .is_err_and(|error| error.status == crate::reviewer::CIRCUIT_BREAKER)
                {
                    blocked = true;
                    execution_cancel.cancel();
                }
                self.record_tool_result(chat, call_id, name, result, timing, emit)?;
            }
            start = end;
        }
        if blocked {
            return Err(Error::new(
                crate::reviewer::CIRCUIT_BREAKER,
                "助手重复申请同一个已拒绝动作，已停止本轮执行。请补充授权或调整任务后再继续。",
            ));
        }
        Ok(())
    }

    fn record_tool_result(
        &self,
        chat: &str,
        call_id: &str,
        name: &str,
        result: Result<String>,
        timing: Value,
        emit: &Emit,
    ) -> Result<()> {
        let (mut output, mut state) = match result {
            Ok(output) => (output, "success"),
            Err(error) => (error.message, "error"),
        };
        if crate::tool_registry::lookup(name)
            .is_some_and(|s| s.access == crate::tool_registry::Access::Shell)
            && serde_json::from_str::<Value>(&output)
                .ok()
                .is_some_and(|v| {
                    matches!(
                        v["status"].as_str(),
                        Some("failed" | "cancelled" | "interrupted" | "timed_out" | "terminated")
                    )
                })
        {
            state = "error";
        }
        if crate::tool_registry::lookup(name).is_some_and(|s| s.image) && state == "success" {
            let mut blocks: Value = serde_json::from_str(&output)?;
            let image_id = uuid::Uuid::new_v4().to_string();
            if let Some(blocks) = blocks.as_array_mut() {
                for (index, block) in blocks.iter_mut().enumerate() {
                    block["object"] = json!("content");
                    block["msg_id"] = json!(image_id);
                    block["delta"] = json!(false);
                    block["index"] = json!(index);
                    block["status"] = json!("completed");
                }
            }
            let frame = protocol::message(&image_id, "message", "assistant", blocks, "completed");
            self.db()?.append(chat, &frame, None)?;
            emit(frame)?;
            // Image bytes belong in display history, never in the next
            // text-only model request (e.g. DeepSeek).
            output = "Image generated and displayed to the user.".into();
        }
        let output_id = uuid::Uuid::new_v4().to_string();
        let mut output_frame = protocol::message(
            &output_id,
            "function_call_output",
            "tool",
            json!([protocol::data(
                &output_id,
                json!({"call_id":call_id,"name":name,"output":output,"state":state})
            )]),
            if state == "success" {
                "completed"
            } else {
                "failed"
            },
        );
        output_frame["metadata"] = timing;
        let wire = json!({"role":"tool","tool_call_id":call_id,"content":output});
        self.db()?.append(chat, &output_frame, Some(&wire))?;
        emit(output_frame)?;
        Ok(())
    }
}

fn execution_state(result: &Result<String>) -> &'static str {
    match result {
        Err(error) if error.status == 499 => "cancelled",
        Err(_) => "failed",
        Ok(output) => {
            let data = serde_json::from_str::<Value>(output).unwrap_or(Value::Null);
            if data["exit_code"].as_i64().is_some_and(|n| n != 0)
                || matches!(
                    data["status"].as_str(),
                    Some("failed" | "timed_out" | "terminated" | "interrupted")
                )
            {
                "failed"
            } else if data["status"] == "cancelled" {
                "cancelled"
            } else {
                "completed"
            }
        }
    }
}
