import { before, after, test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { Miniflare, convertV4MiniflareOptions } from 'miniflare';
import { randomUUID } from 'node:crypto';
import { generateKeyPair, exportJWK, SignJWT } from 'jose';

// These tests execute the real Worker + SQLite Durable Object + WebSocket stack.
// No account credentials, remote Cloudflare bindings, or model calls are used.
let mf: Miniflare;
let workerSource: string;
let signingKey: CryptoKey;
const issuer = 'https://potato-test.cloudflareaccess.com', audience = 'potato-test-audience';
const bootstrap = 'local-fixture-only-'.repeat(4);
before(async () => {
  const keys = await generateKeyPair('RS256'); signingKey = keys.privateKey;
  const jwk = { ...await exportJWK(keys.publicKey), kid: 'fixture-key', alg: 'RS256' };
  const bundle = await build({
    entryPoints: ['src/entry.ts'],
    bundle: true, write: false, format: 'esm', platform: 'browser', external: ['cloudflare:workers', 'node:*'],
  });
  workerSource = bundle.outputFiles[0].text;
  mf = new Miniflare(convertV4MiniflareOptions({ workers: [{
    name: 'remote-fixture',
    modules: true, script: bundle.outputFiles[0].text, compatibilityDate: '2026-09-12', compatibilityFlags: ['nodejs_compat'],
    bindings: { CLOUD_ACCESS_AUD: 'potato-cloud-audience', CLOUD_AUTH_REQUIRED: 'true', CLOUD_ALLOWED_EMAILS: 'same-address@example.test', CLOUD_PROVIDERS: JSON.stringify({ default_model: 'fixture/one', providers: [{ id: 'fixture', name: 'Fixture', endpoint: 'https://model.invalid/chat/completions', api_key: 'synthetic-secret', models: [{ id: 'one', name: 'One' }] }] }), MAX_OUTPUT_TOKENS: '128', CLIENT_TOKEN: bootstrap, REMOTE_ACCESS_TEAM: issuer, REMOTE_ACCESS_AUD: audience, REMOTE_PUBLIC_URL: 'https://remote.example.test' }, durableObjects: { REMOTE_DEVICES: { className: 'RemoteDevice', useSQLite: true }, REMOTE_ACCOUNTS: { className: 'RemoteAccount', useSQLite: true }, REMOTE_LOGINS: { className: 'RemoteLogin', useSQLite: true } },
    outboundService: async (request: Request) => request.url === issuer + '/cdn-cgi/access/certs' ? Response.json({keys:[jwk]}) : new Response('Unexpected upstream request', {status:500}),
    ratelimits: { CHAT_RATE_LIMIT: { namespace_id: '1001', simple: { limit: 10000, period: 60 } }, REMOTE_RATE_LIMIT: { namespace_id: '1005', simple: { limit: 10000, period: 60 } } },
  }] }));
  await mf.ready;
});
after(async () => { await mf?.dispose(); });
const freshToken = () => (randomUUID() + randomUUID()).replaceAll('-', '');
function request(path: string, token?: string, body?: unknown, extra: Record<string, string> = {}) {
  return mf.dispatchFetch('http://localhost/v1/remote/' + path, {
    method: body === undefined ? 'GET' : 'POST', headers: { ...(token ? { Authorization: `Bearer ${token}` } : {}), ...extra },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
}
async function register() {
  const r = await request('register', bootstrap, { name: '测试电脑' }); assert.equal(r.status, 200);
  return await r.json() as { id: string; host_token: string; pair_token: string };
}
async function paired() {
  const device = await register(), phone = freshToken();
  const r = await request(`${device.id}/pair`, undefined, { pair_token: device.pair_token, phone_token: phone });
  assert.equal(r.status, 200); return { ...device, phone };
}
async function connect(device: { id: string; host_token: string }) {
  const r = await request(`${device.id}/connect`, device.host_token, undefined, { Upgrade: 'websocket' });
  assert.equal(r.status, 101); const ws = r.webSocket!; ws.accept(); return ws;
}
function nextMessage(ws: Awaited<ReturnType<typeof connect>>) {
  return new Promise<any>((resolve, reject) => {
    const timer = setTimeout(() => { ws.removeEventListener('message', listener); reject(new Error('No host message')); }, 3000);
    const listener = (event: MessageEvent) => { clearTimeout(timer); ws.removeEventListener('message', listener); resolve(JSON.parse(String(event.data))); };
    ws.addEventListener('message', listener);
  });
}

test('registration requires bootstrap; pairing retries survive lost response but cannot enroll a second phone', async () => {
  assert.equal((await request('register', 'wrong', { name: 'test' })).status, 401);
  const d = await register(), phone = freshToken(), body = { pair_token: d.pair_token, phone_token: phone };
  const first = await request(`${d.id}/pair`, undefined, body); assert.equal(first.status, 200);
  assert.deepEqual(await first.json(), { name: '测试电脑' });
  assert.equal((await request(`${d.id}/pair`, undefined, body)).status, 200);
  assert.equal((await request(`${d.id}/pair`, undefined, { ...body, phone_token: freshToken() })).status, 401);
  assert.equal((await request(`${d.id}/status`, phone)).status, 200);
  assert.equal((await request(`${d.id}/status`, d.host_token)).status, 401);
  assert.equal((await request(`${d.id}/connect`, phone, undefined, { Upgrade: 'websocket' })).status, 404);
});

test('simultaneous pairing claims exactly one phone and rotation revokes its access', async () => {
  const d = await register(), tokens = [freshToken(), freshToken()];
  const attempts = await Promise.all(tokens.map(phone_token => request(`${d.id}/pair`, undefined, { pair_token: d.pair_token, phone_token })));
  assert.deepEqual(attempts.map(r => r.status).sort(), [200, 401]);
  const phone = tokens[attempts.findIndex(r => r.status === 200)];
  const reset = await request(`${d.id}/pairing`, d.host_token, {}); assert.equal(reset.status, 200);
  assert.equal((await request(`${d.id}/status`, phone)).status, 401);
  assert.equal((await request(`${d.id}/pair`, undefined, { pair_token: d.pair_token, phone_token: phone })).status, 401);
});

test('real relay routes commands and results to the scoped desktop and reports offline', async () => {
  const d = await paired(), other = await paired();
  assert.equal((await request(`${other.id}/status`, d.phone)).status, 401);
  assert.deepEqual(await (await request(`${d.id}/status`, d.phone)).json(), { name: '测试电脑', online: false });
  assert.equal((await request(`${d.id}/rpc`, d.phone, { id: randomUUID(), op: 'send', args: { text: 'test' } })).status, 503);
  const ws = await connect(d);
  assert.equal((await (await request(`${d.id}/status`, d.phone)).json()).online, true);
  const received = nextMessage(ws), id = randomUUID();
  const reply = request(`${d.id}/rpc`, d.phone, { id, op: 'overview', args: {}, transport_id: 'untrusted-id' });
  const command = await received;
  assert.equal(command.id, id); assert.notEqual(command.transport_id, 'untrusted-id');
  ws.send(JSON.stringify({ transport_id: command.transport_id, result: { chats: [], projects: [] } }));
  assert.deepEqual(await (await reply).json(), { result: { chats: [], projects: [] } });
  const queued = nextMessage(ws);
  const queueReply = request(`${d.id}/rpc`, d.phone, { id: randomUUID(), op: 'outbox', args: { chat_id: 'chat', action: 'promote', item_id: 'item', expected_run_id: 'run' } });
  const queueCommand = await queued;
  assert.equal(queueCommand.op, 'outbox');
  assert.equal(queueCommand.args.expected_run_id, 'run');
  ws.send(JSON.stringify({ transport_id: queueCommand.transport_id, result: { items: [] } }));
  assert.deepEqual(await (await queueReply).json(), { result: { items: [] } });
  assert.equal((await request(`${d.id}/rpc`, d.phone, { id: randomUUID(), op: '/api/models', args: {} })).status, 400);
  assert.equal((await request(`${d.id}/rpc`, d.phone, { id: randomUUID(), op: 'send', args: { text: 'x'.repeat(132000) } })).status, 400);
  ws.close();
});

test('an old socket close does not fail commands sent to the replacement socket', async () => {
  const d = await paired(), old = await connect(d), current = await connect(d);
  const received = nextMessage(current);
  const reply = request(`${d.id}/rpc`, d.phone, { id: randomUUID(), op: 'overview', args: {} });
  const command = await received;
  old.close();
  current.send(JSON.stringify({ transport_id: command.transport_id, result: { version: 'replacement' } }));
  const result = await reply; assert.equal(result.status, 200); assert.deepEqual(await result.json(), { result: { version: 'replacement' } });
  current.close();
});

test('rotation invalidates pending responses and requests blocked on incoming body data', async () => {
  const d = await paired(), ws = await connect(d);
  let release!: (bytes: Uint8Array) => void;
  const stream = new ReadableStream<Uint8Array>({ start(controller) { release = bytes => { controller.enqueue(bytes); controller.close(); }; } });
  const received = nextMessage(ws);
  const pending = request(`${d.id}/rpc`, d.phone, { id: randomUUID(), op: 'overview', args: {} });
  await received;
  const slow = mf.dispatchFetch(`http://localhost/v1/remote/${d.id}/rpc`, { method: 'POST', headers: { Authorization: `Bearer ${d.phone}` }, body: stream, duplex: 'half' });
  assert.equal((await request(`${d.id}/pairing`, d.host_token, {})).status, 200);
  release(new TextEncoder().encode(JSON.stringify({ id: randomUUID(), op: 'overview', args: {} })));
  assert.equal((await slow).status, 401);
  assert.equal((await pending).status, 503);
  ws.close();
});

async function jwt(sub: string, aud = audience, expiration = '5m') {
  return new SignJWT({ email: 'same-address@example.test' }).setProtectedHeader({ alg: 'RS256', kid: 'fixture-key' }).setIssuer(issuer).setSubject(sub).setAudience(aud).setIssuedAt().setExpirationTime(expiration).sign(signingKey);
}
async function accountLogin(sub: string, role: 'host' | 'phone' | 'cloud') {
  const client_token = freshToken();
  const start = await request('auth/start', undefined, { client_token, role, name: 'Fixture ' + role }); assert.equal(start.status, 200);
  const login = await start.json();
  assert.equal((await request('auth/poll', undefined, {id:login.id,client_token:freshToken()})).status,401);
  assert.equal((await (await request('auth/poll', undefined, {id:login.id,client_token})).json()).status,'pending');
  const page = await request('auth/authorize?id=' + login.id, undefined, undefined, {'cf-access-jwt-assertion':await jwt(sub)});
  assert.equal(page.status,200);
  assert.equal(page.headers.get('Referrer-Policy'), 'same-origin');
  const html = await page.text(), csrf = html.match(/name="csrf" value="([a-f0-9]+)"/)![1];
  assert.ok(html.includes(login.code)); assert.ok(!html.includes(client_token));
  const body = new URLSearchParams({csrf}).toString();
  const approve = await mf.dispatchFetch('http://localhost/v1/remote/auth/authorize?id=' + login.id, {method:'POST',headers:{Origin:'https://remote.example.test','Content-Type':'application/x-www-form-urlencoded','cf-access-jwt-assertion':await jwt(sub)},body});
  assert.equal(approve.status,200);
  const authorized = await (await request('auth/poll',undefined,{id:login.id,client_token})).json();
  assert.equal(authorized.status,'authorized');
  return `${authorized.owner}.${login.id}.${client_token}`;
}

test('Cloudflare signed identity links both clients; same email with another subject grants no access', async () => {
  const host = await accountLogin('owner-one','host'), phone = await accountLogin('owner-one','phone');
  const host_token = freshToken();
  const registration = await request('account/register',host,{host_token}); assert.equal(registration.status,200);
  const device = await registration.json();
  assert.equal((await request('account/register',phone,{host_token:freshToken()})).status,401);
  assert.deepEqual((await (await request('account/devices',phone)).json()).devices,[device]);
  const stranger = await accountLogin('owner-two','phone');
  assert.deepEqual((await (await request('account/devices',stranger)).json()).devices,[]);
  assert.equal((await request(`${device.id}/status`,stranger)).status,401);
  const ws = await connect({id:device.id,host_token});
  assert.equal((await (await request(`${device.id}/status`,phone)).json()).online,true);
  const received = nextMessage(ws);
  const response = request(`${device.id}/rpc`,phone,{id:randomUUID(),op:'overview',args:{}});
  const message = await received; ws.send(JSON.stringify({transport_id:message.transport_id,result:{from:'account-linked-desktop'}}));
  assert.equal((await (await response).json()).result.from,'account-linked-desktop');
  assert.equal((await request('account/revoke',phone,{device_id:device.id})).status,200);
  assert.equal((await request(`${device.id}/status`,phone)).status,404);
  assert.equal((await request('account/register',host,{host_token})).status,401);
  assert.equal((await request('account/logout',phone,{})).status,200);
  assert.equal((await request('account/devices',phone)).status,401);
  const [, loginID, client_token] = phone.split('.');
  assert.equal((await request('auth/poll',undefined,{id:loginID,client_token})).status,401);
  ws.close();
});

test('login rejects forged identity, wrong audience, expired JWT, cross-origin consent and replayed CSRF',async()=>{
  const client_token=freshToken(), login=await (await request('auth/start',undefined,{client_token,role:'phone',name:'iPhone'})).json();
  for (const token of ['forged',await jwt('owner','wrong-audience'),await jwt('owner',audience,'-1m')]) assert.equal((await request('auth/authorize?id='+login.id,undefined,undefined,{'cf-access-jwt-assertion':token})).status,401);
  const token=await jwt('owner');
  const html=await (await request('auth/authorize?id='+login.id,undefined,undefined,{'cf-access-jwt-assertion':token})).text();
  const csrf=html.match(/name="csrf" value="([a-f0-9]+)"/)![1];
  const consent=(origin:string)=>mf.dispatchFetch('http://localhost/v1/remote/auth/authorize?id='+login.id,{method:'POST',headers:{Origin:origin,'cf-access-jwt-assertion':token},body:new URLSearchParams({csrf}).toString()});
  assert.equal((await consent('https://attacker.example')).status,403);
  assert.equal((await consent('null')).status,403);
  assert.equal((await consent('https://remote.example.test')).status,200);
  assert.equal((await consent('https://remote.example.test')).status,403);
});


test('concurrent desktop registration and logout cannot resurrect an account device', async () => {
  const host = await accountLogin('concurrent-owner', 'host');
  const phone = await accountLogin('concurrent-owner', 'phone');
  const host_token = freshToken(), deviceID = host.split('.')[1];
  const results = await Promise.all([
    request('account/register', host, {host_token}),
    request('account/logout', host, {}),
    request('account/register', host, {host_token}),
  ]);
  assert.equal(results[1].status, 200);
  assert.ok([200,401].includes(results[0].status));
  assert.ok([200,401].includes(results[2].status));
  assert.deepEqual((await (await request('account/devices', phone)).json()).devices, []);
  assert.equal((await request(`${deviceID}/connect`,host_token,undefined,{Upgrade:'websocket'})).status,404);
  const [, id, client_token] = host.split('.');
  assert.equal((await request('auth/poll', undefined, {id,client_token})).status,401);
});


test('remote emergency switch disables remote requests while health and existing auth keep working', async () => {
  const disabled = new Miniflare(convertV4MiniflareOptions({workers:[{
    name:'disabled-remote', modules:true, script:workerSource,
    compatibilityDate:'2026-09-12', compatibilityFlags:['nodejs_compat'],
    bindings:{REMOTE_CONTROL_ENABLED:'false',CLIENT_TOKEN:bootstrap,UPSTREAM_API_KEY:'fixture-key'},
  }]}));
  try {
    assert.equal((await disabled.dispatchFetch('http://localhost/v1/remote/auth/start',{method:'POST',body:'{}'})).status,503);
    assert.equal((await disabled.dispatchFetch('http://localhost/v1/remote/register',{method:'POST',body:'{}'})).status,503);
    assert.equal((await disabled.dispatchFetch('http://localhost/health')).status,200);
    assert.equal((await disabled.dispatchFetch('http://localhost/v1/chat/completions',{method:'POST',body:'{}'})).status,401);
  } finally { await disabled.dispose(); }
});

test('cloud-only signed sessions cannot access computers; remote and revoked sessions cannot use models', async () => {
  const phone = await accountLogin('cloud-user', 'cloud'), host = await accountLogin('cloud-user', 'host');
  const catalog = token => mf.dispatchFetch('https://worker.test/v1/models', { headers: { Authorization: `Bearer ${token}` } });
  const response = await catalog(phone); assert.equal(response.status, 200);
  assert.deepEqual((await response.json()).data.map(m => m.id), ['fixture/one']);
  assert.equal((await catalog(host)).status, 401);
  assert.equal((await request('account/devices', phone)).status, 401);
  const registration = await request('account/register', host, { host_token: freshToken() });
  const device = await registration.json(); assert.equal(registration.status, 200);
  assert.equal((await request(`${device.id}/status`, phone)).status, 401);
  assert.equal((await request(`${device.id}/rpc`, phone, { id: randomUUID(), op: 'overview', args: {} })).status, 401);
  assert.equal((await request('account/revoke', phone, { device_id: device.id })).status, 401);
  assert.equal((await request('account/register', phone, { host_token: freshToken() })).status, 401);
  const remotePhone = await accountLogin('cloud-user', 'phone');
  assert.equal((await catalog(remotePhone)).status, 401);
  assert.equal((await catalog(bootstrap)).status, 401);
  assert.equal((await request('account/logout', phone, {})).status, 200);
  assert.equal((await catalog(phone)).status, 401);
});

test('email login uses its own Access audience and cannot authorize remote-control roles', async () => {
  const cloudRequest = (path: string, body?: unknown, headers: Record<string,string> = {}) => mf.dispatchFetch('https://remote.example.test/v1/cloud/' + path, {method: body === undefined ? 'GET' : 'POST', headers, ...(body === undefined ? {} : {body: typeof body === 'string' ? body : JSON.stringify(body)})});
  const client_token = freshToken();
  assert.equal((await cloudRequest('auth/start', {client_token,role:'host',name:'Forbidden'})).status,403);
  const started = await cloudRequest('auth/start', {client_token,role:'cloud',name:'Potato Windows'});
  assert.equal(started.status,200); const login = await started.json();
  assert.match(login.verification_url,/\/v1\/cloud\/auth\/authorize/);
  const path = 'auth/authorize?id=' + login.id;
  assert.equal((await cloudRequest(path,undefined,{'cf-access-jwt-assertion':await jwt('email-user')})).status,401);
  const signed = await jwt('email-user','potato-cloud-audience');
  const page = await cloudRequest(path,undefined,{'cf-access-jwt-assertion':signed});assert.equal(page.status,200);
  const csrf = (await page.text()).match(/name="csrf" value="([a-f0-9]+)"/)![1];
  // The original remote endpoint must not accept a cloud handoff or cloud JWT.
  assert.notEqual((await request(path,undefined,undefined,{'cf-access-jwt-assertion':signed})).status,200);
  assert.equal((await request('auth/poll',undefined,{id:login.id,client_token})).status,401);
  const hostLogin = await (await request('auth/start',undefined,{client_token:freshToken(),role:'host',name:'Host'})).json();
  assert.equal((await cloudRequest('auth/authorize?id=' + hostLogin.id,undefined,{'cf-access-jwt-assertion':signed})).status,410);
  const post = await cloudRequest(path,new URLSearchParams({csrf}).toString(),{Origin:'https://remote.example.test','cf-access-jwt-assertion':signed});assert.equal(post.status,200);
  const result = await (await cloudRequest('auth/poll',{id:login.id,client_token})).json();assert.equal(result.status,'authorized');
  const token = `${result.owner}.${login.id}.${client_token}`;
  assert.equal((await mf.dispatchFetch('https://remote.example.test/v1/models',{headers:{Authorization:`Bearer ${token}`}})).status,200);
  assert.equal((await cloudRequest('account/register',{host_token:freshToken()},{Authorization:`Bearer ${token}`})).status,403);
  assert.equal((await request('account/devices',token)).status,401);
  assert.equal((await cloudRequest('account/logout',{}, {Authorization:`Bearer ${token}`})).status,200);
  assert.equal((await cloudRequest('auth/poll',{id:login.id,client_token})).status,401);
});
