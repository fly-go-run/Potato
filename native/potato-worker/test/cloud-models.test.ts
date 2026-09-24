import test from 'node:test';
import assert from 'node:assert/strict';
import { handle } from '../src/index.ts';
import type { ModelSettings } from '../src/cloud-models.ts';
const owner = 'a'.repeat(64), session = '00000000-0000-4000-8000-000000000001', secret = 'b'.repeat(64);
const token = `${owner}.${session}.${secret}`;
const config = { default_model: 'deepseek/flash', providers: [
  { id: 'deepseek', name: 'DeepSeek', endpoint: 'https://deepseek.invalid/chat/completions', api_key: 'deepseek-secret', models: [{ id: 'flash', name: 'Flash', thinking_modes: ['enabled', 'disabled'] }] },
  { id: 'sub2api', name: 'sub2api', endpoint: 'https://sub2api.invalid/v1/chat/completions', api_key: 'sub2api-secret', models: [{ id: 'gpt-6', name: 'GPT-6', reasoning_effort_options: ['low'] }] }
] };
function environment(email = 'owner@example.test') {
  let saved: ModelSettings | null = null;
  return { CLOUD_PROVIDERS: JSON.stringify(config), CLOUD_ALLOWED_EMAILS: 'owner@example.test,guest@example.test', CLOUD_ADMIN_EMAILS: 'Owner@Example.test', MAX_OUTPUT_TOKENS: '4096',
    CHAT_RATE_LIMIT: { limit: async () => ({ success: true }) },
    REMOTE_ACCOUNTS: { getByName: () => ({ cloudIdentity: async () => ({ email }) }) },
    CLOUD_MODELS: { getByName: () => ({
      read: async () => saved,
      write: async (next, revision: number) => (saved?.revision ?? 0) !== revision ? null : (saved = { ...next, revision: revision + 1 }),
    }) } };
}
const req = (path: string, method = 'GET', body?: unknown) => new Request('https://worker.invalid/v1/' + path, { method, headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) });
const directory = async (url: URL | string) => {
  const host = new URL(url).hostname;
  const data = host === 'sub2api.invalid' ? [{ id: 'gpt-6' }, { id: 'claude-sonnet-5', name: 'Claude Sonnet 5' }, { id: 'text-embedding-3' }] : [{ id: 'flash' }];
  return Response.json({ data });
};

test('admin adds a listed model and removes another; chat follows the saved list', async () => {
  const env = environment();
  const available = await (await handle(req('models/available'), env, directory)).json();
  const sub2api = available.providers.find(p => p.id === 'sub2api');
  assert.deepEqual(sub2api.models.map(m => [m.id, m.enabled]), [['sub2api/gpt-6', true], ['sub2api/claude-sonnet-5', false]]);
  assert.doesNotMatch(JSON.stringify(available), /secret|endpoint|api_key/);

  const saved = await handle(req('models/enabled', 'PUT', { models: ['deepseek/flash', 'sub2api/claude-sonnet-5'], default_model: 'sub2api/claude-sonnet-5', revision: 0 }), env, directory);
  assert.equal(saved.status, 200);
  const catalog = await (await handle(req('models'), env, directory)).json();
  assert.deepEqual(catalog.data.map(m => m.id), ['deepseek/flash', 'sub2api/claude-sonnet-5']);
  assert.equal(catalog.default_model, 'sub2api/claude-sonnet-5'); assert.equal(catalog.revision, 1); assert.equal(catalog.can_edit, true);
  // Curated capabilities survive; an added model gets no guessed thinking controls.
  assert.deepEqual(catalog.data[0].thinking_modes, ['enabled', 'disabled']);
  assert.equal(catalog.data[1].reasoning_effort_options, undefined);

  const chat = (model: string) => req('chat/completions', 'POST', { model, stream: true, max_tokens: 16, messages: [{ role: 'user', content: 'synthetic' }] });
  const routed = await handle(chat('sub2api/claude-sonnet-5'), env, async (url, init) => {
    assert.equal(url.toString(), config.providers[1].endpoint); assert.equal(JSON.parse(init.body).model, 'claude-sonnet-5');
    return new Response('data: [DONE]\n\n', { headers: { 'Content-Type': 'text/event-stream' } });
  });
  assert.equal(routed.status, 200);
  assert.equal((await handle(chat('sub2api/gpt-6'), env, directory)).status, 400);
});

test('saves reject stale revisions, unlisted models and non-admin accounts', async () => {
  const env = environment();
  const put = (body: unknown, e = env) => handle(req('models/enabled', 'PUT', body), e, directory);
  assert.equal((await put({ models: ['sub2api/text-embedding-3'], default_model: 'sub2api/text-embedding-3', revision: 0 })).status, 400);
  assert.equal((await put({ models: ['sub2api/unknown'], default_model: 'sub2api/unknown', revision: 0 })).status, 400);
  assert.equal((await put({ models: ['deepseek/flash'], default_model: 'sub2api/gpt-6', revision: 0 })).status, 400);
  assert.equal((await put({ models: ['deepseek/flash'], default_model: 'deepseek/flash', revision: 0 })).status, 200);
  assert.equal((await put({ models: ['sub2api/gpt-6'], default_model: 'sub2api/gpt-6', revision: 0 })).status, 409);

  const guest = environment('guest@example.test');
  assert.equal((await (await handle(req('models'), guest, directory)).json()).can_edit, false);
  assert.equal((await handle(req('models/available'), guest, directory)).status, 403);
  assert.equal((await put({ models: ['deepseek/flash'], default_model: 'deepseek/flash', revision: 0 }, guest)).status, 403);
});
