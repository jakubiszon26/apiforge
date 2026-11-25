"""REQ-01f mock-up: assembles the comparison board from the review captures
of `review_req01f`. Mock branch only.
python3 tools/mock/req01f-board.py <captures> <out>"""
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

src, out = Path(sys.argv[1]), Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
bold = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans-Bold.ttf", 15)
plain = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSans.ttf", 13)

# The first card of Steps, where the check was clicked, and Run results under
# it: the list and the top of the details. One scale in every panel.
STEPS = (8, 146, 952, 272)
RESULTS = (8, 404, 952, 700)
TITLE = [
    "REQ-01f · Failed is on in Run results; the passed check  status == 403  of No token is clicked in Steps",
    "A drawing of a proposal: A, B and C are drawn on a mock branch and are not built. 960 px wide, cropped to",
    "the card clicked in Steps (above) and to Run results (below).",
]
PANELS = [
    ("Today · Steps marks the check; Run results mark and describe Weight 31 kg, and say nothing", "today"),
    ("A · Failed gives way to All: the row clicked is marked and in view, with its details", "a"),
    ("B · Failed stays: the row is a greyed guest, listed with its request while it is selected", "b"),
    ("C · Failed stays and no row is marked: the details say why, with Show all", "c"),
]

gap, bar, head = 14, 28, 74
width = STEPS[2] - STEPS[0]
height = (STEPS[3] - STEPS[1]) + 6 + (RESULTS[3] - RESULTS[1])
sheet = Image.new(
    "RGB",
    (width + 2 * gap, head + len(PANELS) * (height + bar + gap) + gap),
    (236, 236, 233),
)
draw = ImageDraw.Draw(sheet)
draw.text((gap + 2, 12), TITLE[0], font=bold, fill=(30, 30, 30))
draw.text((gap + 2, 34), TITLE[1], font=plain, fill=(150, 40, 30))
draw.text((gap + 2, 52), TITLE[2], font=plain, fill=(150, 40, 30))
for index, (caption, name) in enumerate(PANELS):
    capture = Image.open(src / f"req01f-{name}-board.png").convert("RGB")
    y = head + index * (height + bar + gap)
    draw.text((gap + 2, y + 5), caption, font=bold, fill=(30, 30, 30))
    top = capture.crop(STEPS)
    sheet.paste(top, (gap, y + bar))
    sheet.paste(capture.crop(RESULTS), (gap, y + bar + top.size[1] + 6))
    draw.rectangle((gap - 1, y + bar - 1, gap + width, y + bar + height), outline=(170, 170, 170))
sheet.save(out / "board-960.png")
