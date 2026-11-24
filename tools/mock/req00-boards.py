"""REQ-00 mock-up: assembles the comparison boards from the review captures.
Mock branch only.  python3 tools/mock/req00-boards.py <captures> <out>"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

src, out = Path(sys.argv[1]), Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
font = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf", 15)
AREA_960 = (248, 98, 952, 490)
AREA_1600 = (248, 98, 1600, 946)
RESPONSE_1600 = (1066, 140, 1592, 946)


def board(name, panels, box):
    crops = [(caption, Image.open(src / f"{file}.png").crop(box)) for caption, file in panels]
    gap, bar = 12, 30
    width = sum(image.width for _, image in crops) + gap * (len(crops) + 1)
    height = max(image.height for _, image in crops) + bar + gap * 2
    sheet = Image.new("RGB", (width, height), (236, 236, 233))
    draw = ImageDraw.Draw(sheet)
    x = gap
    for caption, image in crops:
        draw.text((x + 2, gap + 5), caption, font=font, fill=(30, 30, 30))
        sheet.paste(image, (x, gap + bar))
        draw.rectangle((x - 1, gap + bar - 1, x + image.width, gap + bar + image.height), outline=(170, 170, 170))
        x += image.width + gap
    sheet.save(out / f"{name}.png")


board("q1-sends-960", [
    ("Today · the board (v0.2.1)", "today-labels-canvas-960"),
    ("A · differs from the common request", "q1-a-common-960"),
    ("B · differs from the step above", "q1-b-above-960"),
    ("C · differs from the first request", "q1-c-first-960"),
], AREA_960)
board("q2-inspector-960", [
    ("Today · Canvas, the request in the Inspector", "today-labels-inspector-960"),
    ("A · Steps: the request pane, no Inspector; Canvas stays as today", "q4-a-rail-960"),
], (8, 0, 952, 532))
board("q3-send-alone-1600", [
    ("The last run", "request-body-1600"),
    ("Sent alone", "q3-sent-alone-1600"),
    ("The request changed after the run", "q3-changed-1600"),
    ("Not run yet", "request-not-run-1600"),
], RESPONSE_1600)
board("q4-fold-960", [
    ("No folding · three columns", "q4-today-three-960"),
    ("A · the list folds to a rail", "q4-a-rail-960"),
    ("B · Request | Response as tabs", "q4-b-tabs-960"),
    ("C · the response under the request", "q4-c-under-960"),
], AREA_960)
board("q5-kinds-1600", [
    ("Today · the board (v0.2.1)", "today-rooms-canvas-1600"),
    ("A · every step a row", "q5-a-all-1600"),
    ("B · requests only", "q5-b-requests-1600"),
], (248, 98, 1150, 700))
board("q7-add-960", [
    ("Today · the board's + on a line (DIAG-06b)", "today-labels-canvas-960"),
    ("A · + between cards, Add step at the end", "q7-a-line-960"),
    ("B · Add step on the bar", "q7-b-bar-960"),
], AREA_960)
board("q8-cleanup-1600", [
    ("Today · the board's Main | Cleanup tabs", "today-rooms-canvas-1600"),
    ("A · a section of the list after Main", "q8-a-section-add-1600"),
    ("B · Main | Cleanup tabs in Steps", "q8-b-tabs-1600"),
], (248, 98, 1150, 946))
board("q6-menu-1600", [
    ("The card's menu: Move earlier, Move later, Move to Cleanup", "q6-menu-1600"),
], (248, 98, 1300, 520))
