from http.server import HTTPServer,BaseHTTPRequestHandler
import json

def msg(id,role,kind,text): return dict(id=id,role=role,kind=kind,text=text,status='completed')
class Handler(BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def do_POST(self):
  request=json.loads(self.rfile.read(int(self.headers.get('Content-Length',0))))
  if request.get('op')!='chat': self.send_error(403);return
  chatid=request['args']['chat_id']
  if chatid=='fixture-sidebar':
   messages=[msg('u','user','message','帮我总结一下项目进展。'),msg('a','assistant','message','已完成远程连接和账号关联。\n\n**下一步**是检查手机上的消息样式，确保正文、代码和执行过程易读。\n\n- 保留清晰的段落间距\n- 将执行记录和最终回复区分开')]
  elif chatid=='fixture-review':
   messages=[msg('u','user','message','检查一下项目，告诉我结果。'),msg('r','assistant','reasoning','我先查看项目目录，再读取配置文件，最后整理检查结果。'),msg('c','assistant','function_call','read_file\n{"path":"/fixture/Potato/README.md","limit":100}'),msg('t','tool','function_call_output','项目：Potato\n状态：配置正常\n文件检查完成'),msg('a','assistant','message','检查完成，配置正常。')]
  else:
   messages=[msg('u','user','message','给我代码示例和检查结果表格。'),msg('a','assistant','message','示例代码：\n\n```python\nfor device in devices:\n    print(device.name, device.online)\n```\n\n| 检查项 | 结果 |\n| --- | --- |\n| 账号关联 | 正常 |\n| 消息同步 | 正常 |\n\n详情见 [项目说明](https://example.com/docs)。')]
  chat=dict(id=chatid,session_id=chatid,name='远程消息样式检查',status='completed',pinned=False,project_path=None)
  body=json.dumps(dict(result=dict(chat=chat,status='completed',messages=messages,live=[],approvals=[],questions=[],outcome=dict(status='completed'))),ensure_ascii=False).encode()
  self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
HTTPServer(('127.0.0.1',18999),Handler).serve_forever()
