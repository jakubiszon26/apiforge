"""The release workflow's checks and packing (REL-01, #115).

    python3 -I packaging/release.py version TAG [CARGO_TOML]
    python3 -I packaging/release.py names TAG
    python3 -I packaging/release.py pack DIRECTORY ARCHIVE
    python3 -I packaging/release.py check-archive ARCHIVE MANIFEST
    python3 -I packaging/release.py check-assets DIRECTORY TAG
    python3 -I packaging/release.py sums DIRECTORY TAG

`version` fails unless TAG is `v` and the workspace version, optionally with
`-rc.N`, and prints the version. `names` prints the four asset names.
`pack` writes DIRECTORY, under its own name, into a .tar.gz or a .zip.
`check-archive` fails unless the archive holds exactly the files of MANIFEST
below one top-level directory named like the archive. `check-assets` fails
unless DIRECTORY holds exactly the four assets. `sums` writes SHA256SUMS.txt
for the other three. Standard library only, for Linux and Windows runners.
"""

import hashlib
import os
import re
import stat
import sys
import tarfile
import zipfile

SUMS = "SHA256SUMS.txt"
ARCHIVES = (".tar.gz", ".zip")


def workspace_version(cargo_toml):
    """The `version` of `[workspace.package]` in a Cargo.toml's text."""
    section = re.search(r"^\[workspace\.package\]\s*$(.*?)(?=^\[|\Z)", cargo_toml, re.M | re.S)
    found = section and re.search(r'^version\s*=\s*"([^"]+)"\s*$', section.group(1), re.M)
    if not found:
        raise ValueError("Cargo.toml has no version in [workspace.package]")
    return found.group(1)


def check_tag(tag, version):
    """A release tag is `v<version>` or `v<version>-rc.<N>`."""
    if not re.fullmatch(re.escape("v" + version) + r"(-rc\.(0|[1-9][0-9]*))?", tag):
        raise ValueError(f"tag {tag!r} is neither v{version} nor v{version}-rc.N")


def asset_names(tag):
    return [
        f"apiforge-desktop-{tag}-x86_64.AppImage",
        f"apiforge-{tag}-x86_64-unknown-linux-gnu.tar.gz",
        f"apiforge-{tag}-x86_64-pc-windows-msvc.zip",
        SUMS,
    ]


def check_assets(directory, tag):
    found = sorted(os.listdir(directory))
    expected = sorted(asset_names(tag))
    if found != expected:
        raise ValueError(f"{directory} holds {found}, expected {expected}")


def write_sums(directory, tag):
    lines = []
    for name in asset_names(tag):
        if name == SUMS:
            continue
        digest = hashlib.sha256()
        with open(os.path.join(directory, name), "rb") as file:
            for block in iter(lambda: file.read(1 << 20), b""):
                digest.update(block)
        lines.append(f"{digest.hexdigest()}  {name}\n")
    text = "".join(lines)
    with open(os.path.join(directory, SUMS), "w", encoding="ascii", newline="\n") as file:
        file.write(text)
    return text


def manifest(path):
    """The file names a manifest lists, one per line; `#` starts a comment."""
    with open(path, encoding="utf-8") as file:
        lines = (line.split("#", 1)[0].strip() for line in file)
        return sorted(line for line in lines if line)


def _stem(archive):
    name = os.path.basename(archive)
    for suffix in ARCHIVES:
        if name.endswith(suffix):
            return name[: -len(suffix)]
    raise ValueError(f"{archive} is neither a .tar.gz nor a .zip")


def pack(directory, archive):
    """DIRECTORY with everything below it, under its own name, sorted."""
    _stem(archive)
    root = os.path.basename(os.path.normpath(directory))
    files = []
    for parent, directories, names in os.walk(directory):
        directories.sort()
        for name in sorted(names):
            path = os.path.join(parent, name)
            relative = os.path.relpath(path, directory).replace(os.sep, "/")
            files.append((path, f"{root}/{relative}"))
    if archive.endswith(".zip"):
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as file:
            for path, name in files:
                file.write(path, name)
    else:
        with tarfile.open(archive, "w:gz", format=tarfile.PAX_FORMAT) as file:
            for path, name in files:
                info = file.gettarinfo(path, name)
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                executable = info.mode & stat.S_IXUSR
                info.mode = 0o755 if executable else 0o644
                with open(path, "rb") as content:
                    file.addfile(info, content)


def entries(archive):
    """Every entry of a .tar.gz or a .zip, directories with a trailing `/`."""
    if archive.endswith(".zip"):
        with zipfile.ZipFile(archive) as file:
            return [info.filename for info in file.infolist()]
    with tarfile.open(archive) as file:
        return [member.name + ("/" if member.isdir() else "") for member in file.getmembers()]


def check_archive(archive, manifest_path):
    root = _stem(archive) + "/"
    files, problems = [], []
    for entry in entries(archive):
        if not entry.startswith(root) or ".." in entry.split("/"):
            problems.append(f"{entry} is outside {root}")
        elif not entry.endswith("/"):
            files.append(entry[len(root) :])
    expected = manifest(manifest_path)
    for name in sorted(set(expected) - set(files)):
        problems.append(f"{root}{name} is missing")
    for name in sorted(set(files) - set(expected)):
        problems.append(f"{root}{name} is not in {os.path.basename(manifest_path)}")
    if len(files) != len(set(files)):
        problems.append("an entry appears twice")
    if problems:
        raise ValueError(f"{os.path.basename(archive)}: " + "; ".join(problems))


def main(arguments):
    command, rest = (arguments[0], arguments[1:]) if arguments else ("", [])
    if command == "version" and len(rest) in (1, 2):
        cargo = rest[1] if len(rest) == 2 else os.path.join(os.path.dirname(__file__), "..", "Cargo.toml")
        with open(cargo, encoding="utf-8") as file:
            version = workspace_version(file.read())
        check_tag(rest[0], version)
        print(version)
    elif command == "names" and len(rest) == 1:
        print("\n".join(asset_names(rest[0])))
    elif command == "pack" and len(rest) == 2:
        pack(*rest)
        print(f"{rest[1]}: {len(entries(rest[1]))} entries")
    elif command == "check-archive" and len(rest) == 2:
        check_archive(*rest)
        print(f"{os.path.basename(rest[0])} matches {os.path.basename(rest[1])}")
    elif command == "check-assets" and len(rest) == 2:
        check_assets(*rest)
        print("\n".join(sorted(os.listdir(rest[0]))))
    elif command == "sums" and len(rest) == 2:
        print(write_sums(*rest), end="")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except ValueError as error:
        sys.exit(f"release.py: {error}")
