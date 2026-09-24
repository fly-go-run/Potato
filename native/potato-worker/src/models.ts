// Only exact official models use this documented fallback; never infer capabilities from prefixes.
// https://api-docs.deepseek.com/guides/thinking_mode/ (checked 2026-09-13)
export function documentedCapabilities(upstream: URL, id: string): Record<string, unknown> {
  if (upstream.protocol === 'https:' && upstream.hostname === 'api.deepseek.com' && !upstream.port && ['/chat/completions', '/v1/chat/completions'].includes(upstream.pathname) && ['deepseek-flash', 'deepseek-v4-flash', 'deepseek-v4-pro'].includes(id)) {
    return { reasoning_effort_options: ['low', 'high', 'max'], thinking_modes: ['enabled', 'disabled'] };
  }
  if (upstream.protocol === 'https:' && upstream.hostname === 'api.openai.com' && !upstream.port && upstream.pathname === '/v1/chat/completions' && ['gpt-5.6', 'gpt-5.6-sol', 'gpt-5.6-terra', 'gpt-5.6-luna'].includes(id)) {
    return { reasoning_effort_options: ['none', 'low', 'medium', 'high', 'xhigh', 'max'], thinking_modes: [] };
  }
  return {};
}
function options(value: unknown, allowed?: string[]): string[] | undefined {
  if (value === undefined || value === null) return undefined;
  if (!Array.isArray(value)) return [];
  return [...new Set(value.filter((v): v is string => typeof v === 'string' && /^[a-z0-9_-]{1,32}$/.test(v) && (!allowed || allowed.includes(v))))].slice(0, 32);
}
/** The provider's `/models` entries, or null when the provider publishes no directory. */
export async function modelDirectory(upstream: URL, key: string, signal: AbortSignal, fetcher: typeof fetch): Promise<unknown[] | null> {
  const endpoint = new URL(upstream);
  if (!endpoint.pathname.endsWith('/chat/completions')) throw new Error('No model catalog endpoint.');
  endpoint.pathname = endpoint.pathname.slice(0, -'/chat/completions'.length) + '/models';
  const response = await fetcher(endpoint, { method: 'GET', redirect: 'manual', signal: AbortSignal.any([signal, AbortSignal.timeout(15000)]), headers: { Authorization: `Bearer ${key}`, Accept: 'application/json' } });
  if ([404, 405].includes(response.status)) { await response.body?.cancel(); return null; }
  if (!response.ok || !response.body || !response.headers.get('content-type')?.includes('application/json')) { await response.body?.cancel(); throw new Error('Catalog unavailable.'); }
  const reader = response.body.getReader(), decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: false });
  let text = '', bytes = 0;
  try {
    for (;;) {
      const next = await reader.read(); if (next.done) break;
      bytes += next.value.byteLength; if (bytes > 1_000_000) throw new Error('Catalog too large.');
      text += decoder.decode(next.value, { stream: true });
    }
    text += decoder.decode();
  } finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
  const value = JSON.parse(text);
  if (!Array.isArray(value.data) || value.data.length > 500) throw new Error('Invalid catalog.');
  return value.data;
}
export async function modelCatalog(upstream: URL, allowed: string[], key: string, signal: AbortSignal, fetcher: typeof fetch): Promise<unknown> {
  const directory = await modelDirectory(upstream, key, signal, fetcher);
  const entries: unknown[] = directory ?? allowed.map(id => ({ id })), source = directory ? 'service' : 'configured';
  const seen = new Set<string>(), data = [];
  for (const item of entries) {
    if (!item || typeof item !== 'object' || !('id' in item) || typeof item.id !== 'string' || !allowed.includes(item.id) || seen.has(item.id)) continue;
    seen.add(item.id);
    const raw = item as Record<string, unknown>;
    const defaults = documentedCapabilities(upstream, item.id);
    const effort = raw.thinking_param_style && raw.thinking_param_style !== 'effort' ? [] : options(raw.reasoning_effort_options);
    data.push({ id: item.id, name: typeof raw.name === 'string' ? raw.name.slice(0, 256) : item.id,
      ...defaults, ...(effort !== undefined ? { reasoning_effort_options: effort } : {}),
      ...(options(raw.thinking_modes, ['enabled', 'disabled']) !== undefined ? { thinking_modes: options(raw.thinking_modes, ['enabled', 'disabled']) } : {}) });
  }
  return { object: 'list', data, catalog_source: source };
}
