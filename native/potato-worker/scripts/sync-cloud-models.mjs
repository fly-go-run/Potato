import { desktopCloudConfig, summarizeCloud } from './cloud-config.mjs';
import { cloudConfiguration } from '../src/cloud.ts';
import { spawnSync } from 'node:child_process';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';

const args = process.argv.slice(2);
if (args.some(a => !['--apply', '--probe', '--models-only'].includes(a))) throw new Error('Usage: node scripts/sync-cloud-models.mjs [--probe] [--apply] [--models-only]');
const { config, ownerEmail } = desktopCloudConfig();
const target = JSON.parse(readFileSync('wrangler.jsonc', 'utf8'));
if (target.name !== 'potato-iphone-api' || target.vars.REMOTE_PUBLIC_URL !== 'https://potato-remote.recodex.top') throw new Error('Unexpected Worker target');
if (!ownerEmail || !/^[^\s,@]+@[^\s,@]+\.[^\s,@]+$/.test(ownerEmail)) throw new Error('No verified desktop owner email. Sign in on the desktop before syncing.');
cloudConfiguration(JSON.stringify(config));
if (args.includes('--probe')) {
  for (const p of config.providers) {
    const url = new URL(p.endpoint); url.pathname = url.pathname.replace(/\/chat\/completions$/, '/models');
    const response = await fetch(url, { redirect: 'manual', headers: { Authorization: `Bearer ${p.api_key}` }, signal: AbortSignal.timeout(20000) });
    if (!response.ok) { await response.body?.cancel(); throw new Error(`${p.id}: model directory HTTP ${response.status}`); }
    let text = '', bytes = 0; const decoder = new TextDecoder();
    for await (const chunk of response.body) { bytes += chunk.length; if (bytes > 1000000) throw new Error('Model directory too large'); text += decoder.decode(chunk, { stream: true }); }
    text += decoder.decode();
    let catalog; try { catalog = JSON.parse(text); } catch { throw new Error(`${p.id}: invalid model directory`); }
    const ids = new Set((catalog.data || []).map(m => m.id));
    p.models = p.models.filter(m => ids.has(m.id));
    if (!p.models.length) throw new Error(`${p.id}: none of the configured chat models appear in the directory`);
  }
  if (!config.providers.some(p => p.models.some(m => `${p.id}/${m.id}` === config.default_model))) config.default_model = `${config.providers[0].id}/${config.providers[0].models[0].id}`;
}
console.log(JSON.stringify({ target: target.name, ownerEmail, ...summarizeCloud(config) }, null, 2));
if (args.includes('--apply')) {
  if (!args.includes('--probe')) throw new Error('Publishing requires --probe to exclude unavailable model IDs');
  // Upload code and the new secrets together; preserve existing service secrets.
  const directory = mkdtempSync(join(tmpdir(), 'potato-cloud-deploy-'));
  try {
    const file = join(directory, 'secrets.json');
    // User access is managed in Cloudflare; model updates must never replace it.
    const secrets = { CLOUD_PROVIDERS: JSON.stringify(config), CLOUD_AUTH_REQUIRED: 'true' };
    writeFileSync(file, JSON.stringify(secrets), { mode: 0o600 });
    const result = spawnSync(resolve('node_modules/.bin/wrangler'), ['deploy', '--keep-vars', '--secrets-file', file], { encoding: 'utf8', timeout: 120000, env: { ...process.env, WRANGLER_SEND_METRICS: 'false', WRANGLER_LOG_PATH: join(directory, 'wrangler.log') }, maxBuffer: 2000000 });
    let output = (result.stdout || '') + (result.stderr || '');
    for (const p of config.providers) output = output.replaceAll(p.api_key, '[REDACTED]');
    console.log(output);
    if (result.status !== 0) throw new Error('Cloud deployment failed');
  } finally { rmSync(directory, { recursive: true, force: true }); }
}
