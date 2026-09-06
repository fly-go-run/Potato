import { apiJson } from './api';

export interface UserQuestion {
  request_id: string;
  session_id: string;
  title: string;
  options?: { id: string; label: string }[];
  multiple?: boolean;
  status: 'pending' | 'answered' | 'skipped';
  answer?: { selected: string[]; text: string };
}
export interface QuestionAnswer { selected: string[]; text: string; skip: boolean }
export function prepareAnswer(question: UserQuestion, selected: string[], text: string, skip = false): QuestionAnswer {
  if (skip) return { selected: [], text: '', skip: true };
  const allowed = new Set(question.options?.map(option => option.id));
  const choices = [...new Set(selected)].filter(id => allowed.has(id));
  if (!question.multiple && choices.length > 1) throw new Error('Choose one option');
  if (!choices.length && !text.trim()) throw new Error('An answer is required');
  return { selected: choices, text: text.trim(), skip: false };
}
export const userQuestionsApi = {
  list: (sessionId: string) => apiJson<{ questions: UserQuestion[] }>(`/api/questions?session_id=${encodeURIComponent(sessionId)}`),
  answer: (id: string, answer: QuestionAnswer) => apiJson<unknown>(`/api/questions/${encodeURIComponent(id)}/answer`, { method: 'POST', body: JSON.stringify(answer) }),
};
