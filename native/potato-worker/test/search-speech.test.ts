import test from 'node:test';
import assert from 'node:assert/strict';
import { searchExa, searchQuery } from '../src/search.ts';
import { chatWithSearch } from '../src/search-chat.ts';
import { speechFrame, speechResult, speechConfiguration } from '../src/speech.ts';

test('Exa matches desktop limits and filters unsafe/duplicate source URLs', async () => {
  assert.throws(() => searchQuery({ query: ' ' })); assert.throws(() => searchQuery({ query: '中'.repeat(3000) }));
  const result = await searchExa('official docs', 'exa-test', new AbortController().signal, async (url, init) => {
    assert.equal(url, 'https://api.exa.ai/search'); assert.equal(init.headers['x-api-key'], 'exa-test'); assert.equal(init.redirect, 'manual');
    assert.deepEqual(JSON.parse(init.body), { query: 'official docs', numResults: 5, contents: { text: { maxCharacters: 2000 } } });
    return Response.json({ results: [{ title: 'Docs', url: 'https://example.com/docs', text: 'x'.repeat(3000) }, { url: 'javascript:alert(1)' }, { url: 'https://user:key@example.com/' }, { url: 'https://example.com/docs' }] });
  });
  assert.equal(result.results.length, 1); assert.equal(result.results[0].content.length, 2000);
});
const sse = (...values: unknown[]) => new Response(values.map(value => `data: ${typeof value === 'string' ? value : JSON.stringify(value)}\n\n`).join(''), { headers: { 'content-type': 'text/event-stream' } });
test('model decides to call Exa, streamed tool arguments are joined, reasoning preserved, only final DONE escapes', async () => {
  let modelCalls = 0, searches = 0;
  const response = await chatWithSearch({ model: 'selected-model', thinking: { type: 'enabled' }, reasoning_effort: 'high', messages: [{ role: 'user', content: 'current facts' }], max_tokens: 100, stream: true }, new URL('https://model.example/chat'), { UPSTREAM_API_KEY: 'model-key', EXA_API_KEY: 'exa-key', SEARCH_RATE_LIMIT: { limit: async () => ({ success: true }) } }, new AbortController().signal, async (url, init) => {
    if (String(url).includes('api.exa.ai')) { searches++; return Response.json({ results: [{ title: 'Source', url: 'https://example.com', text: 'verified data' }] }); }
    const body = JSON.parse(init.body); assert.deepEqual(body.tools.map(t => t.function.name), ['web_search']);
    assert.equal(body.model, 'selected-model'); assert.deepEqual(body.thinking, { type: 'enabled' }); assert.equal(body.reasoning_effort, 'high');
    if (modelCalls++ === 0) return sse({ choices: [{ delta: { reasoning_content: 'Need source', tool_calls: [{ index: 0, id: 'call_1', function: { name: 'web_search', arguments: '{"que' } }] } }] }, { choices: [{ delta: { tool_calls: [{ index: 0, function: { arguments: 'ry":"facts"}' } }] }, finish_reason: 'tool_calls' }] }, '[DONE]');
    assert.equal(body.messages.at(-2).reasoning_content, 'Need source'); assert.equal(body.messages.at(-1).role, 'tool'); assert.match(body.messages.at(-1).content, /Untrusted web content/);
    return sse({ choices: [{ delta: { content: 'Answer [Source](https://example.com)' }, finish_reason: 'stop' }] }, '[DONE]');
  });
  const text = await response.text(); assert.equal(searches, 1); assert.equal(modelCalls, 2); assert.equal(text.match(/\[DONE\]/g)?.length, 1); assert.match(text, /"state":"searching"/); assert.match(text, /"state":"complete"/); assert.match(text, /Answer/);
  assert.match(text, /"reasoning_content":"Need source"/); assert.doesNotMatch(text, /"tool_calls"/);
});
test('reasoning and body in one frame both reach the phone before a length failure', async () => {
  const response = await chatWithSearch({ messages: [] }, new URL('https://model.example'), { UPSTREAM_API_KEY: 'key' }, new AbortController().signal,
    async () => sse({ choices: [{ delta: { reasoning_content: '比较🌱', content: '答复' }, finish_reason: 'length' }] }, '[DONE]'));
  const events = (await response.text()).trim().split('\n\n').map(line => JSON.parse(line.slice(6)));
  assert.deepEqual(events[0], { choices: [{ delta: { content: '答复', reasoning_content: '比较🌱' } }] });
  assert.equal(events[1].choices[0].finish_reason, 'length');
});
test('reasoning output limit is checked before forwarding an oversized event', async () => {
  const response = await chatWithSearch({ messages: [] }, new URL('https://model.example'), { UPSTREAM_API_KEY: 'key' }, new AbortController().signal,
    async () => sse({ choices: [{ delta: { reasoning_content: 'x'.repeat(160001) } }] }, '[DONE]'));
  const output = await response.text(); assert.match(output, /"error"/); assert.doesNotMatch(output, /reasoning_content|\[DONE\]/);
});
test('reasoning and body use one UTF-8 byte budget across search rounds', async () => {
  let rounds = 0;
  const response = await chatWithSearch({ messages: [] }, new URL('https://model.example'), { UPSTREAM_API_KEY: 'key', EXA_API_KEY: 'key', SEARCH_RATE_LIMIT: { limit: async () => ({ success: true }) } }, new AbortController().signal, async url => {
    if (String(url).includes('api.exa.ai')) return Response.json({ results: [] });
    const index = rounds++;
    return sse({ choices: [{ delta: { content: '文'.repeat(100000), reasoning_content: '思'.repeat(100000), tool_calls: [{ index: 0, id: `call_${index}`, function: { name: 'web_search', arguments: '{"query":"facts"}' } }] }, finish_reason: 'tool_calls' }] }, '[DONE]');
  });
  const output = await response.text(); assert.equal(rounds, 4); assert.match(output, /"error"/); assert.doesNotMatch(output, /\[DONE\]/);
  const deltas = output.split('\n\n').filter(line => line.startsWith('data: {')).map(line => JSON.parse(line.slice(6)).choices?.[0]?.delta).filter(Boolean);
  assert.equal(deltas.reduce((n, delta) => n + Buffer.byteLength((delta.content ?? '') + (delta.reasoning_content ?? '')), 0), 1800000);
});
test('phone cancellation aborts the upstream reasoning stream', async () => {
  let upstreamAborted = false;
  const response = await chatWithSearch({ messages: [] }, new URL('https://model.example'), { UPSTREAM_API_KEY: 'key' }, new AbortController().signal, async (_, init) => {
    const body = new ReadableStream({ start(controller) {
      controller.enqueue(new TextEncoder().encode('data: {"choices":[{"delta":{"reasoning_content":"started"}}]}\n\n'));
      init.signal.addEventListener('abort', () => { upstreamAborted = true; controller.error(new Error('aborted')); }, { once: true });
    } });
    return new Response(body, { headers: { 'content-type': 'text/event-stream' } });
  });
  const reader = response.body.getReader(); assert.match(new TextDecoder().decode((await reader.read()).value), /reasoning_content/);
  await reader.cancel(); assert.equal(upstreamAborted, true);
});
test('simple answer does not call Exa, interrupted model stream is an error', async () => {
  for (const complete of [true, false]) {
    let count = 0;
    const response = await chatWithSearch({ messages: [], max_tokens: 100 }, new URL('https://model.example'), { UPSTREAM_API_KEY: 'key' }, new AbortController().signal, async url => {
      count++; assert.equal(String(url), 'https://model.example/');
      return sse({ choices: [{ delta: { content: 'hello' } }] }, ...(complete ? ['[DONE]'] : []));
    });
    const output = await response.text(); assert.equal(count, 1); assert.equal(output.includes('"error"'), !complete);
  }
});
test('batched model calls retain all tool results while the four-search budget spans rounds', async () => {
  let modelCalls = 0, searches = 0;
  const response = await chatWithSearch({ messages: [], stream: true }, new URL('https://model.example'), { UPSTREAM_API_KEY: 'key', EXA_API_KEY: 'key', SEARCH_RATE_LIMIT: { limit: async () => ({ success: true }) } }, new AbortController().signal, async (url, init) => {
    if (String(url).includes('api.exa.ai')) { searches++; return Response.json({ results: [{ title: 'Source', url: 'https://example.com', text: 'data' }] }); }
    const body = JSON.parse(init.body), round = modelCalls++;
    if (round === 1) assert.deepEqual(body.messages.slice(-2).map(message => message.tool_call_id), ['call_0_0', 'call_0_1']);
    if (round === 2) {
      assert.equal(body.tool_choice, 'none'); assert.equal(searches, 4);
      assert.deepEqual(body.messages.slice(-3).map(message => message.tool_call_id), ['call_1_0', 'call_1_1', 'call_1_2']);
      assert.match(body.messages.at(-1).content, /budget exhausted/);
      return sse({ choices: [{ delta: { content: 'Final answer' }, finish_reason: 'stop' }] }, '[DONE]');
    }
    return sse({ choices: [{ delta: { tool_calls: Array.from({ length: round === 0 ? 2 : 3 }, (_, index) => ({ index, id: `call_${round}_${index}`, function: { name: 'web_search', arguments: JSON.stringify({ query: `query ${round} ${index}` }) } })) }, finish_reason: 'tool_calls' }] }, '[DONE]');
  });
  const output = await response.text(); assert.equal(searches, 4); assert.equal(modelCalls, 3);
  assert.equal(output.match(/"state":"complete"/g)?.length, 4); assert.equal(output.match(/\[DONE\]/g)?.length, 1); assert.match(output, /Final answer/); assert.doesNotMatch(output, /"error"/);
});
test('Doubao framing preserves partial/final semantics and rejects truncated or oversized frames', () => {
  const content = new TextEncoder().encode(JSON.stringify({ result: { text: '你好', utterances: [{ definite: true }] } }));
  assert.deepEqual(speechResult(speechFrame(9, 0, true, content)), { type: 'partial', text: '你好' });
  assert.deepEqual(speechResult(speechFrame(9, 2, true, content)), { type: 'final', text: '你好' });
  assert.throws(() => speechResult(new Uint8Array([0x11, 0x90, 0x11, 0, 0, 0, 0, 99])));
  assert.throws(() => speechResult(speechFrame(9, 0, true, new TextEncoder().encode('x'.repeat(1_000_001)))));
  assert.equal(speechFrame(2, 2, false, new Uint8Array())[1], 0x22); assert.equal(speechConfiguration.audio.rate, 16000);
});
