# Rust desktop runtime

The desktop defaults to an embedded Rust backend. Tauri and the existing
React/TypeScript UI remain in place. Each macOS/Windows installation has its
own SQLite database; launching the application does not start a Python server,
bind a local HTTP port, scan plugins or connect MCP servers.

This is a substantial runtime migration with known compatibility gaps, **not
full feature parity or a release acceptance report**. The default build has
changed locally; no release has been published and existing installations have
not been upgraded.

## Runtime coverage

- DeepSeek-compatible Chat Completions and configurable sub2api Responses:
  Unicode streaming, reasoning, tool loops, cancellation, saved partial failures,
  chat/provider management and reconnect to a running turn without rerunning it.
  Replay has a 16 MB limit and up to 16 listeners.
- Cached context summaries above 120 KB of pending wire history. Compaction
  keeps the newest two user turns, commits only at a user-turn boundary and
  preserves full display history. Large historical messages are excerpted for
  summarization. Output-token limits and reasoning effort are persisted through
  the original model settings API and mapped to each supported protocol.
- PDF, DOCX, PPTX and spreadsheet (XLSX/XLS/XLSB/XLSM/ODS) text extraction runs
  in Rust without a converter process. Uploads are limited to 20 MB and extracted
  text to 1 MB. Cached spreadsheet values are read without executing formulas or
  macros. XML external entities and unsafe archive paths are rejected. Text is
  retained as inert plain-text data URLs for preview; original binary files,
  document layout and OCR are not retained. Image-only/file-only input is accepted.
- Doubao streaming ASR with PCM/gzip framing, partial/final transcripts, bounded
  recording and cancellation. Compatible multipart audio transcription is also
  available. Credentials are encrypted, masked in settings and retained when
  speech is disabled.
- Image generation and editing through the separately configured image provider:
  /images/generations and multipart /images/edits. Editing uses images attached to
  the current message. Generated images are persisted for display and excluded
  from subsequent text-only tool results.
- Questions with single/multiple/free-text answers, cancellation, persistence,
  session checks and user answers displayed in the chat stream.
- Workspace Markdown, memory notes and configurable prompt files in SQLite.
  Lexical memory search and explicit approved memory writes; no embedding index
  or automatic extraction. Default translated AGENTS templates initialize once
  and never overwrite saved user edits.
- Project selection, creation with git init, directory browsing, git status and
  diff. A conversation can select its own project without changing global state.
- Approved file read/list and project-confined writes. Writes use cap-std
  directory capabilities, compare prior content, reject changed targets and
  replace files atomically. This is not an OS process sandbox.
- Approved foreground-duration shell commands: Unix process groups and Windows
  Job Objects, bounded output, cancellation and timeout. Shell requires explicit
  danger-full-access mode; it runs with the computer account's permissions.
  Background command jobs and workspace-only process sandboxing remain absent.
- Persistent cron and once tasks, IANA timezones, DST handling, pause/resume/manual
  run, history, missed-run handling and restart recovery. Text reminders and
  agent tasks execute while the application is open and the computer is awake.
  Background updates refresh/reconnect the original chat view. No autonomous
  heartbeat or OS wakeup scheduling.
- Markdown skill ZIP upload/list/read/enable/disable/delete. Traversal,
  symlinks, duplicate roots and oversized archives are rejected. Executable
  skill resources and skill-hub installation are not implemented.
- Remote Streamable HTTP and local stdio MCP configuration, encrypted headers
  and environment variables, explicit lazy discovery, cached tool schemas,
  whitelist and per-call approvals. Saving a server or starting the app does not
  connect to it or launch a child process. Local stdio children use process-group
  or Job Object cleanup. Executable commands run with the user's permissions;
  their optional runtimes are not bundled. Legacy SSE, OAuth and principal-specific
  MCP policies remain unsupported.
- Native image plugin install/remove/catalog adapter. This does not load
  arbitrary Python plugins.
- Hosted Responses web search with citations, Exa and Tavily adapters. Exa uses
  EXA_API_KEY; Tavily uses TAVILY_API_KEY when present, otherwise retains the old
  keyless request mode. Availability of that mode is service-dependent. No TLS
  verification bypass. Search text is marked as untrusted source data.
- Rust computer-use adapter for the pinned official Cua driver. It starts lazily,
  binds single-use observations to chat/window/snapshot, sends background input,
  revokes sessions and protects Potato, terminals and system settings. Current
  adapter uses accessibility trees; screenshots and app-wide approval leases
  remain incomplete. Disabling computer use blocks subsequent actions.
- Public history JSON import is transactional, repeatable and never overwrites
  existing chats. Workspace ZIP export contains documents, memories, history,
  Markdown skills and task records; it excludes service keys and external
  project files. It is not a complete automatic backup/restore system.

Credentials use ChaCha20-Poly1305. The separate master key is mode 0600 on Unix;
Windows relies on the per-user app-data directory ACL. The current permissions
mode requires exact per-call approvals; AUTO/OFF policies are not emulated.

## Build

From the repository root, with Node, Rust and the platform Tauri build tools:

```sh
node scripts/native/build-desktop.mjs --debug --preview
```

Omit --debug for release compilation; omit --preview for the normal Potato
identity. The build helper stages only the official Rust computer driver and
uses the existing frontend. It verifies a pinned archive SHA-256 and, on macOS,
the standalone binary signature. Node is a build dependency, not an installed
runtime. Existing build_macos_pyinstaller.sh/build_win_pyinstaller.ps1 entry
points now delegate to this helper by default.

The standalone binary must be selected from the official archive. Extracting
the executable from CuaDriver.app loses its bundle signature context and macOS
terminates it. The native build uses binaries/native-cua-driver separately from
old staged resources. Driver 0.20.0 was re-downloaded, checksum/signature checked
and its --version command succeeded on the development Mac.

The native-runtime Cargo feature is enabled by default. Historical Python
builds require --no-default-features plus tauri.python.conf.json, or
POTATO_LEGACY_BACKEND=1 through the historical build entry points. They are
retained for comparison/recovery; normal desktop startup never selects them.

The preview uses a separate app identifier and no production update endpoints.
Normal and preview builds store native data in the respective Tauri app-data
directory under native-v1. POTATO_NATIVE_DATA_DIR supports disposable test data.
Old Python data is not silently modified or adopted.

## Data migration and remaining compatibility

The existing export helper can obtain public history from a stopped/idle old
backend:

```sh
python3 scripts/native/export_history.py --base-url http://127.0.0.1:PORT --output /path/to/history.json
```

Choose the export in the existing settings page's history import control.
This one-time legacy helper is not shipped or invoked by the Rust application.
The settings page also offers a Rust-only legacy connection importer. It reads
the selected data and secrets directories, decrypts Fernet credentials using
the existing .master_key file, and transactionally re-encrypts supported OpenAI
Chat/Responses providers, Doubao speech and the image plugin connection into
native storage. Configured native connections win on repeat import. No old data
is changed and no Python process is launched. If an older installation has only
a keychain key, open that older application once to export its master key first.
This importer is explicit and does no credential reads during application startup.
Old local attachments and Python agent implementation state remain separate.

Outstanding work before claiming a fully compatible family release:

- Scanned-document OCR, original binary retention and old attachment migration.
- Additional provider-specific generation settings and usage display.
- Remaining MCP transports/authentication, executable skill resources, arbitrary
  plugin migration, background shell jobs and full permission-policy parity.
- Computer screenshots and real end-to-end input verification on both OSes.
- Actual sub2api image/chat and Doubao microphone tests, sleep/resume and gateway
  errors. Local mocks do not prove an individual gateway supports an endpoint.
- Windows installer/runtime checks and release signing. No remote CI or
  production release was run in this task.

## Verification (2026-09-06)

- 44 Rust tests passed: 21 unit and 23 integration. They cover both chat
  protocols, voice framing, encrypted storage, approvals/questions, reconnect,
  images and editing, hosted-search citations, skill archives, project/file
  behavior, process-tree cancellation, cron agent execution, backup exclusions
  computer capability binding through a fake local driver, PDF/Office extraction,
  stdio MCP and synthetic encrypted legacy credentials. The extended image-plugin
  credential migration test also passed separately.
- The full frontend test suite passed; the existing frontend production build and
  Tauri native cargo check/build passed.
- A new unsigned debug macOS preview bundle was built and launched with an empty
  disposable data directory. The original chat view appeared; its About page
  showed the backend online and one loaded agent. The app process had no child
  processes. The bundle contained no Python/Node runtimes.
- An earlier debug bundle measured 141,780,773 bytes before the document parsers
  were added; it is not the current artifact size or a release-size measurement.
  The core readiness example measured first open
  17.46 ms, p50 0.48 ms and p95 1.13 ms over 25 same-process opens. This measures
  core/database initialization, not whole-app cold start.
- .github/workflows/native-runtime.yml defines macOS/Windows checks; a workflow
  file alone is not evidence that CI ran.
- A normal-identity optimized macOS build also completed at
  console/src-tauri/target/release/bundle/macos/Potato.app, with distribution ZIP
  dist/Potato-Tauri-2.0.9-macOS.zip. It was not launched because an existing
  /Applications/Potato.app instance was already running. That instance was left
  untouched. The local package is not a signed/notarized release or a published update.
- scripts/native/smoke-desktop.mjs performs credential-free process/database
  startup checks. The latest local preview run reported native readiness, 8 ms core initialization
  and no legacy child processes. The release workflow uses this check instead
  of waiting for a Python HTTP endpoint. It does not establish UI or installer
  acceptance; those still require platform tests.

The four source AGENTS translations were audited and maintained; obsolete
ignored build copies were removed. See the template maintenance README.
