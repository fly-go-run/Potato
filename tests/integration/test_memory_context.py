# -*- coding: utf-8 -*-
"""Integration tests for memory, system-prompt-files, and
running-config.

Sprint 2.3 — Memory & Context.  All tests use the real ``potato app``
subprocess; A/B classes are pure HTTP.

Existing CRUD roundtrip coverage (not duplicated here):
  - ``test_workspace_files.py``  — memory PUT/GET happy path
  - ``test_workspace_agent_settings.py``  — scoped memory + sys-prompt
  - ``test_workspace_running_config.py``  — running-config roundtrip
"""
from __future__ import annotations

import pytest

from helpers import (
    create_agent,
    default_http_timeout,
    delete_agent_quietly,
    scoped,
    toggle_agent,
)

_HTTP_TIMEOUT = default_http_timeout(15.0)


# ================================================================== #
#  A class — Memory file depth (6 tests)
# ================================================================== #


@pytest.mark.integration
@pytest.mark.p2
def test_memory_file_get_nonexistent_returns_404(
    app_server,
) -> None:
    """GET a memory file that does not exist → 404."""
    resp = app_server.api_request(
        "GET",
        scoped("default", "/workspace/memory/nonexistent_integ.md"),
        timeout=_HTTP_TIMEOUT,
    )
    assert resp.status_code == 404, app_server.logs_tail()


@pytest.mark.integration
@pytest.mark.p0
def test_memory_file_cross_agent_isolated(app_server) -> None:
    """A memory file written to agent_a is not visible to agent_b."""
    agent_a = "integ_mc_iso_a"
    agent_b = "integ_mc_iso_b"
    create_agent(app_server, agent_a)
    create_agent(app_server, agent_b)
    try:
        put_resp = app_server.api_request(
            "PUT",
            scoped(agent_a, "/workspace/memory/isolated_note.md"),
            json={"content": "agent_a private data"},
            timeout=_HTTP_TIMEOUT,
        )
        assert put_resp.status_code == 200, app_server.logs_tail()

        get_b = app_server.api_request(
            "GET",
            scoped(agent_b, "/workspace/memory/isolated_note.md"),
            timeout=_HTTP_TIMEOUT,
        )
        assert (
            get_b.status_code == 404
        ), "agent_b must not see agent_a's memory file"

        get_a = app_server.api_request(
            "GET",
            scoped(agent_a, "/workspace/memory/isolated_note.md"),
            timeout=_HTTP_TIMEOUT,
        )
        assert get_a.status_code == 200
        assert "agent_a private data" in get_a.json()["content"]
    finally:
        delete_agent_quietly(app_server, agent_a)
        delete_agent_quietly(app_server, agent_b)


@pytest.mark.integration
@pytest.mark.p1
def test_memory_file_unicode_content_roundtrip(app_server) -> None:
    """Chinese + emoji content survives PUT → GET roundtrip."""
    content = "你好世界 Hello 🌍 — 测试内容"
    resp = app_server.api_request(
        "PUT",
        scoped("default", "/workspace/memory/unicode_test.md"),
        json={"content": content},
        timeout=_HTTP_TIMEOUT,
    )
    assert resp.status_code == 200, app_server.logs_tail()

    get_resp = app_server.api_request(
        "GET",
        scoped("default", "/workspace/memory/unicode_test.md"),
        timeout=_HTTP_TIMEOUT,
    )
    assert get_resp.status_code == 200
    assert get_resp.json()["content"].strip() == content


@pytest.mark.integration
@pytest.mark.p1
def test_memory_file_persists_after_agent_disable_enable(
    app_server,
) -> None:
    """Memory file survives a disable → re-enable cycle."""
    agent_id = "integ_mc_persist"
    create_agent(app_server, agent_id)
    try:
        app_server.api_request(
            "PUT",
            scoped(agent_id, "/workspace/memory/persist_check.md"),
            json={"content": "persist me"},
            timeout=_HTTP_TIMEOUT,
        )

        toggle_agent(app_server, agent_id, False)
        toggle_agent(app_server, agent_id, True)

        get_resp = app_server.api_request(
            "GET",
            scoped(agent_id, "/workspace/memory/persist_check.md"),
            timeout=_HTTP_TIMEOUT,
        )
        assert get_resp.status_code == 200
        assert get_resp.json()["content"].strip() == "persist me"
    finally:
        delete_agent_quietly(app_server, agent_id)


@pytest.mark.integration
@pytest.mark.p1
def test_memory_file_overwrite_preserves_sibling_files(
    app_server,
) -> None:
    """Overwriting one memory file does not affect siblings."""
    app_server.api_request(
        "PUT",
        scoped("default", "/workspace/memory/sibling_a.md"),
        json={"content": "aaa"},
        timeout=_HTTP_TIMEOUT,
    )
    app_server.api_request(
        "PUT",
        scoped("default", "/workspace/memory/sibling_b.md"),
        json={"content": "bbb"},
        timeout=_HTTP_TIMEOUT,
    )

    app_server.api_request(
        "PUT",
        scoped("default", "/workspace/memory/sibling_a.md"),
        json={"content": "aaa_updated"},
        timeout=_HTTP_TIMEOUT,
    )

    get_b = app_server.api_request(
        "GET",
        scoped("default", "/workspace/memory/sibling_b.md"),
        timeout=_HTTP_TIMEOUT,
    )
    assert get_b.status_code == 200
    assert get_b.json()["content"].strip() == "bbb"


@pytest.mark.integration
@pytest.mark.p2
def test_memory_file_list_metadata_fields_complete(
    app_server,
) -> None:
    """MdFileInfo contains all five documented fields with valid types."""
    app_server.api_request(
        "PUT",
        scoped("default", "/workspace/memory/meta_probe.md"),
        json={"content": "metadata test"},
        timeout=_HTTP_TIMEOUT,
    )

    list_resp = app_server.api_request(
        "GET",
        scoped("default", "/workspace/memory"),
        timeout=_HTTP_TIMEOUT,
    )
    assert list_resp.status_code == 200
    files = list_resp.json()
    assert isinstance(files, list)

    target = [f for f in files if f.get("filename") == "meta_probe.md"]
    assert len(target) == 1, f"meta_probe.md not in list: {files}"
    info = target[0]

    assert isinstance(info["filename"], str)
    assert isinstance(info["path"], str)
    assert isinstance(info["size"], int) and info["size"] > 0
    assert isinstance(info["created_time"], str) and info["created_time"]
    assert isinstance(info["modified_time"], str) and info["modified_time"]


# ================================================================== #
#  B class — System-prompt-files + context config depth (5 tests)
# ================================================================== #


@pytest.mark.integration
@pytest.mark.p1
def test_system_prompt_files_global_put_get_roundtrip(
    app_server,
) -> None:
    """PUT/GET system-prompt-files via X-Agent-Id header route."""
    get_before = app_server.api_request(
        "GET",
        "/api/workspace/system-prompt-files",
        headers={"X-Agent-Id": "default"},
        timeout=_HTTP_TIMEOUT,
    )
    assert get_before.status_code == 200, app_server.logs_tail()
    before = get_before.json()
    assert isinstance(before, list)

    reversed_list = list(reversed(before))
    try:
        put_resp = app_server.api_request(
            "PUT",
            "/api/workspace/system-prompt-files",
            json=reversed_list,
            headers={"X-Agent-Id": "default"},
            timeout=_HTTP_TIMEOUT,
        )
        assert put_resp.status_code == 200

        get_after = app_server.api_request(
            "GET",
            "/api/workspace/system-prompt-files",
            headers={"X-Agent-Id": "default"},
            timeout=_HTTP_TIMEOUT,
        )
        assert get_after.status_code == 200
        assert get_after.json() == reversed_list
    finally:
        app_server.api_request(
            "PUT",
            "/api/workspace/system-prompt-files",
            json=before,
            headers={"X-Agent-Id": "default"},
            timeout=_HTTP_TIMEOUT,
        )


@pytest.mark.integration
@pytest.mark.p0
def test_system_prompt_files_cross_agent_isolated(
    app_server,
) -> None:
    """Modifying agent_a's prompt-files list does not change agent_b."""
    agent_a = "integ_mc_spf_a"
    agent_b = "integ_mc_spf_b"
    create_agent(app_server, agent_a)
    create_agent(app_server, agent_b)
    try:
        base_a = app_server.api_request(
            "GET",
            scoped(agent_a, "/workspace/system-prompt-files"),
            timeout=_HTTP_TIMEOUT,
        ).json()

        base_b = app_server.api_request(
            "GET",
            scoped(agent_b, "/workspace/system-prompt-files"),
            timeout=_HTTP_TIMEOUT,
        ).json()

        modified = list(base_a) + ["CUSTOM_INTEG.md"]
        app_server.api_request(
            "PUT",
            scoped(agent_a, "/workspace/system-prompt-files"),
            json=modified,
            timeout=_HTTP_TIMEOUT,
        )

        after_b = app_server.api_request(
            "GET",
            scoped(agent_b, "/workspace/system-prompt-files"),
            timeout=_HTTP_TIMEOUT,
        ).json()

        assert (
            after_b == base_b
        ), "agent_b prompt-files changed after agent_a modification"

        after_a = app_server.api_request(
            "GET",
            scoped(agent_a, "/workspace/system-prompt-files"),
            timeout=_HTTP_TIMEOUT,
        ).json()
        assert "CUSTOM_INTEG.md" in after_a
    finally:
        app_server.api_request(
            "PUT",
            scoped(agent_a, "/workspace/system-prompt-files"),
            json=base_a,
            timeout=_HTTP_TIMEOUT,
        )
        delete_agent_quietly(app_server, agent_a)
        delete_agent_quietly(app_server, agent_b)


@pytest.mark.integration
@pytest.mark.p1
def test_running_config_approval_level_writeback_to_profile(
    app_server,
) -> None:
    """PUT running-config with approval_level writes it back to the
    agent profile (workspace.py:932-933)."""
    get_before = app_server.api_request(
        "GET",
        scoped("default", "/workspace/running-config"),
        timeout=_HTTP_TIMEOUT,
    )
    assert get_before.status_code == 200
    before = get_before.json()
    original_level = before.get("approval_level", "AUTO")

    try:
        updated = dict(before)
        updated["approval_level"] = "CONFIRM"
        put_resp = app_server.api_request(
            "PUT",
            scoped("default", "/workspace/running-config"),
            json=updated,
            timeout=_HTTP_TIMEOUT,
        )
        assert put_resp.status_code == 200

        profile_resp = app_server.api_request(
            "GET",
            "/api/agents/default",
            timeout=_HTTP_TIMEOUT,
        )
        assert profile_resp.status_code == 200
        profile = profile_resp.json()
        assert (
            profile.get("approval_level") == "CONFIRM"
        ), "approval_level not written back to agent profile"
    finally:
        restore = dict(before)
        restore["approval_level"] = original_level
        app_server.api_request(
            "PUT",
            scoped("default", "/workspace/running-config"),
            json=restore,
            timeout=_HTTP_TIMEOUT,
        )


@pytest.mark.integration
@pytest.mark.p1
def test_running_config_context_compact_fields_roundtrip(
    app_server,
) -> None:
    """Modify light_context_config.context_compact_config fields and
    verify they persist on readback."""
    get_before = app_server.api_request(
        "GET",
        scoped("default", "/workspace/running-config"),
        timeout=_HTTP_TIMEOUT,
    )
    assert get_before.status_code == 200
    before = get_before.json()

    try:
        updated = dict(before)
        lcc = dict(updated.get("light_context_config") or {})
        ccc = dict(lcc.get("context_compact_config") or {})
        ccc["compact_threshold_ratio"] = 0.5
        lcc["context_compact_config"] = ccc
        updated["light_context_config"] = lcc

        put_resp = app_server.api_request(
            "PUT",
            scoped("default", "/workspace/running-config"),
            json=updated,
            timeout=_HTTP_TIMEOUT,
        )
        assert put_resp.status_code == 200

        get_after = app_server.api_request(
            "GET",
            scoped("default", "/workspace/running-config"),
            timeout=_HTTP_TIMEOUT,
        )
        assert get_after.status_code == 200
        after = get_after.json()
        after_ccc = after.get("light_context_config", {}).get(
            "context_compact_config",
            {},
        )
        assert after_ccc.get("compact_threshold_ratio") == 0.5
    finally:
        app_server.api_request(
            "PUT",
            scoped("default", "/workspace/running-config"),
            json=before,
            timeout=_HTTP_TIMEOUT,
        )


@pytest.mark.integration
@pytest.mark.p2
def test_running_config_extra_fields_ignored(
    app_server,
) -> None:
    """PUT with unknown fields succeeds but they are not persisted
    (AgentsRunningConfig uses extra='ignore')."""
    get_before = app_server.api_request(
        "GET",
        scoped("default", "/workspace/running-config"),
        timeout=_HTTP_TIMEOUT,
    )
    assert get_before.status_code == 200
    before = get_before.json()

    try:
        updated = dict(before)
        updated["bogus_field_xyz_integ"] = 42
        put_resp = app_server.api_request(
            "PUT",
            scoped("default", "/workspace/running-config"),
            json=updated,
            timeout=_HTTP_TIMEOUT,
        )
        assert put_resp.status_code == 200

        get_after = app_server.api_request(
            "GET",
            scoped("default", "/workspace/running-config"),
            timeout=_HTTP_TIMEOUT,
        )
        assert get_after.status_code == 200
        assert "bogus_field_xyz_integ" not in get_after.json()
    finally:
        app_server.api_request(
            "PUT",
            scoped("default", "/workspace/running-config"),
            json=before,
            timeout=_HTTP_TIMEOUT,
        )
