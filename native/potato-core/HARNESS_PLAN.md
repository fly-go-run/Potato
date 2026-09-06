# Rust harness upgrade

Scope: improve context, prompt caching, recoverable tool output, and the core
tool workflows identified in the Python/Rust comparison. Preserve existing UI
and provider integration and the user's uncommitted changes.

## Requirements and verification

- Stable system/tools prefix; append runtime facts with the new user message.
  Verify byte-identical request prefixes across turns and process restart.
- Budget the entire request before every model step, including tools, summary,
  images and output reserve. Pressure-fold durable tool output before summary;
  retain the current user request and whole tool exchanges. Verify small-window,
  single-long-turn, CJK, large schema, cancellation and overflow fixtures.
- Keep original history immutable and recoverable with bounded, session-scoped
  history/tool-output paging. Verify exact reconstruction and no cross-chat reads.
- Persist projection/checkpoint changes only after successful summaries; fit
  summary requests independently and retry provider-confirmed overflow once.
- Normalize malformed and interrupted tool histories; report bad arguments as
  tool errors so the model can correct them. Verify Chat and Responses wires.
- Collect provider input/output/cached token usage separately from estimates.
- Improve core file read/edit/append/search, shell/background result handling,
  loop limits and tool-result envelopes. Preserve action authorization and
  project confinement. Verify behavior and affected desktop compilation.
- Run core tests and targeted harness request replays; document residual limits
  honestly (especially tokenizer estimates and unmeasured production cache rate).

## Reference designs (reviewed 2026-09-06)

- Python Potato: `agents/context/scroll`, runtime snapshots, result pruning.
- PI: https://github.com/earendil-works/pi/blob/main/packages/agent/src/harness/compaction/compaction.ts
  Pair-safe cut points, separate summary output reserve, retained turn context.
- DeepSeek Harness: https://github.com/deepseek-ai/deepseek-harness/blob/master/.agents/notes/implemented/feature/2026-06-18-compaction-capability-seam.md
  Between-step pressure, balanced units and durable checkpoints.
- DeepSeek Harness: https://github.com/deepseek-ai/deepseek-harness/blob/master/.agents/notes/implemented/bug-fix/2026-07-21-compaction-summary-prefix-cache-reuse.md
  Reusing warm prefixes for summaries is useful only when the replay fits;
  summary requests must have their own budget. Session projection caching is
  host-side caching, not provider KV caching.
- Goose (Rust): https://github.com/aaif-goose/goose/tree/main/crates/goose-context-management
  Separate model/estimator and compaction adapters. See HARNESS_REFERENCES.md
  for pinned commits, Codex/Rig/VTCode implementation paths and adoption decisions.

## Status

Implemented and verified on 2026-09-06. Source decisions, tool parity boundaries
and residual limits are documented in `HARNESS_REFERENCES.md`.

Verification:

- `cargo test --manifest-path native/potato-core/Cargo.toml --offline`:
  79 passed (35 unit, 10 HTTP/SSE harness, 4 memory integration,
  30 runtime integration; includes concurrent memory-module changes);
  the opt-in live Responses test is ignored by default.
- `cargo clippy --manifest-path native/potato-core/Cargo.toml --offline --lib -- -D warnings`.
- `cargo fmt --manifest-path native/potato-core/Cargo.toml --check` and
  `git diff --check`.
- `cargo check --offline` from each of `native/potato-ui` and
  `native/potato-gpui`; run from the crate directory to honor its toolchain.
  GPUI uses its existing Rust 1.96.1 pin. Its third-party `block 0.1.6`
  dependency emits a future-incompatibility warning.

The harness fixtures verify append-only request prefixes after restart in both
protocols, routing keys, output recovery, active-turn compaction, invalid
arguments, one-shot overflow recovery, failed-summary atomicity, oversized
uncompactable inputs, failed jobs and encrypted Responses reasoning replay.
Unit/integration coverage includes provider usage anchor invalidation, exact
UTF-8 history/output paging, session isolation, file edit conflicts, bounded
search and process-group cancellation. The existing credential test now scans
subdirectories too, because shell output archives add a directory to storage.

The standard checks use deterministic local model fixtures. A separate opt-in
Responses smoke test passed against sub2api/gpt-5.4-mini with three requests,
two synthetic tool calls and two encrypted reasoning items replayed successfully.
No live-model cache hit percentage, task-quality improvement or identical Python
behavior is claimed. See HARNESS_REFERENCES.md for reproduction and scope.

## Architecture review follow-up

Implemented policy/mechanism separation, consumed-evidence protection, explicit
measurement notices, process outcome semantics, a built-in registry, durable user
steering in both native UIs and bounded read concurrency with write barriers.
Configuration, invariants and verification scope are in [HARNESS_POLICY.md](HARNESS_POLICY.md).
