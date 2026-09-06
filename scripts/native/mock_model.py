#!/usr/bin/env python3
"""Loopback-only, credential-free model fixture for desktop IPC smoke tests.

It implements a tiny subset of both model protocols and never calls a remote
service. Configure the printed base URL with any dummy key and model name.
"""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        size = int(self.headers.get("Content-Length", "0"))
        if size > 1_000_000:
            self.send_error(413)
            return
        body = json.loads(self.rfile.read(size))
        text = "Rust 桌面核心连接成功。这是本机模拟回复，没有访问外部模型服务。"
        if self.path.endswith("/responses"):
            frames = [
                {"type": "response.output_text.delta", "delta": text},
                {"type": "response.completed"},
            ]
        elif self.path.endswith("/chat/completions"):
            frames = [{"choices": [{"delta": {"content": text}, "finish_reason": None}]},
                      {"choices": [{"delta": {}, "finish_reason": "stop"}]}]
        else:
            self.send_error(404)
            return
        assert isinstance(body, dict)
        payload = "".join("data: " + json.dumps(frame, ensure_ascii=False) + "\n\n" for frame in frames).encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


if __name__ == "__main__":
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    print(f"http://127.0.0.1:{server.server_port}/v1", flush=True)
    server.serve_forever()
