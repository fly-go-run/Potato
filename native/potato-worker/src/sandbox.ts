import { Sandbox } from '@e2b/code-interpreter';

declare global { interface Env { E2B_API_KEY?: string } }
type InputFile = { name: string; base64: string };
export type SandboxInput = { code: string; files: InputFile[] };
type Artifact = { name: string; mime: string; base64: string };
const MIME: Record<string, string> = { png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', pdf: 'application/pdf', csv: 'text/csv', txt: 'text/plain', md: 'text/markdown', docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document', xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' };
export function validateSandbox(value: unknown): SandboxInput {
  const body = value as Partial<SandboxInput> | null;
  if (!body || typeof body.code !== 'string' || !body.code.trim() || body.code.length > 32_000) throw new Error('Expected Python code up to 32000 characters.');
  if (!Array.isArray(body.files) || body.files.length > 4) throw new Error('Expected up to four files.');
  const names = new Set<string>();
  let total = 0;
  const files = body.files.map(file => {
    if (!file || typeof file.name !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,99}$/.test(file.name) || names.has(file.name)) throw new Error('Use unique simple filenames.');
    if (typeof file.base64 !== 'string' || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(file.base64)) throw new Error('Invalid file encoding.');
    total += file.base64.length;
    if (total > 2_800_000) throw new Error('Input files exceed 2 MB.');
    names.add(file.name); return { name: file.name, base64: file.base64 };
  });
  return { code: body.code, files };
}

// Fresh sandbox per explicit run: no shared session, no provider keys in the container.
export async function executeSandbox(input: SandboxInput, apiKey: string, signal: AbortSignal, create = (options: { template: string; apiKey: string; timeoutMs: number; requestTimeoutMs: number; allowInternetAccess: boolean }) => Sandbox.create(options.template, options), template = 'chat-web-office-pdf') {
  signal.throwIfAborted();
  const sandbox = await create({ template, apiKey, timeoutMs: 120_000, requestTimeoutMs: 20_000, allowInternetAccess: false });
  const abort = () => { void sandbox.kill({ requestTimeoutMs: 10_000 }).catch(() => {}); };
  signal.addEventListener('abort', abort, { once: true });
  try {
    signal.throwIfAborted();
    await sandbox.files.makeDir('/home/user/output');
    for (const file of input.files) {
      signal.throwIfAborted();
      const data = Uint8Array.from(atob(file.base64), c => c.charCodeAt(0));
      await sandbox.files.write('/home/user/' + file.name, data.buffer, { requestTimeoutMs: 10_000, signal });
    }
    let outputSize = 0;
    const checkOutput = (text: string) => { outputSize += text.length; if (outputSize > 4_000_000) throw new Error('Output limit exceeded.'); };
    const execution = await sandbox.runCode(input.code, {
      language: 'python', timeoutMs: 60_000, requestTimeoutMs: 65_000,
      onStdout: value => checkOutput(value.line), onStderr: value => checkOutput(value.line),
      onResult: value => checkOutput(JSON.stringify(value))
    });
    signal.throwIfAborted();
    const artifacts: Artifact[] = [];
    let artifactSize = 0;
    const add = (name: string, mime: string, base64: string) => {
      artifactSize += base64.length;
      if (base64.length > 2_800_000 || Buffer.from(base64, 'base64').byteLength > 2_000_000 || artifactSize > 4_000_000 || artifacts.length >= 8) throw new Error('Artifact limit exceeded.');
      artifacts.push({ name, mime, base64 });
    };
    for (const result of execution.results) {
      if (result.png) add(`chart-${artifacts.length + 1}.png`, 'image/png', result.png);
      else if (result.jpeg) add(`chart-${artifacts.length + 1}.jpg`, 'image/jpeg', result.jpeg);
    }
    for (const file of await sandbox.files.list('/home/user/output', { depth: 1, requestTimeoutMs: 10_000, signal })) {
      const mime = MIME[file.name.split('.').pop()?.toLowerCase() ?? ''];
      if (file.type !== 'file' || !mime || !/^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,99}$/.test(file.name)) continue;
      const stream = await sandbox.files.read('/home/user/output/' + file.name, { format: 'stream', requestTimeoutMs: 10_000, signal });
      const reader = stream.getReader(), chunks: Uint8Array[] = []; let size = 0;
      try {
        for (;;) { const { done, value } = await reader.read(); if (done) break; size += value.length; if (size > 2_000_000) throw new Error('Artifact too large.'); chunks.push(value); }
      } finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
      const bytes = new Uint8Array(size); let offset = 0;
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
      add(file.name, mime, Buffer.from(bytes).toString('base64'));
    }
    return {
      status: execution.error ? 'failed' : 'complete',
      stdout: execution.logs.stdout.join('').slice(0, 32_000), stderr: execution.logs.stderr.join('').slice(0, 16_000),
      error: execution.error ? `${execution.error.name}: ${execution.error.value}`.slice(0, 4_000) : null,
      text: execution.results.map(result => result.text ?? '').join('\n').slice(0, 32_000), artifacts
    };
  } finally {
    signal.removeEventListener('abort', abort);
    // Provider TTL is the backstop if deletion fails or the client disconnects.
    await sandbox.kill({ requestTimeoutMs: 10_000 }).catch(() => {});
  }
}
