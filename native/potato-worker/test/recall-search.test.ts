import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { searchConversations } from '../src/recall-search.ts';
import type { RecallExecutor } from '../src/recall.ts';

const fixture = (name: string) => JSON.parse(readFileSync(new URL(`./fixtures/${name}.json`, import.meta.url), 'utf8'));
const corpus = fixture('recall-corpus'), expected = fixture('recall-expected');
const execute: RecallExecutor = searchConversations;
const canonical = (value: unknown): string => JSON.stringify(value, (_, v) =>
  v && typeof v === 'object' && !Array.isArray(v) ? Object.fromEntries(Object.keys(v).sort().map(k => [k, v[k]])) : v);
for (const [index, query] of corpus.queries.entries()) {
  test(`Python equivalence: ${query.name}`, async () => {
    const actual = await execute({ ...query.input, conversations: corpus.conversations }, new AbortController().signal);
    const reference = expected[index].result;
    assert.equal(expected[index].name, query.name);
    if (query.name === 'case') {
      // Known exception: Python casefold maps Straße to strasse; JS lower-case does not.
      assert.equal(reference.sources.length, 1);
      assert.equal(reference.sources[0].id, corpus.conversations[9].messages[0].id);
      assert.equal(actual.sources.length, 0);
      assert.equal(canonical(actual), canonical({ ...reference, sources: [] }));
      const searchCase = (q: string) => execute({ query: q, conversations: corpus.conversations }, new AbortController().signal);
      assert.equal(canonical(await searchCase('CaMeRa')), canonical(await searchCase('camera')));
    } else {
      assert.equal(canonical(actual), canonical(reference));
    }
  });
}

test('truncates at 4000 code points and preserves Python tie order', async () => {
  const result = await execute({ query: '😀', conversations: corpus.conversations }, new AbortController().signal);
  const long = result.sources.find(s => s.id === corpus.conversations[10].messages[0].id)!;
  assert.equal(Array.from(long.text).length, 4000);
  assert.ok(long.text.endsWith('😀'));
  assert.ok(long.text.length > 4000);
  assert.ok(result.sources.every(s => !('score' in s)));
  const tied = corpus.conversations.map((c: any) => ({ ...c, messages: [c.messages[1]] }));
  const first = await execute({ query: '', conversations: tied }, new AbortController().signal);
  const second = await execute({ query: '', offset: 8, conversations: tied }, new AbortController().signal);
  assert.deepEqual([...first.sources, ...second.sources].map(s => s.id), tied.map((c: any) => c.messages[0].id));
  assert.equal(first.more, true); assert.equal(second.more, false);
});

test('checks cancellation initially and every 50 messages, including filtered messages', async () => {
  await assert.rejects(execute({ query: '', conversations: [] }, AbortSignal.abort()), { name: 'AbortError' });
  const controller = new AbortController();
  let checks = 0;
  const signal = { throwIfAborted() { if (++checks === 3) controller.abort(); controller.signal.throwIfAborted(); } } as AbortSignal;
  const chat = corpus.conversations[0];
  await assert.rejects(execute({ query: '', start: '2099', conversations: [{ ...chat, messages: Array(150).fill(chat.messages[0]) }] }, signal), { name: 'AbortError' });
  assert.equal(checks, 3);
});

test('CPU measurement: eight searches over ten 150 KB conversations (no timing assertion)', async () => {
  const conversations = Array.from({ length: 10 }, (_, i) => {
    const c = { ...corpus.conversations[i], messages: [{ ...corpus.conversations[i].messages[0], text: '' }] };
    // Fill the serialized conversation to exactly 150,000 UTF-8 bytes.
    c.messages[0].text = '';
    const budget = 150_000 - Buffer.byteLength(JSON.stringify(c));
    const unit = 'camera 轻便 😀 ';
    const repeats = Math.floor(budget / Buffer.byteLength(unit));
    c.messages[0].text = unit.repeat(repeats) + 'x'.repeat(budget - repeats * Buffer.byteLength(unit));
    return c;
  });
  const begin = process.cpuUsage();
  for (let i = 0; i < 8; i++) await execute({ query: 'camera 轻便', conversations }, new AbortController().signal);
  const used = process.cpuUsage(begin);
  console.log(`recall CPU: user=${(used.user / 1000).toFixed(3)} ms system=${(used.system / 1000).toFixed(3)} ms total=${((used.user + used.system) / 1000).toFixed(3)} ms; 8 searches, 10 x 150000 bytes`);
});
