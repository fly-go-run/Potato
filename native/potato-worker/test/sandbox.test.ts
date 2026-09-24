import test from 'node:test';
import assert from 'node:assert/strict';
import { validateSandbox, executeSandbox } from '../src/sandbox.ts';
import { handle } from '../src/index.ts';

test('sandbox validates paths, encoding, duplicates and aggregate input budget', () => {
  assert.deepEqual(validateSandbox({ code: 'print(1)', files: [{ name: 'input-1.csv', base64: 'YQ==' }] }).files.length, 1);
  for (const files of [[{ name: '../secret', base64: '' }], [{ name: 'a', base64: '!' }], [{ name: 'a', base64: '' }, { name: 'a', base64: '' }], [{ name: 'a', base64: 'AAAA'.repeat(700001) }]]) {
    assert.throws(() => validateSandbox({ code: 'print(1)', files }));
  }
});

function fixture(fail = false) {
  let killed = 0;
  const writes: string[] = [];
  const sandbox = {
    files: {
      makeDir: async () => {}, write: async (path: string) => { writes.push(path); },
      list: async () => [{ name: 'report.md', type: 'file' }],
      read: async () => new ReadableStream({ start(controller) { controller.enqueue(new TextEncoder().encode('# Report')); controller.close(); } })
    },
    runCode: async () => { if (fail) throw new Error('provider-private-error'); return { logs: { stdout: ['2'], stderr: [] }, results: [{ png: 'YQ==', text: '2' }] }; },
    kill: async () => { killed++; return true; }
  };
  return { sandbox, writes, killed: () => killed };
}
test('execution uses isolated no-internet sandbox, returns artifacts and always destroys it', async () => {
  const f = fixture();
  const result = await executeSandbox({ code: 'print(2)', files: [{ name: 'input-1.csv', base64: 'YQ==' }] }, 'test-key', new AbortController().signal, async options => {
    assert.equal(options.template, 'chat-web-office-pdf'); assert.equal(options.allowInternetAccess, false); assert.equal(options.timeoutMs, 120000); assert.equal('envs' in options, false); return f.sandbox;
  });
  assert.equal(result.status, 'complete'); assert.equal(result.artifacts.length, 2);
  assert.equal(result.artifacts[1].base64, Buffer.from('# Report').toString('base64'));
  assert.deepEqual(f.writes, ['/home/user/input-1.csv']); assert.equal(f.killed(), 1);
});
test('execution failure and cancellation destroy the sandbox', async () => {
  const f = fixture(true);
  await assert.rejects(executeSandbox({ code: '1', files: [] }, 'test-key', new AbortController().signal, async () => f.sandbox));
  assert.equal(f.killed(), 1);
  const controller = new AbortController(); controller.abort();
  await assert.rejects(executeSandbox({ code: '1', files: [] }, 'test-key', controller.signal, async () => { throw new Error('must not create'); }), { name: 'AbortError' });
});
test('sandbox endpoint requires device auth and fails explicitly when E2B is unconfigured', async () => {
  const env = { CLIENT_TOKEN: 'a'.repeat(32), UPSTREAM_API_KEY: 'not-exported', CHAT_RATE_LIMIT: { limit: async () => ({ success: true }) } };
  const request = (token: string) => new Request('https://potato.example/v1/sandbox/run', { method: 'POST', headers: { authorization: 'Bearer ' + token } });
  assert.equal((await handle(request('wrong'), env)).status, 401);
  const response = await handle(request(env.CLIENT_TOKEN), env);
  assert.equal(response.status, 503); assert.match(await response.text(), /not configured/);
});

test('PPTX files are returned with presentation MIME type', async () => {
  const f = fixture();
  f.sandbox.files.list = async () => [{ name: 'deck.pptx', type: 'file' }];
  const result = await executeSandbox({ code: 'pass', files: [] }, 'test-key', new AbortController().signal, async () => f.sandbox);
  assert.equal(result.artifacts[1].name, 'deck.pptx');
  assert.equal(result.artifacts[1].mime, 'application/vnd.openxmlformats-officedocument.presentationml.presentation');
});
