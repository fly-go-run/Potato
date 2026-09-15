import { searchConversations } from '../src/recall-search.ts';
import { before, after, test } from 'node:test';
import assert from 'node:assert/strict';
import { Miniflare, convertV4MiniflareOptions } from 'miniflare';
import { randomUUID } from 'node:crypto';
import { RecallStore, RecallSession, validateConversation } from '../src/recall.ts';
import { chatWithSearch } from '../src/search-chat.ts';
import { handle } from '../src/index.ts';
let mf: Miniflare, bucket: any;
before(async () => {
  mf = new Miniflare(convertV4MiniflareOptions({ workers: [{ name: 'recall-test', modules: true, script: 'export default { fetch() { return new Response("ok") } }', r2Buckets: ['RECALL'], compatibilityDate: '2026-09-12' }] }));
  bucket = await mf.getR2Bucket('RECALL');
});
after(async () => { await mf?.dispose(); });
function fixture() { return { id: randomUUID(), title: '购买相机', excluded: false, messages: [{ id: randomUUID(), role: 'user', text: '我选择买 X100，平时偏好轻便相机。', date: '2026-09-12T10:00:00.000Z' }] }; }
const sync = (store: RecallStore, chat: unknown, base: string | null = null) => store.sync({ content: JSON.stringify(chat), base });
function source(chat: any, revision: string) { return { ...chat.messages[0], conversation: chat.id, title: chat.title, revision }; }
test('immutable synchronization is isolated, idempotent, CAS guarded, and removes superseded files', async () => {
  const store = new RecallStore(bucket, randomUUID()), other = new RecallStore(bucket, randomUUID()), chat = fixture();
  const first = await sync(store, chat); assert.deepEqual(await sync(store, chat), first);
  assert.equal(Object.keys((await other.status()).entries).length, 0);
  await assert.rejects(other.conversation(chat.id), { status: 404 });
  chat.title = 'new'; await assert.rejects(sync(store, chat), { status: 409 });
  const outcomes = await Promise.allSettled([sync(store, chat, first.revision), sync(store, { ...chat, title: 'competing' }, first.revision)]);
  assert.equal(outcomes.filter(r => r.status === 'fulfilled').length, 1);
  assert.equal((await bucket.list({ prefix: store.prefix + 'conversations/' })).objects.length, 1);
  const current = (await store.status()).entries[chat.id];
  await sync(store, { ...chat, excluded: true }, current.revision);
  await assert.rejects(store.conversation(chat.id), { status: 404 });
  const objects = await bucket.list({ prefix: store.prefix + 'conversations/' });
  assert.deepEqual((await (await bucket.get(objects.objects[0].key)).json()).messages, []);
});
test('memories survive appended turns, invalidate edited sources, and respect explicit forgetting', async () => {
  const store = new RecallStore(bucket, randomUUID()), chat = fixture(); let receipt = await sync(store, chat);
  await store.remember('偏好轻便相机', [source(chat, receipt.revision)]);
  chat.messages.push({ ...chat.messages[0], id: randomUUID(), text: '谢谢', date: '2026-09-13T00:00:00.000Z' });
  receipt = await sync(store, chat, receipt.revision);
  assert.equal((await store.status()).memories.length, 1);
  chat.messages[0].text = '我还没有决定买什么'; receipt = await sync(store, chat, receipt.revision);
  assert.equal((await store.status()).memories.length, 0);
  await store.remember('暂未决定相机', [source(chat, receipt.revision)]);
  const memory = (await store.status()).memories[0];
  await store.memory({ id: memory.id, base: memory.revision, text: '', forget: true });
  await assert.rejects(store.remember('不同措辞重新提取', [source(chat, receipt.revision)]), { status: 409 });
  assert.equal((await store.status()).memories.length, 0);
});
test('source verification removes deleted data and failed/aborted answers do not commit memories', async () => {
  const store = new RecallStore(bucket, randomUUID()), chat = fixture(), receipt = await sync(store, chat);
  const session = new RecallSession(store, searchConversations, true);
  const signal = new AbortController().signal;
  await session.call('search_conversations', { query: '相机' }, signal);
  await session.call('remember', { text: '偏好轻便', source_ids: [chat.messages[0].id] }, signal);
  assert.equal((await store.status()).memories.length, 0);
  const aborted = AbortSignal.abort(); await assert.rejects(session.commit(aborted));
  await sync(store, { ...chat, excluded: true }, receipt.revision);
  assert.equal((await store.verify(session.sources)).length, 0);
  await assert.rejects(session.commit(signal));
});
test('Worker searches Chinese safely, uses message timestamps and reports pagination', async () => {
  const chat = fixture();
  chat.messages.push({ ...chat.messages[0], id: randomUUID(), text: '助手之前推荐过富士', date: '2026-09-13T10:00:00.000Z' });
  const run = (query: string) => searchConversations({ query, start: '2026-09-12T00:00:00.000Z', end: '2026-09-13T00:00:00.000Z', conversations: [{ ...chat, revision: 'a'.repeat(64) }] }, new AbortController().signal);
  assert.equal((await run('相机')).sources.length, 1); assert.equal((await run('')).sources.length, 1);
  assert.equal((await run("'); __import__('os').system('false'); #")).sources.length, 0);
  assert.throws(() => validateConversation('{'), { status: 400 });
});
const sse = (value: unknown) => new Response(`data: ${JSON.stringify(value)}\n\ndata: [DONE]\n\n`, { headers: { 'Content-Type': 'text/event-stream' } });
test('recall works without Exa and emits verified source cards while hiding tool calls', async () => {
  const store = new RecallStore(bucket, randomUUID()), chat = fixture(), receipt = await sync(store, chat);
  const session = new RecallSession(store, searchConversations);
  let calls = 0;
  const fetcher: any = async (_: unknown, request: any) => {
    const body = JSON.parse(request.body); assert.ok(body.tools.some((t: any) => t.function.name === 'search_conversations')); assert.ok(!body.tools.some((t: any) => t.function.name === 'web_search'));
    if (!calls++) return sse({ choices: [{ delta: { tool_calls: [{ index: 0, id: 'history', function: { name: 'search_conversations', arguments: '{"query":"相机"}' } }] }, finish_reason: 'tool_calls' }] });
    assert.ok(body.messages.at(-1).content.includes('X100')); return sse({ choices: [{ delta: { content: '昨天你选择了 X100。' }, finish_reason: 'stop' }] });
  };
  const response = await chatWithSearch({ messages: [{ role: 'user', content: '昨天选了什么？' }] }, new URL('https://model.invalid/chat'), {} as any, new AbortController().signal, fetcher, 'fake', 'fake', session);
  const result = await response.text(); assert.ok(result.includes('potato_recall')); assert.ok(result.includes(chat.messages[0].id)); assert.ok(result.endsWith('data: [DONE]\n\n')); assert.ok(!result.includes('tool_calls'));
});
test('recall routes authenticate and ignore a caller-supplied owner', async () => {
  const limit = { limit: async () => ({ success: true }) };
  const env: any = { CLIENT_TOKEN: 'x'.repeat(40), UPSTREAM_API_KEY: 'fake', RECALL_BUCKET: bucket, RECALL_RATE_LIMIT: limit };
  const request = (token: string) => new Request('https://fixture.invalid/v1/recall/status?owner=someone-else', { headers: { Authorization: 'Bearer ' + token } });
  assert.equal((await handle(request('wrong'), env)).status, 401);
  const response = await handle(request(env.CLIENT_TOKEN), env); assert.equal(response.status, 200);
  assert.equal((await response.json() as any).scope, new RecallStore(bucket, 'personal-client').prefix);
});

test('disabled automatic memory hides both memory update tools', async () => {
  const session = new RecallSession(new RecallStore(bucket, randomUUID()), searchConversations, false);
  const response = await chatWithSearch({ messages: [{ role: 'user', content: 'hello' }] }, new URL('https://model.invalid/chat'), {} as any, new AbortController().signal, async (_, init) => {
    const names = JSON.parse(init!.body as string).tools.map((tool: any) => tool.function.name);
    assert.ok(names.includes('search_memory'));
    assert.ok(!names.includes('remember')); assert.ok(!names.includes('forget_memory'));
    return sse({ choices: [{ delta: { content: 'hello' } }] });
  }, 'fake', 'fake', session);
  await response.text();
});

test('disabled automatic memory rejects direct forget_memory calls', async () => {
  const session = new RecallSession(new RecallStore(bucket, randomUUID()), searchConversations, false);
  await assert.rejects(session.call('forget_memory', { id: randomUUID() }, new AbortController().signal), /disabled/);
});

test('memories with empty sources remain valid independently of conversation state', async () => {
  const store = new RecallStore(bucket, randomUUID()), chat = fixture();
  const memory = await store.memory({ id: randomUUID(), text: '个人偏好', forget: false, base: null });
  assert.deepEqual(memory.sources, []);
  const valid = async () => assert.deepEqual(await store.validMemories((await store.load()).state), [memory]);
  await valid();
  let receipt = await sync(store, chat); await valid();
  chat.messages[0].text = 'changed'; receipt = await sync(store, chat, receipt.revision); await valid();
  await sync(store, { ...chat, excluded: true }, receipt.revision); await valid();
  await store.mutate(state => { delete state.entries[chat.id]; }); await valid();
  await store.memory({ id: memory.id, text: '', forget: true, base: memory.revision });
  assert.deepEqual(await store.validMemories((await store.load()).state), []);
});

test('POST recall memory forget requires text, and accepts an empty string', async () => {
  const limit = { limit: async () => ({ success: true }) };
  const env: any = { CLIENT_TOKEN: 'x'.repeat(40), UPSTREAM_API_KEY: 'fake', RECALL_BUCKET: bucket, RECALL_RATE_LIMIT: limit };
  const post = (body: unknown) => handle(new Request('https://fixture.invalid/v1/recall/memory', {
    method: 'POST', headers: { Authorization: `Bearer ${env.CLIENT_TOKEN}`, 'Content-Type': 'application/json' }, body: JSON.stringify(body),
  }), env);
  const id = randomUUID();
  const created = await post({ id, text: 'manual memory', forget: false, base: null });
  assert.equal(created.status, 200);
  const memory = await created.json() as any;
  assert.equal((await post({ id, forget: true, base: memory.revision })).status, 400);
  const forgotten = await post({ id, text: '', forget: true, base: memory.revision });
  assert.equal(forgotten.status, 200); assert.equal((await forgotten.json() as any).forgotten, true);
});

test('chat recall needs only R2 and never charges sandbox rate limits', async () => {
  const store = new RecallStore(bucket, 'personal-client'), chat = fixture();
  await sync(store, chat);
  let sandboxCharges = 0, calls = 0;
  const env: any = {
    CLIENT_TOKEN: 'x'.repeat(40), UPSTREAM_API_KEY: 'fake', RECALL_BUCKET: bucket,
    UPSTREAM_URL: 'https://model.invalid/chat', ALLOWED_MODELS: 'fixture', MAX_OUTPUT_TOKENS: '1024',
    CHAT_RATE_LIMIT: { limit: async () => ({ success: true }) },
    SANDBOX_RATE_LIMIT: { limit: async () => { sandboxCharges++; return { success: false }; } },
  };
  const fetcher: any = async (_: unknown, init: any) => {
    const body = JSON.parse(init.body);
    if (!calls++) return sse({ choices: [{ delta: { tool_calls: [{ index: 0, id: 'history', function: { name: 'search_conversations', arguments: '{"query":"相机"}' } }] }, finish_reason: 'tool_calls' }] });
    assert.ok(body.messages.at(-1).content.includes('X100'));
    return sse({ choices: [{ delta: { content: 'X100' }, finish_reason: 'stop' }] });
  };
  const response = await handle(new Request('https://fixture.invalid/v1/chat/completions', {
    method: 'POST', headers: { Authorization: `Bearer ${env.CLIENT_TOKEN}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ model: 'fixture', messages: [{ role: 'user', content: '相机' }], stream: true, recall: { enabled: true } }),
  }), env, fetcher);
  assert.equal(response.status, 200);
  const output = await response.text();
  assert.ok(output.includes(chat.messages[0].id)); assert.ok(output.endsWith('data: [DONE]\n\n'));
  assert.equal(calls, 2); assert.equal(sandboxCharges, 0);
});
