// Local interaction fixture. No model calls and no changes to user conversations.
import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import '../src/styles/global.css';
import { QuestionCard } from '../src/components/chat/QuestionCard';
import type { QuestionAnswer, UserQuestion } from '../src/lib/userQuestions';
const question: UserQuestion = {request_id:'qa-question', session_id:'qa', status:'pending', title:'你希望保留哪些功能？可以多选，也可以补充其他需求。', multiple:true, options:[{id:'voice',label:'语音输入'},{id:'image',label:'图片生成'},{id:'tasks',label:'定时任务'}]};
function Preview(){
 const [answer,setAnswer]=useState<QuestionAnswer|null>(null);
 const [fail,setFail]=useState(false);
 const [revision,setRevision]=useState(0);
 const [mode,setMode]=useState('multiple');
 const currentQuestion = {...question, title: mode==='single' ? '你希望优先迁移哪项功能？' : mode==='text' ? '语音输入和图片生成目前分别使用什么服务？' : question.title, request_id:`qa-${mode}-${revision}`, multiple:mode==='multiple', options:mode==='text'?[]:question.options};
 return <main className="mx-auto max-w-[48rem] p-6 text-ink">
  <div className="mb-8 flex items-center gap-4 text-[13px] text-ink-secondary"><span>交互验证 · 模拟问题</span><select aria-label="问题类型" value={mode} onChange={event=>{setMode(event.target.value);setAnswer(null)}}><option value="multiple">多选</option><option value="single">单选</option><option value="text">文字</option></select><label><input type="checkbox" checked={fail} onChange={event=>setFail(event.target.checked)}/> 模拟提交失败</label><button onClick={()=>{setAnswer(null);setRevision(x=>x+1)}}>重置</button></div>
  <p className="mb-8 text-[16px] leading-7">我已经检查现有配置。继续迁移前，需要确认你常用的功能。</p>
  {answer ? <div className="ml-auto max-w-[85%] rounded-2xl bg-bubble-user p-4"><p className="mb-2 text-[13px] text-ink-secondary">{currentQuestion.title}</p><p>{answer.skip?'已跳过':`${answer.selected.map(id=>question.options?.find(item=>item.id===id)?.label).join('、')} ${answer.text}`}</p></div> : <QuestionCard key={currentQuestion.request_id} question={currentQuestion} onAnswer={async value=>{if(fail)throw new Error('网络连接中断，请重试。');setAnswer(value)}}/>}
  <div className="mt-4 rounded-2xl border border-line p-4 text-ink-secondary">描述任务…</div>
 </main>
}
createRoot(document.getElementById('root')!).render(<Preview/>);
