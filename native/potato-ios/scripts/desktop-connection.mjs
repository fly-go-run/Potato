import { execFileSync } from 'node:child_process';
import { readFileSync, existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { createDecipheriv } from 'node:crypto';

// Developer handoff only. Read the selected native desktop provider, never log its key.
export function desktopConnection() {
  const root = process.env.POTATO_NATIVE_DATA_DIR || join(homedir(), '.potato/native-v1');
  const database = join(root, 'potato.sqlite3');
  const script = `import json,sqlite3,sys,pathlib
p=pathlib.Path(sys.argv[1])
mode='?mode=ro' if pathlib.Path(str(p)+'-wal').exists() else '?mode=ro&immutable=1'
c=sqlite3.connect(p.as_uri()+mode,uri=True)
def get(k):
 r=c.execute('select value from settings where key=?',(k,)).fetchone()
 return json.loads(r[0]) if r else None
a=get('active') or {}
p=next((p for p in get('providers') or [] if p.get('id')==a.get('provider_id')),None)
if not p: raise SystemExit('No active native provider')
print(json.dumps({'model':a.get('model'),'provider':{k:p.get(k) for k in ['id','base_url','chat_model','api_key']}}))`;
  const { model, provider } = JSON.parse(execFileSync('python3', ['-c', script, database], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }));
  if (provider.chat_model !== 'OpenAIChatModel' || !model) throw new Error('The selected desktop model does not use supported Chat Completions.');
  const endpoint = new URL(provider.base_url.replace(/\/$/, '') + '/chat/completions');
  if (endpoint.protocol !== 'https:' || endpoint.username || endpoint.password || endpoint.search || endpoint.hash) throw new Error('A clean HTTPS provider URL is required.');
  const encrypted = Buffer.from(provider.api_key || '', 'base64');
  if (encrypted.length < 28 || !existsSync(join(root, 'master.key'))) throw new Error('Desktop credential is unavailable.');
  const decipher = createDecipheriv('chacha20-poly1305', readFileSync(join(root, 'master.key')), encrypted.subarray(0, 12), { authTagLength: 16 });
  decipher.setAuthTag(encrypted.subarray(-16));
  const token = Buffer.concat([decipher.update(encrypted.subarray(12, -16)), decipher.final()]).toString('utf8');
  return { endpoint: endpoint.toString(), model, token };
}
