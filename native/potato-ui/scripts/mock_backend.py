"""Local-only UI acceptance server. No model calls, tools, or persistent data.

Run with Python 3. Prompt 'slow' to test stop, 'disconnect' to test reconnect.
"""
import argparse
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

state = {"chat": None, "messages": [], "stop": threading.Event(), "approvals": [], "questions": [], "actions": [], "fail_answer": True}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def send_json(self, body, status=200):
        payload = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def do_GET(self):
        path = urlparse(self.path).path
        if path == "/api/approval/list":
            self.send_json({"pending_approvals": state["approvals"]})
        elif path == "/api/questions":
            self.send_json({"questions": state["questions"]})
        elif path == "/fixture/actions":
            self.send_json(state["actions"])
        elif path == "/api/chats":
            self.send_json([state["chat"]] if state["chat"] else [])
        elif path == "/api/chats/fixture-chat":
            self.send_json({"messages": state["messages"], "status": state["chat"]["status"]})
        elif path == "/api/models/active":
            self.send_json({"active_llm": {"provider_id": "fixture", "model": "迁移验收模型"}})
        elif path == "/api/workspace/running-config":
            self.send_json({"approval_level": "STRICT", "sandbox_mode": "workspace-write"})
        else:
            self.send_json({"error": "not found"}, 404)

    def do_POST(self):
        path = urlparse(self.path).path
        if path in ("/api/approval/approve", "/api/approval/deny"):
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            assert body["scope"] == "exact"
            assert body["session_id"] == "rich-session" and body["user_id"] == "default"
            state["actions"].append({"path": path, "body": body})
            state["approvals"] = [a for a in state["approvals"] if a["request_id"] != body["request_id"]]
            self.send_json({"success": True, "request_id": body["request_id"], "message": "测试记录已确认"})
            return
        if path.startswith("/api/questions/") and path.endswith("/answer"):
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            if state["fail_answer"]:
                state["fail_answer"] = False
                self.send_json({"error": "intentional retry fixture"}, 503)
                return
            request_id = path.split("/")[3]
            state["actions"].append({"path": path, "body": body})
            state["questions"] = [q for q in state["questions"] if q["request_id"] != request_id]
            self.send_json({"success": True})
            return
        if urlparse(self.path).path == "/api/console/chat/stop":
            state["stop"].set()
            self.send_json({"stopped": True})
            return
        if self.path != "/api/console/chat":
            self.send_json({"error": "not found"}, 404)
            return
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        # Catch accidental client-side permission overrides during manual acceptance.
        assert "approval_level" not in body.get("request_context", {})
        assert "sandbox_mode" not in body.get("request_context", {})
        reconnect = body.get("reconnect", False)
        prompt = "reconnect" if reconnect else body["input"][0]["content"][0]["text"]
        state["chat"] = {"id": "fixture-chat", "name": "Rust 聊天迁移验收", "session_id": body["session_id"],
                         "user_id": body["user_id"], "channel": body["channel"], "status": "running"}
        if not reconnect:
            state["messages"].append({"id": f"user-{time.time_ns()}", "role": "user", "content": prompt})
        state["stop"].clear()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()
        self.close_connection = True
        sequence = 0

        def emit(frame):
            nonlocal sequence
            sequence += 1
            frame["sequence_number"] = sequence
            payload = ("data: " + json.dumps(frame, ensure_ascii=False) + "\r\n\r\n").encode()
            # Deliberately split multibyte Chinese characters across writes.
            for offset in range(0, len(payload), 7):
                self.wfile.write(payload[offset:offset + 7])
            self.wfile.flush()

        try:
            response_id = f"response-{time.time_ns()}"
            emit({"object": "response", "id": response_id, "session_id": body["session_id"], "status": "in_progress"})
            if prompt == "disconnect":
                return
            message = {"object": "message", "id": f"assistant-{time.time_ns()}", "type": "message", "role": "assistant",
                       "status": "in_progress", "content": []}
            emit(message.copy())
            result = ""
            pieces = ["这是本地测试服务。", "中文分片已正确合并。", "流式回复、会话保存与续聊验证完成。"]
            if prompt == "slow":
                pieces = [f"生成第 {i + 1} 段。" for i in range(60)]
            for piece in pieces:
                if state["stop"].wait(1 if prompt == "slow" else 0.5):
                    break
                result += piece
                emit({"object": "content", "msg_id": message["id"], "type": "text", "index": 0, "delta": True, "text": piece})
            message["status"] = "completed"
            message["content"] = [{"object": "content", "type": "text", "text": result, "delta": False}]
            state["messages"].append(message)
            state["chat"]["status"] = "idle"
            emit(message.copy())
            emit({"object": "response", "id": response_id, "status": "cancelled" if state["stop"].is_set() else "completed", "output": [message]})
        except (BrokenPipeError, ConnectionResetError):
            pass


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=18765)
    parser.add_argument("--rich", action="store_true", help="seed Markdown, tools, approvals and questions")
    parser.add_argument("--style", action="store_true", help="rich content without pending interactions")
    args = parser.parse_args()
    if args.rich or args.style:
        state["chat"] = {"id": "fixture-chat", "name": "Rust 富文本与交互验收（模拟）", "session_id": "rich-session", "user_id": "default", "channel": "console", "status": "idle"}
        state["messages"] = [
            {"role": "user", "content": "请展示 Markdown 和工具记录。"},
            {"role": "assistant", "content": "## Rust 聊天迁移\n\n支持 **粗体**、*斜体*、`行内代码` 和列表。\n\n- 中文排版\n- 代码复制\n\n```rust\nfn main() {\n    println!(\"你好，Potato\");\n}\n```"},
            {"role": "tool", "type": "tool_call", "status": "completed", "content": [{"type": "data", "data": {"name": "shell（模拟）", "arguments": {"command": "pwd"}, "output": "/fixture/potato"}}]},
        ]
        base = {"root_session_id": "rich-session", "user_id": "default", "tool_name": "fixture", "tool_display_name": "模拟操作，不执行工具", "exact_target": "/fixture/example.txt", "action_detail": "仅记录按钮交互，不修改文件", "severity": "low", "created_at": time.time(), "timeout_seconds": 3600}
        state["approvals"] = [dict(base, request_id="approval-allow"), dict(base, request_id="approval-deny")]
        state["questions"] = [{"request_id": "question-answer", "session_id": "rich-session", "status": "pending", "title": "下一步先验收哪一项？（模拟）", "multiple": False, "options": [{"id": "visual", "label": "界面对齐"}, {"id": "performance", "label": "性能测试"}]}, {"request_id": "question-skip", "session_id": "rich-session", "status": "pending", "title": "这道测试问题可以跳过", "options": []}]

    if args.style:
        state["approvals"] = []
        state["questions"] = []
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"Fixture server: http://127.0.0.1:{server.server_port}", flush=True)
    server.serve_forever()
