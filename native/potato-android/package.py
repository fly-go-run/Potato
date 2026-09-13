#!/usr/bin/env python3
"""Build a verifiable, signed, directly installable APK. Never print signing secrets."""
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent

def run(argv, env):
    subprocess.run([str(v) for v in argv], cwd=ROOT, env=env, check=True)

def main():
    env = os.environ.copy()
    preferred_java = env.get('POTATO_ANDROID_JAVA_HOME')
    bundled_java = Path('/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home')
    if preferred_java:
        env['JAVA_HOME'] = preferred_java
    elif bundled_java.exists():
        env['JAVA_HOME'] = str(bundled_java)
    elif not env.get('JAVA_HOME'):
        raise SystemExit('Set JAVA_HOME or POTATO_ANDROID_JAVA_HOME to JDK 17.')
    env.setdefault('ANDROID_HOME', str(Path.home() / 'Library/Android/sdk'))
    java = Path(env['JAVA_HOME']) / 'bin'
    sdk = Path(env['ANDROID_HOME'])
    signing = ROOT / '.signing'
    signing.mkdir(mode=0o700, exist_ok=True)
    os.chmod(signing, 0o700)
    config_path = signing / 'credentials.json'
    keystore = signing / 'potato-release.jks'
    if not config_path.exists():
        if keystore.exists():
            raise SystemExit('Signing credentials missing; refusing to replace the existing update key.')
        with config_path.open('x') as out:
            os.chmod(config_path, 0o600)
            json.dump({'password': secrets.token_urlsafe(36)}, out)
    config = json.loads(config_path.read_text())
    env.update(POTATO_ANDROID_KEYSTORE=str(keystore), POTATO_ANDROID_STORE_PASSWORD=config['password'], POTATO_ANDROID_KEY_PASSWORD=config['password'])
    if not keystore.exists():
        run([java / 'keytool', '-genkeypair', '-keystore', keystore, '-storetype', 'PKCS12', '-alias', 'potato', '-keyalg', 'RSA', '-keysize', '3072', '-validity', '10000', '-dname', 'CN=Potato Android, OU=Personal Distribution, O=Potato', '-storepass:env', 'POTATO_ANDROID_STORE_PASSWORD', '-keypass:env', 'POTATO_ANDROID_KEY_PASSWORD'], env)
        os.chmod(keystore, 0o600)
    run([ROOT / 'gradlew', ':app:testDebugUnitTest', ':app:lintRelease', ':app:assembleRelease'], env)
    output = ROOT / 'dist'
    output.mkdir(exist_ok=True)
    import re
    version = re.search(r"versionName '([^']+)'", (ROOT / 'app/build.gradle').read_text()).group(1)
    apk = output / f'Potato-Android-{version}.apk'
    shutil.copy2(ROOT / 'app/build/outputs/apk/release/app-release.apk', apk)
    build_tools = sdk / 'build-tools/35.0.0'
    run([build_tools / 'apksigner', 'verify', '--verbose', '--print-certs', apk], env)
    run([build_tools / 'zipalign', '-c', '-P', '16', '4', apk], env)
    import zipfile
    with zipfile.ZipFile(apk) as archive:
        if any(n.endswith('.so') for n in archive.namelist()):
            raise SystemExit('Unexpected native dependency: inspect ABI and 16 KB page alignment before distributing.')
        if any(n.endswith(('.jks', '.keystore', 'credentials.json')) for n in archive.namelist()):
            raise SystemExit('Signing material must never be packaged.')
    digest = hashlib.sha256(apk.read_bytes()).hexdigest()
    apk.with_suffix('.apk.sha256').write_text(f'{digest}  {apk.name}\n')
    print(f'Installable APK: {apk}\nSHA-256: {digest}')

if __name__ == '__main__':
    main()
