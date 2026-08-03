# -*- coding: utf-8 -*-
"""First-run provisioning from a bundled provision.json.

分发场景:给家人打包时,安装器同目录携带一份 provision.json(NSIS
post-install hook 会把它复制到 WORKING_DIR)。后端启动时发现该文件,
走与设置页完全相同的代码路径应用配置(自定义供应商创建、API key
配置、默认模型),因此密钥会用本机的 master key 正常加密落盘。

成功后文件改名为 provision.applied.json 防重复;失败保留原文件,
下次启动自动重试。文件格式(全部字段可选):

{
  "version": 1,
  "custom_providers": [
    {"id": "...", "name": "...", "base_url": "...", "api_key": "...",
     "models": [{"id": "...", "name": "..."}]}
  ],
  "provider_configs": [
    {"id": "deepseek", "api_key": "...", "base_url": "..."}
  ],
  "active": {"provider_id": "...", "model": "..."}
}
"""

from __future__ import annotations

import json
import logging
from pathlib import Path

from ..config.config import (
    ModelSlotConfig,
    load_agent_config,
    save_agent_config,
)
from ..constant import WORKING_DIR
from ..providers.provider import ModelInfo, ProviderInfo
from ..providers.provider_manager import ProviderManager

logger = logging.getLogger(__name__)

PROVISION_FILE = "provision.json"
APPLIED_FILE = "provision.applied.json"


async def apply_provision_file(
    manager: ProviderManager,
    agent_id: str,
    working_dir: Path | None = None,
) -> None:
    """Apply first-run provisioning if a provision file is present.

    Never raises: provisioning failure must not block app startup.
    """
    base = Path(working_dir or WORKING_DIR)
    path = base / PROVISION_FILE
    if not path.exists():
        return
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(data, dict):
            raise ValueError("provision.json root must be an object")
        await _apply(manager, agent_id, data)
    except Exception:  # pylint: disable=broad-except
        logger.exception(
            "[provision] failed to apply %s; will retry next startup",
            path,
        )
        return
    try:
        path.replace(base / APPLIED_FILE)
        logger.info("[provision] applied and archived to %s", APPLIED_FILE)
    except OSError:
        logger.exception("[provision] applied but failed to archive file")


async def _apply(
    manager: ProviderManager,
    agent_id: str,
    data: dict,
) -> None:
    for entry in data.get("custom_providers") or []:
        provider_id = str(entry.get("id") or "").strip()
        if not provider_id:
            continue
        if manager.get_provider(provider_id) is None:
            models = [
                ModelInfo(
                    id=str(model["id"]),
                    name=str(model.get("name") or model["id"]),
                )
                for model in entry.get("models") or []
                if isinstance(model, dict) and model.get("id")
            ]
            await manager.add_custom_provider(
                ProviderInfo(
                    id=provider_id,
                    name=str(entry.get("name") or provider_id),
                    base_url=str(entry.get("base_url") or ""),
                    extra_models=models,
                ),
            )
            logger.info("[provision] created custom provider %s", provider_id)
        api_key = str(entry.get("api_key") or "")
        if api_key:
            manager.update_provider(provider_id, {"api_key": api_key})

    for entry in data.get("provider_configs") or []:
        provider_id = str(entry.get("id") or "").strip()
        if not provider_id or manager.get_provider(provider_id) is None:
            continue
        config: dict = {}
        if entry.get("api_key"):
            config["api_key"] = str(entry["api_key"])
        if entry.get("base_url"):
            config["base_url"] = str(entry["base_url"])
        if config:
            manager.update_provider(provider_id, config)
            logger.info("[provision] configured provider %s", provider_id)

    active = data.get("active")
    if (
        isinstance(active, dict)
        and active.get("provider_id")
        and active.get(
            "model",
        )
    ):
        agent_config = load_agent_config(agent_id)
        agent_config.active_model = ModelSlotConfig(
            provider_id=str(active["provider_id"]),
            model=str(active["model"]),
        )
        save_agent_config(agent_id, agent_config)
        logger.info(
            "[provision] active model set to %s/%s",
            active["provider_id"],
            active["model"],
        )
