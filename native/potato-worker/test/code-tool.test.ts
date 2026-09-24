import test from 'node:test';
import assert from 'node:assert/strict';
import { codeTool } from '../src/code-tool.ts';
import { chatWithSearch } from '../src/search-chat.ts';
import { handle } from '../src/index.ts';
const sse = (...events: unknown[]) => new Response(events.map(e => `data: ${typeof e === 'string' ? e : JSON.stringify(e)}\n\n`).join(''), { headers: { 'content-type': 'text/event-stream' } });
const call = (name='run_python', code='print(17*19)', id='python-1') => sse({ choices: [{ delta: { tool_calls: [{ index: 0, id, function: { name, arguments: JSON.stringify({ code }) } }] }, finish_reason: 'tool_calls' }] }, '[DONE]');
const result = { status: 'complete', stdout: '323', stderr: '', error: null, text: '', artifacts: [{ name: 'report.md', mime: 'text/markdown', base64: Buffer.from('# 323').toString('base64') }] };
const env = { UPSTREAM_API_KEY: 'private-key', E2B_API_KEY: 'e2b-private', SANDBOX_RATE_LIMIT: { limit: async () => ({ success: true }) } };
const body = { model: 'test', messages: [{ role: 'user', content: 'compute and create a report' }], stream: true, max_tokens: 512 };

test('model selects Python as a peer tool, receives real results, and files go only to the client', async () => {
 let runs = 0, rounds = 0;
 const code = { files: [], currentFiles: [{ name: 'report.md', base64: '' }], run: async (source: string) => { runs++; assert.equal(source, 'print(17*19)'); return result; } };
 const response = await chatWithSearch(body, new URL('https://model.example'), {...env, EXA_API_KEY:'exa'}, new AbortController().signal, async (_, init) => {
  const request = JSON.parse(init.body); assert.deepEqual(request.tools.map(t=>t.function.name), ['web_search','run_python']);
  if(rounds++ === 0) { assert.equal(request.tool_choice, 'auto'); return call(); }
  const output = request.messages.at(-1); assert.equal(output.role, 'tool'); assert.equal(output.tool_call_id, 'python-1');
  assert.match(output.content, /available_files/); assert.match(output.content, /\/home\/user\/report.md/); assert.match(output.content, /323/); assert.match(output.content, /report.md/); assert.doesNotMatch(output.content, /base64|private-key|e2b-private/);
  return sse({choices:[{delta:{content:'323, report.md 已生成'},finish_reason:'stop'}]}, '[DONE]');
 }, env.UPSTREAM_API_KEY,'owner',undefined,code);
 const output = await response.text(); assert.equal(runs,1); assert.equal(rounds,2); assert.match(output, /"state":"running"/); assert.match(output, /"potato_execution"/); assert.match(output, new RegExp(result.artifacts[0].base64)); assert.equal(output.match(/\[DONE\]/g)?.length,1);
});

test('greeting does not create a sandbox, and disabled execution never advertises Python', async () => {
 for(const enabled of [true,false]) {
  let ran=false;
  const response=await chatWithSearch(body,new URL('https://model.example'),env,new AbortController().signal,async(_,init)=>{
   const request=JSON.parse(init.body);assert.equal(request.tools.some(t=>t.function.name==='run_python'),enabled);
   assert.equal(request.tools.some(t=>t.function.name==='web_search'),false);
   return sse({choices:[{delta:{content:'你好'}}]},'[DONE]');
  },env.UPSTREAM_API_KEY,'owner',undefined,enabled?{files:[],currentFiles:[],run:async()=>{ran=true;return result;}}:undefined);
  assert.match(await response.text(),/你好/);assert.equal(ran,false);
 }
});

test('Python error is returned to the model so it can correct and retry',async()=>{
 let runs=0,rounds=0;
 const response=await chatWithSearch(body,new URL('https://model.example'),env,new AbortController().signal,async(_,init)=>{
  const request=JSON.parse(init.body);
  if(rounds++===0)return call('run_python','print(1/0)','bad');
  if(rounds===2){assert.match(request.messages.at(-1).content,/ZeroDivisionError/);return call('run_python','print(17*19)','fixed');}
  assert.match(request.messages.at(-1).content,/323/);return sse({choices:[{delta:{content:'323'}}]},'[DONE]');
 },env.UPSTREAM_API_KEY,'owner',undefined,{files:[],currentFiles:[],run:async()=>++runs===1?{...result,status:'failed',stdout:'',error:'ZeroDivisionError: division by zero',artifacts:[]}:result});
 const output=await response.text();assert.equal(runs,2);assert.match(output,/"state":"failed"/);assert.match(output,/"state":"complete"/);assert.match(output,/\[DONE\]/);
});

test('tool call budget bounds repeated Python requests and handles cancellation',async()=>{
 let runs=0,rounds=0;
 const response=await chatWithSearch(body,new URL('https://model.example'),env,new AbortController().signal,async(_,init)=>{
  const request=JSON.parse(init.body);if(request.tool_choice==='none')return sse({choices:[{delta:{content:'执行预算已用尽。'}}]},'[DONE]');
  return call('run_python','print(1)',`call-${rounds++}`);
 },env.UPSTREAM_API_KEY,'owner',undefined,{files:[],currentFiles:[],run:async()=>{runs++;return {...result,artifacts:[]};}});
 assert.match(await response.text(),/执行预算/);assert.equal(runs,3);
 let cancelled=false;
 const response2=await chatWithSearch(body,new URL('https://model.example'),env,new AbortController().signal,async()=>call(),env.UPSTREAM_API_KEY,'owner',undefined,{files:[],currentFiles:[],run:async(_,signal)=>new Promise((resolve,reject)=>{
  if(signal.aborted){cancelled=true;reject(signal.reason);return;}
  signal.addEventListener('abort',()=>{cancelled=true;reject(signal.reason);},{once:true});
 })});
 const reader=response2.body.getReader();assert.match(new TextDecoder().decode((await reader.read()).value),/running/);await reader.cancel();assert.equal(cancelled,true);
});

test('code execution validates uploaded data, uses account rate limit and hides credentials',async()=>{
 assert.equal(codeTool(undefined,env,'owner'),undefined);assert.equal(codeTool({enabled:false},env,'owner'),undefined);
 assert.throws(()=>codeTool({enabled:true,files:[{name:'../private',base64:'YQ=='}]},env,'owner'));
 assert.throws(()=>codeTool({enabled:true},{...env,E2B_API_KEY:undefined},'owner'));
 let key='';const runner=codeTool({enabled:true,files:[{name:'input.csv',base64:'YQ=='}]}, {...env,SANDBOX_RATE_LIMIT:{limit:async options=>{key=options.key;return{success:true}}}},'account-123', async (input,apiKey,signal)=>{
  assert.equal(apiKey,'e2b-private');assert.deepEqual(input.files,[{name:'input.csv',base64:'YQ=='}]);assert.equal('envs' in input,false);return result;
 });
 await runner.run('print(1)',new AbortController().signal);assert.equal(key,'account-123');
});

test('mobile request explicitly enables execution; old clients and desktop do not gain this tool',async()=>{
 const configured={...env,CLIENT_TOKEN:'a'.repeat(32),ALLOWED_MODELS:'test',MAX_OUTPUT_TOKENS:'4096',UPSTREAM_URL:'https://model.example/chat/completions',CHAT_RATE_LIMIT:{limit:async()=>({success:true})}};
 for(const enabled of [true,false]){
  let seen=false;
  const response=await handle(new Request('https://potato.example/v1/chat/completions',{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer '+configured.CLIENT_TOKEN},body:JSON.stringify({...body,...(enabled?{sandbox:{enabled:true,files:[]}}:{})})}),configured,async(_,init)=>{
   const forwarded=JSON.parse(init.body);seen=!!forwarded.tools?.some(t=>t.function.name==='run_python');assert.equal(forwarded.sandbox,undefined);return sse({choices:[{delta:{content:'ok'}}]},'[DONE]');
  });
  assert.equal(response.status,200);await response.text();assert.equal(seen,enabled);
 }
});

test('history and Python are peers and a history result can feed the next Python call', async () => {
 let rounds=0,historyCalls=0,pythonCalls=0;
 const recall={execute:{close:async()=>{}},timezone:'Asia/Shanghai',autoMemory:false,sources:[],store:{verify:async s=>s},commit:async()=>{},call:async(name,args)=>{historyCalls++;assert.equal(name,'search_memory');return {memories:[{text:'合成记录：数量 17，单价 19。'}]};}};
 const response=await chatWithSearch(body,new URL('https://model.example'),{...env,EXA_API_KEY:'exa'},new AbortController().signal,async(_,init)=>{
  const request=JSON.parse(init.body), names=request.tools.map(t=>t.function.name);
  assert.ok(names.includes('web_search')&&names.includes('run_python')&&names.includes('search_conversations'));
  if(rounds++===0)return sse({choices:[{delta:{tool_calls:[{index:0,id:'history',function:{name:'search_memory',arguments:'{"query":"合成记录"}'}}]},finish_reason:'tool_calls'}]},'[DONE]');
  if(rounds===2){assert.match(request.messages.at(-1).content,/单价 19/);return call();}
  return sse({choices:[{delta:{content:'总价 323'}}]},'[DONE]');
 },env.UPSTREAM_API_KEY,'owner',recall,{files:[],currentFiles:[],run:async()=>{pythonCalls++;return {...result,artifacts:[]};}});
 assert.match(await response.text(),/总价 323/);assert.equal(historyCalls,1);assert.equal(pythonCalls,1);
});


test('successful artifacts become input files for the next Python call', async () => {
 let runs = 0;
 const runner = codeTool({ enabled: true, files: [] }, env, 'owner', async input => {
  if (runs++ === 0) assert.deepEqual(input.files, []);
  else assert.deepEqual(input.files, [{ name: 'a.png', base64: 'YQ==' }]);
  return { ...result, artifacts: [{ name: 'a.png', mime: 'image/png', base64: 'YQ==' }] };
 });
 await runner.run('print(1)', new AbortController().signal);
 assert.deepEqual(runner.currentFiles.map(f => f.name), ['a.png']);
 await runner.run('print(2)', new AbortController().signal);
 assert.equal(runs, 2);
});

test('file budget evicts the oldest artifacts and preserves original user files', async () => {
 let runs = 0;
 const original = { name: 'input.csv', base64: 'YQ==' };
 const runner = codeTool({ enabled: true, files: [original] }, env, 'owner', async input => {
  if (runs++ > 0) assert.deepEqual(input.files, [original, ...['b.png', 'c.png', 'd.png'].map(name => ({ name, base64: 'YQ==' }))]);
  return { ...result, artifacts: runs === 1 ? ['a.png', 'b.png', 'c.png', 'd.png'].map(name => ({ name, mime: 'image/png', base64: 'YQ==' })) : [] };
 });
 await runner.run('print(1)', new AbortController().signal);
 assert.deepEqual(runner.currentFiles.map(f => f.name), ['input.csv', 'b.png', 'c.png', 'd.png']);
 await runner.run('print(2)', new AbortController().signal);
});

test('action titles accompany both live and completed execution without becoming code', async () => {
 let rounds = 0, runs = 0;
 const response = await chatWithSearch(body, new URL('https://model.example'), env, new AbortController().signal, async () => {
  if (rounds++ === 0) return sse({ choices: [{ delta: { tool_calls: [{ index: 0, id: 'deck', function: { name: 'run_python', arguments: JSON.stringify({ code: 'print(1)', description: '生成演示文稿' }) } }] }, finish_reason: 'tool_calls' }] }, '[DONE]');
  return sse({ choices: [{ delta: { content: 'done' } }] }, '[DONE]');
 }, env.UPSTREAM_API_KEY, 'owner', undefined, { files: [], currentFiles: [], run: async source => { runs++; assert.equal(source, 'print(1)'); return result; } });
 const text = await response.text(); assert.equal(runs, 1); assert.equal(text.match(/"title":"生成演示文稿"/g)?.length, 2);
});
