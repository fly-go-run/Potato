#!/usr/bin/env python3
"""Loopback-only fixture for the opt-in native draft UI tests; no model/core calls.

Serve deterministic JSON through real URLSession. The first timeout instruction
is accepted and its acknowledgment deliberately lost; retry must keep the exact
ID, target path and payload. Logs contain only synthetic test data.
"""
import argparse
import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--log", type=Path, required=True)
parser.add_argument("--models", action="store_true")
args = parser.parse_args()
receipts = {}
latest = {}
last_choices = {}
running = set()

def model_catalog():
    return {"version": 1, "active": {"provider_id": "fixture", "model": "one", "reasoning_effort": "high"}, "models": [
        {"provider_id": "fixture", "provider_name": "测试服务", "id": "one", "name": "思考模型", "effort_options": ["low", "high"], "default_effort": "high"},
        {"provider_id": "fixture", "provider_name": "测试服务", "id": "unknown", "name": "普通模型", "effort_options": [], "default_effort": None},
    ]} if args.models else None


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        with args.log.open("a") as log:
            log.write(json.dumps({"path": self.path, "body": body}, ensure_ascii=False) + "\n")
        if not self.path.endswith("/rpc"):
            return self.reply(404, {"error": "Fixture only supports RPC"})
        if body.get("op") == "overview":
            return self.reply(200, {"result": {"model_catalog": model_catalog()}})
        if body.get("op") == "send":
            identity = body["id"]
            if identity in receipts:
                old_path, old_args, result = receipts[identity]
                if (self.path, body["args"]) != (old_path, old_args):
                    return self.reply(409, {"error": "Fixture detected a mutated retry"})
                if old_args['text'] == 'fixture-uncertain': return self.reply(409, {'error': '该操作结果仍未确认；不会重复执行'})
                latest[self.path] = body["args"]["text"]
                return self.reply(200, {"result": result})
            text = body["args"]["text"]
            if text not in ("fixture-first", "fixture-timeout", "fixture-recovered", "fixture-uncertain", "fixture-model", "fixture-model-timeout", "fixture-running", "fixture-followup", "先把项目方案整理好，下午再和团队讨论一下。"):
                return self.reply(422, {"error": "Only synthetic fixture instructions are accepted"})
            if text == "fixture-running":
                running.add(self.path)
            if text == "fixture-followup":
                if body["args"].get("expected_run_id") != "fixture-run" or "model_choice" in body["args"]:
                    return self.reply(422, {"error": "Expected exact active run, without model override"})
                running.discard(self.path)
            if text.startswith("fixture-model"):
                choice = body["args"].get("model_choice", {})
                if choice.get("provider_id") != "fixture" or choice.get("model") not in ("one", "unknown") or "reasoning_effort" not in choice:
                    return self.reply(422, {"error": "Fixture requires an explicit resolved model choice"})
                last_choices[self.path] = choice
            else:
                last_choices.pop(self.path, None)
            result = {"chat": self.chat()}
            if text == "fixture-recovered": result['delivery'] = 'recovered'
            receipts[identity] = (self.path, body["args"], result)
            latest[self.path] = text
            if text == 'fixture-uncertain': return self.reply(409, {'error': '该操作结果仍未确认；不会重复执行'})
            if text in ("fixture-timeout", "fixture-recovered", "fixture-model-timeout"):
                return self.reply(504, {"error": "Fixture: response lost after accepting instruction"})
            return self.reply(200, {"result": result})
        if body.get("op") == "chat":
            text = latest.get(self.path, "fixture-first")
            return self.reply(200, {"result": {
                "chat": self.chat(), "status": "running" if self.path in running else "idle", "running_request_id": "fixture-run" if self.path in running else None, "messages": [
                    {"id": "u", "role": "user", "kind": "message", "text": text, "status": "completed"},
                    {"id": "a", "role": "assistant", "kind": "message", "text": "DRAFT_FIXTURE_OK: " + text + (" MODEL=" + json.dumps(last_choices[self.path], sort_keys=True) if self.path in last_choices else ""), "status": "completed"},
                ], "live": [], "approvals": [], "questions": [], "outcome": None,
            }})
        return self.reply(403, {"error": "Unsupported fixture operation"})

    @staticmethod
    def chat():
        return {"id": "fixture-sidebar", "session_id": "fixture-sidebar", "name": "调整侧栏和项目导航", "status": "completed", "pinned": False, "project_path": None}

    def reply(self, status, value):
        data = json.dumps(value, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *unused):
        pass


HTTPServer(("127.0.0.1", 19013), Handler).serve_forever()
