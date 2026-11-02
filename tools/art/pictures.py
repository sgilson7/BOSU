"""Countless Days' pictures, in Lowballer's style (styles/lowballer/STYLE.md).

Each entry maps a name to a function that takes a seeded random.Random and
returns an artlib.Fig. The name is the output path under web/art/ without
".png". Colours are role names from the style pack, never colour values.
Coordinates are centimetres, origin at the bottom left, y up.

- title: the landing page. A street under a mustard sky, bunting between
  the buildings, an oversized boombox on the pavement with the game's
  targets floating above it like balloons, notes in the air, cassettes and
  records at its feet. The upper left stays calm for the title.
- icon: one target with its approach ring, for the browser tab.

No picture holds a letter, word or number: the page sets every word.
"""

import math

from artlib import Fig
import style

motifs = style.load().motifs()

WALLS = ["wallA", "wallB", "wallC", "wallD", "wallE", "wallF", "greenC", "greenA", "pink", "salmon"]
FLAGS = ["plum", "gold", "teal", "coral", "pink", "sage"]
STREET = 1.7  # the kerb line


def building(fig, rng, x0, x1, top, wall):
    """A building from the kerb to `top`, with a cornice, windows and some
    rooftop clutter."""
    fig.rect(x0, STREET, x1, top, wall)
    fig.rect(x0 - 0.12, top - 0.05, x1 + 0.12, top + 0.28, wall)
    cols = max(1, int((x1 - x0 - 0.3) / 0.62))
    rows = max(1, int((top - STREET - 0.5) / 0.72))
    gap = (x1 - x0 - cols * 0.34) / (cols + 1)
    for i in range(cols):
        for j in range(rows):
            wx = x0 + gap + i * (0.34 + gap)
            wy = top - 0.55 - j * 0.72
            fill = "windark" if rng.random() < 0.25 else "window"
            fig.rect(wx, wy - 0.44, wx + 0.34, wy, fill)
    clutter = rng.random()
    mid = (x0 + x1) / 2 + rng.uniform(-0.3, 0.3)
    if clutter < 0.35:
        # an antenna
        fig.stroke([(mid, top + 0.28), (mid, top + 1.3)])
        fig.stroke([(mid - 0.3, top + 1.0), (mid + 0.3, top + 1.0)])
        fig.stroke([(mid - 0.2, top + 0.75), (mid + 0.2, top + 0.75)])
    elif clutter < 0.6:
        # a water tank on legs
        fig.stroke([(mid - 0.25, top + 0.28), (mid - 0.25, top + 0.6)], width=0.9)
        fig.stroke([(mid + 0.25, top + 0.28), (mid + 0.25, top + 0.6)], width=0.9)
        fig.rect(mid - 0.35, top + 0.6, mid + 0.35, top + 1.2, "wood", r=0.08)
    elif clutter < 0.8:
        # a rooftop speaker box
        fig.rect(mid - 0.3, top + 0.28, mid + 0.3, top + 0.95, "dark")
        fig.circle(mid, top + 0.55, 0.17, "metal")
        fig.circle(mid, top + 0.83, 0.07, "metal")


def bunting(fig, rng, x0, y0, x1, y1, sag=0.45):
    """A string of pennants between two points, sagging in the middle."""
    n = int(math.hypot(x1 - x0, y1 - y0) / 0.62)
    pts = []
    for k in range(n + 1):
        t = k / n
        pts.append((x0 + (x1 - x0) * t, y0 + (y1 - y0) * t - sag * 4 * t * (1 - t)))
    fig.stroke(pts, width=0.9, smooth=True)
    for k in range(1, n):
        (ax, ay), (bx, by) = pts[k - 1], pts[k]
        mx, my = (ax + bx) / 2, (ay + by) / 2
        fig.poly([(mx - 0.2, my), (mx + 0.2, my), (mx, my - 0.45)], FLAGS[(k + rng.randrange(2)) % len(FLAGS)])


def target(fig, x, y, r, fill, ring=True, string_to=None):
    """One of the game's targets: a filled circle, its approach ring, and a
    balloon string down to a point."""
    if string_to:
        fig.stroke([(x, y - r), (x + 0.15, y - r - 0.5), string_to], width=0.8, smooth=True)
    if ring:
        fig.stroke([(x + 1.75 * r * math.cos(a / 24 * math.tau), y + 1.75 * r * math.sin(a / 24 * math.tau)) for a in range(24)], width=1.2, closed=True, smooth=True, tension=0.5)
    fig.circle(x, y, r, fill)
    fig.circle(x - 0.32 * r, y + 0.32 * r, 0.18 * r, "cream")


def note(fig, x, y, s=1.0, fill="ink", double=False):
    """A music note: a tilted head and a stem, as a shape, not a letter."""
    fig.ellipse(x, y, 0.26 * s, 0.18 * s, fill, rot=25)
    fig.stroke([(x + 0.22 * s, y + 0.05 * s), (x + 0.22 * s, y + 1.0 * s)], width=1.6)
    if double:
        fig.ellipse(x + 0.65 * s, y + 0.15 * s, 0.26 * s, 0.18 * s, fill, rot=25)
        fig.stroke([(x + 0.87 * s, y + 0.2 * s), (x + 0.87 * s, y + 1.15 * s)], width=1.6)
        fig.poly([(x + 0.22 * s, y + 1.0 * s), (x + 0.87 * s, y + 1.15 * s), (x + 0.87 * s, y + 0.98 * s), (x + 0.22 * s, y + 0.83 * s)], "ink")
    else:
        fig.stroke([(x + 0.22 * s, y + 1.0 * s), (x + 0.5 * s, y + 0.75 * s), (x + 0.45 * s, y + 0.5 * s)], width=1.6, smooth=True)


def boombox(fig, x, y, s):
    """The subject: a retro boombox standing on the pavement, (x, y) the
    middle of its base, s its scale (s=1 is 6 cm wide)."""
    with fig.frame(x, y, s=s):
        # handle
        fig.rect(-2.0, 3.25, -1.7, 4.1, "metal")
        fig.rect(1.7, 3.25, 2.0, 4.1, "metal")
        fig.rect(-2.0, 3.85, 2.0, 4.15, "metal", r=0.12)
        # body and top panel
        fig.rect(-3.0, 0.15, 3.0, 3.35, "plum", r=0.35)
        fig.rect(-2.8, 2.75, 2.8, 3.15, "plumhi", r=0.12)
        for k in range(6):
            fig.rect(-2.5 + k * 0.42, 2.85, -2.2 + k * 0.42, 3.05, ["gold", "coral", "teal", "cream", "sage", "pink"][k])
        # feet
        fig.rect(-2.6, 0.0, -2.0, 0.2, "dark")
        fig.rect(2.0, 0.0, 2.6, 0.2, "dark")
        # two speakers
        for cx in (-1.85, 1.85):
            fig.circle(cx, 1.45, 1.0, "dark")
            fig.circle(cx, 1.45, 0.78, "darkhi")
            fig.circle(cx, 1.45, 0.42, "gold")
            fig.circle(cx, 1.45, 0.15, "golddk")
        # the cassette deck in the middle
        fig.rect(-0.75, 0.55, 0.75, 2.45, "cream", r=0.1)
        fig.rect(-0.6, 1.25, 0.6, 2.25, "coral", r=0.08)
        fig.rect(-0.45, 1.5, 0.45, 1.95, "cream", r=0.05)
        fig.circle(-0.25, 1.72, 0.13, "dark")
        fig.circle(0.25, 1.72, 0.13, "dark")
        for k in range(3):
            fig.rect(-0.55 + k * 0.4, 0.7, -0.25 + k * 0.4, 1.0, ["sage", "gold", "teal"][k], r=0.04)


def cassette(fig, x, y, rot, fill):
    with fig.frame(x, y, s=0.42, rot=rot):
        fig.rect(-1.0, -0.62, 1.0, 0.62, fill, r=0.1)
        fig.rect(-0.7, -0.15, 0.7, 0.42, "cream", r=0.05)
        fig.circle(-0.35, 0.13, 0.13, "dark")
        fig.circle(0.35, 0.13, 0.13, "dark")
        fig.poly([(-0.6, -0.62), (0.6, -0.62), (0.45, -0.35), (-0.45, -0.35)], "darkhi")


def record(fig, x, y, s=0.55, label="coral"):
    fig.ellipse(x, y, 1.0 * s, 0.32 * s, "dark")
    fig.ellipse(x, y, 0.62 * s, 0.2 * s, "darkhi")
    fig.ellipse(x, y, 0.28 * s, 0.09 * s, label)


def title(rng):
    fig = Fig("scene")
    W, H = fig.size
    motifs.sky_bands(fig)
    # clouds right of the calm area, and one small one low in it
    motifs.cloud(fig, 9.2, 9.3, 0.8)
    motifs.cloud(fig, 15.6, 9.55, 0.95)
    motifs.cloud(fig, 12.4, 8.2, 0.55)
    # the back row of buildings, thinning to the left
    x = 3.4
    k = 0
    # keep the left third low and bare, under the title and buttons
    while x < W + 0.5:
        w = rng.uniform(1.6, 2.5)
        top = rng.uniform(4.6, 6.6) if x > 7.5 else rng.uniform(2.9, 3.6)
        building(fig, rng, x, x + w, top, WALLS[k % len(WALLS)])
        x += w + rng.uniform(-0.15, 0.1)
        k += 3
    # a low wall of shops on the far left, under the calm sky
    building(fig, rng, -0.3, 1.9, 2.8, "wallF")
    building(fig, rng, 1.9, 3.5, 3.2, "greenA")
    # bunting across the street
    bunting(fig, rng, 7.4, 5.2, 9.9, 5.9, sag=0.35)
    bunting(fig, rng, 10.1, 6.3, 19.5, 5.4, sag=0.6)
    # pavement and road
    fig.rect(-0.2, -0.2, W + 0.2, STREET, "tan")
    fig.rect(-0.2, -0.2, W + 0.2, 0.75, "ground")
    for k in range(9):
        fig.rect(0.5 + k * 2.2, 0.32, 1.6 + k * 2.2, 0.45, "butter")
    # balloons: the game's targets, tied to the boombox's handle
    bx, by, bs = 13.2, 1.35, 0.78
    tie = (bx, by + 4.15 * bs)
    for (tx, ty, r, fill) in [(10.4, 8.0, 0.55, "coral"), (12.0, 8.9, 0.62, "teal"), (14.2, 8.6, 0.58, "gold"),
                              (15.9, 7.6, 0.5, "plumhi"), (11.0, 6.7, 0.42, "sage")]:
        target(fig, tx, ty, r, fill, string_to=tie)
    # notes drifting from the speakers
    note(fig, 8.6, 4.3, 0.9, "coral")
    note(fig, 9.6, 5.4, 0.7, "gold", double=True)
    note(fig, 17.1, 4.6, 0.9, "teal", double=True)
    note(fig, 18.2, 5.9, 0.7, "pink")
    boombox(fig, bx, by, bs)
    # at its feet
    cassette(fig, 9.3, 0.95, -12, "teal")
    cassette(fig, 10.4, 1.1, 8, "gold")
    cassette(fig, 17.0, 0.9, 15, "coral")
    record(fig, 18.4, 1.25, 0.6, "gold")
    record(fig, 7.6, 1.0, 0.5, "teal")
    motifs.calm_left_third(fig, top=0.6)
    return fig


def icon(rng):
    fig = Fig("icon")
    W, H = fig.size
    fig.rect(0.06, 0.06, W - 0.06, H - 0.06, "sky", r=0.35)
    target(fig, W / 2, H / 2, 0.42, "coral")
    return fig


PICTURES = {"title": title, "icon": icon}
