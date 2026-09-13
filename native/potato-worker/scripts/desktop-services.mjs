import { readFileSync, existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { createDecipheriv } from 'node:crypto';

// Trusted app configuration only. Never print returned values or read a project .env.
function envKey(path, name) {
  if (!existsSync(path)) return '';
  const text = readFileSync(path, 'utf8'); if (Buffer.byteLength(text) > 65536) return '';
  let found = '';
  for (let line of text.split(/\r?\n/)) {
    line = line.trim().replace(/^export /, ''); const equal = line.indexOf('=');
    if (equal < 0 || line.slice(0, equal).trim() !== name) continue;
    let value = line.slice(equal + 1).trim();
    if (value[0] === '"' || value[0] === "'") {
      const end = value.indexOf(value[0], 1); if (end < 0 || (value.slice(end + 1).trim() && !value.slice(end + 1).trim().startsWith('#'))) continue;
      value = value.slice(1, end);
    } else value = value.split(' #')[0].trim();
    if (value) found = value;
  }
  return found;
}
export function desktopServices() {
  const root = process.env.POTATO_NATIVE_DATA_DIR || join(homedir(), '.potato/native-v1');
  const exaKey = process.env.EXA_API_KEY?.trim() || envKey(join(root, '.env'), 'EXA_API_KEY') || (root.endsWith('/.potato/native-v1') ? envKey(join(root, '..', '.env'), 'EXA_API_KEY') : '');
  // Read a consistent copy with WAL; the source DB and its shared-memory files stay untouched.
  const script = `import pathlib,sqlite3,tempfile,shutil,json,sys
p=pathlib.Path(sys.argv[1])
with tempfile.TemporaryDirectory(prefix='potato-services-') as directory:
 target=pathlib.Path(directory)/p.name
 sources=[pathlib.Path(str(p)+suffix) for suffix in ['', '-wal']]
 before=[(s.stat().st_size,s.stat().st_mtime_ns) if s.exists() else None for s in sources]
 for source in sources:
  if source.is_file(): shutil.copyfile(source,str(target)+str(source)[len(str(p)):])
 after=[(s.stat().st_size,s.stat().st_mtime_ns) if s.exists() else None for s in sources]
 if before!=after: raise SystemExit('Desktop settings changed; retry the read.')
 c=sqlite3.connect(str(target));r=c.execute('select value from settings where key=?',('doubao',)).fetchone()
 print(r[0] if r else '{}')`;
  const voice = JSON.parse(execFileSync('python3', ['-c', script, join(root, 'potato.sqlite3')], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }));
  if (!voice.api_key || !voice.enabled) throw new Error('Native Doubao speech is not configured and enabled.');
  const encrypted = Buffer.from(voice.api_key, 'base64');
  const cipher = createDecipheriv('chacha20-poly1305', readFileSync(join(root, 'master.key')), encrypted.subarray(0, 12), { authTagLength: 16 });
  cipher.setAuthTag(encrypted.subarray(-16));
  const doubaoKey = Buffer.concat([cipher.update(encrypted.subarray(12, -16)), cipher.final()]).toString('utf8');
  return { exaKey, doubaoKey, doubaoAppId: voice.app_id || '', resourceId: voice.resource_id || 'volc.seedasr.sauc.duration' };
}
