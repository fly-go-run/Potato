import { DurableObject } from 'cloudflare:workers';
import { createRemoteJWKSet, jwtVerify } from 'jose';
import { response, secret, hash, equal, json } from './remote';

// Access protects only /v1/remote/auth/authorize. Native clients use a
// short-lived browser handoff, then independent revocable application sessions.
export interface AccessConfig { REMOTE_ACCESS_TEAM: string; REMOTE_ACCESS_AUD: string; REMOTE_PUBLIC_URL: string }
type Principal = { owner: string; email: string };
type Session = { email?: string; hash: string; role: 'host' | 'phone' | 'cloud'; name: string; expires: number };
type Device = { id: string; name: string };
type Login = { cloudLogin?: boolean; hash: string; role: 'host' | 'phone' | 'cloud'; name: string; expires: number; code: string; principal?: Principal; csrf?: string; csrfOwner?: string };
const tokenPattern = /^([a-f0-9]{64})\.([a-f0-9-]{36})\.([a-f0-9]{64})$/;
const keysets = new Map<string, ReturnType<typeof createRemoteJWKSet>>();

function config(env: Env, cloudLogin = false) {
  const values = env as Env & AccessConfig;
  const audience = cloudLogin ? env.CLOUD_ACCESS_AUD : values.REMOTE_ACCESS_AUD;
  if (!/^https:\/\/[a-z0-9-]+\.cloudflareaccess\.com$/.test(values.REMOTE_ACCESS_TEAM ?? '') || !audience) throw new Error('Cloudflare 登录尚未配置');
  const url = new URL(values.REMOTE_PUBLIC_URL);
  if (url.protocol !== 'https:' || url.pathname !== '/' || url.search || url.hash || url.username || url.password) throw new Error('登录地址无效');
  return { issuer: values.REMOTE_ACCESS_TEAM, audience, origin: url.origin };
}
export async function principal(request: Request, env: Env, cloudLogin = false): Promise<Principal> {
  const { issuer, audience } = config(env, cloudLogin);
  const token = request.headers.get('cf-access-jwt-assertion');
  if (!token || token.length > 16384) throw new Error('请先使用 Cloudflare 登录');
  let keys = keysets.get(issuer);
  if (!keys) { keys = createRemoteJWKSet(new URL(issuer + '/cdn-cgi/access/certs')); keysets.set(issuer, keys); }
  const { payload } = await jwtVerify(token, keys, { algorithms: ['RS256'], issuer, audience, requiredClaims: ['sub', 'exp', 'iat'] });
  if (!payload.sub || typeof payload.email !== 'string' || !payload.email) throw new Error('需要用户身份登录');
  return { owner: await hash(issuer + '\n' + payload.sub), email: payload.email.slice(0, 320) };
}
export function sessionParts(request: Request) { return (request.headers.get('authorization')?.replace(/^Bearer /, '') ?? '').match(tokenPattern); }
export async function phoneSession(request: Request, env: Env, owner: string): Promise<boolean> {
  const token = sessionParts(request);
  return !!token && token[1] === owner && await env.REMOTE_ACCOUNTS.getByName(owner).verify(token[2], token[3], 'phone');
}

/** One object per verified issuer+subject; never key accounts by supplied email.
 * Account mutations serialize across device RPC awaits, preventing logout and
 * re-registration from resurrecting a revoked device. These RPCs never call back. */
export class RemoteAccount extends DurableObject<Env> {
  async issue(id: string, login: Login) {
    return this.ctx.blockConcurrencyWhile(async () => {
      if (!login.principal || login.expires <= Date.now()) throw new Error('登录已过期');
      if (await this.ctx.storage.get('revoked:' + id)) throw new Error('Session revoked');
      const existing = await this.ctx.storage.get<Session>('session:' + id);
      if (existing) { if (!equal(existing.hash, login.hash)) throw new Error('Session conflict'); return; }
      // Login handoffs expire after five minutes; retain revocation tombstones
      // for a full application-session lifetime, then reclaim them.
      for (const [key, expires] of await this.ctx.storage.list<number>({ prefix: 'revoked:' })) if (typeof expires === 'number' && expires <= Date.now()) await this.ctx.storage.delete(key);
      const sessions = await this.ctx.storage.list<Session>({ prefix: 'session:' });
      for (const [key, value] of sessions) if (value.expires <= Date.now()) { await this.ctx.storage.delete(key); sessions.delete(key); }
      if (sessions.size >= 24) throw new Error('登录设备过多，请先退出旧设备');
      await this.ctx.storage.put('session:' + id, { email: login.principal.email, hash: login.hash, role: login.role, name: login.name, expires: Date.now() + 30 * 86400000 } satisfies Session);
    });
  }
  async verify(id: string, token: string, role?: Session['role']) {
    const digest = await hash(token), value = await this.ctx.storage.get<Session>('session:' + id);
    return !(await this.ctx.storage.get('revoked:' + id)) && !!value && value.expires > Date.now() && (!role || value.role === role) && equal(value.hash, digest);
  }
  async cloudIdentity(id: string, token: string) {
    return this.ctx.blockConcurrencyWhile(async () => {
      if (!await this.verify(id, token, 'cloud')) return null;
      const session = await this.ctx.storage.get<Session>('session:' + id);
      // Older sessions have no verified email. Require a new signed login.
      return session?.email ? { email: session.email } : null;
    });
  }
  async devices(id: string, token: string) {
    if (!await this.verify(id, token, 'phone')) return null;
    return [...(await this.ctx.storage.list<Device>({ prefix: 'device:' })).values()];
  }
  async registerDevice(owner: string, sessionID: string, token: string, hostToken: string) {
    return this.ctx.blockConcurrencyWhile(async () => {
      if (!/^[a-f0-9]{64}$/.test(hostToken) || !await this.verify(sessionID, token, 'host')) return null;
      const session = (await this.ctx.storage.get<Session>('session:' + sessionID))!;
      const devices = await this.ctx.storage.list<Device>({ prefix: 'device:' });
      if (devices.size >= 16 && !devices.has('device:' + sessionID)) throw new Error('已达到电脑数量上限');
      const device = { id: sessionID, name: session.name };
      await this.env.REMOTE_DEVICES.getByName(device.id).registerAccount(owner, device.name, hostToken);
      await this.ctx.storage.put('device:' + device.id, device);
      return device;
    });
  }
  async revokeDevice(owner: string, sessionID: string, token: string, deviceID: string) {
    return this.ctx.blockConcurrencyWhile(async () => {
      if (!await this.verify(sessionID, token, 'phone')) return false;
      if (!await this.ctx.storage.get('device:' + deviceID)) return false;
      await this.env.REMOTE_DEVICES.getByName(deviceID).revokeAccount(owner);
      await this.ctx.storage.put('revoked:' + deviceID, Date.now() + 30 * 86400000);
      await this.ctx.storage.delete('device:' + deviceID); await this.ctx.storage.delete('session:' + deviceID); return true;
    });
  }
  async logout(owner: string, sessionID: string, token: string) {
    return this.ctx.blockConcurrencyWhile(async () => {
      if (!await this.verify(sessionID, token)) return false;
      await this.ctx.storage.put('revoked:' + sessionID, Date.now() + 30 * 86400000);
      if (await this.ctx.storage.get('device:' + sessionID)) { await this.env.REMOTE_DEVICES.getByName(sessionID).revokeAccount(owner); await this.ctx.storage.delete('device:' + sessionID); }
      await this.ctx.storage.delete('session:' + sessionID); return true;
    });
  }
}

/** Native secret never enters a browser URL or an Access cookie. */
export class RemoteLogin extends DurableObject<Env> {
  async start(hash: string, role: Login['role'], name: string, cloudLogin = false) {
    const login: Login = { cloudLogin, hash, role, name, expires: Date.now() + 5 * 60000, code: secret().slice(0, 8).toUpperCase() };
    await this.ctx.storage.put('login', login); await this.ctx.storage.setAlarm(login.expires);
    return { code: login.code, expires: login.expires };
  }
  async page(user: Principal, cloudLogin = false) {
    const login = await this.ctx.storage.get<Login>('login');
    if (!login || login.expires <= Date.now() || !!login.cloudLogin !== cloudLogin || cloudLogin && login.role !== 'cloud') return null;
    const csrf = secret(); login.csrf = await hash(csrf); login.csrfOwner = user.owner;
    await this.ctx.storage.put('login', login);
    return { code: login.code, name: login.name, role: login.role, csrf };
  }
  async authorize(user: Principal, csrf: string, cloudLogin = false) {
    const digest = await hash(csrf);
    return this.ctx.storage.transaction(async tx => {
      const login = await tx.get<Login>('login');
      if (!login || login.expires <= Date.now() || !!login.cloudLogin !== cloudLogin || cloudLogin && login.role !== 'cloud' || !login.csrf || !equal(login.csrf, digest) || login.csrfOwner !== user.owner || login.principal && login.principal.owner !== user.owner) return false;
      login.principal = user; delete login.csrf; delete login.csrfOwner;
      await tx.put('login', login); return true;
    });
  }
  async poll(id: string, token: string, cloudLogin = false) {
    const digest = await hash(token), login = await this.ctx.storage.get<Login>('login');
    if (!login || login.expires <= Date.now() || !!login.cloudLogin !== cloudLogin || !equal(login.hash, digest)) return response({ error: '登录已过期，请重新开始' }, 401);
    if (!login.principal) return response({ status: 'pending' });
    await this.env.REMOTE_ACCOUNTS.getByName(login.principal.owner).issue(id, login);
    return response({ status: 'authorized', owner: login.principal.owner, email: login.principal.email, expires: Date.now() + 30 * 86400000 });
  }
  async alarm() { await this.ctx.storage.deleteAll(); }
}

const escape = (value: string) => value.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!));
// Native form POSTs need a non-null Origin; no-referrer suppresses it in browsers.
// same-origin keeps the login URL private across origins while preserving CSRF checks.
function page(content: string, status = 200) {
  return new Response(`<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>登录 Potato</title><body style="margin:0;background:#f8f7f4;color:#20201e;font:17px -apple-system,system-ui,sans-serif"><main style="max-width:420px;padding:60px 24px;margin:auto"><h1>Potato</h1>${content}</main></body></html>`, { status, headers: { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store', 'Referrer-Policy': 'same-origin', 'Content-Security-Policy': "default-src 'none'; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'", 'X-Content-Type-Options': 'nosniff' } });
}
export async function handleRemoteAccount(request: Request, env: Env): Promise<Response | null> {
  const url = new URL(request.url);
  const cloudLogin = url.pathname.startsWith('/v1/cloud/');
  const path = cloudLogin ? url.pathname.replace('/v1/cloud/', '/v1/remote/') : url.pathname;
  if (!path.startsWith('/v1/remote/auth/') && !path.startsWith('/v1/remote/account/')) return null;
  try {
    if (path === '/v1/remote/auth/start' && request.method === 'POST') {
      const { origin } = config(env, cloudLogin), body = await json(request);
      if (cloudLogin && body.role !== 'cloud') return response({ error: '此入口仅用于云端模型登录' }, 403);
      if (typeof body.client_token !== 'string' || !/^[a-f0-9]{64}$/.test(body.client_token) || !['phone', 'host', 'cloud'].includes(String(body.role)) || typeof body.name !== 'string' || !body.name.trim() || body.name.length > 80) return response({ error: '无效的登录请求' }, 400);
      const id = crypto.randomUUID(), login = await env.REMOTE_LOGINS.getByName(id).start(await hash(body.client_token), body.role as Login['role'], body.name.trim(), cloudLogin);
      return response({ id, verification_url: `${origin}/v1/${cloudLogin ? 'cloud' : 'remote'}/auth/authorize?id=${id}`, ...login });
    }
    if (path === '/v1/remote/auth/poll' && request.method === 'POST') {
      const body = await json(request);
      if (typeof body.id !== 'string' || !/^[a-f0-9-]{36}$/.test(body.id) || typeof body.client_token !== 'string' || !/^[a-f0-9]{64}$/.test(body.client_token)) return response({ error: '无效的登录请求' }, 400);
      return await env.REMOTE_LOGINS.getByName(body.id).poll(body.id, body.client_token, cloudLogin);
    }
    if (path === '/v1/remote/auth/authorize') {
      const { origin } = config(env, cloudLogin), user = await principal(request, env, cloudLogin), id = url.searchParams.get('id') ?? '';
      if (!/^[a-f0-9-]{36}$/.test(id)) return page('<p>登录请求无效。</p>', 400);
      if (cloudLogin && !(env.CLOUD_ALLOWED_EMAILS ?? '').split(',').some(email => email.trim().toLowerCase() === user.email.toLowerCase())) return page('<p>此邮箱尚未获准使用云端模型，请联系邀请你的人。</p>', 403);
      const login = env.REMOTE_LOGINS.getByName(id);
      if (request.method === 'GET') {
        const detail = await login.page(user, cloudLogin);
        if (!detail) return page('<p>登录已过期，请返回 Potato 重新开始。</p>', 410);
        return page(`<h2>${detail.role === 'cloud' ? '登录云端模型' : '关联' + (detail.role === 'host' ? '电脑' : 'iPhone')}</h2><p>${escape(detail.name)}</p><p>当前账号：${escape(user.email)}</p><p>请核对 Potato 中显示的验证码：</p><p style="font-size:30px;letter-spacing:5px">${detail.code}</p><p>${detail.role === 'cloud' ? '此设备仅可使用账号获准的云端模型、语音转写和云端计算，不授予远程电脑访问权限。' : detail.role === 'host' ? '这台电脑开启远程访问后，可供同账号的手机控制。' : '此设备可以控制同账号已开启远程访问的电脑。'}</p><form method="post"><input type="hidden" name="csrf" value="${detail.csrf}"><button style="padding:14px 28px;background:#20201e;color:white;border:0;border-radius:26px;font:inherit">确认登录</button></form>`);
      }
      if (request.method === 'POST') {
        if (request.headers.get('origin') !== origin) return page('<p>登录请求无效。</p>', 403);
        // Bound form input before parsing; Access claims are never taken from form fields.
        const reader = request.body?.getReader(); let data = '';
        if (!reader) return page('<p>登录请求无效。</p>', 400);
        try { for (;;) { const chunk = await reader.read(); if (chunk.done) break; data += new TextDecoder().decode(chunk.value); if (data.length > 256) break; } } finally { await reader.cancel(); }
        if (data.length > 256 || !await login.authorize(user, new URLSearchParams(data).get('csrf') ?? '', cloudLogin)) return page('<p>登录请求已失效，请返回 Potato 重试。</p>', 403);
        return page('<h2>登录成功</h2><p>现在可以关闭此页面，返回 Potato。</p>');
      }
    }
    if (path.startsWith('/v1/remote/account/')) {
      const parts = sessionParts(request);
      if (!parts) return response({ error: '请登录 Cloudflare 账号' }, 401);
      const [, owner, sessionID, token] = parts, account = env.REMOTE_ACCOUNTS.getByName(owner);
      if (cloudLogin && (path !== '/v1/remote/account/logout' || !await account.verify(sessionID, token, 'cloud'))) return response({ error: '此会话仅可使用云端模型' }, 403);
      if (path === '/v1/remote/account/devices' && request.method === 'GET') {
        const devices = await account.devices(sessionID, token);
        return devices ? response({ devices }) : response({ error: '登录已失效，请重新登录' }, 401);
      }
      if (path === '/v1/remote/account/register' && request.method === 'POST') {
        const body = await json(request);
        const device = await account.registerDevice(owner, sessionID, token, String(body.host_token ?? ''));
        return device ? response(device) : response({ error: '需要在电脑上重新登录' }, 401);
      }
      if (path === '/v1/remote/account/revoke' && request.method === 'POST') {
        const body = await json(request);
        return await account.revokeDevice(owner, sessionID, token, String(body.device_id ?? '')) ? response({ ok: true }) : response({ error: '设备不存在或登录失效' }, 401);
      }
      if (path === '/v1/remote/account/logout' && request.method === 'POST') return await account.logout(owner, sessionID, token) ? response({ ok: true }) : response({ error: '登录已失效' }, 401);
    }
    return response({ error: 'Not found' }, 404);
  } catch { return response({ error: 'Cloudflare 登录不可用，请检查服务配置或重新登录' }, 401); }
}
