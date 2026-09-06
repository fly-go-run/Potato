// Builds the existing Tauri/React desktop with the embedded Rust runtime.
// No Python interpreter or Node runtime is included in the application.
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';
import { existsSync, readFileSync, mkdirSync } from 'node:fs';
const root=fileURLToPath(new URL('../../',import.meta.url));
const args=process.argv.slice(2);
if(args.some(arg=>!['--debug','--preview','--no-bundle'].includes(arg)))throw new Error('Supported options: --debug --preview --no-bundle');
for(const directory of ['app','console']) {
  if(!existsSync(join(root,directory,'node_modules'))) {
    execFileSync(process.platform==='win32'?'cmd.exe':'npm',process.platform==='win32'?['/d','/s','/c','npm ci']:['ci'],{cwd:join(root,directory),stdio:'inherit'});
  }
}
execFileSync(process.execPath,[join(root,'scripts/native/stage-driver.mjs')],{cwd:root,stdio:'inherit'});
execFileSync(process.execPath,[join(root,'scripts/pack-tauri/sync_tauri_version.mjs')],{cwd:root,stdio:'inherit'});
const tauri=['build','--features','native-runtime','--config','src-tauri/tauri.version.conf.json'];
if(args.includes('--preview'))tauri.push('--config','src-tauri/tauri.native.conf.json');
if(args.includes('--debug'))tauri.push('--debug');
if(args.includes('--no-bundle'))tauri.push('--no-bundle');
else tauri.push('--bundles',process.platform==='win32'?'nsis':'app');
execFileSync(process.execPath,[join(root,'console/node_modules/@tauri-apps/cli/tauri.js'),...tauri],{cwd:join(root,'console'),stdio:'inherit'});
if(process.platform==='darwin'&&!args.includes('--no-bundle')&&!args.includes('--preview')) {
  const version=readFileSync(join(root,'src/potato/__version__.py'),'utf8').match(/__version__\s*=\s*"([^"]+)"/)?.[1];
  if(!version||!/^[\w.+-]+$/.test(version))throw new Error('Invalid desktop distribution version');
  const dist=process.env.DIST??join(root,'dist');mkdirSync(dist,{recursive:true});
  const bundle=join(root,'console/src-tauri/target',args.includes('--debug')?'debug':'release','bundle/macos/Potato.app');
  execFileSync('ditto',['-c','-k','--sequesterRsrc','--keepParent',bundle,join(dist,`Potato-Tauri-${version}-macOS.zip`)],{stdio:'inherit'});
}
