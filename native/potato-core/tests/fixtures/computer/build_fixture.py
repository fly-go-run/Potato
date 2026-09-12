"""Build the isolated macOS computer-use fixture application."""
from pathlib import Path
import plistlib
import subprocess
import tempfile

root = Path(tempfile.mkdtemp(prefix="cua-native-fixture-"))
bundle = root / "Cua Native Fixture.app"
contents = bundle / "Contents"
(contents / "MacOS").mkdir(parents=True)
(contents / "Info.plist").write_bytes(
    plistlib.dumps(
        {
            "CFBundleExecutable": "fixture",
            "CFBundleIdentifier": "dev.cua.native-fixture",
            "CFBundleName": "Cua Native Fixture",
            "CFBundlePackageType": "APPL",
        },
    ),
)
subprocess.run(
    [
        "swiftc",
        str(Path(__file__).with_name("Fixture.swift")),
        "-o",
        str(contents / "MacOS/fixture"),
        "-module-cache-path",
        str(root / "module-cache"),
    ],
    check=True,
)
subprocess.run(["codesign", "--force", "--sign", "-", str(bundle)], check=True)
print(bundle)
