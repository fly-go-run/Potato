// Official docs and authenticated upstream probes checked 2026-09-13.
// V4.1 Flash's public API ID is deepseek-flash, not deepseek-v4.1-flash.
// gpt-5.6 is the Sol alias; expose one entry for the requested flagship.
export function curatedChatModels(provider) {
  if (provider === 'deepseek') return [{ id: 'deepseek-flash', name: 'DeepSeek V4.1 Flash', thinking_modes: ['enabled', 'disabled'], reasoning_effort_options: ['low', 'high', 'max'] }];
  if (provider === 'sub2api') return [{ id: 'gpt-5.6', name: 'GPT-5.6', thinking_modes: [], reasoning_effort_options: ['none', 'low', 'medium', 'high', 'xhigh', 'max'] }];
  return [];
}
