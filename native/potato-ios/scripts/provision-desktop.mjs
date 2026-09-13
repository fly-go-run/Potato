import { desktopConnection } from './desktop-connection.mjs';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync, existsSync, rmSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

// Explicit local-only simulator handoff. Does not upload credentials to another service.
const simulator = process.argv[2];
if (!simulator) throw new Error('Pass the destination simulator UUID.');
const connection = desktopConnection();
const container = execFileSync('xcrun', ['simctl', 'get_app_container', simulator, 'com.potato.iphone.prototype', 'data'], { encoding: 'utf8' }).trim();
const directory = join(container, 'Library/Application Support');
mkdirSync(directory, { recursive: true });
const handoff = join(directory, 'PotatoConnectionImport.json');
writeFileSync(handoff, JSON.stringify(connection), { mode: 0o600, flag: 'wx' });
try {
  spawnSync('xcrun', ['simctl', 'terminate', simulator, 'com.potato.iphone.prototype'], { stdio: 'ignore' });
  execFileSync('xcrun', ['simctl', 'launch', simulator, 'com.potato.iphone.prototype', '--import-desktop-connection'], { stdio: 'pipe' });
  for (let i = 0; i < 20 && existsSync(handoff); i++) await new Promise(r => setTimeout(r, 250));
  if (existsSync(handoff)) throw new Error('Client did not consume the one-time configuration.');
  const saved = JSON.parse(readFileSync(join(directory, 'Potato/workspace.json'), 'utf8'));
  if (saved.settings.endpoint !== connection.endpoint || saved.settings.demo) throw new Error('Client connection was not saved.');
  console.log(JSON.stringify({ model: saved.settings.model, endpoint: saved.settings.endpoint, demo: saved.settings.demo, handoffRemoved: true }));
} finally { if (existsSync(handoff)) rmSync(handoff); }
