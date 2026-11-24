#!/usr/bin/env python3
"""Composes the DIAG-24 comparison board from the review captures. Throwaway.
   tools/gui/diag24_board.py <captures dir> <out.png>"""
import sys, glob
from PIL import Image, ImageDraw, ImageFont
src, out = sys.argv[1], sys.argv[2]
def font(size, bold=False):
    for path in glob.glob("/usr/share/fonts/**/*.ttf", recursive=True):
        if path.endswith("DejaVuSans-Bold.ttf" if bold else "DejaVuSans.ttf"):
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()
F, B, H = font(13), font(14, True), font(17, True)
rows = [
    ("profile", "On a request: profile (step 4)", {
        "today": "HTTP request: line 9, runs 5th, after profile, unbound.\nValue: reads profile, written last (line 84).",
        "a": "HTTP request: as today (line 9, runs 5th).\nValue: reads profile, written after profile (not drawn).",
        "b": "As today."}),
    ("slot", "On a value: slot (number 2, under begin)", {
        "today": "HTTP request: LAST (line 85, runs 30th of 30),\n`body slot` and an `@config` line on a GET.",
        "a": "HTTP request: line 4, runs 3rd, after begin and slot,\nunbound. A check or value: reads slot.",
        "b": "No step is offered here. A step comes from a step's\nmenu, Add step or the + on a line."}),
    ("big_enough", "On a check: big_enough (checks team, step 5)", {
        "today": "HTTP request: LAST (line 85, runs 30th of 30),\n`body big_enough.value` and an `@config` line.",
        "a": "HTTP request: line 14, runs 7th, right after team\nand its check, unbound.",
        "b": "No step is offered here."}),
    ("booked", "On an if: booked (step 25)", {
        "today": "HTTP request: LAST INSIDE YES (line 81).\nAdd step with booked selected: after the if.",
        "a": "HTTP request: after the if's braces (line 84),\nas Add step. Into Yes / No: the + on those lines.",
        "b": "HTTP request: last inside Yes, as today;\nthe heading says so."}),
]
cols = [("today", "Today (#618)"), ("a", "A · the heading names the place; steps go right after it"), ("b", "B · steps only from a step; the heading says what the menu does")]
W, PAD, CAP = 338, 24, 44
cells = {(h, o): Image.open(f"{src}/diag24-menu-{h}-{o}.png").convert("RGB") for h, _, _ in rows for o, _ in cols}
rowh = [max(cells[(h, o)].height for o, _ in cols) for h, _, _ in rows]
width = PAD + len(cols) * (W + PAD)
height = 50 + sum(r + 30 + CAP + PAD for r in rowh)
board = Image.new("RGB", (width, height), "#f4f4f5")
d = ImageDraw.Draw(board)
for i, (_, title) in enumerate(cols):
    x = PAD + i * (W + PAD)
    words, line, y = title.split(), "", 8
    for w in words:
        if d.textlength(line + " " + w, font=B) > W:
            d.text((x, y), line, fill="#18181b", font=B); y += 18; line = w
        else:
            line = (line + " " + w).strip()
    d.text((x, y), line, fill="#18181b", font=B)
y = 50
for (h, title, notes), rh in zip(rows, rowh):
    d.text((PAD, y), title, fill="#18181b", font=H)
    y += 30
    for i, (o, _) in enumerate(cols):
        x = PAD + i * (W + PAD)
        board.paste(cells[(h, o)], (x, y))
        d.multiline_text((x, y + rh + 6), notes[o], fill="#3f3f46", font=F, spacing=3)
    y += rh + CAP + PAD
board.save(out)
