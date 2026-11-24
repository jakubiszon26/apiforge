#!/usr/bin/env python3
"""Composes the DIAG-23 comparison board from the mock's captures.
   diag23_board.py CAPTURE_DIR CASE SIZE OUT.png"""
import json, math, sys
from PIL import Image, ImageDraw, ImageFont

src, case, size, out = sys.argv[1:5]
FONT = "/usr/share/fonts/TTF/DejaVuSans.ttf"
BOLD = "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"
import glob
def pick(names):
    for n in names:
        hits = glob.glob(f"/usr/share/fonts/**/{n}", recursive=True)
        if hits: return hits[0]
    raise SystemExit("no font")
FONT = pick(["DejaVuSans.ttf", "NotoSans-Regular.ttf", "LiberationSans-Regular.ttf"])
BOLD = pick(["DejaVuSans-Bold.ttf", "NotoSans-Bold.ttf", "LiberationSans-Bold.ttf"])
big = size == "1600"
f_cap = ImageFont.truetype(BOLD, 20 if big else 15)
f_txt = ImageFont.truetype(FONT, 16 if big else 12)
f_tag = ImageFont.truetype(BOLD, 14 if big else 11)
f_head = ImageFont.truetype(BOLD, 24 if big else 17)

GREEN, ORANGE, BLUE, RED, GREY = (22, 140, 60), (217, 119, 6), (37, 99, 235), (220, 38, 38), (90, 90, 100)
KEPT_TODAY = None

def load(option):
    return json.load(open(f"{src}/diag23-case{case}-{option}-{size}.json"))

def dashed(draw, box, colour, width=2, dash=7):
    x0, y0, x1, y1 = box
    for (ax, ay, bx, by) in [(x0, y0, x1, y0), (x1, y0, x1, y1), (x1, y1, x0, y1), (x0, y1, x0, y0)]:
        length = math.hypot(bx - ax, by - ay)
        n = max(1, int(length // dash))
        for i in range(0, n, 2):
            a, b = i / n, min(1, (i + 1) / n)
            draw.line([(ax + (bx - ax) * a, ay + (by - ay) * a), (ax + (bx - ax) * b, ay + (by - ay) * b)], fill=colour, width=width)

def arrow(draw, a, b, colour, width=3):
    draw.line([a, b], fill=colour, width=width)
    ang = math.atan2(b[1] - a[1], b[0] - a[0])
    for d in (2.6, -2.6):
        draw.line([b, (b[0] + 10 * math.cos(ang + d), b[1] + 10 * math.sin(ang + d))], fill=colour, width=width)

def tag(draw, at, text, colour):
    w = draw.textlength(text, font=f_tag)
    h = f_tag.size + 4
    x, y = at
    draw.rectangle([x, y - h, x + w + 8, y], fill=colour)
    draw.text((x + 4, y - h + 1), text, font=f_tag, fill="white")

today = load("today")
bx0, by0, bx1, by1 = today["board"]
zoom = today["zoom"]
W, H = int(bx1 - bx0), int(by1 - by0)
before_cards = {c["handle"]: c for c in today["before"] if c["handle"]}
kept_today = {c["handle"] for c in today["after"] if c["handle"] in before_cards and c["world"] == before_cards[c["handle"]]["world"]}

def local(rect):
    return [rect[0] - bx0, rect[1] - by0, rect[2] - bx0, rect[3] - by0]

def dist(c, o):
    return math.hypot(c["world"][0] - o["world"][0], c["world"][1] - o["world"][1])

def panel(option):
    notes = []
    if option == "before":
        image = Image.open(f"{src}/diag23-case{case}-before-{size}.png").convert("RGB").crop((bx0, by0, bx1, by1))
        draw = ImageDraw.Draw(image)
        for c in today["before"]:
            if c["by_hand"]:
                r = local(c["rect"])
                draw.rectangle(r, outline=BLUE, width=3)
                tag(draw, (r[0], r[1]), "by hand", BLUE)
        return image, notes
    data = load(option)
    image = Image.open(f"{src}/diag23-case{case}-{option}-{size}.png").convert("RGB").crop((bx0, by0, bx1, by1))
    draw = ImageDraw.Draw(image)
    kept_moved = []
    for c in data["after"]:
        if not c["world"]:
            continue
        r = local(c["rect"])
        o = before_cards.get(c["handle"])
        if o is None:
            draw.rectangle(r, outline=GREEN, width=3)
            tag(draw, (r[0], r[1]), "new", GREEN)
        elif c["world"] != o["world"]:
            was_kept = c["handle"] in kept_today
            hand = o["by_hand"]
            if was_kept or hand:
                g = local(o["rect"])
                colour = BLUE if hand else RED
                dashed(draw, g, colour)
                if hand:
                    tag(draw, (g[0], g[3] + f_tag.size + 5), "was here, by hand", BLUE)
                arrow(draw, ((g[0] + g[2]) / 2, (g[1] + g[3]) / 2), ((r[0] + r[2]) / 2, (r[1] + r[3]) / 2), colour)
                if was_kept:
                    kept_moved.append((c["handle"], dist(c, o)))
                    tag(draw, (r[0], r[3] + f_tag.size + 6), f"{c['handle']}: {dist(c, o):.0f} px", RED)
            draw.rectangle(r, outline=RED if was_kept else ORANGE, width=2)
        elif o["by_hand"]:
            draw.rectangle(r, outline=BLUE, width=3)
            tag(draw, (r[0], r[1]), "by hand, kept", BLUE)
    # A card on a card.
    cards = [c for c in data["after"] if c["world"]]
    for i, a in enumerate(cards):
        for b in cards[i + 1:]:
            x0, y0 = max(a["rect"][0], b["rect"][0]), max(a["rect"][1], b["rect"][1])
            x1, y1 = min(a["rect"][2], b["rect"][2]), min(a["rect"][3], b["rect"][3])
            if x1 - x0 > 0.5 and y1 - y0 > 0.5:
                r = local([x0, y0, x1, y1])
                draw.rectangle([r[0] - 3, r[1] - 3, r[2] + 3, r[3] + 3], outline=RED, width=4)
                tag(draw, (r[0] - 3, r[3] + f_tag.size + 9), f"{a['handle']} on {b['handle']}", RED)
                notes.append(f"{a['handle']} on {b['handle']}: {(y1 - y0) / zoom:.0f} px")
    if kept_moved:
        lo, hi = min(d for _, d in kept_moved), max(d for _, d in kept_moved)
        notes.append(f"{len(kept_moved)} of {len(kept_today)} kept cards move {lo:.0f}-{hi:.0f} px (" + ", ".join(f"{h} {d:.0f}" for h, d in kept_moved) + ")")
    # The step after `profile` against `profile`'s row.
    new = next(c for c in data["after"] if c["world"] and c["handle"] not in before_cards)
    prof = next(c for c in data["after"] if c["handle"] == "profile")
    off = new["world"][1] - prof["world"][1]
    notes.append("new step in profile's row" if abs(off) < 1 else f"new step {abs(off):.0f} px above profile's row")
    return image, notes

step = "Sign in (a component call), cards as the board opens" if case == "1" else "HTTP request, Compact cards off"
titles = {
    "before": ("Before the add", "booking-day, tidied; token and rooms then placed by hand"),
    "today": ("Today (main) = C", "steps from the change on stand in the new tidy's frame; the others keep theirs"),
    "a": ("A · measured from the step they follow", "the same arrangement, shifted so that profile is where it stands"),
    "b": ("B · the whole board is tidied", "clean, and every card moves; places given by hand are lost"),
}
cap_h = (f_cap.size + f_txt.size * 2 + 26)
pad = 16
head_h = f_head.size + f_txt.size + 34
legend_h = f_txt.size + 24
sheet = Image.new("RGB", (pad * 3 + W * 2, head_h + 2 * (cap_h + H + pad) + legend_h), "white")
draw = ImageDraw.Draw(sheet)
draw.text((pad, 12), f"DIAG-23 · Add step after profile: {step}", font=f_head, fill=(20, 20, 30))
draw.text((pad, 18 + f_head.size), f"Window {size} x {'540' if size == '960' else '1000'}, cropped to the board, {zoom * 100:.0f} % in every panel. Distances in board px. Annotated captures of the mock.", font=f_txt, fill=GREY)
for index, option in enumerate(["before", "today", "a", "b"]):
    image, notes = panel(option)
    x = pad + (index % 2) * (W + pad)
    y = head_h + (index // 2) * (cap_h + H + pad)
    title, line = titles[option]
    draw.text((x, y), title, font=f_cap, fill=(20, 20, 30))
    draw.text((x, y + f_cap.size + 6), line, font=f_txt, fill=GREY)
    draw.text((x, y + f_cap.size + f_txt.size + 10), " · ".join(notes), font=f_txt, fill=RED if option in ("today", "b") else GREEN if option == "a" else GREY)
    sheet.paste(image, (x, y + cap_h))
    draw.rectangle([x - 1, y + cap_h - 1, x + W, y + cap_h + H], outline=(190, 190, 200))
    print(option, notes)
y = sheet.height - legend_h + 4
x = pad
for colour, text in [(GREEN, "new step"), (ORANGE, "moved by the edit (DIAG-06c: from the change to the end of the chain)"), (BLUE, "placed by hand"), (RED, "a card on a card, or a kept card moved (dashed: where it stood)")]:
    draw.rectangle([x, y + 2, x + 14, y + 14], outline=colour, width=3)
    draw.text((x + 20, y), text, font=f_txt, fill=(20, 20, 30))
    x += 34 + int(draw.textlength(text, font=f_txt))
sheet.save(out)
print(out, sheet.size)
