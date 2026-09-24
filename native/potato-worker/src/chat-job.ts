import { DurableObject } from 'cloudflare:workers';
import { handle } from './index.ts';

type Job = { owner: string; hash: string; state: 'queued' | 'running' | 'complete' | 'failed' | 'stopped' | 'expired'; last: number; bytes: number; expires: number; failure?: string };
const RETENTION = 7 * 86400_000;
// One alarm-owned generation per account + client-generated ID. HTTP readers never own its lifetime.
export class ChatJob extends DurableObject<Env> {
  private controller?: AbortController;
  private readers = new Set<() => void>();
  private revision = 0;
  private changed() { this.revision++; for (const wake of this.readers) wake(); }
  constructor(ctx: DurableObjectState, env: Env) {
    super(ctx, env);
    ctx.storage.sql.exec('CREATE TABLE IF NOT EXISTS payload (kind TEXT, seq INTEGER, part INTEGER, data TEXT, PRIMARY KEY(kind, seq, part))');
  }
  private putPayload(kind: string, seq: number, text: string) {
    for (let start = 0, part = 0; start < text.length; part++) {
      let end = Math.min(start + 16000, text.length);
      if (end < text.length && /[\uD800-\uDBFF]/.test(text[end - 1])) end--;
      this.ctx.storage.sql.exec('INSERT INTO payload VALUES (?, ?, ?, ?)', kind, seq, part, text.slice(start, end));
      start = end;
    }
  }
  private payload(kind: string, seq: number): string {
    return this.ctx.storage.sql.exec<{ data: string }>('SELECT data FROM payload WHERE kind = ? AND seq = ? ORDER BY part', kind, seq).toArray().map(r => r.data).join('');
  }
  async submit(owner: string, body: string): Promise<Response> {
    const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(body))), b => b.toString(16).padStart(2, '0')).join('');
    const old = await this.ctx.storage.get<Job>('job');
    if (old) {
      if (old.state === 'expired') return Response.json({ error: 'Saved reply expired.' }, { status: 410 });
      if (old.hash && old.hash !== hash) return Response.json({ error: 'Job ID already used.' }, { status: 409 });
      return Response.json({ state: old.state });
    }
    if (!(await this.env.CHAT_RATE_LIMIT.limit({ key: owner })).success) return Response.json({ error: 'Please wait before trying again.' }, { status: 429 });
    // Recheck after external I/O so concurrent submits cannot both launch work.
    return this.ctx.storage.transaction(async txn => {
      const existing = await txn.get<Job>('job');
      if (existing) return Response.json({ state: existing.state }, { status: existing.hash && existing.hash !== hash ? 409 : 200 });
      this.putPayload('request', 0, body);
      await txn.put('job', { owner, hash, state: 'queued', last: 0, bytes: 0, expires: Date.now() + RETENTION } satisfies Job);
      await txn.setAlarm(Date.now() + 1);
      return Response.json({ state: 'queued' }, { status: 202 });
    });
  }
  async read(after: number): Promise<Response> {
    const job = await this.ctx.storage.get<Job>('job');
    if (!job) return Response.json({ error: 'Job not found.' }, { status: 404 });
    if (job.state === 'expired' || job.expires < Date.now()) return Response.json({ error: 'Saved reply expired.' }, { status: 410 });
    if (after > job.last) return Response.json({ error: 'Invalid cursor.' }, { status: 400 });
    const events: { seq: number; data: string }[] = [];
    let bytes = 0;
    for (let seq = after + 1; seq <= job.last && events.length < 64; seq++) {
      const data = this.payload('event', seq);
      events.push({ seq, data }); bytes += data.length;
      if (bytes >= 256_000) break;
    }
    return Response.json({ state: job.state, last: job.last, events, failure: job.failure });
  }
  async events(after: number): Promise<Response> {
    const initial = await this.read(after);
    if (!initial.ok) return initial;
    if (this.readers.size >= 8) return Response.json({ error: 'Too many readers.' }, { status: 429 });
    let cursor = after, closed = false, first = true;
    let release: (() => void) | undefined;
    const wake = () => release?.();
    this.readers.add(wake);
    const cleanup = () => { closed = true; this.readers.delete(wake); wake(); clearTimeout(lifetime); };
    // Rotate connections so stalled clients cannot retain subscriber slots indefinitely.
    const lifetime = setTimeout(cleanup, 60_000);
    const encoder = new TextEncoder();
    const stream = new ReadableStream<Uint8Array>({
      pull: async controller => {
        try {
          while (!closed) {
            const revision = this.revision;
            const response = await this.read(cursor);
            if (closed) break;
            if (!response.ok) throw new Error('Reply unavailable');
            const page = await response.json<{ state: string; last: number; events: { seq: number; data: string }[] }>();
            const terminal = !['queued', 'running'].includes(page.state);
            if (first || page.events.length || terminal) {
              first = false;
              cursor = page.events.at(-1)?.seq ?? cursor;
              controller.enqueue(encoder.encode(`data: ${JSON.stringify(page)}\n\n`));
              if (terminal && cursor === page.last) { cleanup(); controller.close(); }
              else {
                // Coalesce fast tokens, keeping disk/UI checkpoint work bounded.
                await new Promise(resolve => setTimeout(resolve, 40));
              }
              return;
            }
            if (revision !== this.revision) continue;
            let timer: ReturnType<typeof setTimeout>;
            await new Promise<void>(resolve => {
              release = resolve;
              timer = setTimeout(resolve, 15_000);
              if (closed || revision !== this.revision) resolve();
            });
            clearTimeout(timer!); release = undefined;
            if (!closed && revision === this.revision) {
              controller.enqueue(encoder.encode(': keepalive\n\n')); return;
            }
          }
          controller.close();
        } catch (error) { cleanup(); controller.error(error); }
      },
      cancel: cleanup,
    }, { highWaterMark: 0 });
    return new Response(stream, { headers: { 'Content-Type': 'text/event-stream; charset=utf-8', 'Cache-Control': 'no-store, no-transform' } });
  }
  async cancel(): Promise<Response> {
    // Persist a tombstone even when cancel races ahead of the initial submission.
    await this.ctx.storage.transaction(async txn => {
      const job = await txn.get<Job>('job');
      if (!job || job.state === 'queued' || job.state === 'running') {
        await txn.put('job', { ...(job ?? { owner: '', hash: '', last: 0, bytes: 0, expires: Date.now() + RETENTION }), state: 'stopped' });
        this.ctx.storage.sql.exec("DELETE FROM payload WHERE kind = 'request'");
        await txn.setAlarm(Date.now() + RETENTION);
      }
    });
    this.controller?.abort();
    this.changed();
    return this.read(0);
  }
  private async finish(state: 'complete' | 'failed', failure?: string) {
    await this.ctx.storage.transaction(async txn => {
      const job = await txn.get<Job>('job');
      if (!job || job.state !== 'running') return;
      await txn.put('job', { ...job, state, failure });
      this.ctx.storage.sql.exec("DELETE FROM payload WHERE kind = 'request'");
      await txn.setAlarm(job.expires);
    });
    this.changed();
  }
  private async append(data: string) {
    await this.ctx.storage.transaction(async txn => {
      const job = await txn.get<Job>('job');
      if (!job || job.state !== 'running') throw new Error('Stopped');
      const bytes = job.bytes + new TextEncoder().encode(data).length;
      if (bytes > 16_000_000 || job.last >= 50_000) throw new Error('Output limit');
      this.putPayload('event', job.last + 1, data);
      await txn.put('job', { ...job, bytes, last: job.last + 1 });
    });
    this.changed();
  }
  async alarm(): Promise<void> {
    const job = await this.ctx.storage.get<Job>('job');
    if (!job) return;
    if (job.expires <= Date.now()) {
      this.ctx.storage.sql.exec('DELETE FROM payload');
      await this.ctx.storage.put('job', { ...job, state: 'expired' });
      return;
    }
    // A runtime crash may have executed tools already: do not silently replay side effects.
    if (job.state === 'running') { await this.finish('failed', '云端运行意外中断，已保留内容，请重新生成。'); return; }
    if (job.state !== 'queued') { await this.ctx.storage.setAlarm(job.expires); return; }
    const body = this.payload('request', 0);
    await this.ctx.storage.put('job', { ...job, state: 'running' });
    this.controller = new AbortController();
    const signal = AbortSignal.any([this.controller.signal, AbortSignal.timeout(600_000)]);
    let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;
    try {
      const response = await handle(new Request('https://internal/v1/chat/completions', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body, signal }), this.env, fetch, job.owner);
      if (!response.ok || !response.body) throw new Error('Model unavailable');
      reader = response.body.getReader();
      const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: false });
      let buffer = '', lines: string[] = [], eventBytes = 0, totalBytes = 0, done = false;
      while (!done) {
        const next = await reader.read();
        if (next.done) break;
        totalBytes += next.value.byteLength;
        if (totalBytes > 16_000_000) throw new Error('Output limit');
        buffer += decoder.decode(next.value, { stream: true });
        if (buffer.length > 5_000_000) throw new Error('Event limit');
        let end: number;
        while ((end = buffer.indexOf('\n')) >= 0) {
          const line = buffer.slice(0, end).replace(/\r$/, ''); buffer = buffer.slice(end + 1);
          if (line.startsWith('data:')) { const data = line.slice(5).replace(/^ /, ''); lines.push(data); eventBytes += data.length; if (eventBytes > 5_000_000) throw new Error('Event limit'); }
          else if (!line && lines.length) {
            const data = lines.join('\n'); lines = []; eventBytes = 0;
            if (data === '[DONE]') { done = true; break; }
            const value = JSON.parse(data);
            if (value.error) throw new Error('Upstream failure');
            await this.append(data);
            if (value.choices?.[0]?.finish_reason === 'length') {
              await this.finish('failed', '回复达到模型输出上限，已保留内容。'); return;
            }
          }
        }
      }
      if (!done) throw new Error('Missing completion marker');
      await this.finish('complete');
    } catch {
      await this.finish('failed', '模型或工具服务未完成回复，已保留内容，请重新生成。');
    } finally {
      this.controller?.abort(); this.controller = undefined;
      await reader?.cancel().catch(() => {});
    }
  }
}
