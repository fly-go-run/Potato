# -*- coding: utf-8 -*-
"""Composer live transcription: browser PCM → Doubao streaming ASR."""
from __future__ import annotations

import asyncio
import json
import logging
from typing import Any, Optional

import aiohttp
from fastapi import APIRouter, WebSocket, WebSocketDisconnect

from ...agents.utils.doubao_asr import (
    has_doubao_credentials,
    resolve_speech_credentials,
)
from ...agents.utils.doubao_stream_asr import (
    STREAM_URL,
    build_stream_headers,
    decode_frame,
    encode_audio,
    encode_full_client_request,
    extract_stream_text,
    resolve_stream_resource_id,
)
from ...config import load_config

logger = logging.getLogger(__name__)

router = APIRouter(prefix="/workspace", tags=["workspace"])

_CONNECT_TIMEOUT_S = 12
_SESSION_TIMEOUT_S = 130


async def open_doubao_stream(
    session: aiohttp.ClientSession,
) -> aiohttp.ClientWebSocketResponse:
    """Open the Doubao streaming socket. Split out so tests can patch it."""
    creds = resolve_speech_credentials()
    if creds is None:
        raise RuntimeError("SPEECH_API_KEY_MISSING")
    api_key, app_id = creds
    headers = build_stream_headers(
        api_key,
        app_id,
        resource_id=resolve_stream_resource_id(),
    )
    return await session.ws_connect(
        STREAM_URL,
        headers=headers,
        heartbeat=20.0,
        timeout=aiohttp.ClientWSTimeout(ws_close=8),
    )


def _guard_error() -> Optional[dict[str, str]]:
    config = load_config()
    provider_type = config.agents.transcription_provider_type
    if provider_type == "disabled":
        return {
            "code": "TRANSCRIPTION_DISABLED",
            "message": "Transcription is disabled.",
        }
    if provider_type == "doubao_asr" and not has_doubao_credentials():
        return {
            "code": "SPEECH_API_KEY_MISSING",
            "message": "Doubao speech credentials missing.",
        }
    return None


@router.websocket("/transcribe-stream")
async def transcribe_stream(websocket: WebSocket) -> None:
    """Proxy 16k PCM from the composer to Doubao streaming ASR."""
    await websocket.accept()
    guard = _guard_error()
    if guard is not None:
        await websocket.send_json({"type": "error", **guard})
        await websocket.close(code=1008)
        return

    timeout = aiohttp.ClientTimeout(total=_SESSION_TIMEOUT_S)
    try:
        async with aiohttp.ClientSession(timeout=timeout) as session:
            try:
                doubao = await asyncio.wait_for(
                    open_doubao_stream(session),
                    timeout=_CONNECT_TIMEOUT_S,
                )
            except Exception as exc:  # noqa: BLE001
                logger.warning("Doubao stream handshake failed: %s", exc)
                await websocket.send_json(
                    {
                        "type": "error",
                        "code": "TRANSCRIPTION_FAILED",
                        "message": "Could not connect to streaming ASR.",
                    }
                )
                await websocket.close(code=1011)
                return
            try:
                await doubao.send_bytes(encode_full_client_request())
                await websocket.send_json({"type": "ready"})
                await _proxy(websocket, doubao)
            finally:
                if not doubao.closed:
                    await doubao.close()
    except WebSocketDisconnect:
        logger.debug("Composer transcribe-stream disconnected")
    except Exception:  # noqa: BLE001
        logger.warning("Composer transcribe-stream failed", exc_info=True)
        try:
            await websocket.send_json(
                {
                    "type": "error",
                    "code": "TRANSCRIPTION_FAILED",
                    "message": "Streaming transcription failed.",
                }
            )
        except Exception:  # noqa: BLE001
            pass


async def _proxy(
    browser: WebSocket,
    doubao: aiohttp.ClientWebSocketResponse,
) -> None:
    sent_last = False

    async def from_browser() -> None:
        nonlocal sent_last
        while True:
            message = await browser.receive()
            kind = message.get("type")
            if kind == "websocket.disconnect":
                return
            text = message.get("text")
            raw = message.get("bytes")
            if text:
                try:
                    payload = json.loads(text)
                except json.JSONDecodeError:
                    continue
                if payload.get("type") == "stop":
                    await doubao.send_bytes(encode_audio(b"", last=True))
                    sent_last = True
                    return
            elif raw:
                await doubao.send_bytes(encode_audio(raw, last=False))

    async def from_doubao() -> None:
        async for msg in doubao:
            if msg.type == aiohttp.WSMsgType.BINARY:
                event = _browser_event(decode_frame(msg.data))
                if event is None:
                    continue
                await browser.send_json(event)
                if event["type"] in {"final", "error"}:
                    return
            elif msg.type in (
                aiohttp.WSMsgType.CLOSED,
                aiohttp.WSMsgType.ERROR,
            ):
                return

    browser_task = asyncio.create_task(from_browser())
    doubao_task = asyncio.create_task(from_doubao())
    try:
        done, _pending = await asyncio.wait(
            {browser_task, doubao_task},
            return_when=asyncio.FIRST_COMPLETED,
        )
        if browser_task in done and sent_last and not doubao_task.done():
            try:
                await asyncio.wait_for(doubao_task, timeout=4)
            except asyncio.TimeoutError:
                pass
    except WebSocketDisconnect:
        pass
    finally:
        for task in (browser_task, doubao_task):
            if not task.done():
                task.cancel()
        await asyncio.gather(browser_task, doubao_task, return_exceptions=True)


def _browser_event(frame: dict[str, Any]) -> Optional[dict[str, Any]]:
    if frame.get("kind") == "error":
        return {
            "type": "error",
            "code": "TRANSCRIPTION_FAILED",
            "message": str(frame.get("message") or "Streaming ASR error"),
        }
    text, definite = extract_stream_text(frame.get("body"))
    final = bool(frame.get("final") or definite)
    if not text and not final:
        return None
    return {"type": "final" if final else "partial", "text": text}
