// Opt-in fixture server. Not part of npm test or the deployed Worker.
import { build } from 'esbuild';
import { Miniflare, convertV4MiniflareOptions } from 'miniflare';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
const output=process.argv[2];if(!output)throw new Error('Pass an output directory');
await mkdir(output,{recursive:true});
const bundle=await build({entryPoints:['src/entry.ts'],bundle:true,write:false,format:'esm',platform:'browser',external:['cloudflare:workers','node:*']});
const mf=new Miniflare(convertV4MiniflareOptions({host:'127.0.0.1',port:0,workers:[{name:'remote-e2e',modules:true,script:bundle.outputFiles[0].text,compatibilityDate:'2026-09-12',compatibilityFlags:['nodejs_compat'],bindings:{CLIENT_TOKEN:'local-fixture-only-'.repeat(4)},durableObjects:{REMOTE_DEVICES:{className:'RemoteDevice',useSQLite:true},REMOTE_ACCOUNTS:{className:'RemoteAccount',useSQLite:true},REMOTE_LOGINS:{className:'RemoteLogin',useSQLite:true}},ratelimits:{REMOTE_RATE_LIMIT:{namespace_id:'1005',simple:{limit:10000,period:60}}}}]}));
const relay=await mf.ready;
const root=await mkdtemp(join(tmpdir(),'potato-remote-e2e-'));
await writeFile(join(output,'fixture.json'),JSON.stringify({relay:relay.toString(),root,pid:process.pid}));
const host=spawn(resolve('../potato-core/target/debug/examples/remote_fixture'),[relay.toString(),join(output,'pairing.json'),join(root,'native')],{stdio:['ignore','inherit','inherit']});
let closing=false;async function close(){if(closing)return;closing=true;host.kill('SIGTERM');await mf.dispose();process.exit(0)}
process.on('SIGTERM',close);process.on('SIGINT',close);host.on('exit',close);setTimeout(close,900000);
