// Credential-free process/database smoke check for the built native desktop.
// This does not replace platform UI, microphone or installer acceptance tests.
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, rm, stat } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
const root=fileURLToPath(new URL('../../',import.meta.url));
const args=process.argv.slice(2);
if(args.some(a=>!['--debug','--preview'].includes(a)))throw new Error('Supported options: --debug --preview');
const profile=args.includes('--debug')?'debug':'release';
const name=args.includes('--preview')?'Potato Rust Preview':'Potato';
const target=join(root,'console/src-tauri/target',profile);
const bundle=join(target,'bundle/macos',`${name}.app`);
const executable=process.platform==='darwin'?join(bundle,'Contents/MacOS/potato-desktop'):join(target,'potato-desktop.exe');
await stat(executable);
const data=await mkdtemp(join(tmpdir(),'potato-native-smoke-'));
const child=spawn(executable,[],{env:{...process.env,POTATO_NATIVE_DATA_DIR:data},stdio:'ignore'});
let failure;
child.on('error',error=>{failure=error;});
let exited=false;
child.on('exit',()=>{exited=true;});
try {
  let ready=false;
  let startup;
  for(let attempt=0;attempt<120;attempt++) {
    if(failure)throw failure;
    if(exited)throw new Error('Desktop exited before native readiness');
    if(existsSync(join(data,'potato.sqlite3'))) {
      let database;
      try {
        database=new DatabaseSync(join(data,'potato.sqlite3'),{readOnly:true});
        const row=database.prepare("SELECT value FROM settings WHERE key='last_startup_ms'").get();
        if(row){startup=JSON.parse(row.value);ready=true;break;}
      } catch {} finally {database?.close();}
    }
    await delay(250);
  }
  if(!ready)throw new Error('Native database never reached readiness');
  await delay(1500);
  if(exited)throw new Error('Desktop exited after native initialization');
  const processes=process.platform==='darwin'
    ? execFileSync('ps',['-axo','pid=,ppid=,comm='],{encoding:'utf8'}).trim().split('\n').map(line=>{const [,pid,ppid,name]=line.match(/^\s*(\d+)\s+(\d+)\s+(.+)$/);return {pid:Number(pid),ppid:Number(ppid),name};})
    : JSON.parse(execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-Command','Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name | ConvertTo-Json -Compress'],{encoding:'utf8'})).map(p=>({pid:p.ProcessId,ppid:p.ParentProcessId,name:p.Name}));
  const owned=new Set([child.pid]);
  for(let previous=-1;previous!==owned.size;) {previous=owned.size;for(const p of processes)if(owned.has(p.ppid))owned.add(p.pid);}
  const forbidden=processes.filter(p=>owned.has(p.pid)&&p.pid!==child.pid&&/(?:^|[\\/])(python(?:\d(?:\.\d+)?)?|node|potato-backend)(?:\.exe)?$/i.test(p.name));
  if(forbidden.length)throw new Error('Native app started a legacy runtime process');
  if(process.platform==='darwin')for(const legacy of ['python-runtime','node-runtime'])if(existsSync(join(bundle,'Contents/Resources/binaries',legacy)))throw new Error(`Unexpected bundled ${legacy}`);
  console.log(JSON.stringify({native_ready:true,core_startup_ms:startup,legacy_child_processes:0,ui_verified:false}));
} finally {
  if(!exited)child.kill('SIGTERM');
  for(let n=0;n<20&&!exited;n++)await delay(100);
  if(!exited){child.kill('SIGKILL');for(let n=0;n<20&&!exited;n++)await delay(100);}
  if(exited||failure)await rm(data,{recursive:true,force:true});
}
