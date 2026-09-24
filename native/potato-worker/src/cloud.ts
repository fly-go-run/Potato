import { documentedCapabilities } from './models.ts';

declare global { interface Env { CLOUD_PROVIDERS?: string; CLOUD_ALLOWED_EMAILS?: string; CLOUD_AUTH_REQUIRED?: string } }
export class CloudError extends Error {
  status: number;
  constructor(status: number, message: string) { super(message); this.status = status; }
}
export type Model = { id: string; name: string; reasoning_effort_options?: string[]; thinking_modes?: string[] };
export type CloudProvider = { id: string; name: string; endpoint: string; api_key: string; models: Model[] };
export type CloudConfiguration = { providers: CloudProvider[]; default_model: string };
const validString = (v: unknown, max: number): v is string => typeof v === 'string' && v.trim().length > 0 && v.length <= max;
const record = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
export function cloudConfiguration(raw: string): CloudConfiguration {
  const fail = () => new CloudError(503, 'Cloud models are not configured correctly.');
  if (raw.length > 60000) throw fail();
  let value: unknown; try { value = JSON.parse(raw); } catch { throw fail(); }
  if (!record(value) || !Array.isArray(value.providers) || !value.providers.length || value.providers.length > 8) throw fail();
  const providers: CloudProvider[] = [], ids = new Set<string>();
  for (const p of value.providers) {
    if (!record(p) || !validString(p.id, 48) || !/^[a-z0-9_-]+$/.test(p.id) || ids.has(p.id) || !validString(p.name, 100) || !validString(p.endpoint, 2048) || !validString(p.api_key, 4096) || !Array.isArray(p.models) || !p.models.length || p.models.length > 100) throw fail();
    let url: URL; try { url = new URL(p.endpoint); } catch { throw fail(); }
    if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash || !url.pathname.endsWith('/chat/completions')) throw fail();
    ids.add(p.id);
    const models: Model[] = [], modelIDs = new Set<string>();
    for (const m of p.models) {
      if (!record(m) || !validString(m.id, 180) || m.id.includes('/') || modelIDs.has(m.id) || !validString(m.name, 150)) throw fail();
      modelIDs.add(m.id);
      const model: Model = { id: m.id, name: m.name };
      for (const field of ['reasoning_effort_options', 'thinking_modes'] as const) {
        if (m[field] === undefined || m[field] === null) continue;
        const options = m[field];
        if (!Array.isArray(options) || options.length > 32 || !options.every(o => typeof o === 'string' && /^[a-z0-9_-]{1,32}$/.test(o) && (field !== 'thinking_modes' || ['enabled', 'disabled'].includes(o)))) throw fail();
        model[field] = [...new Set(options)] as string[];
      }
      models.push(model);
    }
    providers.push({ id: p.id, name: p.name, endpoint: url.toString(), api_key: p.api_key, models });
  }
  if (providers.reduce((n, p) => n + p.models.length, 0) > 500 || typeof value.default_model !== 'string' || !providers.some(p => p.models.some(m => `${p.id}/${m.id}` === value.default_model))) throw fail();
  return { providers, default_model: value.default_model };
}
export function cloudCatalog(config: CloudConfiguration) {
  return { object: 'list', catalog_source: 'configured', default_model: config.default_model,
    data: config.providers.flatMap(p => p.models.map(m => ({ ...documentedCapabilities(new URL(p.endpoint), m.id), ...m, id: `${p.id}/${m.id}` }))) };
}
export function validateCloudThinking(provider: CloudProvider, model: Model, body: Record<string, unknown>) {
  const capabilities = { ...documentedCapabilities(new URL(provider.endpoint), model.id), ...model };
  if (typeof body.reasoning_effort === 'string' && Array.isArray(capabilities.reasoning_effort_options) && !capabilities.reasoning_effort_options.includes(body.reasoning_effort)) {
    throw new CloudError(400, 'This reasoning effort is not supported by the selected model. Refresh the model list.');
  }
  if (record(body.thinking) && Array.isArray(capabilities.thinking_modes) && !capabilities.thinking_modes.includes(String(body.thinking.type))) {
    throw new CloudError(400, 'This thinking mode is not supported by the selected model.');
  }
}
export function cloudRoute(config: CloudConfiguration, id: unknown) {
  for (const provider of config.providers) {
    const model = provider.models.find(m => `${provider.id}/${m.id}` === id);
    if (model) return { provider, model };
  }
  throw new CloudError(400, 'Model is not enabled. Refresh the cloud model list.');
}
export async function cloudIdentity(request: Request, env: Env): Promise<string> {
  return (await cloudAccount(request, env)).owner;
}
export async function cloudAccount(request: Request, env: Env): Promise<{ owner: string; email: string }> {
  const allowed = (env.CLOUD_ALLOWED_EMAILS ?? '').split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
  if (!allowed.length) throw new CloudError(503, 'Cloud access has not been configured.');
  const parts = (request.headers.get('authorization') ?? '').match(/^Bearer ([a-f0-9]{64})\.([a-f0-9-]{36})\.([a-f0-9]{64})$/);
  if (!parts) throw new CloudError(401, 'Sign in to Cloudflare to use cloud models.');
  const identity = await env.REMOTE_ACCOUNTS.getByName(parts[1]).cloudIdentity(parts[2], parts[3]);
  if (!identity) throw new CloudError(401, 'Cloud session expired. Please sign in again.');
  if (!allowed.includes(identity.email.trim().toLowerCase())) throw new CloudError(403, 'This account is not authorized to use cloud models.');
  return { owner: parts[1], email: identity.email };
}
