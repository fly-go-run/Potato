#!/usr/bin/env python3
"""Isolated loopback fixture for the native remote queue UI; no real commands."""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Lock

lock = Lock()
chat = {'id': 'fixture-sidebar', 'session_id': 'fixture-sidebar', 'name': '调整侧栏和项目导航', 'status': 'running', 'pinned': False}
queue = {'items': [], 'paused': False, 'interrupt': False, 'reason': ''}
receipts = {}
history = []
run_id = 'fixture-run'

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.reply(200, {'online': True})

    def do_POST(self):
        global run_id
        body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', '0'))))
        with lock:
            if self.path == '/fixture/reset':
                queue.update(items=[], paused=False, interrupt=False, reason='')
                receipts.clear(); history.clear(); run_id = 'fixture-run'
                return self.reply(200, {'ok': True})
            if self.path == '/fixture/preview':
                queue.update(items=[{'id': 'preview-' + str(i), 'text': text, 'state': 'pending', 'attachments': 0}
                                    for i, text in enumerate(['我认为这里应该有缓存。重新打开软件时，先显示上次的项目和对话，然后在后台更新。', '还有电脑离线时，不要把原来的对话列表隐藏掉。', '最后再检查一下手机端。'])],
                             paused=False, interrupt=False, reason='')
                history.clear(); receipts.clear(); run_id = 'fixture-run'
                return self.reply(200, {'ok': True})
            if self.path == '/fixture/complete-next':
                if queue['items']:
                    item = queue['items'].pop(0)
                    history.extend([{'id': item['id'], 'remote_operation_id': item['id'], 'role': 'user', 'kind': 'message', 'text': item['text']},
                                    {'id': item['id'] + '-reply', 'role': 'assistant', 'kind': 'message', 'text': '已收到，正在继续处理。'}])
                    queue['interrupt'] = False
                return self.reply(200, {'ok': True})
            op = body.get('op'); args = body.get('args', {}); operation = body.get('id')
            if op == 'overview':
                result = {'chats': [chat], 'projects': [], 'model_catalog': None}
            elif op == 'chat':
                result = {'chat': chat, 'status': 'running' if run_id else 'idle', 'running_request_id': run_id,
                          'stop_protocol': 1, 'outbox_protocol': 1, 'outbox': queue,
                          'messages': [{'id': 'u', 'role': 'user', 'kind': 'message', 'text': '整理项目导航，并核对手机端的显示。'},
                                       {'id': 'a', 'role': 'assistant', 'kind': 'message', 'text': '我正在检查侧栏和项目导航。你可以继续添加下一步任务。', 'status': 'in_progress' if run_id else 'completed'}] + history,
                          'live': [], 'approvals': [], 'questions': [], 'outcome': None}
            elif operation in receipts:
                result = receipts[operation]
            elif op == 'send':
                if args.get('delivery_mode') not in ('queue', 'interrupt'):
                    return self.reply(422, {'error': 'Fixture requires an explicit delivery mode'})
                if args['delivery_mode'] == 'interrupt' and args.get('expected_run_id') != run_id:
                    return self.reply(412, {'error': '原任务已改变'})
                item = {'id': operation, 'text': args['text'], 'state': 'pending', 'attachments': 0}
                if args['delivery_mode'] == 'interrupt':
                    queue['items'].insert(0, item); queue['interrupt'] = True
                else:
                    queue['items'].append(item)
                result = {'chat': chat, 'delivery': 'queued'}; receipts[operation] = result
            elif op == 'outbox':
                action = args['action']; item_id = args.get('item_id')
                if action in ('pause', 'resume'):
                    queue['paused'] = action == 'pause'; queue['reason'] = '队列已暂停' if queue['paused'] else ''
                else:
                    item = next((v for v in queue['items'] if v['id'] == item_id), None)
                    if not item: return self.reply(404, {'error': '这条消息已发送'})
                    if action == 'delete': queue['items'].remove(item)
                    if action == 'save': item['text'] = args['text']
                    if action == 'promote':
                        if args.get('expected_run_id') != run_id: return self.reply(412, {'error': '原任务已改变'})
                        queue['items'].remove(item); queue['items'].insert(0, item); queue['interrupt'] = True
                result = json.loads(json.dumps(queue)); receipts[operation] = result
            elif op == 'stop':
                run_id = None; queue.update(paused=True, interrupt=False, reason='已停止，待发送消息已暂停')
                result = {}; receipts[operation] = result
            else:
                return self.reply(403, {'error': 'Fixture only'})
            self.reply(200, {'result': result})

    def reply(self, status, value):
        data = json.dumps(value, ensure_ascii=False).encode()
        self.send_response(status); self.send_header('Content-Type', 'application/json'); self.send_header('Content-Length', str(len(data))); self.end_headers()
        try: self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError): pass

    def log_message(self, *unused): pass

ThreadingHTTPServer(('127.0.0.1', 19017), Handler).serve_forever()
