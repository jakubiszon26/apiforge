"""REL-01 (#115): the release workflow's checks, criteria 2, 3 and 5.
REL-03 (#120): the third-party notices, criteria 1 to 6.
REL-02 (#119): the application icon on Windows, criteria 1 to 6.
REL-04 (#205): no console window behind the desktop app, criteria 1, 2, 4 and
6 to 8 as the workflows and the sources state them.

    python3 -I packaging/test_release.py

`release.py` is what `release.yml` runs to compare the tag with the workspace
version, to name the assets, to pack and check the archives and to write the
sums. These tests run it on small temporary trees; the release workflow runs
them before it builds anything.
"""

import ast
import hashlib
import importlib.util
import io
import os
import re
import struct
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
            self.assertRegex(
                generation,
                r"if grep -q 'failed to validate' [^\n]*; then\n(?:[^\n]*\n)*?\s*exit 1\n\s*fi\n",
            )

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


# REL-02 (#119): the sizes of the Windows icon, in the order of criterion 1.
ICON_SIZES = [16, 32, 48, 64, 128, 256]
ICON = "packaging/icon/apiforge.ico"
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
BUILD_SCRIPTS = {
    # crate directory: (binary, FileDescription)
    "crates/apiforge-desktop": ("apiforge-desktop", "API Forge"),
    "crates/apiforge-cli": ("apiforge", "API Forge command line"),
}


def read_bytes(relative):
    with open(os.path.join(ROOT, *relative.split("/")), "rb") as file:
        return file.read()


def load_icon_module():
    path = os.path.join(HERE, "icon.py")
    spec = importlib.util.spec_from_file_location("icon", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def toml_section(text, header):
    """The lines of the TOML table HEADER, up to the next table."""
    found = re.search(r"^" + re.escape(header) + r"\s*$(.*?)(?=^\[|\Z)", text, re.M | re.S)
    return None if found is None else found.group(1)


class IconFile(unittest.TestCase):
    """Criterion 1: the .ico is the six PNG renders, and cannot drift from them."""

    def test_the_ico_is_built_from_the_pngs(self):
        icon = load_icon_module()
        pngs = [os.path.join(HERE, "icon", f"apiforge-{size}.png") for size in ICON_SIZES]
        self.assertEqual(icon.build(pngs), read_bytes(ICON))

    def test_the_ico_holds_the_six_pngs_in_order_and_verbatim(self):
        # Read with the format itself, not with icon.py, so a fault in the
        # builder cannot hide in its own output.
        data = read_bytes(ICON)
        reserved, kind, count = struct.unpack_from("<HHH", data, 0)
        self.assertEqual((reserved, kind, count), (0, 1, len(ICON_SIZES)))
        for index, size in enumerate(ICON_SIZES):
            width, height, colours, zero, planes, bits, length, offset = struct.unpack_from(
                "<BBBBHHII", data, 6 + 16 * index
            )
            expected = 0 if size == 256 else size
            self.assertEqual((width, height, colours, zero), (expected, expected, 0, 0), size)
            self.assertEqual((planes, bits), (1, 32), size)
            png = read_bytes(f"packaging/icon/apiforge-{size}.png")
            self.assertEqual(data[offset : offset + length], png, size)
            self.assertTrue(png.startswith(PNG_SIGNATURE), size)
            self.assertEqual(struct.unpack(">II", png[16:24]), (size, size), size)
        last = struct.unpack_from("<II", data, 6 + 16 * (len(ICON_SIZES) - 1) + 8)
        self.assertEqual(sum(last), len(data), "nothing follows the last image")

    def test_the_builder_uses_the_standard_library_only(self):
        tree = ast.parse(read("packaging/icon.py"))
        modules = set()
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                modules.update(alias.name.split(".")[0] for alias in node.names)
            elif isinstance(node, ast.ImportFrom):
                modules.add(node.module.split(".")[0])
        self.assertTrue(modules)
        self.assertLessEqual(modules, set(sys.stdlib_module_names))


class WindowsResource(unittest.TestCase):
    """Criteria 2 and 5: both binaries carry the icon and the version strings,
    through a build script that does nothing on another target."""

    def test_each_binary_has_a_build_script_with_the_icon_and_the_version_strings(self):
        for crate, (binary, description) in BUILD_SCRIPTS.items():
            script = read(f"{crate}/build.rs")
            self.assertIn("winresource::WindowsResource::new()", script, crate)
            self.assertIn('"../../packaging/icon/apiforge.ico"', script, crate)
            self.assertIn('.set("ProductName", "API Forge")', script, crate)
            self.assertIn(f'.set("FileDescription", "{description}")', script, crate)
            for field in ["FileVersion", "ProductVersion"]:
                self.assertRegex(script, r'\.set\("' + field + r'", &?version\)', crate)
            self.assertIn('env::var("CARGO_PKG_VERSION")', script, crate)
            self.assertIn(f'.compile_for("{binary}")', script, crate)
            # Only a Windows build runs the resource compiler.
            self.assertIn('CARGO_CFG_TARGET_OS', script, crate)
            self.assertIn("#[cfg(windows)]", script, crate)

    def test_winresource_is_a_windows_only_build_dependency_pinned_by_the_lock(self):
        root = read("Cargo.toml")
        pinned = re.search(r'^winresource = \{ version = "=(\d+\.\d+\.\d+)"', root, re.M)
        self.assertIsNotNone(pinned, "the workspace pins winresource")
        for crate in BUILD_SCRIPTS:
            manifest = read(f"{crate}/Cargo.toml")
            section = toml_section(manifest, "[target.'cfg(windows)'.build-dependencies]")
            self.assertIsNotNone(section, crate)
            self.assertIn("winresource.workspace = true", section, crate)
            self.assertEqual(manifest.count("winresource"), 1, crate)
        lock = read("Cargo.lock")
        self.assertIn(f'name = "winresource"\nversion = "{pinned.group(1)}"', lock)


class IconWorkflows(unittest.TestCase):
    """Criteria 2, 3 and 4 in rust.yml and release.yml."""

    def setUp(self):
        self.rust = read(".github/workflows/rust.yml")
        self.release = read(".github/workflows/release.yml")

    def test_windows_ci_builds_both_release_binaries(self):
        build = step(self.rust, "Build release binaries")
        self.assertIn("cargo build --release --locked -p apiforge-desktop -p apiforge-cli", build)

    def test_every_windows_run_extracts_the_icon_of_both_exes(self):
        icon = step(self.rust, "Show the application icon (Windows)")
        self.assertIn("runner.os == 'Windows'", icon)
        self.assertIn("[System.Drawing.Icon]::ExtractAssociatedIcon", icon)
        for name in ["apiforge.exe", "apiforge-desktop.exe"]:
            self.assertIn(name, icon)
        # 32 x 32, a transparent corner, an opaque centre, the source's pixels.
        self.assertIn("$bitmap.Width -ne 32 -or $bitmap.Height -ne 32", icon)
        self.assertIn("GetPixel(0, 0).A", icon)
        self.assertIn("GetPixel(16, 16).A", icon)
        self.assertIn("packaging/icon/apiforge-32.png", icon)
        self.assertIn("VersionInfo", icon)

    def test_every_windows_run_captures_the_window_with_its_icon(self):
        icon = step(self.rust, "Show the application icon (Windows)")
        self.assertIn("'--folder'", icon)
        self.assertIn("MainWindowHandle", icon)
        self.assertIn("CopyFromScreen", icon)
        self.assertIn("taskkill.exe", icon)
        upload = step(self.rust, "Upload the application icon (Windows)")
        self.assertIn("runner.os == 'Windows'", upload)
        self.assertIn("name: windows-icon", upload)
        self.assertIn("test-results/icon/", upload)

    def test_the_release_checks_the_product_version_of_both_exes(self):
        smoke = step(job(self.release, "build-windows"), "Smoke the binaries from the extracted ZIP")
        self.assertIn(".VersionInfo.ProductVersion", smoke)
        self.assertRegex(smoke, r"-ne \$env:VERSION")


class WindowIconSource(unittest.TestCase):
    """Criterion 3: the window takes the icon on every platform."""

    def test_the_icon_is_not_gated_on_linux(self):
        main = read("crates/apiforge-desktop/src/main.rs")
        self.assertIn(".with_icon(Arc::new(window_icon()))", main)
        self.assertNotRegex(main, r'#\[cfg\(target_os = "linux"\)\]\s*\n[^\n]*with_icon')
        self.assertNotRegex(main, r"allow\(dead_code\)\]\s*\nfn window_icon")


class IconDocs(unittest.TestCase):
    """Criterion 6: the development guide says where the .ico comes from."""

    def test_the_releases_section_says_how_to_rebuild_the_ico(self):
        releases = read("docs/development.md").split("## Releases", 1)[1].split("\n## ", 1)[0]
        self.assertIn("packaging/icon/apiforge.ico", releases)
        self.assertIn("python3 -I packaging/icon.py", releases)
        self.assertIn("winresource", releases)


# REL-04 (#205): the attribute that makes the release desktop build a windowed
# program, exactly as the ticket states it.
WINDOWS_SUBSYSTEM = (
    '#![cfg_attr(all(windows, not(debug_assertions), not(test)), windows_subsystem = "windows")]'
)


def start_process_calls(text):
    """Each `Start-Process` statement of a PowerShell script, with its
    backtick continuations and its argument array."""
    calls = []
    for found in re.finditer(r"Start-Process\b", text):
        end = found.start()
        depth = 0
        while end < len(text):
            char = text[end]
            if char in "(":
                depth += 1
            elif char == ")":
                depth -= 1
            elif char == "\n" and depth == 0 and not text[:end].rstrip(" ").endswith("`"):
                break
            end += 1
        calls.append(text[found.start() : end])
    return calls


def steps_before(workflow, first, second):
    return workflow.index(f"- name: {first}\n") < workflow.index(f"- name: {second}\n")


class ConsoleSubsystem(unittest.TestCase):
    """Criterion 1: the release desktop .exe is WindowsGui; apiforge.exe and the
    debug desktop build stay WindowsCui; every Windows run and the release
    smoke read it from the PE header."""

    def setUp(self):
        self.rust = read(".github/workflows/rust.yml")
        self.release = read(".github/workflows/release.yml")

    def test_the_release_desktop_build_is_a_windowed_program_and_nothing_else_is(self):
        # rustfmt wraps the attribute over four lines; compared on one.
        main = re.sub(r"\s+", " ", read("crates/apiforge-desktop/src/main.rs"))
        main = main.replace("( ", "(").replace(" )", ")")
        self.assertEqual(main.count(WINDOWS_SUBSYSTEM), 1)
        self.assertEqual(main.count("windows_subsystem"), 1)
        for path in ["crates/apiforge-cli/src/main.rs", "crates/apiforge-cli/src/lib.rs"]:
            if os.path.exists(os.path.join(ROOT, *path.split("/"))):
                self.assertNotIn("windows_subsystem", read(path), path)

    def test_every_windows_run_reads_the_subsystem_of_the_three_exes(self):
        icon = step(self.rust, "Show the application icon (Windows)")
        self.assertIn("System.Reflection.PortableExecutable.PEReader", icon)
        self.assertIn(".PEHeaders.PEHeader.Subsystem", icon)
        for exe, subsystem in [
            ("target/release/apiforge-desktop.exe", "WindowsGui"),
            ("target/release/apiforge.exe", "WindowsCui"),
            ("target/debug/apiforge-desktop.exe", "WindowsCui"),
        ]:
            self.assertRegex(icon, re.escape(f"Exe = '{exe}'; Subsystem = '{subsystem}'"), exe)
        self.assertIn("$subsystem -ne $binary.Subsystem", icon)
        # The debug build is the one the test step made before.
        self.assertTrue(steps_before(self.rust, "Run workspace unit and integration tests", "Show the application icon (Windows)"))

    def test_the_release_smoke_reads_the_subsystem_of_both_extracted_exes(self):
        smoke = step(job(self.release, "build-windows"), "Smoke the binaries from the extracted ZIP")
        self.assertIn("System.Reflection.PortableExecutable.PEReader", smoke)
        self.assertIn(".PEHeaders.PEHeader.Subsystem", smoke)
        self.assertIn("'apiforge' = 'WindowsCui'", smoke)
        self.assertIn("'apiforge-desktop' = 'WindowsGui'", smoke)
        self.assertIn("Join-Path $root \"$name.exe\"", smoke)
        self.assertRegex(smoke, r"if \(\$subsystem -ne [^)]+\) \{\s+throw")


class NoConsoleWindow(unittest.TestCase):
    """Criterion 2: the window started the way Explorer starts it owns no
    conhost.exe, and is captured into windows-icon."""

    def setUp(self):
        self.rust = read(".github/workflows/rust.yml")
        self.icon = step(self.rust, "Show the application icon (Windows)")

    def test_the_window_is_started_without_a_console_or_a_redirection(self):
        calls = start_process_calls(self.icon)
        self.assertEqual(len(calls), 1, calls)
        self.assertIn("'--folder'", calls[0])
        self.assertIn("-PassThru", calls[0])
        self.assertNotIn("-NoNewWindow", calls[0])
        self.assertNotIn("-Redirect", calls[0])

    def test_the_open_window_owns_no_conhost(self):
        self.assertIn("Get-CimInstance Win32_Process -Filter \"Name='conhost.exe' AND ParentProcessId=$($process.Id)\"", self.icon)
        self.assertIn("$consoles.Count -ne 0", self.icon)
        # Asked while the window is open: after its handle, before the kill.
        query = self.icon.index("Name='conhost.exe'")
        self.assertLess(self.icon.index("MainWindowHandle"), query)
        self.assertLess(query, self.icon.index("} finally {", query))
        self.assertIn("throw ($problems -join ' ')", self.icon)

    def test_the_desktop_is_captured_into_windows_icon(self):
        self.assertIn("CopyFromScreen", self.icon)
        self.assertIn("'desktop-window.png'", self.icon)
        upload = step(self.rust, "Upload the application icon (Windows)")
        self.assertIn("name: windows-icon", upload)
        self.assertIn("always()", upload)


class DesktopLog(unittest.TestCase):
    """Criteria 4 and 7: started by a parent with no console, the program writes
    the log; cmd's redirection still reaches its file."""

    NAME = "Write the desktop log without a console (Windows)"

    def setUp(self):
        self.rust = read(".github/workflows/rust.yml")
        self.log = step(self.rust, self.NAME)

    def test_it_runs_on_windows_after_the_data_directory_is_owned(self):
        folder = step(self.rust, "Exercise folder projects and managed startup (Windows)")
        self.assertIn("id: folder-windows", folder)
        self.assertIn("Reset-DataDirectory", folder)
        self.assertIn(
            "if: ${{ !cancelled() && runner.os == 'Windows' && steps.folder-windows.outcome == 'success' }}",
            self.log,
        )
        self.assertTrue(steps_before(self.rust, "Exercise folder projects and managed startup (Windows)", self.NAME))
        self.assertTrue(steps_before(self.rust, self.NAME, "Upload the application icon (Windows)"))

    def test_the_parent_has_no_console(self):
        self.assertIn("Invoke-CimMethod -ClassName Win32_Process -MethodName Create", self.log)
        self.assertIn('--check `"$fixture`"', self.log)
        self.assertIn("'fixtures/workspace.json'", self.log)
        self.assertIn("$process.ExitCode -ne 0", self.log)

    def test_the_log_ends_with_the_start_line_and_the_check(self):
        self.assertIn("'apiforge/logs'", self.log)
        self.assertIn("'apiforge-desktop.log'", self.log)
        self.assertIn("^--- apiforge-desktop $([regex]::Escape($version)) .+ ---$", self.log)
        self.assertIn("$lines[-1] -ne \"$valid\"", self.log)
        self.assertIn("'^Valid v\\d+ document: \\d+ project\\(s\\)$'", self.log)

    def test_two_starts_append_two_blocks_and_a_full_log_is_rotated(self):
        self.assertRegex(self.log, r"Invoke-WithoutConsole 'first'\s+Assert-Log 'first' 1\s+Invoke-WithoutConsole 'second'\s+Assert-Log 'second' 2")
        self.assertIn("$full = 1MB + 1", self.log)
        self.assertIn("'apiforge-desktop.log.1'", self.log)
        self.assertIn("(Get-Item $rotated).Length -ne $full", self.log)
        self.assertIn("Assert-Log 'rotated' 1", self.log)

    def test_cmd_redirects_the_version_into_a_file(self):
        self.assertIn('& cmd.exe /c "$program --version > $redirected"', self.log)
        self.assertIn('$line -ne "apiforge-desktop $version"', self.log)


class OutputCapture(unittest.TestCase):
    """Criterion 6: CI reads a windowed program's output deterministically."""

    def setUp(self):
        self.rust = read(".github/workflows/rust.yml")
        self.release = read(".github/workflows/release.yml")

    def test_the_release_smoke_reads_the_checks_output(self):
        smoke = step(job(self.release, "build-windows"), "Smoke the binaries from the extracted ZIP")
        # Assigning a windowed program's output is not enough: PowerShell then
        # neither waits nor reads, and the program writes into a closed pipe.
        self.assertIn(
            "$check = & (Join-Path $root 'apiforge-desktop.exe') --check fixtures/workspace.json | Write-Output",
            smoke,
        )
        self.assertNotRegex(smoke, r"(?m)^\s*& \(Join-Path \$root 'apiforge-desktop\.exe'\) --check")
        self.assertIn("$LASTEXITCODE -ne 0", smoke)
        self.assertIn("\"$check\" -notmatch '^Valid v", smoke)
        self.assertIn("$line = & (Join-Path $root \"$name.exe\") --version | Write-Output", smoke)

    def test_every_windows_step_pipes_what_it_reads_from_the_desktop_binary(self):
        icon = step(self.rust, "Show the application icon (Windows)")
        self.assertIn("(& $exe --version | Write-Output)", icon)
        log = step(self.rust, "Write the desktop log without a console (Windows)")
        self.assertIn("$valid = & $program --check $fixture | Write-Output", log)
        for workflow in [self.rust, self.release]:
            self.assertNotRegex(workflow, r"(?m)= \(?& [^\n|]*--(version|check)[^\n|]*$")

    def test_the_native_validation_reads_the_valid_line(self):
        validate = step(self.rust, "Validate workspace using the native release binary")
        self.assertIn("shell: bash", validate)
        self.assertIn('output="$(./${{ matrix.binary_path }} --check fixtures/workspace.json)"', validate)
        self.assertIn("'Valid v'*)", validate)
        self.assertIn("exit 1", validate)

    def test_every_windowed_probe_redirects_its_errors_and_prints_them(self):
        for name in [
            "Open native window and capture it (Windows)",
            "Exercise folder projects and managed startup (Windows)",
        ]:
            text = step(self.rust, name)
            calls = start_process_calls(text)
            self.assertTrue(calls, name)
            for call in calls:
                self.assertIn("-RedirectStandardError", call, f"{name}: {call}")
            self.assertIn("Show-Failures $failures", text, name)
        native = step(self.rust, "Open native window and capture it (Windows)")
        self.assertEqual(native.count("Show-Failures $failures"), 1)
        folder = step(self.rust, "Exercise folder projects and managed startup (Windows)")
        self.assertEqual(folder.count("Show-Failures $failures"), 2)
        # The icon step's window cannot be redirected (criterion 2); what it
        # writes without a console is in its log, which the step prints.
        icon = step(self.rust, "Show the application icon (Windows)")
        self.assertIn("'logs/apiforge-desktop.log'", icon)
        self.assertIn("Get-Content $log", icon)

    def test_every_windows_start_keeps_its_exit_code(self):
        for name in [
            "Open native window and capture it (Windows)",
            "Exercise folder projects and managed startup (Windows)",
            "Show the application icon (Windows)",
        ]:
            text = step(self.rust, name)
            for call in start_process_calls(text):
                self.assertIn("-PassThru", call, f"{name}: {call}")
            self.assertIn("$null = $process.Handle", text, name)
            self.assertIn("WaitForExit(", text, name)
            self.assertIn("ExitCode", text, name)


class NoNewCrate(unittest.TestCase):
    """Criterion 8: windows-sys only for Windows, at a version the lock already
    holds; apiforge-cli is untouched; the guide says what changed."""

    def test_windows_sys_is_a_windows_only_dependency_at_a_locked_version(self):
        root = read("Cargo.toml")
        pinned = re.search(r'^windows-sys = \{ version = "=(\d+\.\d+\.\d+)", features = \[([^\]]*)\] \}', root, re.M)
        self.assertIsNotNone(pinned, "the workspace pins windows-sys")
        features = re.findall(r'"([^"]+)"', pinned.group(2))
        self.assertEqual(features, ["Win32_Foundation", "Win32_System_Console"])
        self.assertIn(f'name = "windows-sys"\nversion = "{pinned.group(1)}"', read("Cargo.lock"))
        manifest = read("crates/apiforge-desktop/Cargo.toml")
        section = toml_section(manifest, "[target.'cfg(windows)'.dependencies]")
        self.assertIsNotNone(section)
        self.assertIn("windows-sys.workspace = true", section)
        self.assertEqual(manifest.count("windows-sys"), 1)
        self.assertNotIn("windows-sys", read("crates/apiforge-cli/Cargo.toml"))

    def test_running_says_where_the_output_goes_without_a_console(self):
        running = read("docs/development.md").split("## Running", 1)[1].split("\n## ", 1)[0]
        paragraph = next((p for p in running.split("\n\n") if "apiforge-desktop.log" in p), "")
        self.assertTrue(paragraph, "Running names the log")
        for text in [
            "%APPDATA%\\apiforge\\logs\\apiforge-desktop.log",
            "apiforge-desktop.log.1",
            "start /wait",
            "%ERRORLEVEL%",
            "console",
            "apiforge.exe",
        ]:
            self.assertIn(text, " ".join(paragraph.split()), text)


class UnsafeScope(unittest.TestCase):
    """REL-04's two Windows calls are the only unsafe code: the desktop crate
    denies it instead of forbidding it, and allows it in one module only."""

    def test_the_desktop_lints_are_the_workspaces_with_unsafe_code_denied(self):
        root = read("Cargo.toml")
        manifest = read("crates/apiforge-desktop/Cargo.toml")
        self.assertIn('unsafe_code = "forbid"', toml_section(root, "[workspace.lints.rust]"))
        self.assertEqual(toml_section(manifest, "[lints.rust]").strip(), 'unsafe_code = "deny"')
        self.assertEqual(
            toml_section(manifest, "[lints.clippy]").strip(),
            toml_section(root, "[workspace.lints.clippy]").strip(),
        )

    def test_only_the_windows_native_calls_allow_unsafe_code(self):
        allowed = []
        for directory, _, names in os.walk(os.path.join(ROOT, "crates", "apiforge-desktop")):
            for name in names:
                if name.endswith(".rs"):
                    path = os.path.join(directory, name)
                    with open(path, encoding="utf-8") as file:
                        if "allow(unsafe_code)" in file.read():
                            allowed.append(os.path.relpath(path, ROOT).replace(os.sep, "/"))
        self.assertEqual(allowed, ["crates/apiforge-desktop/src/windows_console.rs"])
        source = read("crates/apiforge-desktop/src/windows_console.rs")
        self.assertEqual(source.count("allow(unsafe_code)"), 1)
        self.assertRegex(source, r"#\[cfg\(windows\)\]\nmod native \{\n    #!\[allow\(unsafe_code\)\]")
        self.assertEqual(source.count("unsafe {"), 2)


if __name__ == "__main__":
    unittest.main()
