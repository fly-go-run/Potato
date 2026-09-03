# -*- coding: utf-8 -*-
"""Approval endpoint ownership and stale-action handling."""
from __future__ import annotations

from types import SimpleNamespace

import pytest
from fastapi import HTTPException

from potato.app.routers import approval as approval_router


class _FakeApprovalService:
    def __init__(self, *, resolved=None) -> None:
        self.pending = SimpleNamespace(
            request_id="req-1",
            root_session_id="root-1",
            user_id="alice",
            tool_name="Bash",
        )
        self.resolved = resolved

    async def get_request(self, _request_id):  # noqa: ANN001
        return self.pending

    async def resolve_request(self, *_args, **_kwargs):  # noqa: ANN001
        return self.resolved


async def test_approve_rejects_another_user(monkeypatch) -> None:
    service = _FakeApprovalService(resolved=object())
    monkeypatch.setattr(
        approval_router,
        "get_approval_service",
        lambda: service,
    )

    with pytest.raises(HTTPException) as exc_info:
        await approval_router.post_approval_approve(
            None,  # type: ignore[arg-type]
            approval_router.ApprovalActionRequest(
                request_id="req-1",
                session_id="root-1",
                user_id="bob",
                scope="exact",
            ),
        )

    assert exc_info.value.status_code == 403
    assert "another user's" in str(exc_info.value.detail)


async def test_double_approve_returns_conflict_instead_of_500(
    monkeypatch,
) -> None:
    service = _FakeApprovalService(resolved=None)
    monkeypatch.setattr(
        approval_router,
        "get_approval_service",
        lambda: service,
    )

    with pytest.raises(HTTPException) as exc_info:
        await approval_router.post_approval_approve(
            None,  # type: ignore[arg-type]
            approval_router.ApprovalActionRequest(
                request_id="req-1",
                session_id="root-1",
                user_id="alice",
                scope="exact",
            ),
        )

    assert exc_info.value.status_code == 409
    assert "already resolved" in str(exc_info.value.detail)
