import type { RecallMessage, RecallSource } from './recall.ts';

type SearchInput = {
  query: string; start?: string; end?: string; offset?: number;
  conversations: { id: string; title: string; revision: string; messages: RecallMessage[] }[];
};
const norm = (s: string) => s.normalize('NFKC').toLowerCase();

// RecallSession supplies validated query, dates, pagination and stored messages.
export async function searchConversations(input: unknown, signal: AbortSignal): Promise<{ sources: RecallSource[]; more: boolean }> {
  signal.throwIfAborted();
  const p = input as SearchInput;
  const q = norm(p.query).trim(), terms = q.split(/\s+/u).filter(Boolean);
  const rows: (RecallSource & { score: number })[] = [];
  let processed = 0;
  for (const c of p.conversations) {
    for (const m of c.messages) {
      if (++processed % 50 === 0) signal.throwIfAborted();
      if (p.start && m.date < p.start) continue;
      if (p.end && m.date >= p.end) continue;
      const text = norm(m.text);
      const score = q && text.includes(q) ? 10 : terms.filter(t => text.includes(t)).length;
      if (q && score === 0) continue;
      rows.push({ ...m, conversation: c.id, title: c.title, revision: c.revision, score });
    }
  }
  // Both ECMAScript sort and Python reverse=True preserve the input order of ties.
  rows.sort((a, b) => b.score - a.score || (b.date > a.date ? 1 : b.date < a.date ? -1 : 0));
  const o = p.offset ?? 0;
  return {
    sources: rows.slice(o, o + 8).map(({ score: _score, ...m }) => ({ ...m, text: Array.from(m.text).slice(0, 4000).join('') })),
    more: rows.length > o + 8,
  };
}
