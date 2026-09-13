import { codeTool, type CodeTool } from './code-tool.ts';
import { validateDesktop } from './desktop-chat.ts';
import { RecallError, RecallStore, RecallSession, e2bRecallExecutor } from './recall.ts';
import { timingSafeEqual } from 'node:crypto';
import { executeSandbox, validateSandbox } from './sandbox.ts';
import { connectSpeech } from './speech.ts';
import { chatWithSearch } from './search-chat.ts';
import { modelCatalog } from './models.ts';
import { CloudError, cloudConfiguration, cloudCatalog, cloudRoute, cloudIdentity, validateCloudThinking } from './cloud.ts';

const MAX_BODY = 4 * 1024 * 1024;
class APIError extends Error {
  status: number;
  constructor(status: number, message: string) { super(message); this.status = status; }
}
function errorResponse(status: number, message: string, requestID: string): Response {
  return Response.json({ error: { message, request_id: requestID } }, { status, headers: { 'Cache-Control': 'no-store', 'X-Request-ID': requestID } });
}
export async function boundedJSON(request: Request): Promise<unknown> {
  if (!request.headers.get('content-type')?.toLowerCase().includes('application/json')) throw new APIError(415, 'Content-Type must be application/json.');
  if (Number(request.headers.get('content-length') ?? 0) > MAX_BODY) throw new APIError(413, 'Request exceeds 4 MB.');
  if (!request.body) throw new APIError(400, 'A JSON request body is required.');
  const reader = request.body.getReader(), chunks: Uint8Array[] = [];
  let size = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > MAX_BODY) { await reader.cancel(); throw new APIError(413, 'Request exceeds 4 MB.'); }
      chunks.push(value);
    }
  } finally { reader.releaseLock(); }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
  try { return JSON.parse(new TextDecoder('utf-8', { fatal: true, ignoreBOM: false }).decode(bytes)); }
  catch { throw new APIError(400, 'Invalid JSON.'); }
}
function object(value: unknown): value is Record<string, unknown> { return typeof value === 'object' && value !== null && !Array.isArray(value); }
export function validate(body: unknown, models: string[], maxOutput: number): Record<string, unknown> {
  if (!object(body) || typeof body.model !== 'string' || !models.includes(body.model)) throw new APIError(400, 'Model is not enabled.');
  if (body.stream !== true) throw new APIError(400, 'stream must be true.');
  if (!Array.isArray(body.messages) || body.messages.length === 0 || body.messages.length > 200) throw new APIError(400, 'Expected 1–200 messages.');
  const ids = new Set<string>(), pending = new Set<string>();
  const invalidHistory = (): never => { throw new APIError(400, 'Invalid tool history.'); };
  const messages = body.messages.map((message: unknown) => {
    if (!object(message) || typeof message.role !== 'string' || !['system', 'user', 'assistant', 'tool'].includes(message.role)) throw new APIError(400, 'Invalid message role.');
    if (message.role === 'tool') {
      if (typeof message.tool_call_id !== 'string' || message.tool_call_id.length < 1 || message.tool_call_id.length > 200 ||
          typeof message.content !== 'string' || message.content.length < 1 || message.content.length > 16_000 || !pending.delete(message.tool_call_id)) invalidHistory();
      return { role: 'tool', tool_call_id: message.tool_call_id, content: message.content };
    }
    if (pending.size) invalidHistory();
    if (message.role === 'assistant' && message.tool_calls !== undefined) {
      if (typeof message.content !== 'string' || !Array.isArray(message.tool_calls) || message.tool_calls.length < 1 || message.tool_calls.length > 16) return invalidHistory();
      const tool_calls = message.tool_calls.map((call: unknown) => {
        if (!object(call) || typeof call.id !== 'string' || call.id.length < 1 || call.id.length > 200 || ids.has(call.id) ||
            call.type !== 'function' || !object(call.function) || typeof call.function.name !== 'string' || !/^[a-z_]{1,64}$/.test(call.function.name) ||
            typeof call.function.arguments !== 'string' || call.function.arguments.length > 32_000) return invalidHistory();
        ids.add(call.id); pending.add(call.id);
        return { id: call.id, type: 'function', function: { name: call.function.name, arguments: call.function.arguments } };
      });
      return { role: 'assistant', content: message.content, tool_calls };
    }
    let content: unknown = message.content;
    if (typeof content !== 'string') {
      if (message.role !== 'user' || !Array.isArray(content) || content.length < 1 || content.length > 5) throw new APIError(400, 'Invalid message content.');
      content = content.map((part: unknown) => {
        if (!object(part)) throw new APIError(400, 'Invalid content part.');
        if (part.type === 'text' && typeof part.text === 'string') return { type: 'text', text: part.text };
        if (part.type === 'image_url' && object(part.image_url) && typeof part.image_url.url === 'string' && /^data:image\/(png|jpeg|webp|gif);base64,[A-Za-z0-9+/]+=*$/.test(part.image_url.url)) return { type: 'image_url', image_url: { url: part.image_url.url } };
        throw new APIError(400, 'Images must be inline PNG, JPEG, WebP or GIF.');
      });
    }
    return { role: message.role, content };
  });
  if (pending.size) invalidHistory();
  // Only forward explicitly supported properties, never arbitrary provider options.
  if (body.max_tokens !== undefined && (typeof body.max_tokens !== 'number' || !Number.isSafeInteger(body.max_tokens) || body.max_tokens < 1)) throw new APIError(400, 'max_tokens must be a positive integer.');
  const result: Record<string, unknown> = { model: body.model, messages, stream: true, max_tokens: Math.min(typeof body.max_tokens === 'number' ? body.max_tokens : maxOutput, maxOutput) };
  if (body.thinking !== undefined) {
    if (!object(body.thinking) || !['enabled', 'disabled'].includes(String(body.thinking.type))) throw new APIError(400, 'Invalid thinking mode.');
    result.thinking = { type: body.thinking.type };
  }
  if (body.reasoning_effort !== undefined) {
    if (typeof body.reasoning_effort !== 'string' || !/^[a-z0-9_-]{1,32}$/.test(body.reasoning_effort)) throw new APIError(400, 'Invalid reasoning effort.');
    if (object(body.thinking) && body.thinking.type === 'disabled') throw new APIError(400, 'Disabled thinking cannot specify effort.');
    result.reasoning_effort = body.reasoning_effort;
  }
  return result;
}
export async function handle(request: Request, env: Env, fetcher: typeof fetch = fetch): Promise<Response> {
  const requestID = crypto.randomUUID(), path = new URL(request.url).pathname;
  if (path === '/health' && request.method === 'GET') return Response.json({ service: 'potato-iphone', status: 'ok' }, { headers: { 'Cache-Control': 'no-store' } });
  const recallRoute = ['/v1/recall/status', '/v1/recall/sync', '/v1/recall/memory'].includes(path);
  const desktop = path === '/v1/desktop/chat/completions';
  const voice = path === '/v1/audio/transcriptions';
  const catalog = path === '/v1/models';
  if (!desktop && path !== '/v1/chat/completions' && path !== '/v1/sandbox/run' && !voice && !catalog && !recallRoute) return errorResponse(404, 'Not found.', requestID);
  if (request.method !== (voice || catalog || path === '/v1/recall/status' ? 'GET' : 'POST')) return errorResponse(405, 'Invalid method.', requestID);
  try {
    const cloud = desktop || env.CLOUD_AUTH_REQUIRED === 'true' || !!env.CLOUD_PROVIDERS;
    let rateKey = 'personal-client';
    if (cloud) {
      rateKey = await cloudIdentity(request, env);
    } else {
      if (!env.CLIENT_TOKEN || env.CLIENT_TOKEN.length < 32 || !env.UPSTREAM_API_KEY) throw new APIError(503, 'Service is not configured.');
      const authorization = request.headers.get('authorization') ?? '';
      const actual = new TextEncoder().encode(authorization), expected = new TextEncoder().encode(`Bearer ${env.CLIENT_TOKEN}`);
      if (actual.byteLength !== expected.byteLength || !timingSafeEqual(actual, expected)) throw new APIError(401, 'Invalid connection token.');
    }
    const { success } = await (recallRoute ? env.RECALL_RATE_LIMIT : env.CHAT_RATE_LIMIT).limit({ key: rateKey });
    if (!success) throw new APIError(429, 'Please wait before trying again.');
    if (recallRoute) {
      if (!env.RECALL_BUCKET) throw new APIError(503, 'History storage is not configured.');
      const store = new RecallStore(env.RECALL_BUCKET, rateKey);
      const result = path.endsWith('/status') ? await store.status() : path.endsWith('/sync') ? await store.sync(await boundedJSON(request)) : await store.memory(await boundedJSON(request));
      return Response.json(result, { headers: { 'Cache-Control': 'no-store', 'X-Request-ID': requestID } });
    }
    if (voice) {
      if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') return errorResponse(426, 'Use WebSocket.', requestID);
      if (!env.DOUBAO_API_KEY) return errorResponse(503, 'Speech is not configured.', requestID);
      if (!(await env.SPEECH_RATE_LIMIT.limit({ key: rateKey })).success) return errorResponse(429, 'Please wait before recording again.', requestID);
      try { return await connectSpeech(request, env, fetcher); } catch { return errorResponse(502, 'Speech service is unavailable.', requestID); }
    }
    if (path === '/v1/sandbox/run') {
      if (!env.E2B_API_KEY) return errorResponse(503, 'Cloud execution is not configured.', requestID);
      if (!(await env.SANDBOX_RATE_LIMIT.limit({ key: rateKey })).success) return errorResponse(429, 'Please wait before running more code.', requestID);
      let input;
      try { input = validateSandbox(await boundedJSON(request)); } catch (error) { throw new APIError(error instanceof APIError ? error.status : 400, 'Invalid sandbox input.'); }
      try {
        const result = await executeSandbox(input, env.E2B_API_KEY, request.signal, undefined, env.E2B_TEMPLATE);
        return Response.json(result, { headers: { 'Cache-Control': 'no-store', 'X-Request-ID': requestID } });
      } catch { return errorResponse(502, 'Cloud execution failed or exceeded its time or output limit.', requestID); }
    }
    const maxOutput = Number(env.MAX_OUTPUT_TOKENS);
    if (!Number.isSafeInteger(maxOutput) || maxOutput < 1 || maxOutput > 16384) throw new APIError(503, 'Invalid output limit.');
    let upstreamURL: URL, upstreamKey: string, body: Record<string, unknown>, recall: RecallSession | undefined, code: CodeTool | undefined;
    const readInput = async () => {
      const input = await boundedJSON(request);
      if (!desktop && object(input) && input.sandbox !== undefined) {
        try { code = codeTool(input.sandbox, env, rateKey); }
        catch { throw new APIError(env.E2B_API_KEY ? 400 : 503, 'Invalid or unavailable code execution configuration.'); }
      }
      if (!desktop && object(input) && object(input.recall) && input.recall.enabled === true) {
        if (!env.RECALL_BUCKET || !env.E2B_API_KEY) throw new APIError(503, 'History retrieval is not configured.');
        const engine = e2bRecallExecutor(env.E2B_API_KEY, env.E2B_TEMPLATE);
        let charged = false;
        const execute = Object.assign(async (data: unknown, signal: AbortSignal) => {
          if (!charged && !(await env.SANDBOX_RATE_LIMIT.limit({ key: rateKey })).success) throw new RecallError(429, 'History search limit reached.');
          charged = true;
          return engine(data, signal);
        }, { close: engine.close });
        recall = new RecallSession(new RecallStore(env.RECALL_BUCKET, rateKey), execute, input.recall.auto_memory === true);
        const timezone = typeof input.recall.timezone === 'string' ? input.recall.timezone : 'UTC';
        try { new Intl.DateTimeFormat('en', { timeZone: timezone }).format(); } catch { throw new APIError(400, 'Invalid timezone.'); }
        recall.timezone = timezone;
      }
      return input;
    };
    if (cloud) {
      const config = cloudConfiguration(env.CLOUD_PROVIDERS ?? '');
      if (catalog) return Response.json(cloudCatalog(config), { headers: { 'Cache-Control': 'no-store', 'X-Request-ID': requestID } });
      const input = await readInput();
      const { provider, model } = cloudRoute(config, object(input) ? input.model : undefined);
      body = desktop ? validateDesktop(input, `${provider.id}/${model.id}`, maxOutput) : validate(input, [`${provider.id}/${model.id}`], maxOutput);
      validateCloudThinking(provider, model, body);
      body.model = model.id;
      upstreamURL = new URL(provider.endpoint); upstreamKey = provider.api_key;
    } else {
      const models = env.ALLOWED_MODELS.split(',').map(value => value.trim()).filter(Boolean);
      if (!models.length) throw new APIError(503, 'No models are enabled.');
      upstreamURL = new URL(env.UPSTREAM_URL); upstreamKey = env.UPSTREAM_API_KEY;
      if (upstreamURL.protocol !== 'https:' || upstreamURL.username || upstreamURL.password || upstreamURL.hash || upstreamURL.search) throw new APIError(503, 'Invalid upstream configuration.');
      if (catalog) {
        try { return Response.json(await modelCatalog(upstreamURL, models, upstreamKey, request.signal, fetcher), { headers: { 'Cache-Control': 'no-store', 'X-Request-ID': requestID } }); }
        catch { throw new APIError(502, 'Model catalog is unavailable.'); }
      }
      body = validate(await readInput(), models, maxOutput);
    }
    if (!desktop && (env.EXA_API_KEY || recall || code) && Number(body.max_tokens) > 32) {
      const response = await chatWithSearch(body, upstreamURL, env, request.signal, fetcher, upstreamKey, rateKey, recall, code);
      response.headers.set('X-Request-ID', requestID); return response;
    }
    const upstream = await fetcher(upstreamURL, {
      method: 'POST', redirect: 'manual', signal: request.signal,
      headers: { 'Authorization': `Bearer ${upstreamKey}`, 'Content-Type': 'application/json', 'Accept': 'text/event-stream' },
      body: JSON.stringify(body)
    });
    if (!upstream.ok || !upstream.body || !upstream.headers.get('content-type')?.includes('text/event-stream')) {
      await upstream.body?.cancel();
      throw new APIError(upstream.status === 429 ? 429 : 502, 'Model service is unavailable.');
    }
    return new Response(upstream.body, { headers: { 'Content-Type': 'text/event-stream; charset=utf-8', 'Cache-Control': 'no-store', 'X-Request-ID': requestID } });
  } catch (error) {
    return errorResponse((error instanceof APIError || error instanceof CloudError || error instanceof RecallError) ? error.status : 502, (error instanceof APIError || error instanceof CloudError || error instanceof RecallError) ? error.message : 'Unable to reach model service.', requestID);
  }
}
export default { fetch(request, env) { return handle(request, env); } } satisfies ExportedHandler<Env>;
