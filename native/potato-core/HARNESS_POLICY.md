# Context policy, steering and tool execution

The runtime retains immutable raw history, valid tool exchanges, bounded requests,
atomic checkpoints, process ownership and the configured approval policy. Optional
context decisions are isolated from those mechanisms.

## Boundaries

- `context.rs`: projection, protocol normalization and token measurement. A matching
  provider anchor can raise or lower the estimate; rewritten prefixes, tools or
  providers invalidate it. Measurement source is explicit.
- `context_policy.rs`: a pure planner selects eligible outputs and a pair-safe
  summary boundary. It has no model, database or process access.
- `compaction.rs`: loads history and a consumption checkpoint, executes summaries,
  validates capacity, then commits the projection and statistics in one transaction.
- `tool_registry.rs`: each built-in wire name, schema, typed handler identity,
  access category, image behavior and read-concurrency eligibility is registered
  once. Tool implementations remain in their own modules.
- `tool_execution.rs`: runs bounded batches of independent reads, keeps model-facing
  results in call order, and treats writes/unknown external tools as barriers.
- `steering.rs`: accepts durable user corrections and delivers them at complete
  exchange boundaries. The final queue check and closing of the input boundary
  share the same runtime lock.

## Context behavior

Normal pressure excludes the complete active turn and the five newest tool
results from folding. Short outputs (at most 200 Unicode characters by default)
and replacements that would not save space are skipped. Eligible results are
considered oldest first; planning stops at the lower target, leaving headroom
before the next trigger. Only the final projection is committed. Cache benefit
is a design expectation, not a measured provider guarantee.

At the hard input budget, or after a provider-confirmed overflow, recency protection
may be relaxed for active results already included in a successful model request.
A raw-prefix fingerprint validates that consumption checkpoint. Failed requests
never advance it. Newly returned active results are neither folded nor summarized
before their first successful consumption. Fixed bounded output previews still
carry explicit recovery pointers. If protected input cannot fit, the runtime
returns a capacity error with the raw evidence intact.

Summaries operate on raw source data and retain the latest real user message by
default. A failed, empty, incomplete or tool-producing summary cannot commit its
proposed evictions. Summary requests have their own output reserve and do not
contribute to the conversation usage totals.

A compact runtime notice is appended when pressure crosses a threshold or the
projection changes, with approximate
occupancy, usable budget, measurement source, new summary interval/fold count
and recovery tools. The notice explicitly states that its count precedes the
notice itself. Statistics include the complete prepared request on every step, also available through get_token_usage. Notices live in
the durable projection checkpoint at raw-message boundaries; they do not alter
raw recall indices, old runtime snapshots or earlier request prefixes. A bounded
notice reserve is separate from the output reserve. Source values are `heuristic`,
`provider` and `provider_plus_estimate`; a mixed value remains partly estimated.

## Configuration

`PUT /api/workspace/running-config` accepts `context_policy` and
`max_parallel_reads`. These are persisted runtime settings; this change does not
add a graphical context-policy editor. Example:

```json
{
  "max_parallel_reads": 4,
  "context_policy": {
    "automatic": true,
    "fold": true,
    "summarize": true,
    "protect_recent": 5,
    "min_tool_chars": 200,
    "trigger_ratio": 0.8,
    "target_ratio": 0.55,
    "pin_user": true
  }
}
```

Omitted policy fields use defaults, including a concise continuation-summary
prompt. `summary_prompt` may be overridden. `automatic=false` disables soft-pressure
intervention, retaining emergency recovery. `fold=false` and `summarize=false`
disable those transformations even in emergencies; capacity checks then return
an explicit error when the request cannot fit. Protocol repair and raw evidence
retention remain enabled. Ratios require `0 < target < trigger < 1`; read concurrency
is 1–16 (`1` gives sequential execution).

## Steering and execution results

Both native frontends offer **补充指令** while streaming. Text is queued through
`POST /api/agent/steer` with `session_id` and `text`; attachments remain a next-turn
operation. Accepted corrections are stored before acknowledgment. Pending approvals
and questions are released, already executing operations reach their normal
boundary, and not-yet-started calls receive explicit superseded results. Then the
correction is appended as a real user message, preserving call/result adjacency.
An in-flight model response finishes before the correction is consumed. A model
final answer cannot end the run successfully while a correction is queued.

Queued corrections survive cancellation, failure and restart, and are delivered
exactly once before the next ordinary user message. Late steering receives a 409;
frontends retain the draft on failure. Steering cannot change the run's permissions.

Shell jobs report normal process exit as `completed`, including nonzero codes.
`exit_code` and the compatible `return_code` carry the observed value; stdout and
stderr remain available. Signal termination (`terminated` with `signal`), timeout
(`timed_out`), cancellation and runtime faults have separate states. These are
execution facts, not a judgment that the user's task succeeded or failed.

Read concurrency never bypasses authorization. External/MCP/computer calls, shell,
writes, scheduling and questions are sequential barriers. Background jobs retain
their existing explicit lifetime semantics and are not resumed by app restart.

## Verification scope

Local unit and HTTP/SSE replay tests cover oldest-first target stopping, short and
recent protection, unconsumed active evidence, emergency recovery, policy disabling,
measurement anchoring, prefix stability across restart, pair-safe summaries and
rollback, bounded concurrent approvals with ordered results, write barriers,
steering during approvals/final responses, exact-once restart delivery, and process
exit/timeout/signal distinctions. Native frontend tests exercise steering availability
and preservation of edits made while submission is pending. These fixtures do not
establish production cache hit rates or live-model task-quality improvements.

Verified on 2026-09-06 against the integrated worktree, including the transcript
storage and approval changes from concurrent tasks:

- Core `cargo test --offline`: 115 passed; one credential-dependent live-provider test ignored.
- Native Iced UI `cargo test --offline`: 64 passed.
- GPUI `cargo test --offline`: 11 passed.
- Core library `cargo clippy --offline --lib -- -D warnings` passed.
- Core formatting and `git diff --check` passed. Modified GPUI files were formatted
  with their edition; the shared UI stream module retains its owning crate's edition.
