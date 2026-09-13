#!/usr/bin/env python3
"""Loopback-only remote lifecycle fixture. No core, model or real commands."""
import argparse
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

parser = argparse.ArgumentParser()
parser.add_argument('--log', required=True)
args = parser.parse_args()
lock = threading.Lock()
state = {'mode': 'thinking', 'delay': 0, 'overview_delay': 0, 'run_id': 'fixture-run', 'stop_protocol': 1}
chat = {'id': 'fixture-sidebar', 'session_id': 'fixture-sidebar', 'name': '调整侧栏和项目导航', 'status': 'running', 'pinned': False}

def message(identity, kind, text, status='completed', role='assistant'):
    return {'id': identity, 'kind': kind, 'role': role, 'text': text, 'status': status}

def snapshot(mode, run_id='fixture-run', stop_protocol=1):
    messages = [message('user', 'message', '检查远程状态', role='user'), message('reasoning', 'reasoning', '正在比较两个合成方案。', 'in_progress')]
    approvals, questions, outcome = [], [], None
    if mode == 'tool':
        messages += [message('tool', 'function_call', 'exec_command\n{"command":"synthetic-only"}', 'in_progress')]
    elif mode == 'reply':
        messages += [message('answer', 'message', '已收到正文，思考提示应结束。', 'in_progress')]
    elif mode == 'approval':
        approvals = [{'request_id': 'approval', 'tool_name': 'exec_command', 'findings_summary': '合成测试操作', 'action_detail': '{"command":"synthetic-only"}'}]
    elif mode == 'question':
        questions = [{'request_id': 'question', 'title': '选择合成方案', 'status': 'pending', 'multiple': False, 'options': [{'id': 'a', 'label': '方案 A'}]}]
    elif mode in ('complete', 'cancelled', 'failed'):
        messages[1]['status'] = 'completed'
        if mode == 'complete': messages += [message('answer', 'message', '远程任务已完成，内容仍可阅读。')]
        outcome = {'status': 'completed' if mode == 'complete' else mode}
        if mode == 'failed': outcome['error'] = {'message': '合成任务失败'}
    running = mode not in ('complete', 'cancelled', 'failed')
    return {'chat': chat, 'status': 'running' if running else 'idle', 'running_request_id': run_id if running else None, 'stop_protocol': stop_protocol,
            'messages': messages, 'live': [], 'approvals': approvals, 'questions': questions, 'outcome': outcome}

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))))
        if self.path == '/fixture/control':
            assert body.get('mode', 'thinking') in ('thinking', 'tool', 'reply', 'approval', 'question', 'complete', 'cancelled', 'failed', 'offline')
            with lock:
                state.update(mode=body.get('mode', 'thinking'), delay=min(float(body.get('delay', 0)), 15), overview_delay=min(float(body.get('overview_delay', 0)), 15), run_id=body.get('run_id', 'fixture-run'), stop_protocol=body.get('stop_protocol', 1))
            return self.reply(200, {'ok': True})
        with lock:
            current = state.copy()
            with open(args.log, 'a') as output: output.write(json.dumps({'time': time.time(), 'path': self.path, 'request': body, 'mode': current['mode']}, ensure_ascii=False) + '\n')
        if not self.path.endswith('/rpc'): return self.reply(404, {'error': 'Fixture only'})
        op = body.get('op')
        if op == 'overview':
            time.sleep(current['overview_delay'])
            return self.reply(200, {'result': {'model_catalog': None}})
        if op == 'chat':
            time.sleep(current['delay'])
            if current['mode'] == 'offline': return self.reply(503, {'error': '合成连接中断'})
            return self.reply(200, {'result': snapshot(current['mode'], current['run_id'], current['stop_protocol'])})
        if op in ('stop', 'approval', 'answer'):
            with lock:
                if op == 'stop' and (body.get('args', {}).get('expected_run_id') != state['run_id'] or state['mode'] in ('complete', 'cancelled', 'failed')):
                    return self.reply(412, {'error': '原任务已结束或改变，未停止其他任务，请刷新后确认'})
                state['mode'] = 'cancelled' if op == 'stop' else 'complete'
            return self.reply(200, {'result': {'ok': True}})
        return self.reply(403, {'error': 'Fixture does not execute commands'})

    def reply(self, status, value):
        data = json.dumps(value, ensure_ascii=False).encode()
        self.send_response(status); self.send_header('Content-Type', 'application/json'); self.send_header('Content-Length', str(len(data))); self.end_headers()
        try: self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError): pass

    def log_message(self, *unused): pass

ThreadingHTTPServer(('127.0.0.1', 19014), Handler).serve_forever()
