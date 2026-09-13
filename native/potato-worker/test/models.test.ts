import test from 'node:test';
import assert from 'node:assert/strict';
import { handle } from '../src/index.ts';
import { documentedCapabilities } from '../src/models.ts';
const token = 'synthetic-only-client-token-32-characters';
const env = { CLIENT_TOKEN: token, UPSTREAM_API_KEY: 'synthetic-provider-secret', UPSTREAM_URL: 'https://provider.invalid/api/chat/completions', ALLOWED_MODELS: 'one,two', MAX_OUTPUT_TOKENS: '4096', CHAT_RATE_LIMIT: { limit: async () => ({ success: true }) } };
const listRequest = () => new Request('https://worker.invalid/v1/models', { headers: { Authorization: `Bearer ${token}` } });
const noFetch = async () => { throw new Error('Unexpected upstream call'); };
test('catalog requires authentication and never forwards the device token or unapproved models', async () => {
  assert.equal((await handle(new Request('https://worker.invalid/v1/models'), env, noFetch)).status, 401);
  const response = await handle(listRequest(), env, async (url, init) => {
    assert.equal(url.toString(), 'https://provider.invalid/api/models');
    assert.equal(init.method, 'GET'); assert.equal(init.redirect, 'manual');
    assert.equal(init.headers.Authorization, 'Bearer synthetic-provider-secret');
    return Response.json({ data: [
      { id: 'one', name: 'One', reasoning_effort_options: ['low', 'high', 'low', 42], thinking_modes: ['enabled', 'disabled', 'other'], api_key: 'do-not-forward' },
      { id: 'one', name: 'duplicate' }, { id: 'two', private_url: 'do-not-forward' }, { id: 'not-approved' }
    ] });
  });
  assert.equal(response.status, 200); assert.equal(response.headers.get('cache-control'), 'no-store');
  const body = await response.json();
  assert.equal(body.data.length, 2); assert.deepEqual(body.data[0].reasoning_effort_options, ['low', 'high']);
  assert.deepEqual(body.data[0].thinking_modes, ['enabled', 'disabled']);
  assert.doesNotMatch(JSON.stringify(body), /secret|do-not-forward|not-approved|private_url|api_key/);
});
test('only exact documented origin and models receive capability defaults; explicit unsupported wins', async () => {
  const url = new URL('https://api.deepseek.com/chat/completions');
  assert.deepEqual(documentedCapabilities(url, 'deepseek-v4-pro').reasoning_effort_options, ['low', 'high', 'max']);
  assert.deepEqual(documentedCapabilities(url, 'deepseek-v4.1-flash-private'), {});
  assert.deepEqual(documentedCapabilities(new URL('https://proxy.invalid/chat/completions'), 'deepseek-v4-pro'), {});
  const response = await handle(listRequest(), { ...env, UPSTREAM_URL: url.toString(), ALLOWED_MODELS: 'deepseek-v4-pro' }, async () => Response.json({ data: [{ id: 'deepseek-v4-pro', reasoning_effort_options: [], thinking_modes: [] }] }));
  const body = await response.json(); assert.deepEqual(body.data[0].reasoning_effort_options, []); assert.deepEqual(body.data[0].thinking_modes, []);
});
test('missing model directory falls back to configured IDs without claiming unknown capabilities', async () => {
  const response = await handle(listRequest(), env, async () => new Response('private detail', { status: 404 }));
  const body = await response.json(); assert.equal(body.catalog_source, 'configured');
  assert.deepEqual(body.data.map(m => m.id), ['one', 'two']); assert.equal(body.data[0].reasoning_effort_options, undefined);
});
test('catalog errors, redirects and oversized content fail without disclosing provider body', async () => {
  for (const response of [new Response('private secret', { status: 401 }), new Response(null, { status: 302, headers: { Location: 'https://other.invalid' } }), Response.json({ data: Array.from({ length: 501 }, () => ({ id: 'one' })) }), new Response('x'.repeat(1_000_001), { headers: { 'content-type': 'application/json' } })]) {
    const result = await handle(listRequest(), env, async () => response);
    assert.equal(result.status, 502); assert.doesNotMatch(await result.text(), /private secret|other.invalid/);
  }
});
test('effort and thinking survive the actual proxy request; invalid combinations reject before fetch', async () => {
  const request = (extra: unknown) => new Request('https://worker.invalid/v1/chat/completions', { method: 'POST', headers: { Authorization: `Bearer ${token}`, 'content-type': 'application/json' }, body: JSON.stringify({ model: 'one', stream: true, messages: [{ role: 'user', content: 'synthetic' }], ...extra }) });
  let captured;
  const response = await handle(request({ thinking: { type: 'enabled' }, reasoning_effort: 'high' }), env, async (_, init) => { captured = JSON.parse(init.body); return new Response('data: [DONE]\n\n', { headers: { 'content-type': 'text/event-stream' } }); });
  assert.equal(response.status, 200); assert.equal(captured.reasoning_effort, 'high'); assert.deepEqual(captured.thinking, { type: 'enabled' });
  for (const extra of [{ reasoning_effort: 2 }, { reasoning_effort: 'a'.repeat(33) }, { thinking: { type: 'disabled' }, reasoning_effort: 'high' }]) assert.equal((await handle(request(extra), env, noFetch)).status, 400);
});
