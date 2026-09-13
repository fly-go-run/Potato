declare global { interface Env { EXA_API_KEY?: string } }
export function searchQuery(body: unknown): string {
  const query = (body as { query?: unknown } | null)?.query;
  if (typeof query !== 'string' || !query.trim() || new TextEncoder().encode(query).length > 8000) throw new Error('Invalid search query.');
  return query.trim();
}
export async function searchExa(query: string, key: string, signal: AbortSignal, fetcher: typeof fetch = fetch) {
  const response = await fetcher('https://api.exa.ai/search', {
    method: 'POST', redirect: 'manual', signal: AbortSignal.any([signal, AbortSignal.timeout(30_000)]),
    headers: { 'Content-Type': 'application/json', 'x-api-key': key },
    body: JSON.stringify({ query, numResults: 5, contents: { text: { maxCharacters: 2000 } } })
  });
  if (!response.ok || !response.body) { await response.body?.cancel(); throw new Error('Search service unavailable.'); }
  const reader = response.body.getReader(), chunks: Uint8Array[] = []; let size = 0;
  try {
    for (;;) { const { done, value } = await reader.read(); if (done) break; size += value.length; if (size > 2_000_000) throw new Error('Search response too large.'); chunks.push(value); }
  } finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
  const bytes = new Uint8Array(size); let offset = 0; for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  const body = JSON.parse(new TextDecoder('utf-8', { fatal: true, ignoreBOM: false }).decode(bytes));
  if (!Array.isArray(body.results)) throw new Error('Invalid search response.');
  const seen = new Set<string>();
  const results: { title: string; url: string; content: string; publishedDate?: string }[] = [];
  for (const row of body.results) {
    if (!row || typeof row.url !== 'string' || row.url.length > 2048) continue;
    let url: URL; try { url = new URL(row.url); } catch { continue; }
    if (url.protocol !== 'https:' || url.username || url.password || seen.has(url.href)) continue;
    seen.add(url.href);
    results.push({ title: (typeof row.title === 'string' ? row.title : url.hostname).slice(0, 300), url: url.href, content: (typeof row.text === 'string' ? row.text : '').slice(0, 2000), ...(typeof row.publishedDate === 'string' ? { publishedDate: row.publishedDate.slice(0, 40) } : {}) });
    if (results.length === 5) break;
  }
  return { provider: 'exa', query, results };
}
