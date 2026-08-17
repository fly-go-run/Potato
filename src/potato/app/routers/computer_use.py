# -*- coding: utf-8 -*-
"""Computer Use settings and driver status."""

from __future__ import annotations

import asyncio
import platform
from typing import List

from fastapi import APIRouter, Body, HTTPException
from pydantic import BaseModel, ConfigDict, Field

from ...computer_use.bundle import ensure_driver_binary
from ...computer_use.client import CuaDriverClient, resolve_cua_driver_binary
from ...computer_use.protect import is_protected_app
from ...computer_use.settings import is_stable_app_id
from ...config import load_config, save_config

router = APIRouter(prefix="/computer-use", tags=["computer-use"])


class ComputerUseStatus(BaseModel):
    enabled: bool
    driver_available: bool
    driver_path: str
    driver_version: str = ""
    always_allowed_apps: List[str] = Field(default_factory=list)
    platform: str
    hint: str = ""


class ComputerUseUpdate(BaseModel):
    model_config = ConfigDict(extra="forbid")

    enabled: bool | None = None
    always_allowed_apps: List[str] | None = None


def _hint(driver_path: str, platform_name: str) -> str:
    if not driver_path:
        return (
            "Potato includes a computer-use driver. Turn this on once to "
            "prepare it. If preparation fails, check the network and retry."
        )
    if platform_name == "Darwin":
        return (
            "Grant Accessibility and Screen Recording to Potato in "
            "System Settings → Privacy & Security. You do not need "
            "to install Cua Driver."
        )
    return (
        "The built-in driver is ready. Potato starts it in the background "
        "when Computer Use is on."
    )


def _clean_allow_list(
    values: List[str],
    *,
    reject_invalid: bool,
) -> list[str]:
    seen: set[str] = set()
    cleaned: list[str] = []
    for item in values:
        value = str(item).strip()
        if not is_stable_app_id(value):
            if reject_invalid:
                raise HTTPException(
                    status_code=422,
                    detail=(
                        "Always-allowed apps must use a bundle id or "
                        "Windows AUMID, not a display name."
                    ),
                )
            continue
        key = value.lower()
        if key not in seen and not is_protected_app(bundle_id=value):
            seen.add(key)
            cleaned.append(value)
    return cleaned


@router.get("", response_model=ComputerUseStatus, summary="Computer Use status")
async def get_computer_use() -> ComputerUseStatus:
    config = load_config()
    settings = config.computer_use
    driver_path = resolve_cua_driver_binary()
    version = ""
    if driver_path:
        try:
            version = await CuaDriverClient(binary=driver_path).version()
        except Exception:
            version = ""
    return ComputerUseStatus(
        enabled=settings.enabled,
        driver_available=bool(driver_path),
        driver_path=driver_path,
        driver_version=version,
        always_allowed_apps=_clean_allow_list(
            list(settings.always_allowed_apps),
            reject_invalid=False,
        ),
        platform=platform.system(),
        hint=_hint(driver_path, platform.system()),
    )


@router.put("", response_model=ComputerUseStatus, summary="Update Computer Use")
async def put_computer_use(body: ComputerUseUpdate = Body(...)) -> ComputerUseStatus:
    config = load_config()
    current = config.computer_use
    cleaned = _clean_allow_list(
        list(current.always_allowed_apps),
        reject_invalid=False,
    )
    if body.always_allowed_apps is not None:
        cleaned = _clean_allow_list(
            body.always_allowed_apps,
            reject_invalid=True,
        )

    if body.enabled is not None:
        if body.enabled:
            try:
                prepared = await asyncio.to_thread(ensure_driver_binary)
            except Exception as exc:
                raise HTTPException(
                    status_code=400,
                    detail=(
                        "Potato could not prepare its built-in computer-use "
                        f"driver: {exc}"
                    ),
                ) from exc
            if not prepared:
                raise HTTPException(
                    status_code=400,
                    detail=(
                        "Potato could not prepare its built-in computer-use "
                        "driver."
                    ),
                )
            current.enabled = True
        else:
            current.enabled = False
    current.always_allowed_apps = cleaned
    config.computer_use = current
    save_config(config)
    from ...agents.tools.computer_use import shutdown_computer_use

    await shutdown_computer_use()
    return await get_computer_use()
