"""REL-02 (#119): the Windows icon, packaging/icon/apiforge.ico.

    python3 -I packaging/icon.py          # write the .ico from the PNGs
    python3 -I packaging/icon.py --check  # fail unless the committed .ico matches

The .ico holds the 16, 32, 48, 64, 128 and 256 px renders of
packaging/icon/apiforge.svg, in that order, as PNG-compressed entries: a
6-byte header, one 16-byte directory entry per image (0 stands for 256 in the
width and height bytes), then each PNG file verbatim. Windows reads PNG
entries at every size since Vista. Standard library only, so that no build
needs an image library; test_release.py rebuilds the file and compares it
byte for byte.
"""

import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SIZES = [16, 32, 48, 64, 128, 256]
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
ICON = os.path.join(HERE, "icon", "apiforge.ico")


def sources():
    """The PNG renders the .ico is built from, in the order of its entries."""
    return [os.path.join(HERE, "icon", f"apiforge-{size}.png") for size in SIZES]


def png_size(data):
    """Width and height of a PNG, from its IHDR chunk."""
    if not data.startswith(PNG_SIGNATURE) or data[12:16] != b"IHDR":
        raise ValueError("not a PNG file")
    return struct.unpack(">II", data[16:24])


def build(paths):
    """The .ico bytes holding the square PNG files PATHS, in that order."""
    images = []
    for path in paths:
        with open(path, "rb") as file:
            data = file.read()
        width, height = png_size(data)
        if width != height or not 1 <= width <= 256:
            raise ValueError(f"{path} is {width} x {height}; an icon entry is square, 1 to 256 px")
        images.append((width, data))
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = len(header) + 16 * len(images)
    entries = b""
    for size, data in images:
        side = 0 if size == 256 else size
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return header + entries + b"".join(data for _, data in images)


def main(arguments):
    data = build(sources())
    if arguments == ["--check"]:
        with open(ICON, "rb") as file:
            if file.read() != data:
                print(f"{ICON} does not match its PNGs; run python3 -I packaging/icon.py", file=sys.stderr)
                return 1
        return 0
    if arguments:
        print("usage: python3 -I packaging/icon.py [--check]", file=sys.stderr)
        return 2
    with open(ICON, "wb") as file:
        file.write(data)
    print(f"wrote {ICON} ({len(data)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
