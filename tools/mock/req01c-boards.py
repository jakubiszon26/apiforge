"""REQ-01c mock-up: assembles the comparison boards from the review captures.
Mock branch only.  python3 tools/mock/req01c-boards.py <captures> <out>"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

src, out = Path(sys.argv[1]), Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
font = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf", 15)


def board(name, panels, box, columns=2):
    crops = [(caption, Image.open(src / f"{file}.png").crop(box)) for caption, file in panels]
    gap, bar = 12, 30
    w, h = crops[0][1].size
    rows = -(-len(crops) // columns)
    sheet = Image.new("RGB", (columns * (w + gap) + gap, rows * (h + bar + gap) + gap), (236, 236, 233))
    draw = ImageDraw.Draw(sheet)
    for index, (caption, image) in enumerate(crops):
        x = gap + (index % columns) * (w + gap)
        y = gap + (index // columns) * (h + bar + gap)
        draw.text((x + 2, y + 5), caption, font=font, fill=(30, 30, 30))
        sheet.paste(image, (x, y + bar))
        draw.rectangle((x - 1, y + bar - 1, x + w, y + bar + h), outline=(170, 170, 170))
    sheet.save(out / f"{name}.png")


KINDS = (248, 98, 952, 560)
LABELS = (248, 98, 952, 660)
board("body-kinds-960", [
    ("Today · SETS takes the place of SENDS", "req01c-kinds-today-960tall"),
    ("A · SENDS always, SETS under it", "req01c-kinds-a-960tall"),
    ("B · today, and a BODY fold (open on POST first)", "req01c-kinds-b-960tall"),
    ("C · A and B", "req01c-kinds-c-960tall"),
    ("D · A, and + N more unfolds SENDS in place", "req01c-kinds-d-open-960tall"),
], KINDS)
board("body-kinds-1600", [
    ("Today", "req01c-kinds-today-1600"),
    ("A · SETS a column of its own", "req01c-kinds-a-1600"),
    ("D · unfolded", "req01c-kinds-d-open-1600"),
], (248, 98, 1592, 400), 1)
board("body-labels-960", [
    ("Today, A, D · what differs from the common request", "req01c-labels-a-960tall"),
    ("B, C · BODY folded: a line more on every card", "req01c-labels-c-960tall"),
    ("B, C · Expand bodies: what differs is marked", "req01c-labels-c-all-960tall"),
], LABELS)
