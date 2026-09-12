"""Portable regression tests for Windows packaging and release guards."""

import contextlib
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release
from verify_package import INSTALL_KEY, UNINSTALL_KEY, assert_no_windows_installation


class Registry:
    HKEY_CURRENT_USER = 0
    KEY_READ = 1
    KEY_WOW64_64KEY = 256
    KEY_WOW64_32KEY = 512

    def __init__(self, present=()):
        self.present = present

    def OpenKey(self, hive, name, reserved, access):
        if (name, access & ~self.KEY_READ) in self.present:
            return contextlib.nullcontext()
        raise FileNotFoundError(name)


class PackagingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="potato packaging test ")
        self.addCleanup(self.temporary.cleanup)
        self.work = Path(self.temporary.name)
        self.packages = self.work / "packages"
        self.packages.mkdir()
        self.version = "1.2.3-rc.1"
        for name in release.asset_names(self.version):
            if not name.endswith(".sha256"):
                data = ("fixture " + name).encode()
                (self.packages / name).write_bytes(data)
                (self.packages / (name + ".sha256")).write_text(
                    f"{hashlib.sha256(data).hexdigest()}  {name}\n", encoding="utf-8", newline="\n")

    def test_complete_assets_and_crlf_checksums(self):
        self.assertEqual(len(release.verify_assets(self.packages, self.version)), 12)
        for path in self.packages.glob("*.sha256"):
            path.write_bytes(path.read_bytes().replace(b"\n", b"\r\n"))
        self.assertEqual(len(release.verify_assets(self.packages, self.version)), 12)

    def test_missing_extra_and_wrong_version_are_rejected(self):
        with self.assertRaises(ValueError):
            release.verify_assets(self.packages, "1.2.4")
        extra = self.packages / "stale.zip"
        extra.write_bytes(b"stale")
        with self.assertRaises(ValueError):
            release.verify_assets(self.packages, self.version)
        extra.unlink()
        next(self.packages.glob("*.dmg")).unlink()
        with self.assertRaises(ValueError):
            release.verify_assets(self.packages, self.version)

    def test_corruption_and_checksum_target_substitution_are_rejected(self):
        archive = next(self.packages.glob("*.zip"))
        checksum = archive.with_name(archive.name + ".sha256")
        checksum.write_text(checksum.read_text().replace(archive.name, "other.zip"))
        with self.assertRaises(ValueError):
            release.verify_assets(self.packages, self.version)
        checksum.write_text(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n")
        archive.write_bytes(b"corrupt")
        with self.assertRaises(ValueError):
            release.verify_assets(self.packages, self.version)

    def test_existing_custom_or_legacy_installation_is_never_touched(self):
        absent = self.work / "default installation"
        assert_no_windows_installation(Registry(), absent)
        for key in (INSTALL_KEY, UNINSTALL_KEY):
            for view in (Registry.KEY_WOW64_64KEY, Registry.KEY_WOW64_32KEY):
                with self.subTest(key=key, view=view), self.assertRaises(RuntimeError):
                    assert_no_windows_installation(Registry({(key, view)}), absent)
        absent.mkdir()
        with self.assertRaises(RuntimeError):
            assert_no_windows_installation(Registry(), absent)

    def test_public_release_or_api_failure_cannot_be_overwritten(self):
        existing = [[{"tag_name": "native-v1.2.3-rc.1", "draft": False, "assets": []}]]
        with patch.object(release, "gh", return_value=json.dumps(existing)) as gh:
            with self.assertRaises(ValueError):
                release.publish(self.packages, "native-v1.2.3-rc.1", "owner/repo", self.version)
            self.assertEqual(gh.call_count, 1)
        with patch.object(release, "gh", side_effect=subprocess.CalledProcessError(1, "gh")) as gh:
            with self.assertRaises(subprocess.CalledProcessError):
                release.publish(self.packages, "native-v1.2.3-rc.1", "owner/repo", self.version)
            self.assertEqual(gh.call_count, 1)

    def simulate_publish(self, *, corrupt=False, existing=False):
        calls = []

        def gh(*args):
            calls.append(args)
            if args[0] == "api":
                return json.dumps([[{"tag_name": "native-v1.2.3-rc.1", "draft": True, "assets": []}]] if existing else [[]])
            if args[:2] == ("release", "view"):
                return json.dumps({"isDraft": True, "assets": [{"name": n} for n in release.asset_names(self.version)]})
            if args[:2] == ("release", "download"):
                destination = args[args.index("--dir") + 1]
                shutil.copytree(self.packages, destination, dirs_exist_ok=True)
                if corrupt:
                    next(destination.glob("*.zip")).write_bytes(b"corrupt remote download")
            return ""

        with patch.object(release, "gh", side_effect=gh):
            if corrupt:
                with self.assertRaises(ValueError):
                    release.publish(self.packages, "native-v1.2.3-rc.1", "owner/repo", self.version)
            else:
                release.publish(self.packages, "native-v1.2.3-rc.1", "owner/repo", self.version)
        return [args[:2] for args in calls]

    def test_publish_happens_only_after_readback(self):
        calls = self.simulate_publish()
        self.assertLess(calls.index(("release", "download")), calls.index(("release", "edit")))

    def test_corrupt_upload_leaves_unpublished_draft(self):
        self.assertNotIn(("release", "edit"), self.simulate_publish(corrupt=True))

    def test_rerun_resumes_draft_without_creating_another_release(self):
        calls = self.simulate_publish(existing=True)
        self.assertNotIn(("release", "create"), calls)
        self.assertIn(("release", "edit"), calls)


if __name__ == "__main__":
    unittest.main()
