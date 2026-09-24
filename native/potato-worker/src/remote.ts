import { DurableObject } from 'cloudflare:workers';
import { timingSafeEqual } from 'node:crypto';
import { handleRemoteAccount, phoneSession } from './remote-auth';

export const response = (body: unknown, status = 200) => Response.json(body, { status, headers: { 'Cache-Control': 'no-store' } });
export const secret = () => crypto.randomUUID().replaceAll('-', '') + crypto.randomUUID().replaceAll('-', '');
export async function hash(value: string) { return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value))), b => b.toString(16).padStart(2, '0')).join(''); }
export function equal(a: string, b: string) { const x = Buffer.from(a), y = Buffer.from(b); return x.length === y.length && timingSafeEqual(x, y); }
export async function json(request: Request): Promise<Record<string, unknown>> {
  if (!request.body) throw new Error('Missing body');
  const reader = request.body.getReader(), chunks: Uint8Array[] = []; let size = 0;
  try { for (;;) { const { done, value } = await reader.read(); if (done) break; size += value.length; if (size > 131072) throw new Error('Too large'); chunks.push(value); } }
  finally { await reader.cancel(); }
  const body: unknown = JSON.parse(Buffer.concat(chunks).toString('utf8'));
  if (!body || typeof body !== 'object' || Array.isArray(body)) throw new Error('Invalid body');
  return body as Record<string, unknown>;
}
type Identity = { owner?: string; name: string; host: string; pair: string; expires: number; phone: string; claimedPair?: string; claimedUntil?: number };

/** One coordination object per desktop. Conversation contents are never stored here. */
export class RemoteDevice extends DurableObject<Env> {
  private pending = new Map<string, { socket: WebSocket; resolve: (r: Response) => void; timer: ReturnType<typeof setTimeout> }>();
  async register(name: string) {
    const hostToken = secret(), pairToken = secret();
    await this.ctx.storage.put('identity', { name, host: await hash(hostToken), pair: await hash(pairToken), expires: Date.now() + 300000, phone: '' } satisfies Identity);
    return { host_token: hostToken, pair_token: pairToken };
  }
  async registerAccount(owner: string, name: string, hostToken: string) {
    const digest = await hash(hostToken);
    const current = await this.ctx.storage.get<Identity>('identity');
    if (current) {
      if (current.owner !== owner || !equal(current.host, digest)) throw new Error('Device registration conflict');
      return;
    }
    await this.ctx.storage.put('identity', { owner, name, host: digest, pair: '', expires: 0, phone: '' } satisfies Identity);
  }
  async revokeAccount(owner: string) {
    const identity = await this.ctx.storage.get<Identity>('identity');
    if (identity?.owner !== owner) return;
    await this.ctx.storage.delete('identity');
    for (const ws of this.ctx.getWebSockets('host')) ws.close(1000, 'Access revoked');
    this.failPending('电脑授权已撤销');
  }
  private async authorized(request: Request, identity: Identity, digest: string) {
    if (identity.owner) return phoneSession(request, this.env, identity.owner);
    return !!identity.phone && equal(identity.phone, digest);
  }
  async fetch(request: Request): Promise<Response> {
    try {
      const identity = await this.ctx.storage.get<Identity>('identity');
      if (!identity) return response({ error: '设备不存在' }, 404);
      const path = new URL(request.url).pathname.split('/').at(-1);
      const token = request.headers.get('authorization')?.replace(/^Bearer /, '') ?? '';
      const digest = await hash(token);
      if (path === 'pair' && request.method === 'POST') {
        // The phone durably saves its own random token before sending it. A lost
        // response can be retried, but the same code cannot enroll a second phone.
        const body = await json(request);
        if (typeof body.phone_token !== 'string' || !/^[a-f0-9]{64}$/.test(body.phone_token) || typeof body.pair_token !== 'string' || !/^[a-f0-9]{64}$/.test(body.pair_token)) return response({ error: 'Invalid pairing' }, 400);
        const phoneHash = await hash(body.phone_token), pairHash = await hash(body.pair_token);
        const paired = await this.ctx.storage.transaction(async tx => {
          const current = await tx.get<Identity>('identity');
          if (!current) return false;
          if (current.claimedPair && (current.claimedUntil ?? 0) > Date.now() && equal(current.claimedPair, pairHash) && equal(current.phone, phoneHash)) return true;
          if (current.expires <= Date.now() || !current.pair || !equal(current.pair, pairHash)) return false;
          await tx.put('identity', { ...current, pair: '', expires: 0, phone: phoneHash, claimedPair: pairHash, claimedUntil: Date.now() + 300000 }); return true;
        });
        return paired ? response({ name: identity.name }) : response({ error: '配对码已过期或已使用，请在电脑重新生成' }, 401);
      }
      // Hashing yields: do not use a pre-revocation identity to reconnect.
      const latest = await this.ctx.storage.get<Identity>('identity');
      const host = token.length >= 32 && !!latest && equal(latest.host, digest);
      if (identity.owner && path === 'pairing') return response({ error: '账号设备请使用账号登录' }, 403);
      if (host && path === 'pairing' && request.method === 'POST') {
        const pairToken = secret();
        await this.ctx.storage.put('identity', { ...identity, pair: await hash(pairToken), expires: Date.now() + 300000, phone: '', claimedPair: '', claimedUntil: 0 });
        this.failPending('配对已重置');
        return response({ pair_token: pairToken });
      }
      if (host && path === 'connect' && request.headers.get('upgrade')?.toLowerCase() === 'websocket') {
        for (const ws of this.ctx.getWebSockets('host')) ws.close(1000, 'Reconnected');
        this.failPending('电脑正在重新连接，请刷新');
        const pair = new WebSocketPair(); this.ctx.acceptWebSocket(pair[1], ['host']);
        this.ctx.setWebSocketAutoResponse(new WebSocketRequestResponsePair('ping', 'pong'));
        return new Response(null, { status: 101, webSocket: pair[0] });
      }
      if (!token || !await this.authorized(request, identity, digest)) return response({ error: '连接已失效，请重新配对' }, 401);
      let socket = this.ctx.getWebSockets('host').find(ws => ws.readyState === WebSocket.OPEN);
      if (path === 'status' && request.method === 'GET') return response({ name: identity.name, online: !!socket });
      if (path !== 'rpc' || request.method !== 'POST') return response({ error: 'Not found' }, 404);
      if (!socket) return response({ error: '电脑离线，请保持 Potato 运行并检查网络' }, 503);
      if (this.pending.size >= 8) return response({ error: '请求过多，请稍后再试' }, 429);
      const body = await json(request);
      // Reading the body yields to other requests: a pairing reset or reconnect
      // may have happened meanwhile. Revalidate authorization at dispatch.
      const current = await this.ctx.storage.get<Identity>('identity');
      if (!current || !await this.authorized(request, current, digest)) return response({ error: '连接已失效，请重新配对' }, 401);
      socket = this.ctx.getWebSockets('host').find(ws => ws.readyState === WebSocket.OPEN);
      if (!socket) return response({ error: '电脑连接已断开' }, 503);
      if (this.pending.size >= 8) return response({ error: '请求过多，请稍后再试' }, 429);
      if (typeof body.id !== 'string' || !/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/i.test(body.id) || typeof body.op !== 'string' || !['overview', 'chat', 'send', 'stop', 'approval', 'answer', 'pin', 'outbox'].includes(body.op) || !body.args || typeof body.args !== 'object' || Array.isArray(body.args)) return response({ error: 'Invalid command' }, 400);
      const transportID = crypto.randomUUID();
      const target = socket;
      return await new Promise<Response>(resolve => {
        const timer = setTimeout(() => { this.pending.delete(transportID); resolve(response({ error: '电脑响应超时；操作可能已执行，请刷新后确认' }, 504)); }, 15000);
        this.pending.set(transportID, { socket: target, resolve, timer });
        try { target.send(JSON.stringify({ id: body.id, op: body.op, args: body.args, transport_id: transportID })); }
        catch { clearTimeout(timer); this.pending.delete(transportID); resolve(response({ error: '电脑连接已断开' }, 503)); }
      });
    } catch { return response({ error: '无效的远程请求' }, 400); }
  }
  webSocketMessage(ws: WebSocket, message: string | ArrayBuffer) {
    if (!this.ctx.getWebSockets('host').includes(ws) || typeof message !== 'string' || message.length > 2_000_000) { ws.close(1009, 'Invalid response'); return; }
    try {
      const value = JSON.parse(message), pending = this.pending.get(value.transport_id);
      if (!pending || pending.socket !== ws) return;
      clearTimeout(pending.timer); this.pending.delete(value.transport_id);
      pending.resolve(response(value.error ? { error: value.error } : { result: value.result }, value.error ? (Number.isInteger(value.status) && value.status >= 400 && value.status <= 599 ? value.status : 502) : 200));
    } catch { ws.close(1003, 'Invalid JSON'); }
  }
  webSocketClose(ws: WebSocket) { ws.close(1000, 'Closed'); this.failPending('电脑连接已断开，请刷新确认任务状态', ws); }
  webSocketError(ws: WebSocket) { this.failPending('电脑连接异常', ws); }
  private failPending(error: string, socket?: WebSocket) {
    for (const [id, p] of this.pending) { if (socket && p.socket !== socket) continue; clearTimeout(p.timer); p.resolve(response({ error }, 503)); this.pending.delete(id); }
  }
}

export async function handleRemote(request: Request, env: Env): Promise<Response> {
  const path = new URL(request.url).pathname;
  if (!(await env.REMOTE_RATE_LIMIT.limit({ key: request.headers.get('cf-connecting-ip') ?? 'local' })).success) return response({ error: '请求过多，请稍后重试' }, 429);
  const account = await handleRemoteAccount(request, env);
  if (account) return account;
  if (path === '/v1/remote/register' && request.method === 'POST') {
    if (!env.CLIENT_TOKEN || env.CLIENT_TOKEN.length < 32 || !equal(request.headers.get('authorization') ?? '', `Bearer ${env.CLIENT_TOKEN}`)) return response({ error: '服务连接令牌无效' }, 401);
    try {
      const body = await json(request), name = String(body.name ?? '').trim().slice(0, 80);
      if (!name) return response({ error: '请输入电脑名称' }, 400);
      const id = crypto.randomUUID(), value = await env.REMOTE_DEVICES.getByName(id).register(name);
      return response({ id, name, ...value });
    } catch { return response({ error: '注册请求无效' }, 400); }
  }
  const match = path.match(/^\/v1\/remote\/([a-f0-9-]{36})\/(pair|pairing|connect|status|rpc)$/);
  if (!match) return response({ error: 'Not found' }, 404);
  return env.REMOTE_DEVICES.getByName(match[1]).fetch(request);
}
