"""REQ-17 and REQ-18 mock-up: assembles the comparison boards from the review
captures. Mock branch only.  python3 tools/mock/req17-boards.py <captures> <out>"""
import shutil
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

src, out = Path(sys.argv[1]), Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
font = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf", 15)


def board(name, panels):
    crops = [(caption, Image.open(src / f"{file}.png").crop(box)) for caption, file, box in panels]
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


AREA = (248, 60, 952, 532)
board("board-try-960", [
    ("Today · a scenario, the Inspector, Run results", "req17-today-960", AREA),
    ("A · a request with no file: Send, Keep", "req17-a-960", AREA),
    ("A · Keep, opened", "req17-a-keep-960", AREA),
    ("B · a scenario made for the try, in Steps (decided panes)", "req17-b-960", AREA),
])
DIALOG = (440, 150, 1160, 850)
board("board-component-960", [
    ("Today · the board's dialog for one request (UX-30)", "req18-today-1600", DIALOG),
    ("A · the request's values listed; the changed ones ticked", "req18-a-1600", DIALOG),
    ("B · no dialog: one action, Edit inputs · Undo", "req18-b-1600", (248, 98, 968, 798)),
])
for name in ["req17-a-960", "req17-a-1600", "req17-a-keep-960", "req17-b-960", "req18-a-960", "req18-a-1600", "req18-today-960"]:
    shutil.copy(src / f"{name}.png", out / f"window-{name}.png")
