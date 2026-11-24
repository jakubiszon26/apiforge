#!/usr/bin/env python3
"""Composes the REP-07c boards (one per window size) from the review captures.
   Throwaway.  tools/gui/rep07c_board.py <captures dir> <out dir> <notes.json>"""
import sys, glob, json
from PIL import Image, ImageDraw, ImageFont
src, out, notes = sys.argv[1], sys.argv[2], json.load(open(sys.argv[3]))
def font(size, bold=False):
    for path in glob.glob("/usr/share/fonts/**/*.ttf", recursive=True):
        if path.endswith("DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf"):
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()
F, B, H = font(14), font(15, True), font(18, True)
cols = [("today", "Today (#613)"), ("p1", "P1 · message is the box's first line"),
        ("p2", "P2 · P1 and a taller default"), ("p3", "P3 · P1 without \"The comparison failed.\"")]
PAD = 24
def cell_of(size, option):
    image = Image.open(f"{src}/rep07c-panel-{size}-{option}.png").convert("RGB")
    return image.crop(image.point(lambda v: 255 if v > 8 else 0).getbbox())
for size, title in [("960", "960 × 540"), ("1280", "1280 × 800"), ("1600", "1600 × 1000")]:
    cells = [cell_of(size, option) for option, _ in cols]
    width = PAD * 2 + max(cell.width for cell in cells)
    height = 60 + sum(cell.height + 78 + PAD for cell in cells)
    board = Image.new("RGB", (width, height), "#f4f4f5")
    d = ImageDraw.Draw(board)
    d.text((PAD, 18), f"{title} · Run results as it opens on a failed check · the box is form A in every panel", fill="#18181b", font=H)
    y = 60
    for (option, caption), cell in zip(cols, cells):
        d.text((PAD, y), caption + notes["height"][size][option], fill="#18181b", font=B)
        board.paste(cell, (PAD, y + 26))
        d.rectangle((PAD - 1, y + 25, PAD + cell.width, y + 26 + cell.height), outline="#a1a1aa")
        d.text((PAD, y + 32 + cell.height), notes["shows"][size][option], fill="#3f3f46", font=F)
        y += cell.height + 78 + PAD
    board.save(f"{out}/board-{size}.png")
