#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""生成家人分发用的 provision.json(密钥只进这份文件,不进仓库/CI)。

用法示例:
  python3 scripts/pack-tauri/make_provision.py \
    --provider-id sub2api --provider-name sub2api \
    --base-url https://sub2api.example.com/v1 \
    --api-key sk-xxxx \
    --model gpt-5.6 --model gpt-5.6-luna \
    --active-model gpt-5.6 \
    --output dist/provision.json

交付:把生成的 provision.json 和 Windows 安装器放进同一个 zip 发给对方;
安装器会自动收取同目录的该文件,首次启动即完成全部配置。
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--provider-id", required=True)
    parser.add_argument("--provider-name", default=None)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--api-key", required=True)
    parser.add_argument(
        "--model",
        action="append",
        required=True,
        help="模型 id,可重复;第一个作为显示顺序首位",
    )
    parser.add_argument(
        "--active-model",
        default=None,
        help="默认激活的模型 id(缺省取第一个 --model)",
    )
    parser.add_argument("--output", default="dist/provision.json")
    args = parser.parse_args()

    active = args.active_model or args.model[0]
    if active not in args.model:
        parser.error(f"--active-model {active} 不在 --model 列表里")

    provision = {
        "version": 1,
        "custom_providers": [
            {
                "id": args.provider_id,
                "name": args.provider_name or args.provider_id,
                "base_url": args.base_url,
                "api_key": args.api_key,
                "models": [{"id": m, "name": m} for m in args.model],
            },
        ],
        "active": {"provider_id": args.provider_id, "model": active},
    }

    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(provision, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"已生成 {output}(含明文密钥,只随安装包点对点发送,勿入仓库)")


if __name__ == "__main__":
    main()
