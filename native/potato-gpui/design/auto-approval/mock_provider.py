"""Local-only deterministic provider for native approval UI acceptance.

Run with a /tmp/potato-auto-approval-* directory. Never connects to a real model.
The mock assesses fixed scenarios, not risk; policy behavior is covered separately.
"""
import json
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import sys
import time

root = Path(sys.argv[1])
assert str(root).startswith(("/tmp/potato-auto-approval-", "/private/tmp/potato-auto-approval-"))
outside = root / "outside"
outside.mkdir(parents=True, exist_ok=True)
for name in ("small.py", "second.py", "deny.py", "failure.py"):
    (outside / name).write_text("# isolated approval test\nprint('hello')\n")


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        payload = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        messages = payload["messages"]
        # Use one fresh conversation per scenario. Host runtime-context messages
        # can follow a tool result, so the last wire message need not be a tool.
        result_message = next((m for m in reversed(messages) if m.get("role") == "tool"), None)
        repeat = any("复用测试" in str(m.get("content", "")) for m in messages if m.get("role") == "user")
        result_count = sum(m.get("role") == "tool" for m in messages)
        if not payload.get("tools"):
            # Base authorization and individual actions may be separate messages
            # when the native reviewer extends its bounded conversation.
            parts = []
            for message in messages:
                if message.get("role") != "user":
                    continue
                try:
                    parts.append(json.loads(message["content"]))
                except (ValueError, TypeError):
                    pass
            evidence = next(p for p in reversed(parts) if "trusted_user_authorization" in p)
            action = next(p["untrusted_planned_action"] for p in reversed(parts) if "untrusted_planned_action" in p)
            target = action["exact_target"]
            time.sleep(4)
            if "failure.py" in target:
                self.send_response(503)
                self.end_headers()
                self.wfile.write(b"fixture provider unavailable")
                return
            denied = "deny.py" in target
            assessment = {
                "outcome": "deny" if denied else "allow",
                "risk": "high" if denied else "low",
                "rationale": "测试拒绝：该操作超出本次授权。" if denied else "用户要求读取此文件，仅有本地读取，无写入或外传。",
                "authorization_evidence_ids": [evidence["trusted_user_authorization"][-1]["id"]],
            }
            content = json.dumps(assessment, ensure_ascii=False)
            chunks = [{"choices":[{"delta":{"content":content},"finish_reason":None}]}]
            kind = "review-deny" if denied else "review-allow"
        elif result_message is not None and (not repeat or result_count >= 3):
            result = str(result_message.get("content", ""))
            text = "自动审批流程已结束。工具结果：\n" + result
            chunks = [{"choices":[{"delta":{"content":text},"finish_reason":None}]}]
            kind = "main-after-tool"
        else:
            prompt = str(next((m["content"] for m in reversed(messages) if m.get("role") == "user"), ""))
            name = "second.py" if repeat and result_count == 2 else "failure.py" if "故障" in prompt else "deny.py" if "拒绝" in prompt else "small.py"
            args = json.dumps({"file_path": str(outside / name)}, ensure_ascii=False)
            chunks = [{"choices":[{"delta":{"tool_calls":[{"index":0,"id":f"read-test-{result_count}","type":"function","function":{"name":"read_file","arguments":args}}]},"finish_reason":"tool_calls"}]}]
            kind = "main-tool-request"
        if kind != "main-tool-request":
            chunks.append({"choices":[{"delta":{},"finish_reason":"stop"}]})
        chunks.append({"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":30,"total_tokens":130}})
        with (root / "requests.jsonl").open("a") as out:
            record = {"kind":kind,"tool_count":len(payload.get("tools", [])),"model":payload["model"],"message_count":len(messages)}
            if kind.startswith("review-"):
                record["base_digest"] = hashlib.sha256(json.dumps(messages[:2], sort_keys=True).encode()).hexdigest()
                record["prior_assessments"] = sum(m.get("role") == "assistant" for m in messages)
            out.write(json.dumps(record) + "\n")
        response = "".join("data: " + json.dumps(c, ensure_ascii=False) + "\n\n" for c in chunks) + "data: [DONE]\n\n"
        data = response.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


server = ThreadingHTTPServer(("127.0.0.1", int(sys.argv[2]) if len(sys.argv) > 2 else 0), Handler)
print(f"http://127.0.0.1:{server.server_port}", flush=True)
server.serve_forever()
