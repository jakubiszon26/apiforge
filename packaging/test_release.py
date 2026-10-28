"""REL-01 (#115): the release workflow's checks, criteria 2, 3 and 5.

    python3 -I packaging/test_release.py

`release.py` is what `release.yml` runs to compare the tag with the workspace
version, to name the assets, to pack and check the archives and to write the
sums. These tests run it on small temporary trees; the release workflow runs
them before it builds anything.
"""

import hashlib
import importlib.util
import io
import os
import sys
import tarfile
import tempfile
import unittest
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
# Loading release.py by path would otherwise leave packaging/__pycache__.
sys.dont_write_bytecode = True
_spec = importlib.util.spec_from_file_location("release", os.path.join(HERE, "release.py"))
release = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(release)

CARGO = """[workspace]
members = ["crates/a"]

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
serde = { version = "=1.0.229" }
"""


class Version(unittest.TestCase):
    def test_the_workspace_version_comes_from_workspace_package(self):
        self.assertEqual(release.workspace_version(CARGO), "0.1.0")

    def test_the_repository_declares_a_workspace_version(self):
        with open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8") as file:
            self.assertRegex(release.workspace_version(file.read()), r"^\d+\.\d+\.\d+$")

    def test_a_tag_is_v_and_the_version_with_an_optional_rc_number(self):
        for tag in ["v0.1.0", "v0.1.0-rc.1", "v0.1.0-rc.12"]:
            release.check_tag(tag, "0.1.0")

    def test_any_other_tag_is_refused(self):
        for tag in [
            "v0.1.1",
            "0.1.0",
            "v0.1",
            "v0.1.0-rc",
            "v0.1.0-rc.",
            "v0.1.0-beta.1",
            "v0.1.0-rc.1x",
            "v0.1.0.1",
            "v0.10.0",
            "refs/tags/v0.1.0",
            "",
        ]:
            with self.assertRaises(ValueError, msg=tag):
                release.check_tag(tag, "0.1.0")


class Names(unittest.TestCase):
    def test_four_assets_carry_the_tag_and_the_target(self):
        self.assertEqual(
            release.asset_names("v0.1.0"),
            [
                "apiforge-desktop-v0.1.0-x86_64.AppImage",
                "apiforge-v0.1.0-x86_64-unknown-linux-gnu.tar.gz",
                "apiforge-v0.1.0-x86_64-pc-windows-msvc.zip",
                "SHA256SUMS.txt",
            ],
        )
        self.assertIn("apiforge-desktop-v0.1.0-rc.1-x86_64.AppImage", release.asset_names("v0.1.0-rc.1"))

    def test_the_asset_directory_must_hold_exactly_the_four(self):
        with tempfile.TemporaryDirectory() as directory:
            for name in release.asset_names("v0.1.0"):
                with open(os.path.join(directory, name), "wb") as file:
                    file.write(b"x")
            release.check_assets(directory, "v0.1.0")
            with self.assertRaises(ValueError):
                release.check_assets(directory, "v0.1.0-rc.1")
            with open(os.path.join(directory, "extra.txt"), "wb") as file:
                file.write(b"x")
            with self.assertRaises(ValueError):
                release.check_assets(directory, "v0.1.0")
            os.remove(os.path.join(directory, "extra.txt"))
            os.remove(os.path.join(directory, "SHA256SUMS.txt"))
            with self.assertRaises(ValueError):
                release.check_assets(directory, "v0.1.0")


class Sums(unittest.TestCase):
    def test_one_sha256sum_line_per_asset(self):
        with tempfile.TemporaryDirectory() as directory:
            contents = {}
            for index, name in enumerate(release.asset_names("v0.1.0")[:3]):
                contents[name] = bytes([index]) * (index + 1)
                with open(os.path.join(directory, name), "wb") as file:
                    file.write(contents[name])
            text = release.write_sums(directory, "v0.1.0")
            with open(os.path.join(directory, "SHA256SUMS.txt"), encoding="ascii") as file:
                self.assertEqual(file.read(), text)
            lines = text.splitlines()
            self.assertEqual(len(lines), 3)
            self.assertTrue(text.endswith("\n"))
            for line in lines:
                digest, name = line.split("  ", 1)
                self.assertEqual(digest, hashlib.sha256(contents[name]).hexdigest())


MANIFEST = """# a comment
apiforge
README.md

licenses/LICENSE.txt
"""


def stage(root, names):
    for name in names:
        path = os.path.join(root, *name.split("/"))
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as file:
            file.write(name.encode())


class Archives(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.manifest = os.path.join(self.temporary.name, "manifest.txt")
        with open(self.manifest, "w", encoding="utf-8") as file:
            file.write(MANIFEST)

    def packed(self, names, suffix):
        stem = "apiforge-v0.1.0-target"
        # A stage of its own per call, so no case inherits another's files.
        directory = os.path.join(tempfile.mkdtemp(dir=self.temporary.name), stem)
        stage(directory, names)
        archive = os.path.join(self.temporary.name, stem + suffix)
        release.pack(directory, archive)
        return archive

    def test_a_packed_archive_holds_its_directory_and_matches_the_manifest(self):
        for suffix in [".tar.gz", ".zip"]:
            archive = self.packed(["apiforge", "README.md", "licenses/LICENSE.txt"], suffix)
            release.check_archive(archive, self.manifest)
            self.assertIn("apiforge-v0.1.0-target/licenses/LICENSE.txt", release.entries(archive))

    def test_a_missing_or_an_extra_file_fails_the_check(self):
        for suffix in [".tar.gz", ".zip"]:
            for names in [
                ["apiforge", "README.md"],
                ["apiforge", "README.md", "licenses/LICENSE.txt", "licenses/OTHER.txt"],
                ["apiforge", "README.md", "licenses/LICENSE.txt", "target/debug.log"],
            ]:
                archive = self.packed(names, suffix)
                with self.assertRaises(ValueError, msg=(names, suffix)):
                    release.check_archive(archive, self.manifest)
                os.remove(archive)

    def test_an_entry_outside_the_top_level_directory_fails_the_check(self):
        archive = os.path.join(self.temporary.name, "apiforge-v0.1.0-target.zip")
        with zipfile.ZipFile(archive, "w") as file:
            for name in ["apiforge", "README.md", "licenses/LICENSE.txt"]:
                file.writestr("apiforge-v0.1.0-target/" + name, b"x")
            file.writestr("stray.txt", b"x")
        with self.assertRaises(ValueError):
            release.check_archive(archive, self.manifest)

        archive = os.path.join(self.temporary.name, "apiforge-v0.1.0-target.tar.gz")
        with tarfile.open(archive, "w:gz") as file:
            for name in ["apiforge", "README.md", "licenses/LICENSE.txt"]:
                info = tarfile.TarInfo("other-directory/" + name)
                info.size = 1
                file.addfile(info, io.BytesIO(b"x"))
        with self.assertRaises(ValueError):
            release.check_archive(archive, self.manifest)

    def test_the_tar_keeps_the_executable_bit(self):
        stem = "apiforge-v0.1.0-target"
        directory = os.path.join(self.temporary.name, "modes", stem)
        stage(directory, ["apiforge", "README.md", "licenses/LICENSE.txt"])
        os.chmod(os.path.join(directory, "apiforge"), 0o755)
        os.chmod(os.path.join(directory, "README.md"), 0o644)
        archive = os.path.join(self.temporary.name, stem + ".tar.gz")
        release.pack(directory, archive)
        with tarfile.open(archive) as file:
            modes = {member.name: member.mode for member in file.getmembers()}
        if os.name == "posix":
            self.assertEqual(modes[stem + "/apiforge"] & 0o777, 0o755)
        self.assertEqual(modes[stem + "/README.md"] & 0o111, 0)

    def test_the_committed_manifests_name_both_binaries(self):
        for manifest, suffix in [("manifest-linux.txt", ""), ("manifest-windows.txt", ".exe")]:
            names = release.manifest(os.path.join(HERE, manifest))
            self.assertIn("apiforge" + suffix, names)
            self.assertIn("apiforge-desktop" + suffix, names)
            self.assertIn("CLAUDE-CODE.md", names)
            self.assertIn("LICENSE", names)
            self.assertTrue(any(name.startswith("licenses/") for name in names))


if __name__ == "__main__":
    unittest.main()
