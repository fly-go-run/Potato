import { CloudError, cloudConfiguration, type CloudConfiguration, type Model } from './cloud.ts';
import { modelDirectory } from './models.ts';

// Providers and keys stay in the deployed CLOUD_PROVIDERS secret. The enabled model list is
// stored separately so an admin can change it from the phone without redeploying.
export type StoredModel = { id: string; name: string };
export type ModelSettings = { revision: number; models: StoredModel[]; default_model: string };
export type ModelSettingsStore = {
  read(): Promise<ModelSettings | null>;
  write(next: Omit<ModelSettings, 'revision'>, revision: number): Promise<ModelSettings | null>;
};
declare global { interface Env { CLOUD_MODELS?: { getByName(name: string): ModelSettingsStore }; CLOUD_ADMIN_EMAILS?: string } }

const MODEL_ID = /^[A-Za-z0-9._:@+-]{1,180}$/;
// Directories also list embedding, speech and image models that cannot answer a chat.
const NON_CHAT = /(embed|tts|whisper|transcri|speech|audio|image|dall-e|moderation|rerank|realtime)/i;
const settings = (env: Env) => env.CLOUD_MODELS?.getByName('global');

export function isModelAdmin(env: Env, email: string): boolean {
  const admins = (env.CLOUD_ADMIN_EMAILS ?? '').split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
  return !!email && admins.includes(email.trim().toLowerCase());
}

/** Saved models replace each provider's configured list; curated entries keep their verified capabilities. */
export function applyModelSettings(base: CloudConfiguration, saved: ModelSettings): CloudConfiguration {
  const providers = base.providers.map(p => ({ ...p, models: saved.models.filter(m => m.id.startsWith(p.id + '/')).map((m): Model => {
    const id = m.id.slice(p.id.length + 1);
    return p.models.find(c => c.id === id) ?? { id, name: m.name };
  }) })).filter(p => p.models.length);
  if (!providers.length) return base;
  const ids = providers.flatMap(p => p.models.map(m => `${p.id}/${m.id}`));
  return { providers, default_model: ids.includes(saved.default_model) ? saved.default_model : ids[0] };
}

export async function loadCloudModels(env: Env): Promise<{ config: CloudConfiguration; revision: number }> {
  const base = cloudConfiguration(env.CLOUD_PROVIDERS ?? '');
  const saved = await settings(env)?.read();
  return saved ? { config: applyModelSettings(base, saved), revision: saved.revision } : { config: base, revision: 0 };
}

type DirectoryModel = { id: string; name: string };
async function directory(provider: CloudConfiguration['providers'][number], signal: AbortSignal, fetcher: typeof fetch): Promise<DirectoryModel[] | null> {
  const entries = await modelDirectory(new URL(provider.endpoint), provider.api_key, signal, fetcher);
  if (!entries) return null;
  const seen = new Set<string>(), models: DirectoryModel[] = [];
  for (const item of entries) {
    if (!item || typeof item !== 'object') continue;
    const raw = item as Record<string, unknown>;
    if (typeof raw.id !== 'string' || !MODEL_ID.test(raw.id) || NON_CHAT.test(raw.id) || seen.has(raw.id)) continue;
    seen.add(raw.id);
    models.push({ id: raw.id, name: typeof raw.name === 'string' && raw.name.trim() ? raw.name.trim().slice(0, 150) : raw.id });
  }
  return models;
}

export async function availableModels(env: Env, signal: AbortSignal, fetcher: typeof fetch) {
  const base = cloudConfiguration(env.CLOUD_PROVIDERS ?? '');
  const { config, revision } = await loadCloudModels(env);
  const enabled = new Set(config.providers.flatMap(p => p.models.map(m => `${p.id}/${m.id}`)));
  const providers = await Promise.all(base.providers.map(async p => {
    let listed: DirectoryModel[] | null = null, unavailable = false;
    try { listed = await directory(p, signal, fetcher); } catch { unavailable = true; }
    const models = [...p.models.map(m => ({ id: m.id, name: m.name })), ...(listed ?? []).filter(m => !p.models.some(c => c.id === m.id))];
    return { id: p.id, name: p.name, ...(unavailable ? { unavailable: true } : {}),
      models: models.slice(0, 300).map(m => ({ id: `${p.id}/${m.id}`, name: m.name, enabled: enabled.has(`${p.id}/${m.id}`) })) };
  }));
  return { revision, default_model: config.default_model, providers };
}

/** Only models already enabled, curated, or listed by the provider right now can be saved. */
export async function saveModels(env: Env, input: unknown, signal: AbortSignal, fetcher: typeof fetch): Promise<ModelSettings> {
  const store = settings(env);
  if (!store) throw new CloudError(503, 'Model settings are not configured.');
  const value = input && typeof input === 'object' && !Array.isArray(input) ? input as Record<string, unknown> : {};
  const ids = value.models, revision = value.revision;
  if (!Array.isArray(ids) || !ids.length || ids.length > 100 || !ids.every(id => typeof id === 'string') || new Set(ids).size !== ids.length) throw new CloudError(400, 'Choose between 1 and 100 models.');
  if (typeof value.default_model !== 'string' || !ids.includes(value.default_model)) throw new CloudError(400, 'The default model must be enabled.');
  if (!Number.isSafeInteger(revision) || (revision as number) < 0) throw new CloudError(400, 'Invalid revision.');
  const base = cloudConfiguration(env.CLOUD_PROVIDERS ?? '');
  const { config } = await loadCloudModels(env);
  const directories = new Map<string, Promise<DirectoryModel[] | null>>();
  const models: StoredModel[] = [];
  for (const id of ids as string[]) {
    const provider = base.providers.find(p => id.startsWith(p.id + '/'));
    const modelID = provider ? id.slice(provider.id.length + 1) : '';
    if (!provider || !MODEL_ID.test(modelID)) throw new CloudError(400, 'Unknown model.');
    const known = provider.models.find(m => m.id === modelID) ?? config.providers.find(p => p.id === provider.id)?.models.find(m => m.id === modelID);
    if (known) { models.push({ id, name: known.name }); continue; }
    if (!directories.has(provider.id)) directories.set(provider.id, directory(provider, signal, fetcher).catch(() => { throw new CloudError(502, 'Model directory is unavailable.'); }));
    const listed = (await directories.get(provider.id)!)?.find(m => m.id === modelID);
    if (!listed) throw new CloudError(400, 'Unknown model.');
    models.push({ id, name: listed.name });
  }
  const saved = await store.write({ models, default_model: value.default_model }, revision as number);
  if (!saved) throw new CloudError(409, 'Models changed on another device. Refresh and try again.');
  return saved;
}
