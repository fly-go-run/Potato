// Official docs and authenticated upstream probes checked 2026-09-13.
// V4.1 Flash's public API ID is deepseek-flash, not deepseek-v4.1-flash.
// Use the explicitly requested Sol ID rather than the unsuffixed alias.
export function curatedChatModels(provider) {
  if (provider === 'deepseek') return [{ id: 'deepseek-flash', name: 'DeepSeek V4.1 Flash', thinking_modes: ['enabled', 'disabled'], reasoning_effort_options: ['low', 'high', 'max'] }];
  if (provider === 'sub2api') return [
    { id: 'gpt-5.6-sol', name: 'GPT-5.6 Sol', thinking_modes: [], reasoning_effort_options: ['none', 'low', 'medium', 'high', 'xhigh', 'max'] },
    // Default and low were verified against the configured upstream on 2026-09-20.
    { id: 'gpt-6', name: 'GPT-6', thinking_modes: [], reasoning_effort_options: ['low'] },
  ];
  return [];
}
