# -*- coding: utf-8 -*-
import os
from pathlib import Path
from dotenv import load_dotenv

from .user_home import (
    is_working_data_home,
    resolve_secret_dir,
    resolve_working_dir,
)


def user_env_file_paths() -> tuple[Path, ...]:
    """User-level secret files. ``~/.potato/.env`` is the canonical one."""
    home = Path.home()
    return (
        home / ".potato" / ".env",
        home / ".qwenpaw" / ".env",
        home / ".copaw" / ".env",
    )


def repo_env_file_path() -> Path:
    """Checkout ``.env`` — used by local `qwenpaw app`, not the .app."""
    return Path(__file__).resolve().parent.parent.parent / ".env"


def load_dotenv_files() -> list[Path]:
    """Load user then repo ``.env`` files. Already-set keys win.

    Desktop Potato.app has no repo ``.env``. Secrets belong in
    ``~/.potato/.env``. Creating that file must not steal
    ``WORKING_DIR`` from an existing ``~/.qwenpaw``.
    """
    loaded: list[Path] = []
    # Isolated runtimes (pytest, POTATO_WORKING_DIR=tmp) must not inherit
    # the developer's ~/.potato/.env. Desktop .app does not set this.
    if os.environ.get("POTATO_WORKING_DIR"):
        work_env = (
            Path(os.environ["POTATO_WORKING_DIR"]).expanduser() / ".env"
        )
        candidates = (work_env, repo_env_file_path())
    else:
        candidates = (*user_env_file_paths(), repo_env_file_path())
    for path in candidates:
        if path.is_file():
            load_dotenv(path, override=False)
            loaded.append(path)
    return loaded


load_dotenv_files()


def _get_env(key: str, default: str = "") -> str:
    """Look up an env var with QwenPaw and CoPaw legacy fallbacks.

    Primary key is always used as-is.  When the primary key starts with
    ``POTATO_``, the corresponding ``QWENPAW_`` and ``COPAW_`` variants are
    checked in that order so existing deployments keep working.
    """
    if key in os.environ:
        return os.environ[key]
    if key.startswith("POTATO_"):
        suffix = key[len("POTATO_"):]
        for prefix in ("QWENPAW_", "COPAW_"):
            legacy_key = prefix + suffix
            if legacy_key in os.environ:
                return os.environ[legacy_key]
    return default


class EnvVarLoader:
    """Utility to load and parse environment variables with type safety
    and defaults. Pass POTATO_* keys; QWENPAW_* and COPAW_* legacy variants are
    checked automatically as a fallback inside _get_env.
    """

    @staticmethod
    def get_bool(env_var: str, default: bool = False) -> bool:
        """Get a boolean environment variable,
        interpreting common truthy values."""
        val = _get_env(env_var, str(default)).lower()
        return val in ("true", "1", "yes")

    @staticmethod
    def get_float(
        env_var: str,
        default: float = 0.0,
        min_value: float | None = None,
        max_value: float | None = None,
        allow_inf: bool = False,
    ) -> float:
        """Get a float environment variable with optional bounds
        and infinity handling."""
        try:
            value = float(_get_env(env_var, str(default)))
            if min_value is not None and value < min_value:
                return min_value
            if max_value is not None and value > max_value:
                return max_value
            if not allow_inf and (
                value == float("inf") or value == float("-inf")
            ):
                return default
            return value
        except (TypeError, ValueError):
            return default

    @staticmethod
    def get_int(
        env_var: str,
        default: int = 0,
        min_value: int | None = None,
        max_value: int | None = None,
    ) -> int:
        """Get an integer environment variable with optional bounds."""
        try:
            value = int(_get_env(env_var, str(default)))
            if min_value is not None and value < min_value:
                return min_value
            if max_value is not None and value > max_value:
                return max_value
            return value
        except (TypeError, ValueError):
            return default

    @staticmethod
    def get_str(env_var: str, default: str = "") -> str:
        """Get a string environment variable with a default fallback."""
        return _get_env(env_var, default)


CUSTOM_AGENT_STARTUP_CONCURRENCY_ENV = (
    "POTATO_CUSTOM_AGENT_STARTUP_CONCURRENCY"
)
DEFAULT_CUSTOM_AGENT_STARTUP_CONCURRENCY = 5
CUSTOM_AGENT_STARTUP_CONCURRENCY = EnvVarLoader.get_int(
    CUSTOM_AGENT_STARTUP_CONCURRENCY_ENV,
    default=DEFAULT_CUSTOM_AGENT_STARTUP_CONCURRENCY,
    min_value=1,
)


# WORKING_DIR / SECRET_DIR: old installs keep ~/.qwenpaw; new ones
# get ~/.potato only. See resolve_working_dir.
WORKING_DIR = resolve_working_dir(explicit=_get_env("POTATO_WORKING_DIR"))
SECRET_DIR = resolve_secret_dir(
    WORKING_DIR,
    explicit=EnvVarLoader.get_str("POTATO_SECRET_DIR", ""),
)

# Env key for overriding the OS keychain account used for the master key.
KEYRING_ACCOUNT_ENV = "POTATO_KEYRING_ACCOUNT"

PROJECT_NAME = "Potato"

# Message metadata tags shared across agent middleware and memory managers.
POTATO_MESSAGE_TAG_KEY = "potato_tag"
QWENPAW_MESSAGE_TAG_KEY = "qwenpaw_tag"


def get_message_tag(metadata: object) -> object | None:
    """Read a Potato message tag, falling back to QwenPaw session data."""

    if not isinstance(metadata, dict):
        return None
    return metadata.get(POTATO_MESSAGE_TAG_KEY) or metadata.get(
        QWENPAW_MESSAGE_TAG_KEY,
    )


SCROLL_MEMORY_MESSAGE_TAG = "scroll_memory"
AUTO_MEMORY_SEARCH_BLOCK_IDS_KEY = "auto_memory_search_block_ids"
EXTERNAL_USER_QUERY_MESSAGE_TAG = "external_user_query"
AUTO_CONTINUE_MESSAGE_TAG = "auto_continue"
LOOP_CONTINUATION_MESSAGE_TAG = "loop_continuation"
RUBRIC_EVALUATION_MESSAGE_TAG = "rubric_evaluation"
RUNTIME_CONTEXT_MESSAGE_TAG = "runtime_context"
# User-role messages the runtime injects to keep a turn going. They are NOT
# new requests: the scroll active-turn anchor (live scan + SQL floor) must
# skip them, or the anchor jumps to the stub and the REAL request becomes
# evictable/searchable again (the #5746 failure mode, loop-session flavor).
# ``runtime_context`` is an append-only env/policy snapshot, not a user turn.
SYNTHETIC_USER_MESSAGE_TAGS = frozenset(
    {
        AUTO_CONTINUE_MESSAGE_TAG,
        LOOP_CONTINUATION_MESSAGE_TAG,
        RUBRIC_EVALUATION_MESSAGE_TAG,
        RUNTIME_CONTEXT_MESSAGE_TAG,
    },
)
AUTO_MEMORY_SEARCH_TEXT = (
    "I'll check memory for relevant context before answering."
)
AUTO_MEMORY_SEARCH_THINKING_PREFIX = (
    "I should search long-term memory before answering."
)

# Subdirectory name inside each agent's workspace that holds cloned / imported
# coding projects.
# Full path = <workspace_dir> / CODING_PROJECT_SUBDIR / <name>
CODING_PROJECT_SUBDIR = "coding_projects"


def _resolve_docs_dir() -> Path | None:
    """Find Potato documentation directory across all install methods."""
    _pkg_docs = Path(__file__).resolve().parent / "docs"
    if _pkg_docs.is_dir() and any(_pkg_docs.glob("*.md")):
        return _pkg_docs
    _src_docs = (
        Path(__file__).resolve().parents[2] / "website" / "public" / "docs"
    )
    if _src_docs.is_dir() and any(_src_docs.glob("*.md")):
        return _src_docs
    return None


DOCS_DIR: Path | None = _resolve_docs_dir()

# Default media directory for channels (cross-platform)
DEFAULT_MEDIA_DIR = WORKING_DIR / "media"

# Default local provider directory
DEFAULT_LOCAL_PROVIDER_DIR = WORKING_DIR / "local_models"

JOBS_FILE = EnvVarLoader.get_str("POTATO_JOBS_FILE", "jobs.json")

CHATS_FILE = EnvVarLoader.get_str("POTATO_CHATS_FILE", "chats.json")


# Builtin Q&A helper profile.  agent_id keeps "Potato" prefix for existing
# workspaces and agent.json; do not rename.
def _discover_agent_languages() -> frozenset[str]:
    md_root = Path(__file__).resolve().parent / "agents" / "md_files"
    if md_root.is_dir():
        langs = {
            d.name
            for d in md_root.iterdir()
            if d.is_dir()
            and not d.name.startswith(".")
            and any(d.glob("*.md"))
        }
        if langs:
            return frozenset(langs)
    return frozenset({"en", "zh", "ru"})


SUPPORTED_AGENT_LANGUAGES: frozenset[str] = _discover_agent_languages()

TOKEN_USAGE_FILE = EnvVarLoader.get_str(
    "POTATO_TOKEN_USAGE_FILE",
    "token_usage.json",
)

CONFIG_FILE = EnvVarLoader.get_str("POTATO_CONFIG_FILE", "config.json")

HEARTBEAT_FILE = EnvVarLoader.get_str("POTATO_HEARTBEAT_FILE", "HEARTBEAT.md")
HEARTBEAT_DEFAULT_EVERY = "6h"
HEARTBEAT_DEFAULT_TARGET = "main"
HEARTBEAT_DEFAULT_TIMEOUT_SECONDS = 300
HEARTBEAT_MAX_TIMEOUT_SECONDS = 3600
HEARTBEAT_TARGET_LAST = "last"

# Debug history file for /dump_history and /load_history commands
DEBUG_HISTORY_FILE = EnvVarLoader.get_str(
    "POTATO_DEBUG_HISTORY_FILE",
    "debug_history.jsonl",
)
MAX_LOAD_HISTORY_COUNT = 10000

# Env key for app log level (used by CLI and app load for reload child).
LOG_LEVEL_ENV = "POTATO_LOG_LEVEL"

# Fixed desktop backend port. When set, get_stable_port() uses this port
# instead of auto-assigning.
POTATO_DESKTOP_PORT = _get_env("POTATO_DESKTOP_PORT")

# Env to indicate running inside a container (e.g. Docker). Set to 1/true/yes.
RUNNING_IN_CONTAINER = EnvVarLoader.get_bool(
    "POTATO_RUNNING_IN_CONTAINER",
    False,
)

# Timeout in seconds for checking if a provider is reachable.
MODEL_PROVIDER_CHECK_TIMEOUT = EnvVarLoader.get_float(
    "POTATO_MODEL_PROVIDER_CHECK_TIMEOUT",
    5.0,
    min_value=0,
    allow_inf=False,
)

# Playwright: use system Chromium when set (e.g. in Docker).
PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH_ENV = "PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH"

# When True, expose /docs, /redoc, /openapi.json
# (dev only; keep False in prod).
DOCS_ENABLED = EnvVarLoader.get_bool("POTATO_OPENAPI_DOCS", False)

# Memory directory
MEMORY_DIR = WORKING_DIR / "memory"

# Backup directory
BACKUP_DIR = (
    Path(
        EnvVarLoader.get_str(
            "POTATO_BACKUP_DIR",
            f"{WORKING_DIR}.backups",
        ),
    )
    .expanduser()
    .resolve()
)


# Plugin directory (installed via `potato plugin install`)
PLUGINS_DIR = WORKING_DIR / "plugins"

# Local models directory
MODELS_DIR = WORKING_DIR / "models"

MEMORY_COMPACT_KEEP_RECENT = EnvVarLoader.get_int(
    "POTATO_MEMORY_COMPACT_KEEP_RECENT",
    3,
    min_value=0,
)

# Memory compaction configuration
MEMORY_COMPACT_RATIO = EnvVarLoader.get_float(
    "POTATO_MEMORY_COMPACT_RATIO",
    0.7,
    min_value=0,
    allow_inf=False,
)

# CORS configuration — comma-separated list of allowed origins for dev mode.
# Example: POTATO_CORS_ORIGINS="http://localhost:5173,http://127.0.0.1:5173"
# When unset, CORS middleware is not applied.
CORS_ORIGINS = EnvVarLoader.get_str("POTATO_CORS_ORIGINS", "").strip()

# Default finite limit used by upload endpoints when no override is configured.
DEFAULT_UPLOAD_MAX_SIZE_MB = 200

# Optional upload size override (MB).  None = use the endpoint default.
UPLOAD_MAX_SIZE_MB: int | None = (
    int(v)
    if (v := EnvVarLoader.get_str("POTATO_UPLOAD_MAX_SIZE_MB", ""))
    .strip()
    .isdigit()
    else None
)

# LLM API retry configuration
LLM_MAX_RETRIES = EnvVarLoader.get_int(
    "POTATO_LLM_MAX_RETRIES",
    3,
    min_value=0,
)

LLM_BACKOFF_BASE = EnvVarLoader.get_float(
    "POTATO_LLM_BACKOFF_BASE",
    1.0,
    min_value=0.1,
)

LLM_BACKOFF_CAP = EnvVarLoader.get_float(
    "POTATO_LLM_BACKOFF_CAP",
    10.0,
    min_value=0.5,
)

# LLM concurrency control
# Maximum number of concurrent in-flight LLM calls; excess requests wait on
# the semaphore.  Tune to your API quota: start conservatively at 3-5 and
# increase (e.g. OpenAI Tier 1 ~500 QPM allows ~25 at 3 s/call average).
LLM_MAX_CONCURRENT = EnvVarLoader.get_int(
    "POTATO_LLM_MAX_CONCURRENT",
    10,
    min_value=1,
)

# Maximum queries per minute (QPM), enforced via a 60-second sliding window.
# New requests that would exceed this limit will wait before being dispatched
# to the API — proactively preventing 429s rather than reacting to them.
# 0 = unlimited (disabled).
# Examples: Anthropic Tier-1 ≈ 50 QPM; OpenAI Tier-1 ≈ 500 QPM.
LLM_MAX_QPM = EnvVarLoader.get_int(
    "POTATO_LLM_MAX_QPM",
    600,
    min_value=0,
)

# Default global pause duration (seconds) applied to all waiters when a 429
# is received.  Overridden by the API's Retry-After header when present.
LLM_RATE_LIMIT_PAUSE = EnvVarLoader.get_float(
    "POTATO_LLM_RATE_LIMIT_PAUSE",
    5.0,
    min_value=1.0,
)

# Random jitter range (seconds) added on top of the pause remaining time so
# concurrent waiters stagger their wake-up and avoid a new burst.
LLM_RATE_LIMIT_JITTER = EnvVarLoader.get_float(
    "POTATO_LLM_RATE_LIMIT_JITTER",
    1.0,
    min_value=0.0,
)

# Maximum time (seconds) a caller will wait for a semaphore slot before
# giving up with a RuntimeError rather than blocking indefinitely.
LLM_ACQUIRE_TIMEOUT = EnvVarLoader.get_float(
    "POTATO_LLM_ACQUIRE_TIMEOUT",
    300.0,
    min_value=10.0,
)

# Tool guard approval timeout (seconds).
try:
    TOOL_GUARD_APPROVAL_TIMEOUT_SECONDS = max(
        float(
            _get_env("POTATO_TOOL_GUARD_APPROVAL_TIMEOUT_SECONDS", "300"),
        ),
        1.0,
    )
except (TypeError, ValueError):
    TOOL_GUARD_APPROVAL_TIMEOUT_SECONDS = 300.0


# Tool guard approval heartbeat interval (seconds).
# Sends periodic heartbeat messages during approval wait to keep SSE
# connection alive. Should be less than browser/proxy timeout (30-60s).
try:
    TOOL_GUARD_APPROVAL_HEARTBEAT_INTERVAL = max(
        float(
            _get_env("POTATO_TOOL_GUARD_APPROVAL_HEARTBEAT_INTERVAL", "15"),
        ),
        5.0,
    )
except (TypeError, ValueError):
    TOOL_GUARD_APPROVAL_HEARTBEAT_INTERVAL = 15.0

# Marker prepended to every truncation notice.
# Format:
#   <<<TRUNCATED>>>
#   The output above was truncated.
#   The full content is saved to the file and contains Z lines in total.
#   This excerpt starts at line X and covers the next N bytes.
#   If the current content is not enough, call `read_file` with
#   file_path=<path> start_line=Y to read more.
#
# Split output on this marker to recover the original (untruncated) portion:
#   original = output.split(TRUNCATION_NOTICE_MARKER)[0]
TRUNCATION_NOTICE_MARKER = "<<<TRUNCATED>>>"

# Placeholder text used when media blocks are stripped from messages
# because the model does not support multimodal content.
MEDIA_UNSUPPORTED_PLACEHOLDER = (
    "[Media content removed - model does not support this media type]"
)
