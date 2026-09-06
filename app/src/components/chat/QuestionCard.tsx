import { useEffect, useId, useRef, useState } from 'react';
import { ChevronDown, CircleHelp, LoaderCircle } from 'lucide-react';
import { useTranslation } from '../../lib/i18n';
import { ApiError } from '../../lib/api';
import { prepareAnswer, userQuestionsApi, type QuestionAnswer, type UserQuestion } from '../../lib/userQuestions';
import { Button } from '../ui';

const drafts = new Map<string, { selected: string[]; text: string }>();

export function QuestionCard({ question, onAnswer }: { question: UserQuestion; onAnswer: (answer: QuestionAnswer) => Promise<void> }) {
  const { language } = useTranslation();
  const en = language === 'en';
  const id = useId();
  const draftKey = `${question.session_id}:${question.request_id}`;
  const [selected, setSelected] = useState<string[]>(() => drafts.get(draftKey)?.selected ?? []);
  const [text, setText] = useState(() => drafts.get(draftKey)?.text ?? '');
  useEffect(() => { drafts.set(draftKey, { selected, text }); }, [draftKey, selected, text]);
  const [busy, setBusy] = useState(false);
  const submitting = useRef(false);
  const [collapsed, setCollapsed] = useState(false);
  const [error, setError] = useState('');
  const submit = async (skip = false) => {
    if (submitting.current) return;
    submitting.current = true;
    setBusy(true); setError('');
    try { await onAnswer(prepareAnswer(question, selected, text, skip)); drafts.delete(draftKey); }
    catch (reason) { setError(reason instanceof Error ? reason.message : (en ? 'Could not send. Try again.' : '提交失败，请重试。')); }
    finally { submitting.current = false; setBusy(false); }
  };
  return <section aria-labelledby={id} className="overflow-hidden rounded-2xl border border-line-strong bg-surface shadow-sm">
    <button type="button" onClick={() => setCollapsed(value => !value)} aria-expanded={!collapsed} className="flex w-full items-center gap-2 px-4 py-3 text-left text-[13px] text-ink-secondary focus-visible:outline-2 focus-visible:outline-ink">
      <CircleHelp size={16} /><span>{en ? 'Question' : '需要你的回答'}</span><ChevronDown size={15} className={`ml-auto ${collapsed ? '-rotate-90' : ''}`} />
    </button>
    <div hidden={collapsed}>
      <form onSubmit={event => { event.preventDefault(); void submit(); }}>
        <div className="max-h-[38vh] space-y-3 overflow-y-auto overscroll-contain px-4 pb-4">
          <h3 id={id} className="whitespace-pre-wrap text-[15px] font-medium leading-6 text-ink">{question.title}</h3>
          {!!question.options?.length && <fieldset className="space-y-2"><legend className="sr-only">{question.title}</legend>{question.options.map(option => <label key={option.id} className={`flex cursor-pointer items-start gap-3 rounded-xl border px-3 py-2.5 text-[14px] leading-5 ${selected.includes(option.id) ? 'border-ink bg-fill-hover text-ink' : 'border-line text-ink-secondary hover:bg-fill-hover'}`}>
            <input type={question.multiple ? 'checkbox' : 'radio'} name={id} value={option.id} checked={selected.includes(option.id)} disabled={busy} className="mt-0.5 accent-[var(--ink)]" onChange={() => setSelected(previous => question.multiple ? previous.includes(option.id) ? previous.filter(value => value !== option.id) : [...previous, option.id] : [option.id])} />
            <span>{option.label}</span>
          </label>)}</fieldset>}
          <textarea aria-label={en ? 'Your answer or additional details' : '回答或补充说明'} placeholder={en ? (question.options?.length ? 'Add details or write your own answer…' : 'Your answer…') : (question.options?.length ? '补充说明，或直接填写自己的答案…' : '填写回答…')} value={text} onChange={event => setText(event.target.value)} disabled={busy} rows={2} className="block w-full resize-y rounded-xl border border-line-strong bg-surface px-3 py-2 text-[15px] leading-6 text-ink outline-none focus:border-ink" />
          {error && <p role="alert" className="text-[13px] text-danger">{error}</p>}
        </div>
        <div className="flex justify-end gap-2 border-t border-line px-4 py-3">
          <Button type="button" variant="ghost" shape="pill" disabled={busy} onClick={() => void submit(true)}>{en ? 'Skip' : '跳过'}</Button>
          <Button type="submit" variant="primary" shape="pill" disabled={busy || (!selected.length && !text.trim())}>{busy && <LoaderCircle size={14} className="animate-spin" />}{en ? 'Send' : '提交'}</Button>
        </div>
      </form>
    </div>
  </section>;
}

export function QuestionDock({ sessionId, active }: { sessionId: string; active: boolean }) {
  const [questions, setQuestions] = useState<UserQuestion[]>([]);
  const completed = useRef(new Set<string>());
  const [error, setError] = useState('');
  const { language } = useTranslation();
  useEffect(() => {
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const response = await userQuestionsApi.list(sessionId);
        if (disposed) return;
        for (const question of response.questions) {
          if (question.status !== 'pending') drafts.delete(`${question.session_id}:${question.request_id}`);
        }
        setQuestions(response.questions.filter(question => question.session_id === sessionId && question.status === 'pending' && !completed.current.has(question.request_id)));
        setError('');
      } catch (reason) {
        if (disposed) return;
        // Older backends have no question capability. Ordinary chat still works.
        if (reason instanceof ApiError && (reason.status === 404 || reason.status === 501)) { setQuestions([]); return; }
        setError(language === 'en' ? 'Could not refresh questions. Retrying…' : '问题刷新失败，正在重试…');
      }
      if (!disposed) timer = setTimeout(poll, active ? 1500 : 5000);
    };
    void poll();
    return () => { disposed = true; clearTimeout(timer); };
  }, [sessionId, active, language]);
  if (!questions.length) return null;
  return <div className="mx-auto w-full max-w-[48rem] space-y-2 px-6 pb-3 sm:px-8">
    {error && <p role="status" className="text-[13px] text-warn">{error}</p>}
    {questions.slice(0, 1).map(question => <QuestionCard key={question.request_id} question={question} onAnswer={async answer => {
      await userQuestionsApi.answer(question.request_id, answer);
      completed.current.add(question.request_id);
      setQuestions(previous => previous.filter(item => item.request_id !== question.request_id));
    }} />)}
    {questions.length > 1 && <p className="text-[12px] text-ink-secondary">{language === 'en' ? `${questions.length - 1} more questions` : `还有 ${questions.length - 1} 个问题`}</p>}
  </div>;
}
