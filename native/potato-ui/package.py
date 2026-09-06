"""Package the locally built release binary; ad-hoc signs macOS bundles without installing them."""
import pathlib
import plistlib
import shutil
import sys
import subprocess

root = pathlib.Path(__file__).resolve().parent
if sys.platform == "darwin":
    bundle = root / "dist" / "Potato Native.app"
    executable = bundle / "Contents" / "MacOS" / "potato-ui"
    executable.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(root / "target" / "release" / "potato-ui", executable)
    with (bundle / "Contents" / "Info.plist").open("wb") as output:
        plistlib.dump({
            "CFBundleExecutable": "potato-ui",
            "CFBundleIdentifier": "dev.potato.native-preview",
            "CFBundleName": "Potato Native",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": "0.1.0",
            "CFBundleVersion": "1",
            "NSHighResolutionCapable": True,
            "NSMicrophoneUsageDescription": "将你的语音转换为聊天输入文字，仅在点击麦克风时录音。",
        }, output)
    # Re-sign the assembled local bundle: the linker signature belongs to a standalone binary.
    subprocess.run(["codesign", "--force", "--sign", "-", str(bundle)], check=True)
    print(bundle)
elif sys.platform == "win32":
    destination = root / "dist" / "potato-ui.exe"
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(root / "target" / "release" / "potato-ui.exe", destination)
    print(destination)
else:
    raise SystemExit("Packaging currently targets macOS and Windows.")
