import { before, after, test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { Miniflare, convertV4MiniflareOptions } from 'miniflare';
import { randomUUID } from 'node:crypto';

let mf: Miniflare;
const calls = new Map<string, number>();
const frame = (content: string, finish?: string) => `data: ${JSON.stringify({ choices: [{ delta: { content }, finish_reason: finish }] })}\n\n`;
before(async () => {
  const bundle = await build({ stdin: { contents: `
    export { ChatJob } from './src/chat-job.ts';
    import { handle } from './src/index.ts';
    import { DurableObject } from 'cloudflare:workers';
    export class Identity extends DurableObject {
      async cloudIdentity() { return { email: 'fixture@example.test' }; }
    }
    export default { fetch: (request, env) => handle(request, env) };
  `, resolveDir: process.cwd() }, bundle: true, write: false, format: 'esm', platform: 'browser', external: ['cloudflare:workers', 'node:*'] });
  mf = new Miniflare(convertV4MiniflareOptions({ workers: [{
    name: 'chat-job-fixture', modules: true, script: bundle.outputFiles[0].text,
    compatibilityDate: '2026-09-12', compatibilityFlags: ['nodejs_compat'],
    bindings: { CLOUD_ALLOWED_EMAILS: 'fixture@example.test', MAX_OUTPUT_TOKENS: '4096', CLOUD_PROVIDERS: JSON.stringify({ default_model: 'fixture/one', providers: [{ id: 'fixture', name: 'Fixture', endpoint: 'https://model.invalid/chat/completions', api_key: 'synthetic', models: [{ id: 'one', name: 'One' }] }] }) },
    durableObjects: { CHAT_JOBS: { className: 'ChatJob', useSQLite: true }, REMOTE_ACCOUNTS: { className: 'Identity', useSQLite: true } },
    ratelimits: { CHAT_RATE_LIMIT: { namespace_id: '1', simple: { limit: 10000, period: 60 } }, REMOTE_RATE_LIMIT: { namespace_id: '2', simple: { limit: 10000, period: 60 } } },
    outboundService: async (request: Request) => {
      const body = await request.json() as { messages: { content: string }[] };
      const text = body.messages[0].content;
      calls.set(text, (calls.get(text) ?? 0) + 1);
      const content = text.startsWith('large') ? '中🌱'.repeat(20000) : '完整回复🌱';
      const stream = new ReadableStream({ async start(c) {
        c.enqueue(new TextEncoder().encode(frame('开头：')));
        await new Promise(r => setTimeout(r, 350));
        try {
          c.enqueue(new TextEncoder().encode(frame(content, text.startsWith('limited') ? 'length' : 'stop')));
          if (!text.startsWith('broken')) c.enqueue(new TextEncoder().encode('data: [DONE]\n\n'));
          c.close();
        } catch { /* cancellation is expected in stop tests */ }
      } });
      return new Response(stream, { headers: { 'Content-Type': 'text/event-stream' } });
    },
  }] }));
  await mf.ready;
});
after(async () => { await mf?.dispose(); });
function request(id: string, method = 'GET', text?: string, owner = 'a') {
  return mf.dispatchFetch(`http://localhost/v1/chat/jobs/${id}`, { method,
    headers: { 'Authorization': `Bearer ${owner.repeat(64)}.${'0'.repeat(8)}-0000-0000-0000-${'0'.repeat(12)}.${'b'.repeat(64)}`, 'Content-Type': 'application/json' },
    ...(text === undefined ? {} : { body: JSON.stringify({ model: 'fixture/one', stream: true, messages: [{ role: 'user', content: text }] }) }),
  });
}
async function complete(id: string) {
  for (let attempt = 0; attempt < 150; attempt++) {
    const response = await request(id); assert.equal(response.status, 200);
    const page = await response.json() as any;
    if (!['queued', 'running'].includes(page.state)) return page;
    await new Promise(r => setTimeout(r, 50));
  }
  assert.fail('Generation did not finish');
}
test('accepted reply finishes with no reader; lost receipt + concurrent retry never duplicate generation', async () => {
  const id = randomUUID(), text = 'detached-' + id;
  const results = await Promise.all([request(id, 'PUT', text), request(id, 'PUT', text)]);
  assert.ok(results.every(r => [200, 202].includes(r.status)));
  // Do not read the job while the provider runs: simulates a terminated app.
  await new Promise(r => setTimeout(r, 1200));
  const page = await complete(id);
  assert.equal(page.state, 'complete'); assert.equal(calls.get(text), 1);
  assert.equal(page.events.map((e: any) => JSON.parse(e.data).choices[0].delta.content).join(''), '开头：完整回复🌱');
  assert.equal((await request(id, 'PUT', text)).status, 200);
  assert.equal(calls.get(text), 1);
  assert.equal((await request(id, 'PUT', text + 'different')).status, 409);
  const tail = await (await request(id + '?after=1')).json() as any;
  assert.deepEqual(tail.events.map((e: any) => e.seq), [2]);
  assert.equal((await request(id + '?after=99')).status, 400);
});
test('authenticated accounts cannot read or cancel another account job', async () => {
  const id = randomUUID(); await request(id, 'PUT', id);
  assert.equal((await request(id, 'GET', undefined, 'c')).status, 404);
  assert.equal((await mf.dispatchFetch(`http://localhost/v1/chat/jobs/${id}`)).status, 401);
  await request(id + '/cancel', 'POST', undefined, 'c');
  assert.equal((await complete(id)).state, 'complete');
});
test('stop before submit leaves a tombstone, preventing late network submission from starting a model', async () => {
  const id = randomUUID(); await request(id + '/cancel', 'POST');
  await request(id, 'PUT', id);
  assert.equal((await complete(id)).state, 'stopped'); assert.equal(calls.get(id), undefined);
});
test('explicit stop during generation is terminal and preserves checkpointed output', async () => {
  const id = randomUUID(); await request(id, 'PUT', id);
  for (let n = 0; n < 100; n++) {
    const page = await (await request(id)).json() as any;
    if (page.events.length) break;
    await new Promise(r => setTimeout(r, 10));
  }
  const stopped = await (await request(id + '/cancel', 'POST')).json() as any;
  assert.equal(stopped.state, 'stopped');
  await new Promise(r => setTimeout(r, 500));
  assert.deepEqual(await complete(id), stopped);
});
test('upstream truncation and token limit are failures, never false completion', async () => {
  for (const mode of ['broken', 'limited']) {
    const id = randomUUID(); await request(id, 'PUT', mode + id);
    const page = await complete(id); assert.equal(page.state, 'failed'); assert.ok(page.events.length > 0);
  }
});
test('large events are stored in SQLite chunks without corrupting multibyte content', async () => {
  const id = randomUUID(); await request(id, 'PUT', 'large' + id);
  const page = await complete(id); assert.equal(page.state, 'complete');
  assert.equal(JSON.parse(page.events[1].data).choices[0].delta.content, '中🌱'.repeat(20000));
});

async function* pages(response: Response) {
  assert.equal(response.status, 200);
  assert.match(response.headers.get('content-type')!, /text\/event-stream/);
  const reader = response.body!.getReader(), decoder = new TextDecoder();
  let buffer = '';
  try {
    for (;;) {
      const next = await reader.read(); if (next.done) return;
      buffer += decoder.decode(next.value, { stream: true });
      let end: number;
      while ((end = buffer.indexOf('\n\n')) >= 0) {
        const frame = buffer.slice(0, end); buffer = buffer.slice(end + 2);
        if (frame.startsWith('data: ')) yield JSON.parse(frame.slice(6));
      }
    }
  } finally { await reader.cancel(); }
}
test('subscription delivers first output before completion; disconnect does not stop generation and reconnect replays only missing events', async () => {
  const id = randomUUID(); await request(id, 'PUT', id);
  let cursor = 0;
  for await (const page of pages(await request(id + '/events?after=0') as unknown as Response)) {
    if (page.events.length) {
      assert.equal(page.state, 'running'); assert.equal(page.events[0].seq, 1);
      cursor = page.events.at(-1).seq; break;
    }
  }
  assert.equal(cursor, 1);
  const received = [];
  for await (const page of pages(await request(id + '/events?after=1') as unknown as Response)) received.push(page);
  assert.equal(received.at(-1).state, 'complete');
  assert.deepEqual(received.flatMap(p => p.events.map((e: any) => e.seq)), [2]);
  assert.equal(calls.get(id), 1);
});
test('subscription stop is terminal and multiple readers see identical persisted events', async () => {
  const id = randomUUID(); await request(id, 'PUT', id);
  const first = pages(await request(id + '/events') as unknown as Response);
  const second = pages(await request(id + '/events') as unknown as Response);
  for await (const page of first) {
    if (page.events.length) { await request(id + '/cancel', 'POST'); break; }
  }
  const received = [];
  for await (const page of second) received.push(page);
  assert.equal(received.at(-1).state, 'stopped');
  assert.equal(received.flatMap(p => p.events).length, 1);
  assert.equal((await complete(id)).state, 'stopped');
});
test('subscription validates owner, cursor and method; completed large Unicode replay stays intact', async () => {
  const id = randomUUID(); await request(id, 'PUT', 'large' + id); const result = await complete(id);
  assert.equal((await request(id + '/events', 'GET', undefined, 'c')).status, 404);
  assert.equal((await request(id + '/events?after=-1')).status, 400);
  assert.equal((await request(id + '/events?after=99')).status, 400);
  assert.equal((await request(id + '/events', 'POST')).status, 405);
  assert.equal((await mf.dispatchFetch(`http://localhost/v1/chat/jobs/${id}/events`)).status, 401);
  const events = [];
  for await (const page of pages(await request(id + '/events') as unknown as Response)) events.push(...page.events);
  assert.deepEqual(events, result.events);
});
