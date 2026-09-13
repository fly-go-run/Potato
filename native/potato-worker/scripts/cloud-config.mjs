import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { createDecipheriv } from 'node:crypto';
import { curatedChatModels } from './cloud-model-policy.mjs';

// Read only native settings; never print the returned credentials.
export function desktopCloudConfig() {
  const root = process.env.POTATO_NATIVE_DATA_DIR || join(homedir(), '.potato/native-v1');
  const script = `import pathlib,sqlite3,tempfile,shutil,json,sys,os
p=pathlib.Path(sys.argv[1])
with tempfile.TemporaryDirectory(prefix='potato-cloud-') as d:
 t=pathlib.Path(d)/p.name
 src=[pathlib.Path(str(p)+suffix) for suffix in ('','-wal')]
 before=[(s.stat().st_size,s.stat().st_mtime_ns) if s.exists() else None for s in src]
 for s in src:
  if s.exists():
   target=str(t)+str(s)[len(str(p)):]
   shutil.copyfile(s,target);os.chmod(target,0o600)
 if before!=[(s.stat().st_size,s.stat().st_mtime_ns) if s.exists() else None for s in src]:raise SystemExit('Settings changed; retry')
 c=sqlite3.connect(t)
 def get(k):
  r=c.execute('select value from settings where key=?',(k,)).fetchone()
  return json.loads(r[0]) if r else {}
 print(json.dumps({'providers':get('providers'),'active':get('active'),'owner_email':get('remote_config').get('email')}))`;
  const data = JSON.parse(execFileSync('python3', ['-c', script, join(root, 'potato.sqlite3')], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }));
  const master = readFileSync(join(root, 'master.key')), providers = [];
  for (const p of data.providers) {
    if (!['deepseek', 'sub2api'].includes(p.id) || p.chat_model !== 'OpenAIChatModel' || !p.api_key) continue;
    const endpoint = new URL(p.base_url.replace(/\/$/, '') + '/chat/completions');
    if (endpoint.protocol !== 'https:' || endpoint.username || endpoint.password || endpoint.search || endpoint.hash) throw new Error('Provider endpoint is not a clean HTTPS URL');
    const encrypted = Buffer.from(p.api_key, 'base64');
    const cipher = createDecipheriv('chacha20-poly1305', master, encrypted.subarray(0, 12), { authTagLength: 16 });
    cipher.setAuthTag(encrypted.subarray(-16));
    const api_key = Buffer.concat([cipher.update(encrypted.subarray(12, -16)), cipher.final()]).toString('utf8');
    const models = curatedChatModels(p.id);
    if (models.length) providers.push({ id: p.id, name: p.name, endpoint: endpoint.toString(), api_key, models });
  }
  const wanted = 'deepseek/deepseek-flash';
  const default_model = providers.some(p => p.models.some(m => `${p.id}/${m.id}` === wanted)) ? wanted : `${providers[0]?.id}/${providers[0]?.models[0]?.id}`;
  return { config: { providers, default_model }, ownerEmail: data.owner_email };
}
export function summarizeCloud(config) {
  return { default_model: config.default_model, providers: config.providers.map(p => ({ id: p.id, host: new URL(p.endpoint).host, models: p.models.map(m => m.id), credential_present: !!p.api_key })) };
}
