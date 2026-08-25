# -*- coding: utf-8 -*-
"""Plaintext user dotenv at ``~/.potato/.env``.

Desktop Potato.app cannot see the git checkout ``.env``. All process-level
secrets belong here: Exa, speech, Tavily, and provider API keys
(``SUB2API_API_KEY``, ``DEEPSEEK_API_KEY``, …). Family installs do not
need the OS keychain.
"""
from __future__ import annotations

import os
from pathlib import Path

from potato.constant import repo_env_file_path, user_env_file_paths

_DOTENV_HEADER = (
    "# Potato user secrets. Desktop Potato.app reads this file.\n"
    "# chmod 600. Do not commit.\n"
)

# Legacy / provision aliases → the names the runtime actually reads.
_ALIASES = {
    "apikey": "VOLCENGINE_SPEECH_API_KEY",
    "APIKEY": "VOLCENGINE_SPEECH_API_KEY",
    "new_api_key": "VOLCENGINE_SPEECH_API_KEY",
    "keyid": "VOLCENGINE_SPEECH_APP_ID",
    "KEYID": "VOLCENGINE_SPEECH_APP_ID",
}

_SKIP_KEYS = frozenset(
    {
        "POTATO_WORKING_DIR",
        "POTATO_SECRET_DIR",
        "POTATO_KEYRING_ACCOUNT",
    },
)


def potato_dotenv_path() -> Path:
    return Path.home() / ".potato" / ".env"


def should_touch_user_dotenv() -> bool:
    """Tests set POTATO_WORKING_DIR and must not rewrite ~/.potato/.env."""
    return not bool(os.environ.get("POTATO_WORKING_DIR", "").strip())


def parse_dotenv(path: Path) -> dict[str, str]:
    if not path.is_file():
        return {}
    out: dict[str, str] = {}
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        key = key.strip()
        if not key:
            continue
        out[key] = value.strip().strip("'").strip('"')
    return out


def canonicalize_env_keys(envs: dict[str, str]) -> dict[str, str]:
    """Copy values onto canonical names. Existing canonical keys win."""
    result = {
        key: value
        for key, value in envs.items()
        if key not in _SKIP_KEYS and str(value).strip()
    }
    for alias, canonical in _ALIASES.items():
        value = (envs.get(alias) or "").strip()
        if value and not (result.get(canonical) or "").strip():
            result[canonical] = value
    for alias in _ALIASES:
        result.pop(alias, None)
    return result


def upsert_dotenv(path: Path, updates: dict[str, str]) -> list[str]:
    """Append missing keys. Never overwrite a non-empty existing value."""
    updates = {
        key: value.strip()
        for key, value in canonicalize_env_keys(updates).items()
        if value.strip() and key not in _ALIASES
    }
    existing = parse_dotenv(path)
    added: list[str] = []
    lines: list[str]
    if path.is_file():
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except OSError:
            lines = [_DOTENV_HEADER.rstrip(), ""]
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        lines = [_DOTENV_HEADER.rstrip(), ""]

    for key, value in sorted(updates.items()):
        if (existing.get(key) or "").strip():
            continue
        lines.append(f"{key}={value}")
        existing[key] = value
        added.append(key)

    if added or not path.is_file():
        text = "\n".join(lines).rstrip() + "\n"
        path.write_text(text, encoding="utf-8")
        try:
            os.chmod(path, 0o600)
        except OSError:
            pass
    return added


def overwrite_dotenv(path: Path, updates: dict[str, str]) -> list[str]:
    """Replace the explicitly supplied keys and preserve everything else.

    Provisioning files are an intentional configuration hand-off.  Their
    values must win over an older family install's ``.env``; otherwise the
    environment layer would silently shadow the newly encrypted provider
    configuration on every later launch.
    """
    updates = {
        key: value.strip()
        for key, value in canonicalize_env_keys(updates).items()
        if value.strip() and key not in _ALIASES
    }
    if not updates:
        return []

    if path.is_file():
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except OSError:
            lines = [_DOTENV_HEADER.rstrip(), ""]
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        lines = [_DOTENV_HEADER.rstrip(), ""]

    kept: list[str] = []
    for line in lines:
        stripped = line.strip()
        if stripped and not stripped.startswith("#") and "=" in stripped:
            name = stripped.partition("=")[0].strip()
            if name in updates:
                continue
        kept.append(line)
    for key, value in sorted(updates.items()):
        kept.append(f"{key}={value}")

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(kept).rstrip() + "\n", encoding="utf-8")
    try:
        os.chmod(path, 0o600)
    except OSError:
        pass
    return sorted(updates)


def collect_legacy_dotenv_values() -> dict[str, str]:
    """Merge checkout and legacy home dotenv files. First value wins."""
    merged: dict[str, str] = {}
    for path in (*user_env_file_paths(), repo_env_file_path()):
        if path == potato_dotenv_path():
            continue
        for key, value in parse_dotenv(path).items():
            if key in _SKIP_KEYS:
                continue
            if (merged.get(key) or "").strip():
                continue
            if value.strip():
                merged[key] = value.strip()
    return merged


def migrate_env_secrets_to_potato_dotenv(
    extra: dict[str, str] | None = None,
    *,
    dest: Path | None = None,
) -> list[str]:
    """Copy missing env secrets into ``~/.potato/.env``.

    *extra* is typically the decrypted ``envs.json`` mapping. Existing
    dotenv values are kept. Returns the keys that were newly written.
    """
    if dest is None:
        if not should_touch_user_dotenv():
            return []
        dest = potato_dotenv_path()
    incoming = collect_legacy_dotenv_values()
    if extra:
        for key, value in extra.items():
            if key in _SKIP_KEYS:
                continue
            text = str(value).strip()
            if text and not (incoming.get(key) or "").strip():
                incoming[key] = text
    return upsert_dotenv(dest, incoming)


def remove_dotenv_key(path: Path, key: str) -> bool:
    if not path.is_file():
        return False
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return False
    kept: list[str] = []
    removed = False
    for line in lines:
        stripped = line.strip()
        if stripped and not stripped.startswith("#") and "=" in stripped:
            name = stripped.partition("=")[0].strip()
            if name == key:
                removed = True
                continue
        kept.append(line)
    if removed:
        path.write_text("\n".join(kept).rstrip() + "\n", encoding="utf-8")
    return removed
