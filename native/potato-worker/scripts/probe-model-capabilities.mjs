import { desktopCloudConfig } from './cloud-config.mjs';
import { writeFileSync, mkdirSync } from 'node:fs';
// Makes real, potentially billable requests using the existing native providers.
// Reports omit credentials. Inspect per-request statuses; expected negative controls fail.
const args = process.argv.slice(2);
if (args.length !== 1 || !['--chat', '--images'].includes(args[0])) {
 throw new Error('Usage: node scripts/probe-model-capabilities.mjs --chat|--images');
}
const {config}=desktopCloudConfig();const records=[];const jobs=[];
const safe = s => config.providers.reduce((v,p)=>v.replaceAll(p.api_key,'[REDACTED]'),String(s)).slice(0,240);
async function read(r,max=2000000){const chunks=[];let size=0;for await(const c of r.body){size+=c.length;if(size>max)throw Error('output_limit');chunks.push(c);}return Buffer.concat(chunks);}
function add(provider,model,params,label){jobs.push({provider,model,params,label});}
if(process.argv.includes('--chat')){
 for(const model of ['deepseek-v4.1-flash','deepseek-flash']) add('deepseek',model,{thinking:{type:'disabled'}},'alias-disabled');
 for(const effort of ['low','high','max']) add('deepseek','deepseek-flash',{thinking:{type:'enabled'},reasoning_effort:effort},effort);
 for(const model of ['gpt-5.6','gpt-5.6-sol','gpt-5.6-terra','gpt-5.6-luna']) for(const effort of ['none','low','medium','high','xhigh','max']) add('sub2api',model,{reasoning_effort:effort},effort);
 for(const [p,m] of [['deepseek','deepseek-flash'],['sub2api','gpt-5.6']])add(p,m,{reasoning_effort:'potato_invalid'},'invalid-control');
}
if(process.argv.includes('--images'))for(const model of ['gpt-image-1','gpt-image-1.5','gpt-image-2','gpt-image-2.5-sunburst','gpt-image-2.5-flare'])add('sub2api',model,{quality:'low',size:'1024x1024',n:1},'image-low');
const mode=process.argv.includes('--images')?'images':'chat';
async function run(job){
 const p=config.providers.find(p=>p.id===job.provider);const url=new URL(p.endpoint);const started=Date.now();const image=job.label.startsWith('image');
 const result={provider:p.id,model:job.model,setting:job.label};
 try{
  if(image)url.pathname=url.pathname.replace(/\/chat\/completions$/,'/images/generations');
  const body=image?{model:job.model,prompt:'A small solid blue circle centered on a plain white background, no text.',...job.params}:{model:job.model,messages:[{role:'user',content:'Compute 17 multiplied by 19. Reply with only the integer.'}],max_tokens:1536,stream:false,...job.params};
  const r=await fetch(url,{method:'POST',headers:{Authorization:`Bearer ${p.api_key}`,'Content-Type':'application/json'},redirect:'error',body:JSON.stringify(body),signal:AbortSignal.timeout(image?120000:90000)});
  result.http=r.status;const raw=await read(r,image?18000000:2000000);let data;try{data=JSON.parse(raw);}catch{result.error='non_json';}
  if(data){
   result.returned_model=data.model;result.usage=data.usage;result.finish_reason=data.choices?.[0]?.finish_reason;
   if(data.error){result.error=safe(data.error.message||data.error);result.error_code=safe(data.error.code||'');}
   if(image){result.image_count=data.data?.length||0;result.image_bytes=data.data?.[0]?.b64_json?Buffer.from(data.data[0].b64_json,'base64').length:0;result.url_present=!!data.data?.[0]?.url;
    if(result.image_bytes){mkdirSync('/tmp/potato-model-probe-images',{recursive:true});writeFileSync(`/tmp/potato-model-probe-images/${job.model}.png`,Buffer.from(data.data[0].b64_json,'base64'));}
   }else{const msg=data.choices?.[0]?.message;result.answer=String(msg?.content||'').slice(0,80);result.correct=result.answer.trim()==='323';result.reasoning_chars=String(msg?.reasoning_content||'').length;}
  }
 }catch(e){result.error=safe(e.name+': '+e.message);}
 result.elapsed_ms=Date.now()-started;records.push(result);writeFileSync(`/tmp/potato-model-probe-${mode}.json`,JSON.stringify(records,null,2));console.log(JSON.stringify(result));
}
let index=0;await Promise.all([0,1].map(async()=>{while(index<jobs.length){const job=jobs[index++];await run(job);}}));
