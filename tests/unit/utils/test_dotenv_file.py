# -*- coding: utf-8 -*-
from __future__ import annotations

from pathlib import Path

from potato.envs.dotenv_file import (
    canonicalize_env_keys,
    overwrite_dotenv,
    parse_dotenv,
    upsert_dotenv,
)


def test_canonicalize_maps_speech_aliases():
    out = canonicalize_env_keys(
        {"apikey": "speech-key", "keyid": "app-1", "EXA_API_KEY": "exa"},
    )
    assert out["VOLCENGINE_SPEECH_API_KEY"] == "speech-key"
    assert out["VOLCENGINE_SPEECH_APP_ID"] == "app-1"
    assert out["EXA_API_KEY"] == "exa"


def test_canonicalize_does_not_override_canonical_key():
    out = canonicalize_env_keys(
        {
            "VOLCENGINE_SPEECH_API_KEY": "keep",
            "apikey": "ignore",
        },
    )
    assert out["VOLCENGINE_SPEECH_API_KEY"] == "keep"


def test_upsert_dotenv_appends_missing_and_keeps_existing(tmp_path: Path):
    dest = tmp_path / ".potato" / ".env"
    dest.parent.mkdir()
    dest.write_text("EXA_API_KEY=old\n", encoding="utf-8")

    added = upsert_dotenv(
        dest,
        {
            "EXA_API_KEY": "new",
            "VOLCENGINE_SPEECH_API_KEY": "speech",
            "apikey": "alias-speech",
        },
    )

    assert added == ["VOLCENGINE_SPEECH_API_KEY"]
    parsed = parse_dotenv(dest)
    assert parsed["EXA_API_KEY"] == "old"
    assert parsed["VOLCENGINE_SPEECH_API_KEY"] == "speech"
    assert dest.stat().st_mode & 0o777 == 0o600


def test_overwrite_dotenv_replaces_only_supplied_keys(tmp_path: Path):
    dest = tmp_path / ".potato" / ".env"
    dest.parent.mkdir()
    dest.write_text(
        "# keep this comment\nEXA_API_KEY=old\nUNRELATED=keep\n",
        encoding="utf-8",
    )

    written = overwrite_dotenv(
        dest,
        {"EXA_API_KEY": "new", "apikey": "new-speech"},
    )

    assert written == ["EXA_API_KEY", "VOLCENGINE_SPEECH_API_KEY"]
    parsed = parse_dotenv(dest)
    assert parsed == {
        "UNRELATED": "keep",
        "EXA_API_KEY": "new",
        "VOLCENGINE_SPEECH_API_KEY": "new-speech",
    }
    assert "# keep this comment" in dest.read_text(encoding="utf-8")
