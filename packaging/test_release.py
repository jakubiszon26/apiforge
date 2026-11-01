"""REL-01 (#115): the release workflow's checks, criteria 2, 3 and 5.
REL-03 (#120): the third-party notices, criteria 1 to 6.

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
import re
import subprocess
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


# REL-03 (#120). The order of criterion 1: a crate offering a choice is listed
# under the first of these it offers.
ACCEPTED = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "Unlicense",
    "Unicode-3.0",
    "BSL-1.0",
    "MPL-2.0",
    "CDLA-Permissive-2.0",
    "OFL-1.1",
    "Ubuntu-font-1.0",
]
TRIPLES = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"]
NOTICES = "licenses/THIRD-PARTY-NOTICES.md"
LIBRARIES = "packaging/appimage-libraries.txt"
FONT_FILES = [
    "fonts/Hack-Regular.txt",
    "fonts/UFL.txt",
    "fonts/OFL.txt",
    "fonts/emoji-icon-font-mit-license.txt",
]


def read(relative):
    with open(os.path.join(ROOT, *relative.split("/")), encoding="utf-8") as file:
        return file.read()


def step(workflow, name):
    """The text of the workflow step called NAME, up to the next step or job."""
    found = re.search(
        r"^(\s*)- name: " + re.escape(name) + r"\s*$(.*?)(?=^\1- name: |^  [A-Za-z0-9_-]+:\s*$|\Z)",
        workflow,
        re.M | re.S,
    )
    if not found:
        raise AssertionError(f"no step named {name!r}")
    return found.group(2)


def job(workflow, name):
    """The text of the job NAME of a workflow."""
    found = re.search(r"^  " + re.escape(name) + r":\s*$(.*?)(?=^  [A-Za-z0-9_-]+:\s*$|\Z)", workflow, re.M | re.S)
    if not found:
        raise AssertionError(f"no job named {name!r}")
    return found.group(1)


# Shaped like the generated file: one line per crate, `- <name> <version>`.
NOTICE_TEXT = """# Third-party notices

## Fonts embedded by egui

- Hack (`Hack-Regular.ttf`)
- Ubuntu-Light (`Ubuntu-Light.ttf`)
- NotoEmoji (`NotoEmoji-Regular.ttf`)
- emoji-icon-font (`emoji-icon-font.ttf`)

## MIT License (`MIT`)

Used by:

- indexmap 2.14.2
- ash 0.38.0+1.3.281
- epaint_default_fonts 0.36.2

```text
MIT License

Copyright (c) 2014 John Slegers
```

```text
BITSTREAM VERA LICENSE
```

## SIL Open Font License 1.1 (`OFL-1.1`)

```text
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
```

## Ubuntu Font Licence v1.0 (`Ubuntu-font-1.0`)

```text
UBUNTU FONT LICENCE Version 1.0
```
"""

# `cargo tree --prefix none --format "{p}"` for two triples, one after the
# other: workspace crates carry their path, repeats `(*)`.
TREE = """apiforge-cli v0.1.0 (/home/runner/work/apiforge/crates/apiforge-cli)
apiforge-core v0.1.0 (/home/runner/work/apiforge/crates/apiforge-core)
indexmap v2.14.2
ash v0.38.0+1.3.281 (*)
epaint_default_fonts v0.36.2
apiforge-desktop v0.1.0 (D:\\a\\apiforge\\apiforge\\crates\\apiforge-desktop)
indexmap v2.14.2 (*)
"""


class Notices(unittest.TestCase):
    """Criteria 2 and 3: every crate of both binaries and the egui fonts."""

    def test_a_complete_file_passes(self):
        release.check_notices(NOTICE_TEXT, TREE)

    def test_a_crate_missing_from_the_file_fails_and_is_named(self):
        tree = TREE + "serde_derive v1.0.229 (proc-macro)\n"
        with self.assertRaisesRegex(ValueError, r"serde_derive 1\.0\.229"):
            release.check_notices(NOTICE_TEXT, tree)

    def test_another_version_of_a_listed_crate_is_missing(self):
        with self.assertRaisesRegex(ValueError, r"indexmap 1\.9\.3"):
            release.check_notices(NOTICE_TEXT, TREE + "indexmap v1.9.3\n")

    def test_a_workspace_crate_is_not_required(self):
        tree = TREE + "apiforge-mcp v0.1.0 (/home/runner/work/apiforge/crates/apiforge-mcp)\n"
        release.check_notices(NOTICE_TEXT, tree)
        self.assertNotIn(("apiforge-mcp", "0.1.0"), release.tree_crates(tree))
        self.assertNotIn(("apiforge-desktop", "0.1.0"), release.tree_crates(tree))
        self.assertIn(("ash", "0.38.0+1.3.281"), release.tree_crates(tree))

    def test_an_empty_tree_fails(self):
        with self.assertRaises(ValueError):
            release.check_notices(NOTICE_TEXT, "")

    def test_each_egui_font_name_is_required(self):
        for font in ["Hack", "Ubuntu-Light", "NotoEmoji", "emoji-icon-font"]:
            text = NOTICE_TEXT.replace(font + " (", "(")
            with self.assertRaisesRegex(ValueError, re.escape(font), msg=font):
                release.check_notices(text, TREE)

    def test_each_egui_font_licence_text_is_required(self):
        for marker in [
            "BITSTREAM VERA LICENSE",
            "UBUNTU FONT LICENCE Version 1.0",
            "SIL OPEN FONT LICENSE Version 1.1",
            "Copyright (c) 2014 John Slegers",
        ]:
            with self.assertRaises(ValueError, msg=marker):
                release.check_notices(NOTICE_TEXT.replace(marker, ""), TREE)

    def test_the_command_reads_the_tree_on_stdin(self):
        with tempfile.TemporaryDirectory() as directory:
            notices = os.path.join(directory, "THIRD-PARTY-NOTICES.md")
            with open(notices, "w", encoding="utf-8") as file:
                file.write(NOTICE_TEXT)
            command = [sys.executable, "-I", os.path.join(HERE, "release.py"), "check-notices", notices]
            passed = subprocess.run(command, input=TREE, capture_output=True, text=True)
            self.assertEqual(passed.returncode, 0, passed.stderr)
            failed = subprocess.run(command, input=TREE + "zlib-rs v0.5.0\n", capture_output=True, text=True)
            self.assertNotEqual(failed.returncode, 0)
            self.assertIn("zlib-rs 0.5.0", failed.stderr)


class AboutConfig(unittest.TestCase):
    """Criteria 1 and 3: about.toml, read as text (Python 3.10 has no tomllib)."""

    def setUp(self):
        self.about = read("about.toml")

    def array(self, key):
        found = re.search(r"^" + re.escape(key) + r"\s*=\s*\[(.*?)\]", self.about, re.M | re.S)
        self.assertIsNotNone(found, key)
        return re.findall(r'"([^"]*)"', found.group(1))

    def test_exactly_the_licences_of_the_workspace_in_their_order(self):
        self.assertEqual(self.array("accepted"), ACCEPTED)
        for refused in ["GPL-2.0-only", "LGPL-2.1", "GPL-3.0", "AGPL"]:
            self.assertNotIn(refused, self.about)

    def test_both_release_targets_and_no_workspace_crate(self):
        self.assertEqual(self.array("targets"), TRIPLES)
        self.assertRegex(self.about, r"(?m)^private\s*=\s*\{\s*ignore\s*=\s*true\s*\}")
        self.assertRegex(self.about, r"(?m)^ignore-dev-dependencies\s*=\s*true")
        self.assertRegex(self.about, r"(?m)^ignore-build-dependencies\s*=\s*true")

    def test_the_egui_fonts_are_clarified_with_their_texts(self):
        section = self.about.split("[epaint_default_fonts.clarify]", 1)
        self.assertEqual(len(section), 2, "no clarify entry for epaint_default_fonts")
        clarify = section[1]
        self.assertEqual(re.findall(r'(?m)^path\s*=\s*"([^"]+)"', clarify), FONT_FILES)
        self.assertEqual(len(re.findall(r'(?m)^checksum\s*=\s*"[0-9a-f]{64}"', clarify)), len(FONT_FILES))

    def test_each_font_file_has_its_own_licence_and_checksum(self):
        # Review of #179: each `files` record binds a path to a licence and a
        # checksum, not only the right numbers of each.
        clarify = self.about.split("[epaint_default_fonts.clarify]", 1)[1]
        records = clarify.split("[[epaint_default_fonts.clarify.files]]")[1:]
        expected = dict(zip(FONT_FILES, ["MIT", "Ubuntu-font-1.0", "OFL-1.1", "MIT"]))
        found = {}
        for record in records:
            fields = dict(re.findall(r'(?m)^(path|license|checksum)\s*=\s*"([^"]+)"', record))
            self.assertRegex(fields.get("checksum", ""), r"^[0-9a-f]{64}$", record)
            found[fields["path"]] = fields.get("license")
        self.assertEqual(found, expected)
        self.assertEqual(len(records), len(FONT_FILES))

    def test_the_template_lists_each_crate_with_its_version(self):
        template = read("packaging/about.hbs")
        self.assertIn("- {{crate.name}} {{crate.version}}", template)
        self.assertIn("{{{text}}}", template)


class NoticeWorkflows(unittest.TestCase):
    """Criteria 1, 2, 4 and 5 in rust.yml and release.yml."""

    def setUp(self):
        self.rust = read(".github/workflows/rust.yml")
        self.release = read(".github/workflows/release.yml")

    def pins(self, workflow):
        url = re.findall(r"CARGO_ABOUT_URL: (\S+)", workflow)
        digest = re.findall(r"CARGO_ABOUT_SHA256: (\S+)", workflow)
        return url, digest

    def test_one_pinned_prebuilt_cargo_about_in_both_workflows(self):
        rust_url, rust_digest = self.pins(self.rust)
        release_url, release_digest = self.pins(self.release)
        self.assertEqual(len(rust_url), 1)
        self.assertEqual((rust_url, rust_digest), (release_url, release_digest))
        self.assertRegex(
            rust_url[0],
            r"^https://github\.com/EmbarkStudios/cargo-about/releases/download/(\d+\.\d+\.\d+)/"
            r"cargo-about-\1-x86_64-unknown-linux-musl\.tar\.gz$",
        )
        self.assertRegex(rust_digest[0], r"^[0-9a-f]{64}$")
        for workflow in [self.rust, self.release]:
            download = step(workflow, "Download cargo-about")
            self.assertIn('"$CARGO_ABOUT_URL"', download)
            self.assertIn("sha256sum -c", download)

    def generation(self, workflow, name):
        text = step(workflow, name)
        self.assertIn("cargo about generate", text)
        self.assertIn("--fail", text)
        self.assertIn("packaging/about.hbs", text)
        for triple in TRIPLES:
            self.assertIn(triple, text)
        self.assertIn("release.py check-notices", text)
        self.assertIn("-p apiforge-desktop -p apiforge-cli", text)
        return text

    def test_every_push_generates_and_checks_the_notices_on_linux(self):
        self.assertIn("push:", self.rust.split("jobs:", 1)[0])
        text = self.generation(self.rust, "Generate and check the third-party notices")
        self.assertIn("runner.os == 'Linux'", text)

    def test_the_release_generates_the_notices_once_and_shares_them(self):
        version = job(self.release, "version")
        self.generation(version, "Generate and check the third-party notices")
        upload = step(version, "Upload the third-party notices")
        self.assertIn("name: notices", upload)
        for name in ["build-linux", "build-windows", "smoke-appimage"]:
            download = step(job(self.release, name), "Download the third-party notices")
            self.assertIn("name: notices", download, name)
        linux = job(self.release, "build-linux")
        self.assertIn("THIRD-PARTY-NOTICES.md", step(linux, "Pack and check the tar.gz"))
        self.assertIn("THIRD-PARTY-NOTICES.md", step(linux, "Build the AppImage"))
        windows = job(self.release, "build-windows")
        self.assertIn("THIRD-PARTY-NOTICES.md", step(windows, "Pack and check the ZIP"))

    def test_a_stale_clarification_fails_the_generation(self):
        # Review of #179: cargo-about only warns when a clarify checksum fails.
        for workflow, job_name in [(self.rust, None), (self.release, "version")]:
            text = workflow if job_name is None else job(workflow, job_name)
            generation = step(text, "Generate and check the third-party notices")
            self.assertIn("failed to validate", generation)
            self.assertIn("exit 1", generation)

    def test_the_appimage_carries_and_proves_the_library_copyright_files(self):
        build = step(job(self.release, "build-linux"), "Build the AppImage")
        self.assertIn("licenses/appimage-libraries", build)
        self.assertIn(LIBRARIES, build)
        self.assertIn("/usr/share/doc/", build)
        smoke = step(job(self.release, "smoke-appimage"), "Check the AppImage's licence files")
        self.assertIn("--appimage-extract", smoke)
        self.assertIn(NOTICES, smoke)
        self.assertIn(LIBRARIES, smoke)
        self.assertIn(".copyright", smoke)
        self.assertIn("cmp", smoke)


class NoticeManifestsAndDocs(unittest.TestCase):
    """Criteria 5 and 6."""

    def test_both_manifests_list_the_notices(self):
        for manifest in ["manifest-linux.txt", "manifest-windows.txt"]:
            self.assertIn(NOTICES, release.manifest(os.path.join(HERE, manifest)), manifest)

    def test_six_libraries_have_their_copyright_file_named(self):
        libraries = [line.split("#", 1)[0].strip() for line in read(LIBRARIES).splitlines()]
        self.assertEqual(len([library for library in libraries if library]), 6)

    def test_the_six_libraries_are_distinct(self):
        # Review of #179: six entries, not one entry six times.
        libraries = [line.split("#", 1)[0].strip() for line in read(LIBRARIES).splitlines()]
        libraries = [library for library in libraries if library]
        self.assertEqual(len(set(libraries)), 6)

    def test_the_readme_development_guide_and_font_sources_point_to_the_notices(self):
        licence = read("README.md").split("## License", 1)[1]
        self.assertIn("THIRD-PARTY-NOTICES.md", licence)
        releases = read("docs/development.md").split("## Releases", 1)[1].split("\n## ", 1)[0]
        self.assertIn("cargo-about", releases)
        self.assertIn("THIRD-PARTY-NOTICES.md", releases)
        self.assertIn("about.toml", releases)
        version = re.search(r"/releases/download/(\d+\.\d+\.\d+)/", read(".github/workflows/rust.yml"))
        self.assertIsNotNone(version)
        self.assertIn("cargo-about " + version.group(1), releases)
        self.assertIn("THIRD-PARTY-NOTICES.md", read("crates/apiforge-desktop/assets/fonts/FONT-SOURCES.md"))


if __name__ == "__main__":
    unittest.main()
