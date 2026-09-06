#!/usr/bin/env python3
"""One-time public-API export. No packages, model calls, or credential export.

Run while the previous backend is still running, then import the JSON through
the Rust preview's settings. Text/tool history is retained; old local attachment
files and Python runtime state are deliberately not copied by this exporter.
"""
import argparse
import json
import os
from pathlib import Path
from urllib.parse import quote, urlparse
from urllib.request import Request, urlopen


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", required=True, help="Running local backend, e.g. http://127.0.0.1:8088")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--token-env", default="POTATO_MIGRATION_TOKEN", help="Environment variable containing an existing login token, if needed")
    args = parser.parse_args()
    parsed = urlparse(args.base_url)
    if parsed.scheme != "http" or parsed.hostname not in {"127.0.0.1", "localhost", "::1"} or parsed.username or parsed.password:
        parser.error("base-url must be a local HTTP backend without embedded credentials")
    token = os.environ.get(args.token_env, "")

    def get(path):
        headers = {"Authorization": f"Bearer {token}"} if token else {}
        with urlopen(Request(args.base_url.rstrip("/") + path, headers=headers), timeout=30) as response:
            return json.load(response)

    chats = []
    for spec in get("/api/chats?archived=false"):
        history = get("/api/chats/" + quote(spec["id"], safe=""))
        if history.get("status") == "running":
            raise SystemExit("Stop all running turns before exporting history")
        chats.append({"spec": spec, "messages": history["messages"]})
    # Exclusive creation protects an earlier export. Never print conversation
    # bodies or tokens, including on a failed request.
    fd = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as output:
        json.dump({"format": "potato-native-history-v1", "chats": chats}, output, ensure_ascii=False)
    print(f"Exported {len(chats)} conversations to {args.output}")


if __name__ == "__main__":
    main()
