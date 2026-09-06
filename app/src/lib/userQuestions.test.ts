import { describe, expect, it } from 'vitest';
import { prepareAnswer, type UserQuestion } from './userQuestions';
const question: UserQuestion = { request_id: 'q1', session_id: 's1', title: 'Choose', status: 'pending', options: [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }] };
describe('structured answers', () => {
  it('does not submit an untouched question', () => expect(() => prepareAnswer(question, [], '  ')).toThrow());
  it('accepts an option with additional context', () => expect(prepareAnswer(question, ['a'], ' detail ')).toEqual({ selected: ['a'], text: 'detail', skip: false }));
  it('supports free-form replies without choosing an option', () => expect(prepareAnswer(question, [], 'Another approach').selected).toEqual([]));
  it('rejects multiple selections for single-choice questions', () => expect(() => prepareAnswer(question, ['a', 'b'], '')).toThrow());
  it('deduplicates multi-choice selections', () => expect(prepareAnswer({ ...question, multiple: true }, ['a', 'a', 'b'], '').selected).toEqual(['a', 'b']));
  it('never submits invalid option IDs as an answer', () => expect(() => prepareAnswer(question, ['unknown'], '')).toThrow());
  it('skipping does not accidentally send draft selections or text', () => expect(prepareAnswer(question, ['a'], 'draft', true)).toEqual({ selected: [], text: '', skip: true }));
});
