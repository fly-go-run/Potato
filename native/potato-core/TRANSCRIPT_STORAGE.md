# File-backed conversation history

Rust Potato now keeps conversation records in ordinary JSONL files. SQLite
continues to hold the conversation catalog, settings, credentials, questions and
context checkpoints. It no longer holds the `messages` table. This follows
Codex's division between rollout files and database state, and Claude Code's
project/session transcripts and large-result files. It does not copy either
product's private wire format.

## Files and discovery

Under the existing native data directory:

```
workspace/
  history/
    README.md                       # generated navigation and format guide
    projects/
      <project-id>.jsonl             # small, rebuildable conversation index
      unassigned.jsonl              # imported history with no known project
    sessions/
      <archive-id>/
        transcript.jsonl             # authoritative display + model records
        artifacts/<uuid>.txt         # complete large UTF-8 strings
        journal.lock
        recovery-tail-<uuid>.bin     # preserved interrupted final write, if any
    deleted/<archive-id>            # temporary durable deletion marker
    jobs/
      <job-id>/
        state.json                   # session, cwd, command, status
        stdout
        stderr
  memory/                           # existing user-wide Markdown notes
```

`archive-id` is UUID v5 of the public conversation ID, so older non-UUID IDs remain
valid and cannot become path traversal. Project IDs are UUID v5 of the canonical
project path. A conversation that visits several projects appears in each
project's index. Project selection is recorded in the journal; imported sessions
remain unassigned until their project is known. File locations are centralized
under the native workspace, not automatically written into the user's Git repo.
Project notes still live in `<project>/.potato/memory/`. Newly created conversation
IDs are stable UUID v5 values derived from the session ID, so retry after a
published-journal/failed-SQL write can adopt the same original archive.

The runtime announces the current project's index, current transcript and command
artifact directory in every turn's environment snapshot, independently of memory
preview fingerprint gating. It does not inject other
conversations or automatically summarize them into long-term memory. Existing
file/search/shell tools can retrieve those files on demand under their normal
access rules. Built-in model file writes reject the runtime-owned history subtree,
including canonical aliases and missing descendants. Direct shell cwd inside that
subtree is rejected too, but an authorized unsandboxed shell remains unsandboxed;
this is not OS-level write isolation. User/project notes remain editable.
`recall_history` remains an optional current-session convenience and
keeps stable model-message indices and exact UTF-8 byte paging. `/api/chats/<id>/archive`
returns actual paths for clients; no new model tool is required.

Indexes and `history/README.md` are generated files. Put user-authored guidance in
the existing prompt documents, project files or memory notes, rather than editing
the generated index. A small navigation file is the retrieval starting point;
it is not a mechanism for automatically granting permissions from historical text.

## Record format

Each physical line is one committed transaction envelope:

```json
{"format":"potato-transcript-v1","record":{"version":1,"events":[{"type":"append","frame":{"type":"function_call_output"},"wire":{"role":"tool","content":"preview…"}}]},"text_refs":[{"pointer":"/events/0/wire/content","path":"artifacts/<uuid>.txt"}]}
```

`text_refs` JSON pointers select strings inside `record`; referenced files contain
their full original UTF-8 text. Strings over 16 KiB are replaced inline by a short
preview. An explicit outer envelope avoids interpreting similarly shaped user
content as a storage directive. Hydration restores both display frames and model
wire values exactly, including Responses replay metadata. Shell streams keep
their existing independent stream archives and bounds.

Events are `session` (catalog metadata), `append` (display frame plus optional
model wire), `replace_frame`, `replace_wire`, and `deleted`. Replacement indices
refer to the raw append sequence, including display-only frames. These events
preserve the original record: steering delivery and environment attachment append
new events instead of rewriting old JSONL lines. One steering transaction both
marks queued display frames delivered and adds their model-visible input, so a
restart cannot deliver only half of the correction. Compaction remains a model
projection; it never overwrites the raw transcript.

## Migration and recovery

Opening an older native database migrates each conversation's ordered display and
wire records to a durably published transcript. A SQLite transaction then retires
the legacy `messages` table and advances the schema to version 3. Existing identical
files make interrupted migration retryable; conflicting files cause an explicit
error while legacy SQL rows remain. Current native conversations still win on
repeat portable history imports. The existing display-history export/import API
continues to work; it is not a full native-state backup.

Journal writes are synced before committing catalog updates. Startup reads catalog
events without hydrating message sidecars (unusual legacy metadata sidecars are
hydrated selectively). A damaged session is retained with a `history_error` in its
catalog entry; other sessions and settings remain usable. Missing artifacts are
detected when that session is read. `/api/native/history-health` exposes archive,
index and job migration warnings. Existing catalog ownership wins over a duplicate
orphan session; the conflicting file is preserved and reported, not silently merged.

Deletion publishes a small durable marker outside the transcript before removing
the session directory and committing the catalog deletion. It therefore works on
corrupt or full journals. Restart completes an interrupted deletion; the marker is
removed after the catalog commit, permitting an explicit same-ID reimport later.
Legacy `deleted` events remain understood. Deletion also clears context checkpoints.
Shell job archives retain their independent lifetime.

A final incomplete JSONL line is copied byte-for-byte into a recovery file before
truncation to the last newline. Malformed complete lines and missing artifacts are
session errors; original content is never replaced with empty history. Scoped file
handles reject symlink escapes. Per-session file locks serialize archive operations;
SQLite write transactions serialize catalog-affecting writes between runtimes.

Navigation files are regenerated after the authoritative catalog commit, and only
changed content is replaced. A navigation write failure becomes a health warning,
not a failed conversation/import or failed app startup. Disjoint legacy jobs move
into the new directory even when both locations exist; conflicting old entries
remain in place and are reported. Loaded job state exposes current output paths.

## Boundaries and validation

This is file-backed retrieval, without embeddings, automatic memory extraction,
confidence scoring or a background summarization pipeline. The model still has to
choose useful search terms and verify stale information. No retrieval-quality,
semantic-recall or production-latency improvement is claimed from storage changes.

Current bounds are 32 MiB per hydrated transaction and 256 MiB per hydrated
session/log. The archive keeps one bounded in-process session cache, shared for
read inspection; append maintains byte totals instead of reserializing the old
history. Unix cache validation checks inode, mtime/ctime and all referenced artifact
stamps, including same-size replacement or deletion. Other platforms conservatively
reload. An invalidated cache is reconstructed from the files. Steering checks avoid
cloning tool bodies and no-op delivery avoids replaying all frames again.

Cross-conversation full-text search remains an explicit linear scan, with unreadable
sessions skipped rather than breaking the whole search. It has no new FTS/vector
index or background service. An externally restored archive may still need manual
inspection of orphan files reported by the health endpoint. There is no distributed
transaction with arbitrary external filesystem writers, nor automatic orphan-artifact
garbage collection. Native settings/checkpoints still require SQLite; rebuilding the
chat catalog does not restore missing credentials or settings.

Verification after the Fable review fixes on 2026-09-06: **138 tests passed, 0 failed,
1 live-provider test ignored**. Counts: 83 unit tests, 16 harness, 4 memory, 30 runtime,
and 5 transcript integration tests. Both desktop crates pass offline `cargo check`;
GPUI retains the pre-existing third-party `block` future-compatibility warning.

New regressions cover corrupt-session isolation/delete/reimport, missing-sidecar
lazy loading, index write failure/recovery, SQL-failure retry adoption, conflicting
orphan preservation, interrupted deletion, job-directory coexistence, unchanged
index preservation, same-size cache invalidation, cached append without old-record
reload, write guards and repeated-turn history navigation. No live-model recall
quality or production latency benchmark is claimed.
