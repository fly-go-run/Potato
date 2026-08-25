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
  "active": {"provider_id": "...", "model": "..."},
  "envs": {"apikey": "...", "keyid": "..."},
  "transcription_provider_type": "doubao_asr"
}

``envs`` 走加密的 envs store(``SECRET_DIR/envs.json``),因此语音密钥这
类只认环境变量的配置也能随安装包交付。预配置中明确携带的密钥会覆盖
旧安装的同名 ``.env`` 值；未携带的配置、聊天和记忆保持不变。
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


def _apply_envs(envs: object) -> None:
    """Authoritatively seed named env vars into both secret stores."""
    if not isinstance(envs, dict):
        return
    from ..envs import set_env_var
    from ..envs.dotenv_file import (
        canonicalize_env_keys,
        overwrite_dotenv,
        potato_dotenv_path,
        should_touch_user_dotenv,
    )

    supplied: dict[str, str] = {}
    for key, value in envs.items():
        name = str(key).strip()
        if not name or value is None:
            continue
        supplied[name] = str(value)

    canonical = canonicalize_env_keys(supplied)
    for name, value in canonical.items():
        set_env_var(name, value)
        # 只记键名,值是密钥。
        logger.info("[provision] set env var %s", name)
    if canonical and should_touch_user_dotenv():
        overwrite_dotenv(potato_dotenv_path(), canonical)


def _overwrite_provider_env_key(provider_id: str, api_key: str) -> None:
    """Make a provisioned provider key win over an older user dotenv."""
    if not api_key:
        return
    import os

    from ..envs.dotenv_file import (
        overwrite_dotenv,
        potato_dotenv_path,
        should_touch_user_dotenv,
    )
    from ..providers.env_api_key import provider_env_ident

    name = f"{provider_env_ident(provider_id)}_API_KEY"
    os.environ[name] = api_key
    if should_touch_user_dotenv():
        overwrite_dotenv(potato_dotenv_path(), {name: api_key})


def _apply_transcription(provider_type: object) -> None:
    """Enable a transcription backend (voice input is off by default)."""
    value = str(provider_type or "").strip()
    if not value:
        return
    from ..config import load_config, save_config

    config = load_config()
    config.agents.transcription_provider_type = value
    save_config(config)
    logger.info("[provision] transcription provider set to %s", value)


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
                    reasoning_effort=(
                        str(model["reasoning_effort"])
                        if model.get("reasoning_effort")
                        else None
                    ),
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
            _overwrite_provider_env_key(provider_id, api_key)

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
            if config.get("api_key"):
                _overwrite_provider_env_key(
                    provider_id,
                    str(config["api_key"]),
                )
            logger.info("[provision] configured provider %s", provider_id)

    _apply_envs(data.get("envs"))
    _apply_transcription(data.get("transcription_provider_type"))

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
