#!/usr/bin/env python3
"""Composes the REP-07c board from the review captures. Throwaway.
   tools/gui/rep07c_board.py <captures dir> <out.png>"""
import sys, glob
from PIL import Image, ImageDraw, ImageFont
src, out = sys.argv[1], sys.argv[2]
def font(size, bold=False):
    for path in glob.glob("/usr/share/fonts/**/*.ttf", recursive=True):
        if path.endswith("DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf"):
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()
F, H = font(14), font(17, True)
rows = [("960", "960 × 540 · Run results at its default height there (180)"),
        ("1280", "1280 × 800 · default height 320"),
        ("1600", "1600 × 1000 · default height 320")]
cols = [("today", "Today (#613, box in form A)"), ("proposed", "Proposed · message in the box, the pane opens on the box")]
PAD = 24
def cell_of(s, o):
    image = Image.open(f"{src}/rep07c-panel-{s}-{o}.png").convert("RGB")
    # The capture's margin outside the window is black: cut it.
    box = image.point(lambda v: 255 if v > 8 else 0).getbbox()
    return image.crop(box)
cells = {(s, o): cell_of(s, o) for s, _ in rows for o, _ in cols}
notes = {
    ("960", "today"): "Shows the check's title and the top edge of the message. Box and response are below.",
    ("960", "proposed"): "Shows the whole box. The response and its mark are one scroll down; the title one scroll up.",
    ("1280", "today"): "Shows title, message, box and two lines of the response. The marked field is below the fold.",
    ("1280", "proposed"): "Shows the box and the response down to the marked field, whose line is cut at half. Title one scroll up.",
    ("1600", "today"): "As at 1280.",
    ("1600", "proposed"): "As at 1280.",
}
width = PAD + sum(cells[("1600", o)].width + PAD for o, _ in cols)
height = PAD + sum(cells[(s, "today")].height + 86 + PAD for s, _ in rows)
board = Image.new("RGB", (width, height), "#f4f4f5")
d = ImageDraw.Draw(board)
y = PAD
for s, title in rows:
    d.text((PAD, y), title, fill="#18181b", font=H)
    x = PAD
    for o, caption in cols:
        cell = cells[(s, o)]
        d.text((x, y + 30), caption, fill="#18181b", font=F)
        d.text((x, y + 60 + cell.height), notes[(s, o)], fill="#3f3f46", font=F)
        board.paste(cell, (x, y + 54))
        d.rectangle((x - 1, y + 53, x + cell.width, y + 54 + cell.height), outline="#a1a1aa")
        x += cells[("1600", o)].width + PAD
    y += cells[(s, "today")].height + 86 + PAD
board.save(out)
