# QA Agent removal report

## Scope and changed files

Removed the built-in `QwenPaw_QA_Agent_0.2` / legacy `CoPaw_QA_Agent_0.1beta1` implementation, its `qa` agent template, QA-only workspace markdown templates, and `QA_source_index` skill. The task-related files changed are:

- Backend: `src/qwenpaw/constant.py`, `src/qwenpaw/agents/templates.py`, `src/qwenpaw/config/config.py`, `src/qwenpaw/app/migration.py`, `src/qwenpaw/app/_app.py`, `src/qwenpaw/app/multi_agent_manager.py`, `src/qwenpaw/app/routers/workspace.py`, `src/qwenpaw/cli/init_cmd.py`.
- QA-specific setup compatibility: `src/qwenpaw/agents/utils/setup_utils.py` and `src/qwenpaw/agents/utils/__init__.py`.
- Deleted assets: `src/qwenpaw/agents/skills/QA_source_index-en/`, `src/qwenpaw/agents/skills/QA_source_index-zh/`, and `src/qwenpaw/agents/md_files/qa/`.
- Tests: `tests/unit/app/test_multi_agent_manager_startup.py`, new `tests/unit/app/test_migration.py`, `tests/unit/cli/test_cli_agents.py`, `tests/integration/test_agents.py`, `tests/unit/agents/utils/test_setup_utils.py`, and `tests/unit/app/test_agents_workspace_initialization.py`.
- Frontend and docs: `app/src/lib/skillPresentation.ts`, `website/public/docs/persona.en.md`, `website/public/docs/persona.zh.md`, `website/public/docs/multi-agent.en.md`, `website/public/docs/multi-agent.zh.md`, `website/public/docs/skills.en.md`, and `website/public/docs/skills.zh.md`.

`scripts/pack-tauri/qwenpaw.spec` was inspected and intentionally unchanged: it collects the complete skills and markdown trees rather than naming these QA paths. `console/src` and `website/public/release-notes/` were also intentionally left unchanged.

## Migration behavior

Startup now calls `remove_builtin_qa_agent_profiles()` after the default-agent migrations. It:

- removes the two obsolete profile IDs from `config.agents.profiles`;
- moves `active_agent` through `_fallback_active_agent_id()` when it points at a removed profile;
- logs each retained workspace path and does not delete workspace data;
- calls `save_config()` only when a profile was removed, so repeated startup is idempotent.

The default agent is now the only core startup phase. Its successful startup triggers `on_core_ready`; custom agents then start with the existing bounded concurrency. The generic `guidance` skill and old console multi-agent mechanisms remain available.

## Verification

- Targeted affected tests: `./.venv/bin/pytest tests/unit/app/test_multi_agent_manager_startup.py tests/unit/app/test_migration.py tests/unit/cli/test_cli_agents.py tests/unit/agents/utils/test_setup_utils.py tests/unit/app/test_agents_workspace_initialization.py -q` — **50 passed**.
- Frontend: `npx tsc --noEmit` — **passed**.
- Frontend: `npx vitest run` — **22 files, 133 tests passed**.
- Full unit suite via the existing environment: `./.venv/bin/pytest tests/unit -q` — **5566 passed, 8 skipped, 8 failed**. All failures were environment-sensitive local socket/port tests: five OneBot lifecycle/watchdog tests, two llama.cpp server setup tests, and one Tauri socket test; the sandbox rejects local binds with `PermissionError: [Errno 1] Operation not permitted`.
- Integration: `./.venv/bin/pytest tests/integration/test_agents.py -q` — **4 setup errors** because the `app_server` fixture cannot bind `127.0.0.1` in this sandbox, with the same `PermissionError`.
- `git diff --check` — **passed**.
- Pre-commit: AST, encoding, private-key, whitespace, trailing-comma, and flake8 hooks passed; Black reformatted `src/qwenpaw/app/multi_agent_manager.py`. The overall run remained non-green because mypy/pylint use the repository's Python 3.9 check against existing `X | Y` annotations, pylint cannot write its cache under `~/Library/Caches`, and pylint also reports pre-existing provider import issues.

The requested `uv run ...` commands could not start in this environment: the first attempt was blocked by access to the existing uv cache, the escalation request was rejected by the environment policy, and an offline uv retry hit a uv runtime panic. The repository's existing `.venv` was used for the equivalent pytest commands above.

## Residual scan

The requested scan, excluding no source directories, now reports only the intentional migration literals:

```text
src/qwenpaw/app/migration.py:733:            "QwenPaw_QA_Agent_0.2",
src/qwenpaw/app/migration.py:734:            "CoPaw_QA_Agent_0.1beta1",
```

Historical `app/docs/*report` and `*brief` files may still mention the old agent for historical context; they were not part of the requested source/docs scan and were intentionally not edited.

No git commit was created.
