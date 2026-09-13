import { executeSandbox, validateSandbox, type SandboxInput } from './sandbox.ts';

export const pythonTool = { type: 'function', function: {
  name: 'run_python',
  description: 'Execute Python in an isolated E2B sandbox for calculations, file analysis, charts and documents. Decide when execution helps. Each call rebuilds the sandbox, but files from earlier successful calls in this answer are restored as input files in /home/user/; see available_files in tool results. Send a complete script. No internet, credentials or direct history database access. Use history tools to retrieve permitted past conversations. Save output files in /home/user/output with simple ASCII names. Tool output includes stdout, errors and file names. On a Python error, correct the complete script and retry within the budget.',
  parameters: { type: 'object', properties: { code: { type: 'string', description: 'Complete Python script, up to 32000 characters.' } }, required: ['code'], additionalProperties: false }
} };

export type CodeTool = {
  files: SandboxInput['files'];
  readonly currentFiles: Readonly<SandboxInput['files']>;
  fileNotes?: string[];
  run: (code: string, signal: AbortSignal) => ReturnType<typeof executeSandbox>;
};
export function codeTool(input: unknown, env: Env, rateKey: string, execute = executeSandbox): CodeTool | undefined {
  if (input === undefined) return;
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw new Error('Invalid code execution request.');
  const request = input as Record<string, unknown>;
  if (request.enabled === false) return;
  if (request.enabled !== true) throw new Error('Invalid code execution request.');
  const { files } = validateSandbox({ code: 'pass', files: request.files ?? [] });
  const fileNotes = request.file_notes ?? [];
  if (!Array.isArray(fileNotes) || fileNotes.length > 8 || !fileNotes.every(n => typeof n === 'string' && n.length <= 500)) throw new Error('Invalid file notes.');
  if (!env.E2B_API_KEY) throw new Error('Code execution is not configured.');
  let current: SandboxInput['files'] = [...files];
  const originalNames = new Set(files.map(f => f.name));
  return { files, fileNotes, get currentFiles() { return current; }, async run(code, signal) {
    const validated = validateSandbox({ code, files: current });
    signal.throwIfAborted();
    if (!(await env.SANDBOX_RATE_LIMIT.limit({ key: rateKey })).success) throw new Error('Code execution rate limit.');
    const execution = await execute(validated, env.E2B_API_KEY!, signal, undefined, env.E2B_TEMPLATE);
    if (execution.status === 'complete') for (const artifact of execution.artifacts) {
      let file: SandboxInput['files'][number];
      try { [file] = validateSandbox({ code: 'pass', files: [{ name: artifact.name, base64: artifact.base64 }] }).files; }
      catch { continue; }
      // Replacements are the newest arrival. Original input names are never evicted.
      const candidate = [...current.filter(f => f.name !== file.name), file];
      const overBudget = () => candidate.length > 4 || candidate.reduce((sum, f) => sum + Buffer.from(f.base64, 'base64').byteLength, 0) > 2_000_000;
      while (overBudget()) {
        const oldest = candidate.findIndex(f => f.name !== file.name && !originalNames.has(f.name));
        if (oldest < 0) break;
        candidate.splice(oldest, 1);
      }
      if (!overBudget()) current = candidate;
    }
    return execution;
  } };
}
