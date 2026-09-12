"""Validate native release assets; optionally upload and publish a verified prerelease.

Requires Python 3.11+. Publishing also requires authenticated GitHub CLI.
Without --publish this only checks local files and never contacts GitHub.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from package import version


def asset_names(release_version):
    prefix = f"Potato-GPUI-{release_version}"
    packages = [f"{prefix}-Windows-x86_64-{suffix}" for suffix in ("setup.exe", "portable.zip")]
    packages += [f"{prefix}-macOS-{arch}.{suffix}"
                 for arch in ("arm64", "x86_64") for suffix in ("dmg", "zip")]
    return sorted(packages + [name + ".sha256" for name in packages])


def verify_assets(directory, release_version):
    expected = asset_names(release_version)
    actual = sorted(path.name for path in directory.iterdir())
    if actual != expected:
        raise ValueError(f"Release assets differ: missing={sorted(set(expected) - set(actual))}, "
                         f"unexpected={sorted(set(actual) - set(expected))}")
    digests = {}
    for name in expected:
        path = directory / name
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"Not a nonempty regular release file: {name}")
        with path.open("rb") as stream:
            digests[name] = hashlib.file_digest(stream, "sha256").hexdigest()
        if not name.endswith(".sha256"):
            checksum = directory / (name + ".sha256")
            if checksum.read_text(encoding="utf-8") != f"{digests[name]}  {name}\n":
                raise ValueError(f"Checksum mismatch or incorrect checksum filename: {name}")
    return digests


NOTES = """Native Rust desktop preview: GPUI + in-process potato-core.

- Windows x86_64: run the setup EXE, or extract the portable ZIP.
- macOS 15+: choose arm64 (Apple Silicon) or x86_64 (Intel), open the DMG and drag Potato GPUI to Applications.
- Python and WebView are not required to run the application. The native computer driver is bundled.
- Packages are not developer-signed/notarized; OS trust prompts may appear.
- CI checks packaged core startup and Windows install, upgrade, in-use rejection and uninstall. GPU rendering, IME and microphone interaction still need real-device testing.
- User data is retained on uninstall. This preview does not use the Tauri updater feed.
"""


def gh(*args):
    result = subprocess.run(["gh", *map(str, args)], check=False, capture_output=True,
                            text=True, encoding="utf-8", timeout=300)
    if result.returncode:
        raise RuntimeError(f"GitHub CLI failed ({result.returncode}): {result.stderr.strip()}")
    return result.stdout


def publish(directory, tag, repository, release_version):
    local = verify_assets(directory, release_version)
    # Distinguish a missing release from authentication/network/server failure.
    # `gh release view` alone returns a generic nonzero exit status for all four.
    pages = json.loads(gh("api", "--paginate", "--slurp", f"repos/{repository}/releases?per_page=100"))
    existing = next((r for page in pages for r in page if r["tag_name"] == tag), None)
    if existing is not None and not existing["draft"]:
        raise ValueError("Refusing to replace an already published release")
    if existing is not None:
        unexpected = {a["name"] for a in existing["assets"]} - set(local)
        if unexpected:
            raise ValueError(f"Draft contains unexpected assets; review them before retrying: {sorted(unexpected)}")
    with tempfile.TemporaryDirectory(prefix="potato-release-") as temporary:
        work = Path(temporary)
        notes = work / "notes.md"
        notes.write_text(NOTES, encoding="utf-8")
        if existing is None:
            gh("release", "create", tag, "--repo", repository, "--verify-tag", "--draft", "--prerelease",
               "--title", f"Potato GPUI {tag}", "--notes-file", notes)
        gh("release", "upload", tag, "--repo", repository, "--clobber", *[directory / n for n in sorted(local)])
        remote = json.loads(gh("release", "view", tag, "--repo", repository, "--json", "isDraft,assets"))
        if not remote["isDraft"] or sorted(a["name"] for a in remote["assets"]) != sorted(local):
            raise ValueError("Remote release state or asset list changed; leaving it unpublished")
        downloaded = work / "downloaded"
        downloaded.mkdir()
        gh("release", "download", tag, "--repo", repository, "--dir", downloaded, "--pattern", "*")
        if verify_assets(downloaded, release_version) != local:
            raise ValueError("Uploaded assets differ from local verified packages; leaving the draft unpublished")
        gh("release", "edit", tag, "--repo", repository, "--draft=false", "--prerelease", "--latest=false",
           "--notes-file", notes)
        return gh("release", "view", tag, "--repo", repository, "--json", "url", "--jq", ".url").strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--packages", type=Path, required=True, help="Directory containing only the 12 release files")
    parser.add_argument("--tag", required=True, help="native-v<version>; must match Cargo.toml")
    parser.add_argument("--repo", help="GitHub owner/repository; required with --publish")
    parser.add_argument("--publish", action="store_true", help="Upload, read back and publish the GitHub prerelease")
    args = parser.parse_args()
    release_version = version()
    if args.tag != f"native-v{release_version}":
        parser.error(f"Tag must match Cargo.toml: native-v{release_version}")
    if args.publish and not args.repo:
        parser.error("--publish requires --repo")
    directory = args.packages.resolve()
    verify_assets(directory, release_version)
    if args.publish:
        print(publish(directory, args.tag, args.repo, release_version))
    else:
        print(f"Verified {len(asset_names(release_version))} release files for {args.tag}")


if __name__ == "__main__":
    main()
