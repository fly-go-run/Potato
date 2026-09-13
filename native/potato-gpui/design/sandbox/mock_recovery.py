"""Local-only GPUI acceptance fixture; no real model or credentials."""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import sys
import threading

root = Path(sys.argv[1]).resolve()
if not str(root).startswith('/private/tmp/potato-auto-approval-'):
    raise SystemExit('Use an isolated /private/tmp/potato-auto-approval-* directory')
root.mkdir(parents=True, exist_ok=True)
gate = threading.Lock()
main_count = 0


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        data = b'fixture'
        self.send_response(200)
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        global main_count
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        with gate:
            if not request.get('tools'):
                kind = 'review-allow'
                delta = {'content': json.dumps({'outcome': 'allow', 'risk': 'low',
                         'rationale': '本地验收操作已获授权', 'authorization_evidence_ids': ['user-0']}, ensure_ascii=False)}
            else:
                index = main_count
                main_count += 1
                kind = f'main-{index}'
                if index in (0, 2):
                    args = {'command': f'/usr/bin/curl -fsS --connect-timeout 1 --max-time 2 http://127.0.0.1:{self.server.server_port}/fixture'}
                    if index == 0:
                        args['command'] = 'echo once >> marker; ' + args['command']
                        args['run_in_background'] = True
                    else:
                        args['network_access'] = True
                        args['justification'] = '仅继续失败的本地下载，保留文件隔离'
                    delta = {'tool_calls': [{'index': 0, 'id': f'fixture-{index}', 'type': 'function',
                            'function': {'name': 'execute_shell_command', 'arguments': json.dumps(args)}}]}
                else:
                    delta = {'content': '后台命令正在执行。' if index == 1 else '已自动继续并完成下载。marker 只写入了一次，联网执行仍保留文件沙箱。'}
            with (root / 'requests.jsonl').open('a') as out:
                out.write(json.dumps({'kind': kind}) + '\n')
        payload = ('data: ' + json.dumps({'choices': [{'delta': delta, 'finish_reason': 'stop'}]}, ensure_ascii=False)
                   + '\n\ndata: [DONE]\n\n').encode()
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.send_header('Content-Length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
(root / 'endpoint').write_text(f'http://127.0.0.1:{server.server_port}')
print((root / 'endpoint').read_text(), flush=True)
server.serve_forever()
