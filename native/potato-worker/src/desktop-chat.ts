import { CloudError } from './cloud.ts';

const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
const text = (v: unknown, max: number): v is string => typeof v === 'string' && v.length > 0 && v.length <= max;
const fail = (message: string): never => { throw new CloudError(400, message); };
const name = (v: unknown) => text(v, 128) && /^[A-Za-z0-9_.:-]+$/.test(v);

/** Desktop owns the tool loop. Never run mobile search/recall tools for this route. */
export function validateDesktop(input: unknown, model: string, maxOutput: number): Record<string, unknown> {
  if (!object(input) || input.model !== model || input.stream !== true) fail('Expected an enabled model and streaming request.');
  const body = input as Record<string, unknown>;
  if (!Array.isArray(body.messages) || !body.messages.length || body.messages.length > 200) fail('Expected 1–200 messages.');
  const pending = new Set<string>();
  const seen = new Set<string>();
  const messages = (body.messages as unknown[]).map(raw => {
    if (!object(raw)) return fail('Invalid message.');
    const role = raw.role;
    if (!['system', 'user', 'assistant', 'tool'].includes(String(role))) return fail('Invalid message role.');
    const result: Record<string, unknown> = { role };
    if (role === 'tool') {
      if (!text(raw.tool_call_id, 256) || !pending.delete(raw.tool_call_id) || typeof raw.content !== 'string') return fail('Tool result must match a preceding tool call.');
      return { role, tool_call_id: raw.tool_call_id, content: raw.content };
    }
    if (pending.size) return fail('Missing tool results.');
    if (typeof raw.content === 'string') result.content = raw.content;
    else if (role === 'assistant' && raw.content == null && Array.isArray(raw.tool_calls) && raw.tool_calls.length) result.content = null;
    else if (role === 'user' && Array.isArray(raw.content) && raw.content.length > 0 && raw.content.length <= 32) {
      result.content = raw.content.map(part => {
        if (!object(part)) return fail('Invalid content part.');
        if (part.type === 'text' && typeof part.text === 'string') return { type: 'text', text: part.text };
        if (part.type === 'image_url' && object(part.image_url) && typeof part.image_url.url === 'string' && /^data:image\/(png|jpeg|webp|gif);base64,[A-Za-z0-9+/]+=*$/.test(part.image_url.url)) return { type: 'image_url', image_url: { url: part.image_url.url } };
        return fail('Images must be inline PNG, JPEG, WebP or GIF.');
      });
    } else return fail('Invalid message content.');
    if (role === 'assistant') {
      if (typeof raw.reasoning_content === 'string') result.reasoning_content = raw.reasoning_content;
      if (raw.tool_calls !== undefined) {
        if (!Array.isArray(raw.tool_calls) || !raw.tool_calls.length || raw.tool_calls.length > 128) return fail('Invalid tool calls.');
        result.tool_calls = raw.tool_calls.map(call => {
          if (!object(call) || !text(call.id, 256) || seen.has(call.id) || call.type !== 'function' || !object(call.function) || !name(call.function.name) || typeof call.function.arguments !== 'string') return fail('Invalid function call.');
          pending.add(call.id); seen.add(call.id);
          return { id: call.id, type: 'function', function: { name: call.function.name, arguments: call.function.arguments } };
        });
      }
    }
    return result;
  });
  if (pending.size) fail('Missing tool results.');
  const result: Record<string, unknown> = { model, messages, stream: true, stream_options: { include_usage: true }, max_tokens: maxOutput };
  if (body.max_tokens !== undefined) {
    if (typeof body.max_tokens !== 'number' || !Number.isSafeInteger(body.max_tokens) || body.max_tokens < 1) fail('max_tokens must be a positive integer.');
    result.max_tokens = Math.min(body.max_tokens as number, maxOutput);
  }
  if (body.tools !== undefined) {
    if (!Array.isArray(body.tools) || body.tools.length > 128) fail('Invalid tool definitions.');
    const names = new Set<string>();
    result.tools = (body.tools as unknown[]).map(tool => {
      if (!object(tool) || tool.type !== 'function' || !object(tool.function)) return fail('Invalid function definition.');
      const f = tool.function;
      if (!name(f.name) || names.has(String(f.name)) || !object(f.parameters) || f.parameters.type !== 'object' || (f.description !== undefined && typeof f.description !== 'string')) return fail('Invalid function schema.');
      names.add(String(f.name));
      return { type: 'function', function: { name: f.name, parameters: f.parameters, ...(typeof f.description === 'string' ? { description: f.description } : {}) } };
    });
  }
  if (body.thinking !== undefined) {
    if (!object(body.thinking) || !['enabled', 'disabled'].includes(String(body.thinking.type))) fail('Invalid thinking mode.');
    result.thinking = { type: (body.thinking as Record<string, unknown>).type };
  }
  if (body.reasoning_effort !== undefined) {
    if (typeof body.reasoning_effort !== 'string' || !/^[a-z0-9_-]{1,32}$/.test(body.reasoning_effort) || (object(body.thinking) && body.thinking.type === 'disabled')) fail('Invalid reasoning effort.');
    result.reasoning_effort = body.reasoning_effort;
  }
  return result;
}
