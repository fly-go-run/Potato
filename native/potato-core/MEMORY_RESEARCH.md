# Memory mechanisms: bounded source audit

Reviewed 2026-09-06 from the read-only checkouts recorded in
`/tmp/potato-harness-reference.CL5beY/references.json`. Paths below are relative
to the named repository at its pinned commit; temporary checkouts may disappear.
This document distinguishes inspected implementations from optional integrations.

| Repository | Reviewed commit |
| --- | --- |
| PI (`earendil-works/pi`) | `9767ba275f3e9a5ee0f5c5342249b629ab1b2282` |
| DeepSeek Harness (`deepseek-ai/deepseek-harness`) | `d347e703908d0406b7a7ef80e3a0e594d86b2215` |
| Rig (`0xPlaygrounds/rig`) | `d9ed455cf5d0c8f13207ab03c7843982a0f4898e` |
| Codex (`openai/codex`) | `6af345407d9c2a568da9d01b6c4b81a9e61495c0` |
| Goose (`aaif-goose/goose`) | `5e90925962f05acf8e255032de44d16c4a7768a2` |
| VTCode (`vinhnx/VTCode`) | `ba2832c62fb293a79ffe2b5841cd751bd4826d85` |

## Terms and evidence limits

Transcript persistence retains ordered interaction evidence for resuming or
searching a conversation. Compaction changes the model-visible history; it does
not by itself extract reusable facts across independently created sessions.
Curated cross-session knowledge instead stores selected preferences, procedures,
or facts under a scope that survives creation of a new conversation.

Storage authority and retrieval are separate questions. An editable file can be
authoritative while an index is rebuilt from it. Conversely, an in-process map
called “memory” need not survive restart. These snapshots do not establish an
industry-wide trend of replacing vector databases with files.

## PI: instruction files and durable sessions

- `packages/coding-agent/src/core/resource-loader.ts:71` reads the first usable
  candidate among `AGENTS.override.md`, `AGENTS.md`, uppercase variants, and
  `CLAUDE.md`. Lines 119–156 load global agent-directory and ancestor context,
  handling duplicate paths and shadowed linked-worktree context.
- `packages/coding-agent/src/core/system-prompt.ts:151` inserts file contents in
  `<project_context>` / `<project_instructions path=...>`. Files are the authority;
  this path performs no vector or full-text retrieval.
- `packages/coding-agent/src/core/session-manager.ts:411` reconstructs the active
  compaction-aware branch; lines 949, 993, and 1035 implement JSONL naming,
  rewriting, and appending. This is session persistence, not a learned fact store.
- `packages/coding-agent/docs/extensions.md:15` documents `pi.appendEntry()` for
  session-persistent extension state. Lines 530 and 675 describe prompt/message
  injection hooks. Such hooks enable extensions but do not prove a shipped
  cross-session knowledge extraction/retrieval mechanism.

No built-in automatic cross-session fact store was found in this bounded audit.
Instruction loading is concrete; automatic curation would be an additional policy.

## DeepSeek Harness: transcript index plus optional memory servers

- `packages/session/session-persistence-jsonl/src/storage.ts:1` implements the
  JSONL provider's read/write handles, mutation ordering, and live write buffering.
- `packages/session-query/session-query-sqlite/src/index.ts:1` implements session
  search with SQLite FTS5 over a live-preferred corpus. Lines 69–76 explicitly
  identify the database as a disposable derived query index.
- `packages/session-query/session-query-sqlite/src/schema.ts:127` and `:158`
  create persisted/live FTS5 tables. Searching historical sessions is implemented;
  these tables are not evidence of automatically curated preference memory.
- `packages/context/agent-instructions/src/index.ts:1` describes baseline context
  before the first request and refresh after filesystem touches. Lines 137–165
  load/add baseline content; line 72 recognizes read/write/edit touches. This
  supplies file instructions, including changes and removals, through context.
- `docs/user/guide/mcp-memory.md:5`, `:11`, and `:33` explicitly describe
  default-off third-party memory configurations. DSH launches/connects the MCP
  client and exposes tools; the external provider owns persistence and retrieval.
- `apps/cli/config/examples/mcp-memory/mcp-reference-memory.cordis.yml:12`
  passes a JSONL path to the optional reference server. The guide describes its
  graph/observation tools and substring search without embeddings. The external
  server implementation was not independently audited here.
- `packages/workflow/tool-ralph/src/index.ts:157` instructs fresh workers to treat
  the shared workspace as authority, with a bounded prior report. This is a
  cross-round handoff instruction, not a separate knowledge database.

## Rig: conversation backend seam and separate vector RAG

- `crates/rig-core/src/memory.rs:93` defines conversation-ID `load`, `append`, and
  `clear`. Lines 381–382 implement `InMemoryConversationMemory` with
  `Arc<Mutex<HashMap<ConversationId, Vec<Message>>>>`; it does not survive restart.
- `crates/rig-agent/src/agent/runner.rs:430` loads history when both a backend and
  conversation ID are configured. Explicit caller history bypasses load and save.
- `crates/rig-agent/src/agent/engine.rs:1496` appends completed-run messages;
  failures are logged while the final answer is still returned.
- `crates/rig-memory/README.md:6` describes sliding/token-window policies for
  shaping loaded history. `crates/rig-core/src/memory.rs:278` and `:343` provide
  demotion/compactor extension traits, not concrete durable fact storage.
- `crates/rig-agent/src/agent/builder.rs:35` queries the configured vector index
  using the prompt or latest available history text and adds returned documents
  as extra context. Line 182 exposes optional `dynamic_context`. The application
  supplies the index; conversation-memory writes do not automatically populate it.

The example phrase “persistent memory” in a model preamble must not be mistaken
for restart durability or cross-session fact extraction in the in-process backend.

## Codex

`codex-rs/memories/README.md` documents a gated background pipeline: per-rollout
model extraction into the state DB, followed by serialized global consolidation
into Markdown artifacts and a dedicated consolidation agent. This is substantial
write-side orchestration, even though the read interface is file-oriented.

The executable read prompt is at
`codex-rs/ext/memories/templates/memories/read_path.md` (the README's older
read/templates path does not exist in this checkout). It supplies a small summary,
then directs ordinary searches over `MEMORY.md` and linked evidence. It also
separates stale remembered claims from currently verified facts. The read crate
at `codex-rs/memories/read/src/lib.rs` is independent of the write pipeline.
Potato borrows the small entrypoint and source-oriented lookup, not the two-phase
extraction/consolidation pipeline or its prescribed lookup steps.

## Goose

`crates/goose-mcp/src/memory/mod.rs:118` constructs a memory MCP extension.
It distinguishes project-local `.goose/memory/` from user-wide configuration
memory. `:184` resolves a category to a `.txt` file; `:260` appends data and tags;
`:287` retrieves categories; `:327` removes a matching entry. No vector index is
used in this implementation. At construction (`:143` onward), it retrieves all
global memories and includes them in extension instructions. Potato borrows
explicit scope and inspectable files, but avoids injecting every saved note.
This finding is about this extension, not every optional Goose integration.

## VTCode

`crates/codegen/vtcode-core/src/persistent_memory/mod.rs` defines Markdown
artifacts including `MEMORY.md`, `memory_summary.md`, preferences, repository
facts, notes, and rollout summaries. Records retain a source alongside the fact.
`reader.rs:35` normalizes a query, then `:40` filters collected facts/sources by
substring. `llm_ops.rs` also implements model-based classification and planning
of memory updates. `src/agent/runloop/unified/turn/session/memory_prompt.rs`
contains remember/forget intent handling, candidate planning and confirmation.
This is file-based persistence with significant policy above it. Potato adopts
scope/source awareness and user-editable notes; it does not add keyword intent
routing, normalized-fact schemas, candidate wizards or background cleanup.

## Evidence-based tradeoffs

- PI-style direct file injection makes the exact loaded content inspectable and
  avoids index synchronization, but includes all selected text in model context;
  it does not itself select relevant facts or resolve stale/conflicting entries.
- DeepSeek's disposable FTS index separates durable evidence from search state.
  It adds lexical retrieval and index maintenance; transcript hits still require
  interpretation and are not equivalent to validated reusable knowledge.
- Rig separates conversation storage, history shaping, and vector context. This
  allows independent backends, but an application must supply durable storage
  and any knowledge write/retrieval policy beyond conversation replay.
- Optional MCP memory moves lifecycle/storage behavior to another component.
  Integration examples prove interoperability, not that the host implements
  every advertised external memory capability.


## Python and Rust Potato before this change

Python `src/potato/agents/memory/agent_md_manager.py` manages actual Markdown
working/memory/digest files. `reme_light_memory_manager.py` delegates memory,
search, auto-memory and auto-dream to ReMe. `reme_config.py:161` configures hybrid
vector + BM25 search; `:617` configures the BM25 keyword index; `:650` removes
embedding components when embedding is disabled. Thus even the Python design
already separates file authority from optional embedding assistance. The
external ReMe dependency itself was not fully audited here.

Rust `workspace.rs` previously stored logical Markdown documents in a SQLite
JSON value and searched them with an AND of literal query terms. There was no
Rust vector database to remove, and a shell could not see those logical notes.
It lacked direct file interoperability and model-visible memory locations.

## Interpretation of the proposed direction

The user's hypothesis is useful for this personal/project assistant: capable
models can navigate named files, reformulate keywords, inspect relevant ranges
and maintain small notes. That supports a file-first baseline with a small
bootstrap and ordinary tools. It does not prove that stronger models eliminate
retrieval cost, missed synonyms or the need for indexes in large corpora.

Anthropic's [context engineering discussion](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
explicitly describes just-in-time file/tool retrieval alongside precomputed
retrieval, including the latency tradeoff and hybrid approaches. It supports
progressive discovery, not a universal replacement claim.

Storage and retrieval can evolve independently: if measured searches later
become slow or miss relevant paraphrases, add a rebuildable full-text or semantic
index over the same files. Keep exact source paths available. There is no such
index or embedding dependency in this Rust change.

Claude Code's [memory documentation](https://code.claude.com/docs/en/memory)
provides another direct precedent: an editable `MEMORY.md` index and separate
topic notes, with only a bounded index loaded initially and ordinary file tools
used for details. Its current documented limit is 200 lines or 25 KB; Potato's
smaller 2 KB per-scope preview is our own policy, not a copied model limit.

## Implemented Rust design

- User-wide notes live at `<native-data-root>/workspace/memory/`; project notes
  at `<conversation-project>/.potato/memory/`. Project storage is created on first
  write. No cross-project corpus is implicitly searched.
- The new user-turn runtime snapshot announces both paths and small optional
  `MEMORY.md` previews (at most 2000 UTF-8 bytes each). Existing history prefixes
  remain unchanged. Other notes are not automatically injected. There is no
  required schema or automatic extraction/merge job.
- `read_file`, `grep_search`, `glob_search`, file edits and permitted shell tools
  can operate on these ordinary files. `memory_search`/`memory_write` remain
  convenience tools for compatibility, now with explicit global/project scope.
  Search is literal, live and paginated; no match is not a semantic absence proof.
- Generic file edits may target the announced global memory directory even when
  a separate project is selected. This does not grant access to its parent or
  native credentials. Existing exact action approvals remain; project writes
  still require a writable project mode. Shell is optional, not a prerequisite
  for memory retrieval.
- The existing memory editor API reads/writes the user-wide files directly.
  External edits appear immediately. The location API exposes the real directory.
  Project notes can be edited through ordinary file tools/editors; the existing
  global memory page is not a project-memory browser.
- SQLite notes migrate once. Existing different files win; the legacy version
  is placed in `legacy-import/`. Only after successful migration does one DB
  transaction mark completion and retire the old copy. Restart does not recreate
  deleted notes. Interrupted migration can be retried. A conflicting import
  backup causes an explicit error rather than an overwrite.
- Writes use bounded UTF-8 files, optimistic content checks and atomic rename.
  Symlink escapes are rejected. Source/date/uncertainty and correcting stale
  claims are model guidance, not inferred truth labels or a forced parser.
- No new dependency is added. Persistent memory remains distinct from raw
  conversation history and its compaction/recall mechanisms.

Limits: note editing is capped at 1 MB; directory enumeration at 20000 entries;
the convenience search scans at most 32 MB and reports when capped. Search
pagination is live, not a snapshot under concurrent edits. Large or semantic
corpora may need an index later. Arbitrary external writers do not participate
in app locks; optimistic checks cannot provide a distributed transaction.
Workspace exports include user-wide memory; project notes travel with their
project and need its normal backup/version-control process. No live-model memory
quality or shell-vs-vector recall benchmark has been claimed.

Deleting a memory file does not rewrite earlier conversation transcripts or their
runtime snapshots. Current user corrections and current files take precedence;
history deletion is a separate operation. Project scope is a retrieval default,
not an additional OS sandbox or automatic cross-worktree synchronization.

## Verification (2026-09-06)

Final complete core suite: 86 passed, 0 failed, 1 ignored (the existing live-model
fixture requires explicit credentials/configuration). Counts: 42 unit, 10 harness,
4 memory integration and 30 runtime integration tests. Three memory unit tests
exercise actual model-tool routing and approvals, project/global scoping, generic
file edits, external-write conflicts, live lexical pagination and bounded index
bootstrap. Four memory integration tests cover migration (including retirement
of the old DB copy), file authority, restart/delete, optimistic saves and symlinks.
Two existing harness fixtures explicitly disable folding so their summary-failure
and summary-overflow assertions continue to test summarization independently of
the concurrently updated folding policy.

Both desktop crates pass `cargo check --offline` from their crate directories.
Formatting and diff whitespace checks pass. The latest shared workspace emits
unused steering/parallel-registry warnings; a warning-free final Clippy run is
not claimed. GPUI also reports its existing third-party future-incompatibility
warning. No live-model retrieval quality, production latency, or semantic recall
comparison was measured.


## Follow-up: conversation files (2026-09-06)

The subsequent authorized implementation also moved conversation bodies from
SQLite to JSONL, with large-text artifacts, project navigation indexes and public
shell archives. SQLite remains for catalog/runtime state. See
[TRANSCRIPT_STORAGE.md](TRANSCRIPT_STORAGE.md) for the actual format, migration,
recovery guarantees, limits and newer verification counts. The earlier Markdown
notes implementation and research comparisons above remain applicable.
