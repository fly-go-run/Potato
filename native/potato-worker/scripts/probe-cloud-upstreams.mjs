import { desktopCloudConfig } from './cloud-config.mjs';
const { config } = desktopCloudConfig();
for (const p of config.providers) {
  const model = p.id === 'deepseek' ? 'deepseek-v4-pro' : 'gpt-5.6';
  const body = { model, stream: true, messages: [{ role: 'user', content: 'Reply with exactly: OK' }], max_tokens: 512, ...(p.id === 'deepseek' ? { thinking: { type: 'disabled' } } : { reasoning_effort: 'low' }) };
  const response = await fetch(p.endpoint, { method: 'POST', redirect: 'manual', headers: { Authorization: `Bearer ${p.api_key}`, 'Content-Type': 'application/json' }, body: JSON.stringify(body), signal: AbortSignal.timeout(60000) });
  const result = { provider: p.id, model, status: response.status, sse: !!response.headers.get('content-type')?.includes('text/event-stream'), content: false, done: false };
  if (response.ok && result.sse) {
    let text = '', count = 0; const decoder = new TextDecoder();
    for await (const chunk of response.body) { count += chunk.length; if (count > 1000000) throw new Error('Probe exceeded size limit'); text += decoder.decode(chunk, { stream: true }); }
    for (const line of text.split('\n').filter(s => s.startsWith('data:'))) {
      const data = line.slice(5).trim(); if (data === '[DONE]') { result.done = true; continue; }
      try { if (JSON.parse(data).choices?.some(c => c.delta?.content)) result.content = true; } catch {}
    }
  } else await response.body?.cancel();
  console.log(JSON.stringify(result));
  if (!result.content || !result.done) process.exitCode = 1;
}
