#!/usr/bin/env python3
"""Composes the REQ-20 comparison board from the review captures. Throwaway.
   tools/gui/req20_board.py <captures dir> <out.png>"""
import sys, glob
from PIL import Image, ImageDraw, ImageFont
src, out = sys.argv[1], sys.argv[2]
def font(size, bold=False):
    for path in glob.glob("/usr/share/fonts/**/*.ttf", recursive=True):
        if path.endswith("DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf"):
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()
F, B, H = font(13), font(15, True), font(17, True)
zoom = lambda d: round(float(open(f"{src}/req20-{d}-open-v1.txt").read()) * 100)
rows = [
    ("compact-100", "Compact cards, 100 %"),
    ("compact-open", f"Compact cards, as the board opens at 1600 × 1000 ({zoom('compact')} %)"),
    ("full-100", "Full cards, 60 %"),
    ("full-open", f"Full cards, as the board opens at 1600 × 1000 ({zoom('full')} %)"),
]
cols = [("today", "Today: nothing on the card"), ("v1", "A · V1: a badge on the card's corner"), ("v2", "A · V2: a line under the card")]
PAD = 24
cells = {(r, c): Image.open(f"{src}/req20-{r}-{c}.png").convert("RGB") for r, _ in rows for c, _ in cols}
W = max(i.width for i in cells.values())
notices = [(Image.open(f"{src}/req20-notice-{k}.png").convert("RGB"), t) for k, t in [("run", "After Run, once"), ("save", "After Save, once")]]
width = PAD + 3 * (W + PAD)
height = 44 + sum(max(cells[(r, c)].height for c, _ in cols) + 34 + PAD for r, _ in rows) + 40 + sum(n.height + 26 + 8 for n, _ in notices) + 60
board = Image.new("RGB", (width, height), "#f4f4f5")
d = ImageDraw.Draw(board)
for i, (_, title) in enumerate(cols):
    d.text((PAD + i * (W + PAD), 12), title, fill="#18181b", font=B)
y = 44
for r, title in rows:
    d.text((PAD, y), title, fill="#18181b", font=H)
    y += 30
    rh = max(cells[(r, c)].height for c, _ in cols)
    for i, (c, _) in enumerate(cols):
        board.paste(cells[(r, c)], (PAD + i * (W + PAD), y))
    y += rh + PAD + 4
d.text((PAD, y), "Run and Save say so once, in the board's notice (wording proposed)", fill="#18181b", font=H)
y += 34
for image, title in notices:
    d.text((PAD, y), title, fill="#3f3f46", font=F)
    board.paste(image, (PAD, y + 20))
    y += image.height + 34
d.multiline_text((PAD, y), "Mocked: the mark is painted over the real card of `buildings`, as if its address held `/buildings-v2?x={{`; V2's line is laid under the card and takes no room\nof its own yet. The notices are the board's own notice widget with the proposed sentences. V1's hover text (not drawn): \"Address: unfinished text — `{{` is not closed\".", fill="#3f3f46", font=F, spacing=4)
board.save(out)
