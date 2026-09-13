import { desktopConnection } from '../../potato-ios/scripts/desktop-connection.mjs';
import { randomBytes } from 'node:crypto';
import { mkdtempSync, writeFileSync, readFileSync, mkdirSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync, execFileSync } from 'node:child_process';

// Explicit local deployment/provisioning command. Rotates the personal device token.
const simulator = process.argv[2];
if (!simulator) throw new Error('Pass the destination simulator UUID.');
const connection = desktopConnection();
const config = JSON.parse(readFileSync('wrangler.jsonc', 'utf8'));
if (config.vars.UPSTREAM_URL !== connection.endpoint || config.vars.ALLOWED_MODELS !== connection.model) throw new Error('Align the Worker config with the selected desktop provider before deploying.');
const token = randomBytes(32).toString('base64url');
const temporary = mkdtempSync(join(tmpdir(), 'potato-worker-secrets-'));
const sanitize = text => String(text || '').replaceAll(connection.token, '[REDACTED]').replaceAll(token, '[REDACTED]');
try {
  const secrets = join(temporary, 'secrets.json');
  writeFileSync(secrets, JSON.stringify({ CLIENT_TOKEN: token, UPSTREAM_API_KEY: connection.token }), { mode: 0o600 });
  const result = spawnSync(resolve('node_modules/.bin/wrangler'), ['deploy', '--strict', '--secrets-file', secrets], {
    encoding: 'utf8', timeout: 120000, env: { ...process.env, WRANGLER_SEND_METRICS: 'false' }, maxBuffer: 1024 * 1024
  });
  console.log(sanitize(result.stdout));
  if (result.status !== 0) { console.error(sanitize(result.stderr)); throw new Error('Worker deployment did not complete.'); }
  const url = result.stdout.match(/https:\/\/potato-iphone-api\.[a-z0-9-]+\.workers\.dev/)?.[0];
  if (!url) throw new Error('Deployment finished, but its workers.dev URL was not returned.');
  const container = execFileSync('xcrun', ['simctl', 'get_app_container', simulator, 'com.potato.iphone.prototype', 'data'], { encoding: 'utf8' }).trim();
  const directory = join(container, 'Library/Application Support');
  mkdirSync(directory, { recursive: true });
  const handoff = join(directory, 'PotatoConnectionImport.json');
  writeFileSync(handoff, JSON.stringify({ endpoint: url + '/v1/chat/completions', model: connection.model, token }), { mode: 0o600, flag: 'wx' });
  spawnSync('xcrun', ['simctl', 'terminate', simulator, 'com.potato.iphone.prototype'], { stdio: 'ignore' });
  execFileSync('xcrun', ['simctl', 'launch', simulator, 'com.potato.iphone.prototype', '--import-desktop-connection'], { stdio: 'pipe' });
  for (let i = 0; i < 20 && existsSync(handoff); i++) await new Promise(r => setTimeout(r, 250));
  if (existsSync(handoff)) { rmSync(handoff); throw new Error('Client did not consume its one-time connection file.'); }
  const saved = JSON.parse(readFileSync(join(directory, 'Potato/workspace.json'), 'utf8'));
  if (saved.settings.endpoint !== url + '/v1/chat/completions' || saved.settings.demo) throw new Error('Client connection was not saved.');
  const health = await fetch(url + '/health', { signal: AbortSignal.timeout(20000) });
  const unauthorized = await fetch(url + '/v1/chat/completions', { method: 'POST', signal: AbortSignal.timeout(20000) });
  const deployment = { url, model: connection.model, upstream: connection.endpoint, deployedAt: new Date().toISOString(), healthStatus: health.status, unauthorizedStatus: unauthorized.status, clientProvisioned: true };
  writeFileSync('deployment.json', JSON.stringify(deployment, null, 2) + '\n');
  console.log(JSON.stringify(deployment));
} finally { rmSync(temporary, { recursive: true, force: true }); }
