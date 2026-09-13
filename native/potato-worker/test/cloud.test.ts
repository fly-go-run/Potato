import test from 'node:test';
import assert from 'node:assert/strict';
import { handle } from '../src/index.ts';
import { cloudConfiguration } from '../src/cloud.ts';
const owner = 'a'.repeat(64), session = '00000000-0000-4000-8000-000000000001', secret = 'b'.repeat(64);
const token = `${owner}.${session}.${secret}`;
const config = { default_model: 'deepseek/shared', providers: [
  { id: 'deepseek', name: 'DeepSeek', endpoint: 'https://deepseek.invalid/chat/completions', api_key: 'deepseek-secret', models: [{ id: 'shared', name: 'Shared DeepSeek' }] },
  { id: 'sub2api', name: 'sub2api', endpoint: 'https://sub2api.invalid/v1/chat/completions', api_key: 'sub2api-secret', models: [{ id: 'shared', name: 'Shared sub2api', reasoning_effort_options: ['high'] }] }
] };
function environment() {
  const rates: string[] = [];
  return { CLOUD_PROVIDERS: JSON.stringify(config), CLOUD_ALLOWED_EMAILS: 'Owner@Example.test', MAX_OUTPUT_TOKENS: '4096',
    CLIENT_TOKEN: 'old-device-token-'.repeat(4), CHAT_RATE_LIMIT: { limit: async ({ key }) => { rates.push(key); return { success: true }; } },
    REMOTE_ACCOUNTS: { getByName: (name) => { assert.equal(name, owner); return { cloudIdentity: async (id, proof) => id === session && proof === secret ? { email: 'owner@example.test' } : null }; } }, rates };
}
const req = (path = 'models', credential = token, body?: unknown) => new Request('https://worker.invalid/v1/' + path, { method: body === undefined ? 'GET' : 'POST', headers: { Authorization: `Bearer ${credential}`, 'Content-Type': 'application/json' }, ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
const chat = (model = 'sub2api/shared') => ({ model, stream: true, max_tokens: 16, messages: [{ role: 'user', content: 'synthetic' }] });
const noFetch = async () => { throw new Error('Unexpected external request'); };
test('account catalog separates duplicate IDs, publishes default, never publishes secrets or endpoints', async () => {
  const env = environment(); const response = await handle(req(), env, noFetch);
  assert.equal(response.status, 200); const body = await response.json();
  assert.deepEqual(body.data.map(m => m.id), ['deepseek/shared', 'sub2api/shared']);
  assert.equal(body.default_model, 'deepseek/shared'); assert.equal(body.catalog_source, 'configured');
  assert.doesNotMatch(JSON.stringify(body), /secret|endpoint|api_key|invalid/); assert.deepEqual(env.rates, [owner]);
});
test('cloud policy rejects old shared token, forged email, revoked identity and empty allowlist on every route', async () => {
  for (const path of ['models', 'chat/completions', 'sandbox/run', 'audio/transcriptions']) {
    const body = ['models', 'audio/transcriptions'].includes(path) ? undefined : chat();
    const env = environment();
    assert.equal((await handle(req(path, env.CLIENT_TOKEN, body), env, noFetch)).status, 401);
    const forged = req(path, token, body); forged.headers.set('cf-access-authenticated-user-email', 'owner@example.test');
    assert.equal((await handle(forged, { ...env, CLOUD_ALLOWED_EMAILS: 'other@example.test' }, noFetch)).status, 403);
    assert.equal((await handle(req(path, token, body), { ...env, CLOUD_ALLOWED_EMAILS: '' }, noFetch)).status, 503);
    assert.equal((await handle(req(path, `${owner}.${session}.${'c'.repeat(64)}`, body), env, noFetch)).status, 401);
  }
});
test('selected provider controls upstream URL, credential and unqualified model, never caller routing fields', async () => {
  for (const p of config.providers) {
    const response = await handle(req('chat/completions', token, { ...chat(`${p.id}/shared`), endpoint: 'https://attacker.invalid', api_key: 'caller-secret', reasoning_effort: 'high' }), environment(), async (url, init) => {
      assert.equal(url.toString(), p.endpoint); assert.equal(init.redirect, 'manual'); assert.equal(init.headers.Authorization, `Bearer ${p.api_key}`);
      const body = JSON.parse(init.body); assert.equal(body.model, 'shared'); assert.equal(body.reasoning_effort, 'high');
      assert.equal(body.endpoint, undefined); assert.equal(body.api_key, undefined);
      return new Response('data: [DONE]\n\n', { headers: { 'Content-Type': 'text/event-stream' } });
    });
    assert.equal(response.status, 200);
  }
  assert.equal((await handle(req('chat/completions', token, chat('shared')), environment(), noFetch)).status, 400);
});
test('malformed cloud config and absent secrets fail closed instead of falling back to shared auth', async () => {
  const env = environment();
  for (const value of ['', '{', JSON.stringify({ ...config, default_model: 'missing' }), JSON.stringify({ ...config, providers: [{ ...config.providers[0], endpoint: 'http://127.0.0.1/chat/completions' }] })]) {
    assert.throws(() => cloudConfiguration(value));
    assert.equal((await handle(req(), { ...env, CLOUD_AUTH_REQUIRED: 'true', CLOUD_PROVIDERS: value }, noFetch)).status, 503);
    assert.equal((await handle(req('models', env.CLIENT_TOKEN), { ...env, CLOUD_AUTH_REQUIRED: 'true', CLOUD_PROVIDERS: value }, noFetch)).status, 401);
  }
});
test('search continuation stays on the selected provider and uses the account rate key', async () => {
  let round = 0; const env = { ...environment(), EXA_API_KEY: 'exa-secret', SEARCH_RATE_LIMIT: { limit: async ({ key }) => { assert.equal(key, owner); return { success: true }; } } };
  const response = await handle(req('chat/completions', token, { ...chat(), max_tokens: 128 }), env, async (url, init) => {
    if (url.toString().includes('api.exa.ai')) return Response.json({ results: [] });
    assert.equal(url.toString(), config.providers[1].endpoint); assert.equal(init.headers.Authorization, 'Bearer sub2api-secret'); assert.equal(JSON.parse(init.body).model, 'shared');
    const data = round++ === 0 ? { choices: [{ delta: { tool_calls: [{ index: 0, id: 'search1', type: 'function', function: { name: 'web_search', arguments: '{"query":"synthetic test"}' } }] }, finish_reason: 'tool_calls' }] } : { choices: [{ delta: { content: 'synthetic answer' }, finish_reason: 'stop' }] };
    return new Response(`data: ${JSON.stringify(data)}\n\ndata: [DONE]\n\n`, { headers: { 'Content-Type': 'text/event-stream' } });
  });
  assert.equal(response.status, 200); assert.match(await response.text(), /synthetic answer/); assert.equal(round, 2);
});

test('desktop preserves local tools, tool results, thinking and usage without running mobile search', async () => {
  const tools = [{ type: 'function', function: { name: 'read_file', description: 'Read a local file', parameters: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'] } } }];
  const messages = [{ role: 'user', content: 'Read the synthetic file' }, { role: 'assistant', content: null, reasoning_content: 'Public thought', tool_calls: [{ id: 'call_1', type: 'function', function: { name: 'read_file', arguments: '{"path":"fixture.txt"}' } }] }, { role: 'tool', tool_call_id: 'call_1', content: 'Synthetic file content' }];
  let calls = 0;
  const result = await handle(req('desktop/chat/completions', token, { ...chat(), max_tokens: 128, tools, messages, reasoning_effort: 'high', api_key: 'caller-key', endpoint: 'https://attacker.invalid' }), { ...environment(), EXA_API_KEY: 'unused-search-key' }, async (url, init) => {
    calls++; assert.equal(url.toString(), config.providers[1].endpoint);
    const body = JSON.parse(init.body); assert.deepEqual(body.messages, messages); assert.deepEqual(body.tools, tools);
    assert.equal(body.reasoning_effort, 'high'); assert.deepEqual(body.stream_options, { include_usage: true });
    assert.equal(body.api_key, undefined); assert.equal(body.endpoint, undefined);
    return new Response('data: {"choices":[{"delta":{"content":"Synthetic result"},"finish_reason":"stop"}]}\n\ndata: [DONE]\n\n', { headers: { 'Content-Type': 'text/event-stream' } });
  });
  assert.equal(result.status, 200); assert.match(await result.text(), /Synthetic result/); assert.equal(calls, 1);
});

test('desktop rejects malformed tool exchanges and unauthorized credentials before upstream calls', async () => {
  const base = { ...chat(), tools: [] };
  const invalid = [
    { ...base, messages: [{ role: 'tool', tool_call_id: 'orphan', content: 'fake' }] },
    { ...base, messages: [{ role: 'assistant', content: null, tool_calls: [{ id: 'call', type: 'function', function: { name: 'read_file', arguments: '{}' } }] }] },
    { ...base, tools: [{ type: 'web_search' }] },
    { ...base, tools: [{ type: 'function', function: { name: 'f', parameters: { type: 'string' } } }] },
    { ...base, messages: [{ role: 'user', content: [{ type: 'image_url', image_url: { url: 'https://private.invalid/image.png' } }] }] },
  ];
  for (const body of invalid) assert.equal((await handle(req('desktop/chat/completions', token, body), environment(), noFetch)).status, 400);
  assert.equal((await handle(req('desktop/chat/completions', 'old-token', base), environment(), noFetch)).status, 401);
  assert.equal((await handle(req('desktop/chat/completions', token, base), { ...environment(), CLOUD_ALLOWED_EMAILS: 'other@example.test' }, noFetch)).status, 403);
});
