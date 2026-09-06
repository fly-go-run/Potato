# Rust harness: implementation and reference decisions

Reviewed 2026-09-06. Current policy and execution details: [HARNESS_POLICY.md](HARNESS_POLICY.md). Reference checkouts are read-only at
`/tmp/potato-harness-reference.CL5beY`; the manifest is `references.json`.
The temporary copies may be removed by the OS; commits below identify the
reviewed sources independently of those copies.

| Project | Reviewed commit |
| --- | --- |
| codex | `6af345407d9c2a568da9d01b6c4b81a9e61495c0` |
| deepseek-harness | `d347e703908d0406b7a7ef80e3a0e594d86b2215` |
| goose | `5e90925962f05acf8e255032de44d16c4a7768a2` |
| pi | `9767ba275f3e9a5ee0f5c5342249b629ab1b2282` |
| rig | `d9ed455cf5d0c8f13207ab03c7843982a0f4898e` |
| vtcode | `ba2832c62fb293a79ffe2b5841cd751bd4826d85` |

## Keep the harness focused

The model chooses tasks, decomposition, tools and when it has enough evidence.
The runtime owns valid protocol exchanges, bounded requests, durable evidence,
process lifetime and the application's existing action authorization. This
change does not introduce mandatory plans, task scoring, reflection loops,
role hierarchies, a batch DSL or a second workflow engine.

PI's `packages/coding-agent/README.md` Philosophy section explicitly leaves
plans, todos and subagents to extensions. Its `packages/agent/docs/harness.md`
also contains prospective design: it is not evidence that every described
feature is implemented. We checked executable compaction code separately.
Codex's `codex-rs/core/src/context_manager/{history,normalize}.rs` illustrates
why a thin execution loop still needs careful history normalization and token
accounting. Thin means fewer imposed decisions, not disposable process output
or unbounded model requests.

## What was borrowed, and what was not

| Source implementation | Decision in Potato |
| --- | --- |
| Python `agents/context/scroll` | Keep raw history distinct from the model window; recall archived evidence. Use bounded structured arguments instead of exposing arbitrary SQL/Python context manipulation. |
| PI `packages/agent/src/harness/compaction/compaction.ts` | Cut between complete tool exchanges, reserve summary output, retain the active user request even inside a long turn. |
| DeepSeek Harness implemented compaction-capability-seam and summary-prefix-cache-reuse notes | Check pressure between tool steps. Persist checkpoints. Independently budget summarization. Do not replay an oversized warm prefix merely to chase cache hits. |
| Codex context manager | Normalize missing/orphan/duplicate tool results. Anchor a stable prefix to reported input usage and estimate only appended items; invalidate the anchor after prefix/tools/provider changes. |
| Goose `crates/goose-context-management` | Useful existing Rust compaction crate with model/estimator traits and summary adapters. Its public messages/providers remain part of Goose's type ecosystem. We reference its separation of concerns without replacing Potato's provider/store/UI interfaces. |
| Rig `crates/rig-agent/src/run` | A real sans-IO `AgentRun` runtime, not just a provider SDK. Its caller-owned IO/context policy is a good design reference. Adopting it now would require adapting Potato's streaming, approvals, cancellation and durable wires without a demonstrated gain for this scope. |
| VTCode `crates/codegen/vtcode-core/src/tools/output_spooler.rs` | Archive command output and return bounded previews with explicit recovery. Do not bring across task-specific orchestration or mandatory structured task envelopes. |

Direct dependencies added for this work are `regex` and `globset`; no reference
repository is vendored. The code is written against Potato's existing types.
These are scope decisions, not a claim that the Rust ecosystem lacks suitable
harness implementations or that Potato has outperformed them.

## Behavior now implemented

- A stable system/tools prefix; current time and project facts are attached once
  to the new user message. Tool definitions are sorted. Responses uses a stable
  per-chat `prompt_cache_key`; host projection reuse and provider KV caching are
  distinct. Existing persisted prefixes are not rewritten on restart.
- Every model step budgets system, tools, history, images and an output reserve.
  Pressure selects old eligible tool results up to a target and commits one
  projection, then summarizes complete closed exchanges as needed. Normal
  pressure protects the active turn and recent results; emergency recovery
  requires prior model consumption of active evidence. The latest user request remains available. Failed or
  incomplete summaries never advance the durable checkpoint. Confirmed provider
  context overflow has one retry after an actual projection change.
- Raw history stays in JSONL with large text sidecars; SQLite holds catalog and
  context checkpoints. `recall_history` searches/previews replayed history,
  pages a full message with `expand` + `message_index`, or pages exact tool text
  with `recall_tool`. Cursors use UTF-8 byte boundaries and are session-scoped.
  Responses message, reasoning and function-call items are stored together in
  output-index order. Opaque replay metadata is omitted from textual recall,
  summaries and heuristic token estimates (visible content is counted once).
- Interrupted/malformed historical exchanges are normalized without inventing
  successful results. Invalid JSON arguments and unadvertised tool calls become
  tool errors. Removed/unannounced calls lose their preceding reasoning items
  in the projection; failed completions publish no replay metadata. Responses
  encrypted reasoning is requested and replayed with `store:false`; Chat
  requests omit this protocol-specific metadata.
- Provider input/output/cache counters and cumulative conversation usage are
  separate from estimates. Missing counters remain unavailable. A matching
  provider count replaces the heuristic estimate for the unchanged prefix,
  with estimates added only for appended messages. Built-in DeepSeek chat and
  reasoner entries default to 128,000 context tokens; missing limits in saved
  entries are filled without overwriting explicit settings.
- File primitives now include line ranges, exact edits with ambiguity/conflict
  checks, append, regex search and glob search. Existing confinement and action
  file confinement is preserved. Approval policy is independently configurable;
  see the [thin approval research](../../docs/rfc/rust-thin-approval-2026-09-06.md).
  Search results and reads expose paging cursors.
- Shell commands have an explicit working directory, foreground/background
  execution, durable stdout/stderr archives, bounded head/tail previews, status
  and return code. `job_output`, `job_list`, `job_kill` belong to the creating
  session. Cancellation/timeout terminate the process group. Normal exits are
  completed with an exit code, including nonzero codes. Jobs do not implicitly resume after runtime restart.
- `max_iters` is configurable (default 100, range 1..1000); it is a resource
  ceiling, not a forced decomposition policy.

## Relation to the Python original

The main file/shell/search/history/usage primitives now cover the gaps identified
in the initial audit. This is not a line-for-line port or an identical tool API.
The Python implementation still has specialized AST/LSP, external-agent and
batch/delegation tools that this change does not add. Existing Rust browser,
computer, skill, memory, scheduling, search and media integration is retained.
The reference projects inform behavior; passing local tests is not evidence of
identical model performance.

## Practical limits

- Input estimates are heuristic, including a fixed image allowance; provider
  tokenizers and image accounting vary. Oversized indivisible input can still
  require an explicit local error. Production cache hit rate and task quality
  were not benchmarked against a live model.
- Summary calls have a separate routing key and budget. Their usage is excluded
  from the conversation usage totals and that scope is returned explicitly.
  Warm-prefix summary replay is not implemented.
- Search has bounded traversal/file sizes; large/binary files and symlinks are
  skipped. File edits are limited to UTF-8 text up to 1 MB. Reads accept regular
  UTF-8 files up to 16 MB. These are stated execution limits, not semantic parsers.
- Shell archives cap each stream at 64 MB and report overflow as failure with
  partial output retained. Text paging replaces invalid UTF-8 bytes. Archived
  jobs persist without an automatic retention policy. Background work runs only
  while the runtime is alive; it does not automatically start another model turn.
- Ordinary project file work uses AUTO by default; boundary actions retain
  explicit approval and optional exact session grants. Native shell still has
  no OS sandbox; PI's container assumptions do not directly transfer to this app.

## Review fixes and protocol validation

The Responses replay follows the [official function-calling example](https://developers.openai.com/api/docs/guides/function-calling),
which appends returned output items before tool results. The existing DeepSeek
alias defaults use the [documented V3.1 128K capacity](https://api-docs.deepseek.com/news/news250821/);
this change does not migrate model names or assert availability of legacy aliases.

New output metadata preserves native item IDs, text content and output order.
Legacy `_responses_reasoning` wires did not save the original item order; only
single-reasoning/single-output histories retain that legacy replay path. Ambiguous
legacy reasoning is omitted from the projection, with original history unchanged.
No tokenizer estimate is inferred from encrypted byte length; unknown reasoning
cost still depends on provider usage and the overflow backstop.

On 2026-09-06, `model::replay_tests::live_responses_replay` passed through the
configured sub2api Responses endpoint using `gpt-5.4-mini`: three requests, two
synthetic tool calls and two encrypted reasoning items, with successful replay.
Only synthetic probe messages/tool results were sent. The test imports credentials
into a disposable encrypted store and never prints them. This checks this service
path, not every OpenAI deployment or every possible interleaved response. Exact
multi-reasoning/message/call interleaving and restart persistence are covered by
the deterministic HTTP/SSE harness.

Opt-in reproduction (ordinary test runs skip billed service calls):

```sh
POTATO_LIVE_LEGACY_DIR=/absolute/path/to/legacy \
POTATO_LIVE_SECRET_DIR=/absolute/path/to/legacy.secret \
POTATO_LIVE_MODEL=gpt-5.4-mini \
cargo test --manifest-path native/potato-core/Cargo.toml --offline \
  live_responses_replay -- --ignored --nocapture
```
