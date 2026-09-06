"""Package the locally built release binary; ad-hoc signs macOS bundles without installing them."""
import pathlib
import platform
import os
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
    resources = bundle / "Contents" / "Resources"
    resources.mkdir(parents=True, exist_ok=True)
    # Reuse the shipped application's artwork instead of a preview placeholder.
    shutil.copyfile(
        root.parents[1] / "console" / "src-tauri" / "icons" / "icon.icns",
        resources / "icon.icns",
    )
    with (bundle / "Contents" / "Info.plist").open("wb") as output:
        plistlib.dump({
            "CFBundleExecutable": "potato-ui",
            "CFBundleIdentifier": "dev.potato.native-preview",
            "CFBundleName": "Potato Native",
            "CFBundleDisplayName": "Potato Native",
            "CFBundleIconFile": "icon.icns",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": "0.1.0",
            "CFBundleVersion": "1",
            "NSHighResolutionCapable": True,
            "NSMicrophoneUsageDescription": "将你的语音转换为聊天输入文字，仅在点击麦克风时录音。",
        }, output)
    # Re-sign the assembled local bundle: the linker signature belongs to a standalone binary.
    subprocess.run(["codesign", "--force", "--sign", "-", str(bundle)], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(bundle)], check=True)
    # In-place rebuilds must invalidate Finder's cached bundle metadata/icon.
    os.utime(bundle, None)
    print(bundle)
    print(shutil.make_archive(
        str(root / "dist" / f"Potato-Native-macOS-{platform.machine()}"),
        "zip", bundle.parent, bundle.name,
    ))
elif sys.platform == "win32":
    destination = root / "dist" / "potato-ui.exe"
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(root / "target" / "release" / "potato-ui.exe", destination)
    print(destination)
else:
    raise SystemExit("Packaging currently targets macOS and Windows.")
