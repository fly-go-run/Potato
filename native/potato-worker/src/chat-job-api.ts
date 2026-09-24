import { boundedJSON, validate } from './index.ts';
import { cloudRoute, validateCloudThinking } from './cloud.ts';
import { loadCloudModels } from './cloud-models.ts';

export async function handleChatJob(request: Request, env: Env, owner: string): Promise<Response> {
  const url = new URL(request.url);
  const match = url.pathname.match(/^\/v1\/chat\/jobs\/([a-f0-9-]{36})(\/(?:cancel|events))?$/);
  if (!match || !/^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(match[1])) return Response.json({ error: 'Invalid job ID.' }, { status: 400 });
  const job = env.CHAT_JOBS.getByName(`${owner}:${match[1]}`);
  let result: Response;
  if (request.method === 'PUT' && !match[2]) {
    const input = await boundedJSON(request);
    const { provider, model } = cloudRoute((await loadCloudModels(env)).config, (input as Record<string, unknown>)?.model);
    const max = Number(env.MAX_OUTPUT_TOKENS);
    if (!Number.isSafeInteger(max) || max < 1 || max > 16384) return Response.json({ error: 'Invalid output limit.' }, { status: 503 });
    const body = validate(input, [`${provider.id}/${model.id}`], max);
    validateCloudThinking(provider, model, body);
    result = await job.submit(owner, JSON.stringify(input));
  } else if (request.method === 'GET' && (!match[2] || match[2] === '/events')) {
    const after = Number(url.searchParams.get('after') ?? '0');
    if (!Number.isSafeInteger(after) || after < 0) return Response.json({ error: 'Invalid cursor.' }, { status: 400 });
    result = match[2] === '/events' ? await job.events(after) : await job.read(after);
  } else if (request.method === 'POST' && match[2] === '/cancel') result = await job.cancel();
  else result = Response.json({ error: 'Invalid method.' }, { status: 405 });
  result.headers.set('Cache-Control', 'no-store, no-transform');
  return result;
}
