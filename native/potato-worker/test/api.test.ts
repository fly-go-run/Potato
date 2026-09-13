import test from 'node:test';
import assert from 'node:assert/strict';
import { handle } from '../src/index.ts';

const token = 'test-only-device-token-with-32-characters';
const env = { CLIENT_TOKEN: token, UPSTREAM_API_KEY: 'test-provider-key', UPSTREAM_URL: 'https://provider.example/v1/chat/completions', ALLOWED_MODELS: 'test-model', MAX_OUTPUT_TOKENS: '100', CHAT_RATE_LIMIT: { limit: async () => ({ success: true }) } };
function request(body = { model: 'test-model', stream: true, messages: [{ role: 'user', content: '你好' }] }, headers = {}) { return new Request('https://potato.example/v1/chat/completions', { method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${token}`, ...headers }, body: JSON.stringify(body) }); }
const noFetch = async () => { throw new Error('Unexpected upstream request'); };
test('unauthenticated requests cannot reach upstream', async () => {
  const response = await handle(request(undefined, { authorization: 'Bearer invalid' }), env, noFetch);
  assert.equal(response.status, 401);
});
test('missing secrets fail closed', async () => { assert.equal((await handle(request(), { ...env, CLIENT_TOKEN: '' }, noFetch)).status, 503); });
test('small client output budget is preserved and invalid budgets are rejected', async () => {
  const body = { model: 'test-model', stream: true, messages: [{ role: 'user', content: 'OK' }], max_tokens: 16, thinking: { type: 'disabled', private_extra: 'drop' } };
  const response = await handle(request(body), env, async (_, init) => {
    assert.equal(JSON.parse(init.body).max_tokens, 16);
    assert.deepEqual(JSON.parse(init.body).thinking, { type: 'disabled' });
    return new Response('data: [DONE]\n\n', { headers: { 'content-type': 'text/event-stream' } });
  });
  assert.equal(response.status, 200);
  for (const limit of [0, -1, 1.5, '16']) assert.equal((await handle(request({ ...body, max_tokens: limit }), env, noFetch)).status, 400);
});
test('model allowlist and rate limit', async () => {
  assert.equal((await handle(request({ model: 'other', stream: true, messages: [] }), env, noFetch)).status, 400);
  assert.equal((await handle(request(), { ...env, CHAT_RATE_LIMIT: { limit: async () => ({ success: false }) } }, noFetch)).status, 429);
});
test('untrusted remote image URLs are rejected', async () => {
  const body = { model: 'test-model', stream: true, messages: [{ role: 'user', content: [{ type: 'image_url', image_url: { url: 'http://internal.example/secret' } }] }] };
  assert.equal((await handle(request(body), env, noFetch)).status, 400);
});
test('stream forwards content, replaces auth, drops extra options', async () => {
  let captured;
  const response = await handle(request({ model: 'test-model', stream: true, messages: [{ role: 'user', content: '你好' }], tools: [{ type: 'dangerous' }], max_tokens: 999999 }), env, async (url, init) => {
    assert.equal(url.toString(), env.UPSTREAM_URL); captured = init;
    return new Response('data: {"choices":[{"delta":{"content":"你好"}}]}\n\ndata: [DONE]\n\n', { headers: { 'content-type': 'text/event-stream' } });
  });
  assert.equal(response.status, 200); assert.match(await response.text(), /你好/);
  assert.equal(captured.headers.Authorization, 'Bearer test-provider-key');
  assert.equal(captured.redirect, 'manual');
  const body = JSON.parse(captured.body); assert.equal(body.max_tokens, 100); assert.equal(body.tools, undefined);
});
test('provider errors are sanitized, no leaked provider body', async () => {
  const response = await handle(request(), env, async () => new Response('secret account details', { status: 401 }));
  assert.equal(response.status, 502); assert.doesNotMatch(await response.text(), /secret account/);
});
test('oversized body rejected even without content-length', async () => {
  const body = { model: 'test-model', stream: true, messages: [{ role: 'user', content: 'x'.repeat(4 * 1024 * 1024) }] };
  assert.equal((await handle(request(body), env, noFetch)).status, 413);
});
test('invalid JSON and unsupported routes', async () => {
  const bad = new Request('https://potato.example/v1/chat/completions', { method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` }, body: '{broken' });
  assert.equal((await handle(bad, env, noFetch)).status, 400);
  assert.equal((await handle(new Request('https://potato.example/notfound'), env, noFetch)).status, 404);
});

const historyCall = (id: string) => ({ role: 'assistant', content: '', tool_calls: [{ id, type: 'function', function: { name: 'web_search', arguments: '{"query":"potato"}' } }] });
const historyResult = (id: string) => ({ role: 'tool', tool_call_id: id, content: '{"results":[]}' });
const historyBody = (messages: unknown[]) => ({ model: 'test-model', stream: true, messages });
test('paired tool history is forwarded intact through direct and search paths', async () => {
  const messages = [historyCall('search-1'), historyResult('search-1'), { role: 'assistant', content: 'Previous answer' }, { role: 'user', content: 'Continue' }];
  for (const search of [false, true]) {
    let forwarded;
    const response = await handle(request(historyBody(messages)), { ...env, ...(search ? { EXA_API_KEY: 'fixture' } : {}) }, async (_, init) => {
      forwarded = JSON.parse(init.body).messages;
      return new Response('data: {"choices":[{"delta":{"content":"ok"}}]}\n\ndata: [DONE]\n\n', { headers: { 'content-type': 'text/event-stream' } });
    });
    assert.equal(response.status, 200); await response.text();
    assert.deepEqual(forwarded.slice(-messages.length), messages);
  }
});
for (const [name, messages] of [
  ['missing result', [historyCall('a')]],
  ['extra tool result', [historyCall('a'), historyResult('a'), historyResult('extra')]],
  ['duplicate ID', [historyCall('a'), historyResult('a'), historyCall('a'), historyResult('a')]],
  ['nonadjacent tool result', [historyCall('a'), { role: 'assistant', content: 'interruption' }, historyResult('a')]],
] as const) test(`invalid tool history: ${name}`, async () => {
  const response = await handle(request(historyBody([...messages])), env, noFetch);
  assert.equal(response.status, 400);
  assert.equal((await response.json()).error.message, 'Invalid tool history.');
});
