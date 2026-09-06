// Build-time only. Downloads a pinned official Rust binary, never Python/Node runtimes.
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, writeFile, readFile, chmod, rename, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const root = fileURLToPath(new URL('../../', import.meta.url));
const version = '0.20.0';
const platform = process.platform === 'darwin' ? 'darwin-universal.tar.gz'
  : process.platform === 'win32' ? `windows-${process.arch === 'arm64' ? 'arm64' : 'x86_64'}-binary.zip`
  : null;
if (!platform) throw new Error('Native desktop driver packaging supports macOS and Windows');
const archiveName = `cua-driver-rs-${version}-${platform}`;
const hashes = {
  'darwin-universal.tar.gz': 'd5e61fecebd9a620e50c2b8b608c8e7e8141f74c6faebc2ae9ef5d0d96cce7b8',
  'windows-x86_64-binary.zip': 'c020fefee01aacc174a27fea84a0cb77d47ef8290bfc772b3db7e3e06670d2b2',
  'windows-arm64-binary.zip': 'e8d47cb35c7a719f12c3012caa12f1166ecd1614548afe9b18a2c600116f8bde',
};
const destination = resolve(process.argv[2] ?? join(root, 'console/src-tauri/binaries/native-cua-driver'));
const temporary = await mkdtemp(join(tmpdir(), 'potato-native-driver-'));
try {
  const archive = join(temporary, archiveName);
  const url = `https://github.com/trycua/cua/releases/download/cua-driver-rs-v${version}/${archiveName}`;
  // curl is present on supported macOS/Windows build hosts. No shell interpolation.
  execFileSync(process.platform === 'win32' ? 'curl.exe' : 'curl', ['--fail', '--location', '--retry', '3', '--connect-timeout', '30', '--max-time', '300', '--output', archive, url], { stdio: 'inherit' });
  const digest = createHash('sha256').update(await readFile(archive)).digest('hex');
  if (digest !== hashes[platform]) throw new Error('Official driver archive checksum mismatch');
  const binaryName = process.platform === 'win32' ? 'cua-driver.exe' : 'cua-driver';
  const members = execFileSync('tar', ['-tf', archive], { encoding: 'utf8', maxBuffer: 2_000_000 }).split(/\r?\n/)
    .filter(name => name === binaryName || name.endsWith(`/${binaryName}`));
  // A binary extracted from an .app retains a signature bound to that bundle.
  // Select the standalone executable supplied in the same official archive.
  const standalone = members.filter(name => !name.includes('.app/Contents/MacOS/'));
  const member = standalone.length === 1 ? standalone[0] : null;
  if (!member || member.startsWith('-') || member.split('/').includes('..')) throw new Error('Ambiguous or invalid driver archive');
  const bytes = execFileSync('tar', ['-xOf', archive, member], { maxBuffer: 200_000_000 });
  const staged = join(temporary, binaryName);
  await writeFile(staged, bytes, { mode: 0o755 });
  if (process.platform === 'darwin') execFileSync('codesign', ['--verify', '--strict', staged], { stdio: 'inherit' });
  await mkdir(destination, { recursive: true });
  const pending = join(destination, `${binaryName}.pending`);
  await writeFile(pending, bytes, { mode: 0o755 });
  await chmod(pending, 0o755);
  await rename(pending, join(destination, binaryName));
  await writeFile(join(destination, 'VERSION'), `${version}\n`);
  console.log(`Verified native driver staged in ${destination}`);
} finally {
  await rm(temporary, { recursive: true, force: true });
}
