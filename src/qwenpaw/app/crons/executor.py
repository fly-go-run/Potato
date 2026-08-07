# -*- coding: utf-8 -*-
from __future__ import annotations

import asyncio
import logging
import uuid
from typing import Any, Dict

from .models import CronJobSpec
from ...security.tool_guard.execution_level import ToolExecutionLevel
from ...schemas import RunStatus

logger = logging.getLogger(__name__)


class CronExecutor:
    def __init__(self, *, workspace: Any, channel_manager: Any):
        self._workspace = workspace
        self._channel_manager = channel_manager

    # pylint: disable=too-many-statements,too-many-branches
    async def execute(self, job: CronJobSpec) -> dict[str, Any]:
        """Execute one job once.

        - task_type text: send fixed text to channel
        - task_type agent: ask agent with prompt, send reply to channel (
            stream_query + send_event)
        - final delivery: consume the stream, then send only the last
            completed message
        - silent agent task: consume the full agent stream without channel
            delivery, while preserving session state
        """
        target_user_id = job.dispatch.target.user_id
        target_session_id = job.dispatch.target.session_id
        target_channel = job.dispatch.channel
        dispatch_meta: Dict[str, Any] = dict(job.dispatch.meta or {})
        if job.task_type == "agent":
            # Agent cron replies still print to the console channel, but
            # should not raise frontend push bubbles.
            dispatch_meta["suppress_console_push"] = True
        logger.info(
            "cron execute: job_id=%s channel=%s task_type=%s "
            "target_user_id=%s target_session_id=%s",
            job.id,
            target_channel,
            job.task_type,
            target_user_id[:40] if target_user_id else "",
            target_session_id[:40] if target_session_id else "",
        )

        if job.task_type == "text" and job.text:
            logger.info(
                "cron send_text: job_id=%s channel=%s len=%s",
                job.id,
                target_channel,
                len(job.text or ""),
            )
            text_delivery_error: str | None = None
            try:
                await self._channel_manager.send_text(
                    channel=target_channel,
                    user_id=target_user_id,
                    session_id=target_session_id,
                    text=job.text.strip(),
                    meta=dispatch_meta,
                )
            except Exception as e:  # pylint: disable=broad-except
                text_delivery_error = repr(e)
                logger.warning(
                    "cron text delivery failed: job_id=%s channel=%s error=%s",
                    job.id,
                    job.dispatch.channel,
                    text_delivery_error,
                )
            return {
                "task_type": "text",
                "run_id": None,
                "final_text": job.text.strip(),
                "delivery_status": (
                    "failed" if text_delivery_error else "success"
                ),
                "delivery_error": text_delivery_error,
            }
        # agent: run request as the dispatch target user so context matches
        logger.info(
            "cron agent: job_id=%s channel=%s stream_query then send_event",
            job.id,
            job.dispatch.channel,
        )
        assert job.request is not None
        req: Dict[str, Any] = job.request.model_dump(mode="json")

        req["channel"] = target_channel
        req["user_id"] = target_user_id or "cron"
        raw_context = req.get("request_context")
        request_context = (
            dict(raw_context) if isinstance(raw_context, dict) else {}
        )
        request_context["source"] = "cron"
        request_context["cron_job_id"] = job.id or ""
        request_context["approval_level"] = (
            ToolExecutionLevel.AUTO.value
            if job.runtime.tool_safety
            else ToolExecutionLevel.OFF.value
        )
        req["request_context"] = request_context

        # Determine session_id based on share_session
        share_session = job.runtime.share_session
        if share_session:
            req["session_id"] = target_session_id or f"cron:{job.id}"
        else:
            # Use job.id (not run_id) so all runs of this job accumulate in the
            # same dedicated session, giving users a complete history.
            req["session_id"] = (
                f"{target_session_id}:cron:{job.id}"
                if target_session_id
                else f"cron:{job.id}"
            )
            req["session_source"] = "cron"

        # Register a ChatSpec so the session appears in the frontend list.
        chat_manager = getattr(self._workspace, "chat_manager", None)
        _chat_spec = None
        if chat_manager is not None:
            try:
                _chat_spec = await chat_manager.get_or_create_chat(
                    session_id=req["session_id"],
                    user_id=req.get("user_id", "cron"),
                    channel=target_channel,
                    name=job.name or f"Cron: {job.id}",
                    source="cron",
                )
            except Exception:
                logger.debug(
                    "cron: failed to register chat spec for job %s",
                    job.id,
                    exc_info=True,
                )

        delivery_error: str | None = None
        run_id = str(uuid.uuid4())

        async def _run() -> None:
            nonlocal delivery_error

            async def _deliver(event: Any) -> None:
                nonlocal delivery_error
                try:
                    await self._channel_manager.send_event(
                        channel=target_channel,
                        user_id=target_user_id,
                        session_id=target_session_id,
                        event=event,
                        meta=dispatch_meta,
                    )
                except Exception as e:  # pylint: disable=broad-except
                    if delivery_error is None:
                        delivery_error = repr(e)
                        logger.warning(
                            "cron agent delivery failed: job_id=%s "
                            "channel=%s error=%s",
                            job.id,
                            job.dispatch.channel,
                            delivery_error,
                        )

            final_event: Any | None = None
            async for event in self._workspace.stream_query(req):
                if job.dispatch.silent:
                    continue
                if job.dispatch.mode == "final":
                    if (
                        getattr(event, "object", None) == "message"
                        and getattr(event, "status", None)
                        == RunStatus.Completed
                    ):
                        final_event = event
                    continue
                await _deliver(event)

            if final_event is not None:
                await _deliver(final_event)

        try:
            await asyncio.wait_for(
                _run(),
                timeout=job.runtime.timeout_seconds,
            )
            if job.dispatch.silent:
                delivery_status = "suppressed"
            elif delivery_error:
                delivery_status = "failed"
            else:
                delivery_status = "success"
            return {
                "task_type": "agent",
                "run_id": run_id,
                "delivery_status": delivery_status,
                "delivery_error": delivery_error,
            }
        except asyncio.TimeoutError:
            logger.warning(
                "cron execute: job_id=%s timed out after %ss",
                job.id,
                job.runtime.timeout_seconds,
            )
            raise
        except asyncio.CancelledError:
            logger.info("cron execute: job_id=%s cancelled", job.id)
            raise
        finally:
            if _chat_spec is not None and chat_manager is not None:
                try:
                    await chat_manager.touch_chat(_chat_spec.id)
                except Exception:
                    logger.debug(
                        "cron: failed to touch chat for job %s",
                        job.id,
                        exc_info=True,
                    )
