import { gzipSync, gunzipSync } from 'node:zlib';
declare global { interface Env { DOUBAO_API_KEY?: string; DOUBAO_APP_ID?: string } }
export function speechFrame(kind: number, flags: number, json: boolean, data: Uint8Array) {
  const compressed = gzipSync(data), frame = new Uint8Array(8 + compressed.length);
  frame.set([0x11, (kind << 4) | flags, json ? 0x11 : 0x01, 0]);
  new DataView(frame.buffer).setUint32(4, compressed.length); frame.set(compressed, 8); return frame;
}
export function speechResult(bytes: Uint8Array): { type: 'partial' | 'final'; text: string } | null {
  if (bytes.length < 8 || bytes.length > 1_000_000 || bytes[0] >> 4 !== 1) throw new Error('Invalid speech frame.');
  const kind = bytes[1] >> 4, flags = bytes[1] & 15; let offset = (bytes[0] & 15) * 4;
  if (offset < 4) throw new Error('Invalid speech header.');
  if (flags & 1) offset += 4;
  if (kind === 15) throw new Error('Speech service rejected the request.');
  if (kind !== 9) return null;
  if (offset + 4 > bytes.length) throw new Error('Truncated speech frame.');
  const size = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(offset);
  if (offset + 4 + size !== bytes.length) throw new Error('Truncated speech payload.');
  const payload = bytes.subarray(offset + 4);
  const decoded = (bytes[2] & 15) === 1 ? gunzipSync(payload, { maxOutputLength: 1_000_000 }) : payload;
  const value = JSON.parse(new TextDecoder('utf-8', { fatal: true, ignoreBOM: false }).decode(decoded));
  const result = Array.isArray(value.result) ? value.result[0] : value.result;
  const text = typeof result?.text === 'string' ? result.text : '';
  if (text.length > 30000) throw new Error('Speech result too large.');
  if (!text && !(flags & 2)) return null;
  return { type: flags & 2 ? 'final' : 'partial', text };
}
export const speechConfiguration = { user: { uid: 'potato-iphone' }, audio: { format: 'pcm', codec: 'raw', rate: 16000, bits: 16, channel: 1 }, request: { model_name: 'bigmodel', enable_itn: true, enable_punc: true, show_utterances: true, result_type: 'full' } };
export async function connectSpeech(request: Request, env: Env, fetcher: typeof fetch = fetch): Promise<Response> {
  const headers: Record<string, string> = { Upgrade: 'websocket', 'X-Api-Resource-Id': env.DOUBAO_RESOURCE_ID, 'X-Api-Connect-Id': crypto.randomUUID() };
  if (env.DOUBAO_APP_ID) { headers['X-Api-App-Key'] = env.DOUBAO_APP_ID; headers['X-Api-Access-Key'] = env.DOUBAO_API_KEY!; }
  else headers['X-Api-Key'] = env.DOUBAO_API_KEY!;
  const handshake = new AbortController(), handshakeTimer = setTimeout(() => handshake.abort(), 12000);
  let response: Response;
  try { response = await fetcher('https://openspeech.bytedance.com/api/v3/sauc/bigmodel_async', { headers, redirect: 'manual', signal: AbortSignal.any([request.signal, handshake.signal]) }); }
  finally { clearTimeout(handshakeTimer); }
  const upstream = response.webSocket;
  if (!upstream) { await response.body?.cancel(); throw new Error('Speech connection failed.'); }
  // Current Workers deliver Blob by default; this protocol decodes synchronous byte frames.
  upstream.binaryType = 'arraybuffer'; upstream.accept();
  const pair = new WebSocketPair(), client = pair[0], server = pair[1];
  server.binaryType = 'arraybuffer'; server.accept();
  let finished = false, stopped = false, total = 0;
  let timer: ReturnType<typeof setTimeout>;
  const end = (error?: string) => {
    if (finished) return; finished = true; clearTimeout(timer);
    if (error) { try { server.send(JSON.stringify({ type: 'error', message: error })); } catch {} }
    try { server.close(error ? 1011 : 1000, error ? 'Transcription failed' : 'Finished'); } catch {}
    try { upstream.close(1000, 'Finished'); } catch {}
  };
  timer = setTimeout(() => end('语音连接已超时，请重新录音。'), 70_000);
  server.addEventListener('message', event => {
    try {
      if (finished) return;
      if (typeof event.data === 'string') {
        if (event.data.length > 100) throw new Error();
        const control = JSON.parse(event.data);
        if (control.type === 'cancel') { end(); return; }
        if (control.type !== 'stop' || stopped) throw new Error();
        stopped = true; clearTimeout(timer); timer = setTimeout(() => end('等待语音结果超时，请重试。'), 8000);
        upstream.send(speechFrame(2, 2, false, new Uint8Array()));
      } else {
        const bytes = new Uint8Array(event.data);
        total += bytes.length;
        if (stopped || !bytes.length || bytes.length % 2 || bytes.length > 65536 || total > 1_920_000) throw new Error();
        upstream.send(speechFrame(2, 0, false, bytes));
      }
    } catch { end('录音数据无效或超过60秒，请重新录音。'); }
  });
  upstream.addEventListener('message', event => {
    try {
      if (finished || typeof event.data === 'string') return;
      const result = speechResult(new Uint8Array(event.data));
      if (result) { server.send(JSON.stringify(result)); if (result.type === 'final') end(); }
    } catch { end('豆包语音识别失败，请检查服务配置或稍后重试。'); }
  });
  upstream.addEventListener('close', () => end(finished ? undefined : '语音连接中断，已识别的文字已保留。'));
  upstream.addEventListener('error', () => end('语音服务连接失败，请稍后重试。'));
  server.addEventListener('close', () => end()); server.addEventListener('error', () => end());
  try { upstream.send(speechFrame(1, 0, true, new TextEncoder().encode(JSON.stringify(speechConfiguration)))); server.send(JSON.stringify({ type: 'ready' })); }
  catch { end('语音服务连接失败，请稍后重试。'); }
  return new Response(null, { status: 101, webSocket: client });
}
