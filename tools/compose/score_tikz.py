"""Draws a score (data/scores/<id>.json) in TikZ: a form page (the sections, which part plays in which bar, and
the charts' targets under it when they exist), then the full score, four bars a system, one staff a part.

    python3 tools/compose/score_tikz.py data/scores/ID.json docs/scores/ID     writes ID.tex, ID.pdf (pdflatex)
"""
import json
import os
import subprocess
import sys
from fractions import Fraction as F

GAP = 1.5            # mm between staff lines
STAFF_SEP = 12.5     # mm from one staff's bottom line to the next's
BAR_W = 63.0         # mm a bar
LEFT = 30.0          # mm for the names, the clef and the key
BARS_A_SYSTEM = 4
KEYS = {  # sharps in each key's signature, as letters
    "G major": ["F"], "E minor": ["F"], "A major": ["F", "C", "G"], "D major": ["F", "C"],
}
LETTERS = "CDEFGAB"
SHARP_SPELL = [("C", 0), ("C", 1), ("D", 0), ("D", 1), ("E", 0), ("F", 0), ("F", 1), ("G", 0), ("G", 1),
               ("A", 0), ("A", 1), ("B", 0)]
# where the key signature's sharps sit (diatonic steps above the bottom line), treble and bass
SIG_STEPS = {"treble": {"F": 8, "C": 5, "G": 9}, "bass": {"F": 6, "C": 3, "G": 7}}
DRUM_STEP = {36: 0, 39: 4, 42: 6, 81: 9}   # kick low, clap, shaker, triangle high
DRUM_X = {39, 42, 81}                     # drawn with a cross


def spell(pitch):
    letter, sharp = SHARP_SPELL[pitch % 12]
    octave = pitch // 12 - 1
    return letter, sharp, octave


def step_of(pitch, clef):
    """Diatonic steps above the staff's bottom line (E4 in treble, G2 in bass)."""
    letter, _, octave = spell(pitch)
    d = octave * 7 + LETTERS.index(letter)
    base = 4 * 7 + LETTERS.index("E") if clef == "treble" else 2 * 7 + LETTERS.index("G")
    return d - base


def tex_escape(s):
    return s.replace("#", r"\#").replace("&", r"\&").replace("_", r"\_")


def chord_tex(sym):
    return sym.replace("#", r"$\sharp$").replace("b", r"$\flat$") if sym != "-" else ""


class Page:
    def __init__(self):
        self.out = []

    def add(self, s):
        self.out.append(s)


def treble_clef(x, y):
    """A treble clef from curves, in staff spaces: the curl round the G line, the loop above the staff, the tail
    below it ending in a dot."""
    u = GAP
    g = y + 1 * u   # the G line, the second from the bottom

    def pt(a, b):
        return f"({x + a * u:.2f},{g + b * u:.2f})"

    return (rf"\draw[line width=0.5pt] {pt(0.9, 0.2)} .. controls {pt(0.2, 0.2)} and {pt(0.2, 1.1)} .. {pt(0.9, 1.15)} "
            rf".. controls {pt(1.9, 1.15)} and {pt(1.9, -0.9)} .. {pt(0.9, -0.95)} "
            rf".. controls {pt(-0.4, -0.95)} and {pt(-0.5, 1.6)} .. {pt(0.9, 2.6)} "
            rf".. controls {pt(1.7, 3.2)} and {pt(1.7, 4.6)} .. {pt(1.1, 4.4)} "
            rf".. controls {pt(0.6, 4.2)} and {pt(0.7, 2.5)} .. {pt(1.15, -1.9)} "
            rf".. controls {pt(1.2, -2.6)} and {pt(0.5, -2.7)} .. {pt(0.35, -2.2)};"
            rf"\fill {pt(0.45, -2.15)} circle (0.42);")


def bass_clef(x, y):
    f = y + 3 * GAP
    return (rf"\fill ({x + 0.8:.2f},{f:.2f}) circle (0.55);"
            rf"\draw[line width=0.8pt] ({x + 0.5:.2f},{f + 0.2:.2f}) .. controls ({x + 0.8:.2f},{f + 1.6:.2f}) and "
            rf"({x + 3.6:.2f},{f + 1.8:.2f}) .. ({x + 3.4:.2f},{f - 0.6:.2f}) .. controls ({x + 3.2:.2f},{f - 2.6:.2f}) and "
            rf"({x + 1.6:.2f},{f - 3.8:.2f}) .. ({x + 0.4:.2f},{f - 4.4:.2f});"
            rf"\fill ({x + 4.3:.2f},{f + 0.75:.2f}) circle (0.3); \fill ({x + 4.3:.2f},{f - 0.75:.2f}) circle (0.3);")


def sharp_sign(x, y):
    return (rf"\draw[line width=0.3pt] ({x - 0.25:.2f},{y - 1.1:.2f}) -- ({x - 0.25:.2f},{y + 1.2:.2f}) "
            rf"({x + 0.35:.2f},{y - 1.2:.2f}) -- ({x + 0.35:.2f},{y + 1.1:.2f});"
            rf"\draw[line width=0.7pt] ({x - 0.65:.2f},{y - 0.25:.2f}) -- ({x + 0.75:.2f},{y + 0.15:.2f}) "
            rf"({x - 0.65:.2f},{y + 0.45:.2f}) -- ({x + 0.75:.2f},{y + 0.85:.2f});")


def natural_sign(x, y):
    return (rf"\draw[line width=0.3pt] ({x - 0.35:.2f},{y + 1.2:.2f}) -- ({x - 0.35:.2f},{y - 0.4:.2f}) "
            rf"({x + 0.35:.2f},{y + 0.4:.2f}) -- ({x + 0.35:.2f},{y - 1.2:.2f});"
            rf"\draw[line width=0.7pt] ({x - 0.35:.2f},{y - 0.4:.2f}) -- ({x + 0.35:.2f},{y - 0.1:.2f}) "
            rf"({x - 0.35:.2f},{y + 0.1:.2f}) -- ({x + 0.35:.2f},{y + 0.4:.2f});")


def quarter_rest(x, y):
    m = y + 2 * GAP
    return (rf"\draw[line width=0.9pt] ({x - 0.3:.2f},{m + 2.2:.2f}) -- ({x + 0.5:.2f},{m + 1.0:.2f}) -- "
            rf"({x - 0.3:.2f},{m:.2f}) -- ({x + 0.5:.2f},{m - 1.1:.2f}) .. controls ({x - 0.6:.2f},{m - 1.4:.2f}) and "
            rf"({x - 0.2:.2f},{m - 2.4:.2f}) .. ({x + 0.3:.2f},{m - 2.4:.2f});")


def bar_rest(x, y):
    return rf"\fill ({x - 1.6:.2f},{y + 3 * GAP - 0.75:.2f}) rectangle ({x + 1.6:.2f},{y + 3 * GAP:.2f});"


def draw_staff(pg, x0, y, x1, clef, key, name):
    lines = 1 if clef == "perc" else 5
    for k in range(lines):
        yy = y + (k * GAP if lines == 5 else 2 * GAP)
        pg.add(rf"\draw[line width=0.25pt] ({x0:.2f},{yy:.2f}) -- ({x1:.2f},{yy:.2f});")
    pg.add(rf"\node[anchor=east,font=\scriptsize\sffamily] at ({x0 - 1:.2f},{y + 2 * GAP:.2f}) {{{tex_escape(name)}}};")
    if clef == "treble":
        pg.add(treble_clef(x0 + 1.2, y))
    elif clef == "bass":
        pg.add(bass_clef(x0 + 1.0, y))
    else:
        pg.add(rf"\fill ({x0 + 1.5:.2f},{y + 0.5:.2f}) rectangle ({x0 + 2.1:.2f},{y + 3.5:.2f});"
               rf"\fill ({x0 + 2.9:.2f},{y + 0.5:.2f}) rectangle ({x0 + 3.5:.2f},{y + 3.5:.2f});")
    if clef in SIG_STEPS:
        for k, letter in enumerate(KEYS[key]):
            pg.add(sharp_sign(x0 + 7.2 + 1.6 * k, y + SIG_STEPS[clef][letter] * GAP / 2))
    if clef != "perc":
        pg.add(rf"\node[font=\bfseries\footnotesize] at ({x0 + 12.4:.2f},{y + 3 * GAP:.2f}) {{3}};"
               rf"\node[font=\bfseries\footnotesize] at ({x0 + 12.4:.2f},{y + 1 * GAP:.2f}) {{4}};")


def note_glyph(pg, x, y, steps, length, clef, sharp_need, stem_up, drum=None):
    """Heads at `steps`, a stem, a dot; returns the stem's end for beaming."""
    hollow = length >= 2
    dotted = length in (F(3), F(3, 2), F(3, 4))
    for st in steps:
        yy = y + st * GAP / 2
        if drum is not None and drum in DRUM_X:
            pg.add(rf"\draw[line width=0.5pt] ({x - 0.65:.2f},{yy - 0.55:.2f}) -- ({x + 0.65:.2f},{yy + 0.55:.2f}) "
                   rf"({x - 0.65:.2f},{yy + 0.55:.2f}) -- ({x + 0.65:.2f},{yy - 0.55:.2f});")
        elif hollow:
            pg.add(rf"\draw[line width=0.45pt,rotate around={{20:({x:.2f},{yy:.2f})}}] ({x:.2f},{yy:.2f}) "
                   rf"ellipse (0.92 and 0.6);")
        else:
            pg.add(rf"\fill[rotate around={{20:({x:.2f},{yy:.2f})}}] ({x:.2f},{yy:.2f}) ellipse (0.92 and 0.62);")
        if dotted:
            dy = 0 if st % 2 else GAP / 4
            pg.add(rf"\fill ({x + 1.6:.2f},{yy + dy:.2f}) circle (0.25);")
        if clef != "perc":
            # ledger lines
            for led in range(10, st + 1, 2):
                pg.add(rf"\draw[line width=0.3pt] ({x - 1.4:.2f},{y + led * GAP / 2:.2f}) -- ({x + 1.4:.2f},{y + led * GAP / 2:.2f});")
            for led in range(-2, st - 1, -2):
                pg.add(rf"\draw[line width=0.3pt] ({x - 1.4:.2f},{y + led * GAP / 2:.2f}) -- ({x + 1.4:.2f},{y + led * GAP / 2:.2f});")
    for st, kind in sharp_need:
        yy = y + st * GAP / 2
        pg.add(sharp_sign(x - 2.2, yy) if kind == "sharp" else natural_sign(x - 2.2, yy))
    if length >= 3 and not dotted or length >= 6:
        return None
    lo, hi = min(steps), max(steps)
    if stem_up:
        sx, sy0, sy1 = x + 0.85, y + lo * GAP / 2 + 0.2, y + hi * GAP / 2 + 5.2
    else:
        sx, sy0, sy1 = x - 0.85, y + hi * GAP / 2 - 0.2, y + lo * GAP / 2 - 5.2
    pg.add(rf"\draw[line width=0.35pt] ({sx:.2f},{sy0:.2f}) -- ({sx:.2f},{sy1:.2f});")
    return (sx, sy1)


def flag(pg, end, up):
    sx, sy = end
    d = -1 if up else 1
    pg.add(rf"\draw[line width=0.6pt] ({sx:.2f},{sy:.2f}) .. controls ({sx + 0.4:.2f},{sy + 1.6 * d:.2f}) and "
           rf"({sx + 2.0:.2f},{sy + 1.8 * d:.2f}) .. ({sx + 1.4:.2f},{sy + 3.6 * d:.2f});")


def draw_part_bar(pg, part, events, bar, x, y, key, bar_beats, tpb):
    """One part's notes in one bar. `events`: (start tick in the bar, ticks, [pitches]) after splitting at bar lines;
    `tied` marks a piece carried in from the bar before."""
    clef = part["clef"]
    sig = set(KEYS[key])
    beat_w = BAR_W / bar_beats
    if not events:
        pg.add(bar_rest(x + BAR_W / 2, y))
        return []
    # empty beats: a quarter rest each
    busy = [False] * bar_beats
    for t, n, _, _ in events:
        for b in range(bar_beats):
            if t < (b + 1) * tpb and t + n > b * tpb:
                busy[b] = True
    for b in range(bar_beats):
        if not busy[b] and clef != "perc":
            pg.add(quarter_rest(x + (b + 0.45) * beat_w, y))
    carried = {}
    groups = {}  # beat -> eighth/triplet stems to beam
    # a beat's short notes share one stem direction: up when they sit low on the staff
    beat_up = {}
    for b in range(bar_beats):
        short = [p for t, n, ps, _ in events if t // tpb == b and n < tpb for p in ps]
        if short and clef != "perc":
            beat_up[b] = sum(step_of(p, clef) for p in short) / len(short) < 4
    tie_ends = []
    for t, n, pitches, tied in events:
        length = F(n, tpb)
        nx = x + 3 + (t / tpb) * beat_w * 0.92
        if clef == "perc":
            steps = [DRUM_STEP.get(p, 4) for p in pitches]
            need = []
        else:
            steps = [step_of(p, clef) for p in pitches]
            need = []
            for p, st in zip(pitches, steps):
                letter, sharp, _ = spell(p)
                want = "sharp" if sharp else ("natural" if letter in sig else None)
                in_key = (sharp and letter in sig) or (not sharp and letter not in sig)
                prev = carried.get(st)
                if (not in_key and prev != want) or (in_key and prev is not None and prev != want):
                    need.append((st, want))
                carried[st] = want
        up = beat_up.get(t // tpb, (sum(steps) / len(steps)) < 4)
        drum = pitches[0] if clef == "perc" else None
        end = note_glyph(pg, nx, y, steps, length, clef, need, up, drum)
        if tied:
            tie_ends.append((nx, y + steps[0] * GAP / 2, up))
        if end and length < 1:
            beat = t // tpb
            groups.setdefault((beat, up), []).append((end, length, t))
    for (beat, up), stems in groups.items():
        triplet = any(l == F(1, 3) for _, l, _ in stems)
        if len(stems) == 1:
            flag(pg, stems[0][0], up)
            continue
        (x0, y0), (x1, y1) = stems[0][0], stems[-1][0]
        ybeam = max(s[0][1] for s in stems) if up else min(s[0][1] for s in stems)
        for (sx, sy), _, _ in stems:
            pg.add(rf"\draw[line width=0.35pt] ({sx:.2f},{sy:.2f}) -- ({sx:.2f},{ybeam:.2f});")
        pg.add(rf"\draw[line width=1.6pt] ({x0:.2f},{ybeam:.2f}) -- ({x1:.2f},{ybeam:.2f});")
        if any(l == F(1, 4) for _, l, _ in stems):
            off = -0.9 if up else 0.9
            pg.add(rf"\draw[line width=1.3pt] ({x0:.2f},{ybeam + off:.2f}) -- ({x1:.2f},{ybeam + off:.2f});")
        if triplet:
            ty = ybeam + (1.4 if up else -1.4)
            pg.add(rf"\node[font=\tiny\itshape] at ({(x0 + x1) / 2:.2f},{ty:.2f}) {{3}};")
    return tie_ends


def split(part, bar_ticks):
    """Each part's chords (notes starting together with one length), split at bar lines: bar -> pieces."""
    chords = {}
    for t, n, p, v in part["notes"]:
        chords.setdefault((t, n), []).append(p)
    out = {}
    for (t, n), ps in sorted(chords.items()):
        tied = False
        while n > 0:
            bar = t // bar_ticks
            room = (bar + 1) * bar_ticks - t
            take = min(n, room)
            out.setdefault(bar, []).append((t - bar * bar_ticks, take, sorted(ps), tied))
            t, n, tied = t + take, n - take, True
    return out


def form_page(pg, score, charts, w, h):
    """The form: sections across the top, a lane a part (dark where it plays), the charts' targets below."""
    bars = score["bars"]
    x0, x1 = 34.0, w - 10
    bw = (x1 - x0) / bars
    top = h - 30
    pg.add(rf"\node[anchor=west,font=\Large\sffamily\bfseries] at (12,{h - 14}) {{{tex_escape(score['title'])}}};")
    pg.add(rf"\node[anchor=west,font=\small\sffamily] at (12,{h - 20}) "
           rf"{{A waltz in {score['bar_beats']}/4 at {score['bpm']} beats a minute: the form, the band, the charts}};")
    for s in score["sections"]:
        a, b = x0 + s["bar"] * bw, x0 + (s["bar"] + s["bars"]) * bw
        pg.add(rf"\draw[fill=black!8] ({a:.2f},{top:.2f}) rectangle ({b:.2f},{top + 7:.2f});")
        pg.add(rf"\node[font=\scriptsize\sffamily] at ({(a + b) / 2:.2f},{top + 3.5:.2f}) {{{tex_escape(s['name'])}}};")
        pg.add(rf"\node[font=\tiny\sffamily,anchor=north] at ({(a + b) / 2:.2f},{top - 0.4:.2f}) {{{s['key']}}};")
    bar_ticks = score["bar_beats"] * score["ticks_per_beat"]
    y = top - 8
    for part in score["parts"]:
        y -= 6.2
        pg.add(rf"\node[anchor=east,font=\scriptsize\sffamily] at ({x0 - 1.5:.2f},{y + 2:.2f}) {{{tex_escape(part['name'])}}};")
        weight = part["chart_weight"]
        played = {}
        for t, n, p, v in part["notes"]:
            played[t // bar_ticks] = played.get(t // bar_ticks, 0) + 1
        for b, count in played.items():
            shade = min(85, 20 + count * 6)
            pg.add(rf"\fill[black!{shade}] ({x0 + b * bw:.2f},{y:.2f}) rectangle ({x0 + (b + 1) * bw - 0.15:.2f},{y + 4:.2f});")
        pg.add(rf"\node[anchor=west,font=\tiny\sffamily] at ({x1 + 0.8:.2f},{y + 2:.2f}) {{{weight}}};")
    pg.add(rf"\node[anchor=west,font=\tiny\sffamily] at ({x1 - 2:.2f},{top - 4:.2f}) {{weight}};")
    if charts:
        y -= 10
        pg.add(rf"\node[anchor=west,font=\small\sffamily\bfseries] at (12,{y + 6:.2f}) {{The charts: a tick a target, a bar a hold}};")
        beat_us = 60_000_000 // score["bpm"]
        for name, c in charts:
            y -= 7
            pg.add(rf"\node[anchor=east,font=\scriptsize\sffamily] at ({x0 - 1.5:.2f},{y + 2:.2f}) {{{name} ({len(c['targets'])})}};")
            for tg in c["targets"]:
                bar = (tg["t"] - score["start_us"]) / (beat_us * score["bar_beats"])
                xx = x0 + bar * bw
                if tg.get("hold"):
                    xe = x0 + (tg["hold"]["end"] - score["start_us"]) / (beat_us * score["bar_beats"]) * bw
                    pg.add(rf"\fill[black!45] ({xx:.2f},{y + 1:.2f}) rectangle ({xe:.2f},{y + 3:.2f});")
                pg.add(rf"\draw[line width=0.2pt] ({xx:.2f},{y:.2f}) -- ({xx:.2f},{y + 4:.2f});")
    for b in range(0, bars + 1, 8):
        pg.add(rf"\node[font=\tiny\sffamily] at ({x0 + b * bw:.2f},{y - 4:.2f}) {{{b + 1}}};")


def main(src, out_base):
    score = json.load(open(src))
    bar_beats, tpb = score["bar_beats"], score["ticks_per_beat"]
    bar_ticks = bar_beats * tpb
    W, H = 297, 210
    pieces = [split(p, bar_ticks) for p in score["parts"]]
    key_of = {}
    for s in score["sections"]:
        for b in range(s["bar"], s["bar"] + s["bars"]):
            key_of[b] = s["key"]
    section_at = {s["bar"]: s["name"] for s in score["sections"]}
    charts = []
    for k, name in enumerate(["easy", "normal", "hard", "expert", "master"]):
        path = os.path.join(os.path.dirname(src), "..", "charts", f"{score['id']}-{name}.json")
        if os.path.exists(path):
            charts.append((name.capitalize(), json.load(open(path))))
    pages = []
    pg = Page()
    form_page(pg, score, charts, W, H)
    pages.append(pg)
    for first in range(0, score["bars"], BARS_A_SYSTEM):
        pg = Page()
        key = key_of[first]
        x0 = LEFT
        top = H - 22
        n = len(score["parts"])
        ys = [top - 8 - k * STAFF_SEP for k in range(n)]
        x_end = x0 + 16 + BARS_A_SYSTEM * BAR_W
        pg.add(rf"\node[anchor=west,font=\small\sffamily\bfseries] at (12,{H - 10}) {{{tex_escape(score['title'])}}};")
        pg.add(rf"\node[anchor=east,font=\scriptsize\sffamily] at ({W - 10},{H - 10}) "
               rf"{{bars {first + 1}--{first + BARS_A_SYSTEM} \quad {key} \quad $\frac{{3}}{{4}}$, $\bullet$ = {score['bpm']}}};")
        for part, y in zip(score["parts"], ys):
            draw_staff(pg, x0, y, x_end, part["clef"], key, part["name"])
        # the time signature on the first system of a section with one
        pg.add(rf"\draw[line width=0.6pt] ({x0:.2f},{ys[-1] + 2 * GAP:.2f}) -- ({x0:.2f},{ys[0] + 4 * GAP:.2f});")
        for k in range(BARS_A_SYSTEM):
            bar = first + k
            bx = x0 + 16 + k * BAR_W
            line_w = 0.6 if bar + 1 in section_at or k == BARS_A_SYSTEM - 1 else 0.3
            pg.add(rf"\draw[line width={line_w}pt] ({bx + BAR_W:.2f},{ys[-1] + 2 * GAP:.2f}) -- "
                   rf"({bx + BAR_W:.2f},{ys[0] + 4 * GAP:.2f});")
            pg.add(rf"\node[font=\tiny\sffamily,anchor=south west] at ({bx:.2f},{ys[0] + 4 * GAP + 6:.2f}) {{{bar + 1}}};")
            if bar in section_at:
                pg.add(rf"\node[draw,font=\scriptsize\sffamily\bfseries,anchor=south west,inner sep=1.2pt] at "
                       rf"({bx + 5:.2f},{ys[0] + 4 * GAP + 5.6:.2f}) {{{tex_escape(section_at[bar])}}};")
            sym = score["chords"][bar]
            pg.add(rf"\node[font=\footnotesize\sffamily\bfseries,anchor=south west] at ({bx + 2:.2f},{ys[0] + 4 * GAP + 1.4:.2f}) "
                   rf"{{{chord_tex(sym)}}};")
            for part, y, pc in zip(score["parts"], ys, pieces):
                ties = draw_part_bar(pg, part, pc.get(bar, []), bar, bx, y, key_of[bar], bar_beats, tpb)
                for (tx, ty, up) in ties:
                    d = -1 if up else 1
                    pg.add(rf"\draw[line width=0.4pt] ({bx - 10:.2f},{ty + 1.0 * d:.2f}) .. controls "
                           rf"({bx - 6:.2f},{ty + 2.4 * d:.2f}) and ({tx - 4:.2f},{ty + 2.4 * d:.2f}) .. ({tx - 0.8:.2f},{ty + 1.0 * d:.2f});")
        pages.append(pg)
    tex = [r"\documentclass{article}", r"\usepackage[paperwidth=297mm,paperheight=210mm,margin=0mm]{geometry}",
           r"\usepackage{tikz}", r"\pagestyle{empty}", r"\begin{document}"]
    for pg in pages:
        tex.append(r"\noindent\begin{tikzpicture}[x=1mm,y=1mm]")
        tex.append(rf"\useasboundingbox (0,0) rectangle ({W},{H - 1});")
        tex.extend(pg.out)
        tex.append(r"\end{tikzpicture}\newpage")
    tex.append(r"\end{document}")
    os.makedirs(os.path.dirname(out_base) or ".", exist_ok=True)
    with open(out_base + ".tex", "w") as f:
        f.write("\n".join(tex) + "\n")
    d, b = os.path.dirname(out_base) or ".", os.path.basename(out_base)
    r = subprocess.run(["pdflatex", "-interaction=nonstopmode", "-halt-on-error", b + ".tex"], cwd=d,
                       capture_output=True, text=True)
    if r.returncode:
        print(r.stdout[-3000:])
        sys.exit(1)
    for ext in (".aux", ".log"):
        try:
            os.remove(out_base + ext)
        except FileNotFoundError:
            pass
    print(f"{out_base}.pdf: {len(pages)} pages")


if __name__ == "__main__":
    main(*sys.argv[1:])
