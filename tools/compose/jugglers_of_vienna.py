"""Jugglers of Vienna: the composition. An original waltz in 3/4 for an ocarina lead and a small café band
(harp, organ, bass, music box), and in its middle a juggling act: three-note cascades tossed from the marimba on
the left to the kalimba on the right, a ball a beat.

Writes data/scores/jugglers-of-vienna.json, the score everything else follows: tools/compose/score_tikz.py draws it,
tools/compose/make.sh plays it through the Vector37 engine, and the chart tool charts it (crates/core/src/score.rs).

    python3 tools/compose/jugglers_of_vienna.py
"""
import json
import os

TICKS = 12          # ticks a beat: eighths (6) and triplets (4) both fall on ticks
BAR = 3             # beats a bar
BPM = 160
START_US = 500_000  # where tick 0 is in the audio

NAMES = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def midi(name):
    """'F#5' -> 78."""
    pc = NAMES[name[0]]
    rest = name[1:]
    while rest and rest[0] in "#b":
        pc += 1 if rest[0] == "#" else -1
        rest = rest[1:]
    return 12 * (int(rest) + 1) + pc


QUALITY = {"": [0, 4, 7], "m": [0, 3, 7], "7": [0, 4, 7, 10], "m7": [0, 3, 7, 10], "dim": [0, 3, 6]}


def chord(sym):
    """'Em' -> (root pitch class, intervals); 'G/B' keeps its bass as the root's place."""
    sym = sym.split("/")[0]
    pc = NAMES[sym[0]]
    q = sym[1:]
    if q.startswith("#"):
        pc, q = pc + 1, q[1:]
    elif q.startswith("b"):
        pc, q = pc - 1, q[1:]
    return pc % 12, QUALITY[q]


def bass_pc(sym):
    if "/" in sym:
        return chord(sym.split("/")[1])[0]
    return chord(sym)[0]


def near(pc, low):
    """The pitch of class pc at or above `low`."""
    p = low + (pc - low) % 12
    return p


def transpose(sym, by):
    if sym == "-":
        return sym
    names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
    flats = {"A#": "Bb", "D#": "Eb", "G#": "G#", "C#": "C#", "F#": "F#"}

    def one(s):
        pc, _ = chord(s)
        q = s[1:]
        if q[:1] in "#b" and q:
            q = q[1:]
        return names[(pc + by) % 12] + q

    parts = sym.split("/")
    out = "/".join(one(p) for p in parts)
    return out


class Score:
    def __init__(self):
        self.parts = {}
        self.sections = []
        self.chords = []   # one symbol a bar

    def part(self, name, preset, gain, pan, weight, clef="treble", lead=False):
        self.parts[name] = {"name": name, "preset": preset, "gain": gain, "pan": pan, "chart_weight": weight,
                            "clef": clef, "lead": lead, "notes": []}

    def note(self, part, bar, beat, length_beats, pitch, vel):
        t = round((bar * BAR + beat) * TICKS)
        n = round(length_beats * TICKS)
        self.parts[part]["notes"].append([t, n, pitch if isinstance(pitch, int) else midi(pitch), vel])

    def line(self, part, bar, text, vel=96, shift=0):
        """A melody, bar by bar: 'D5:2 B4 | G4 A4 B4'; a length in beats after ':', 1 when none; 'r' a rest;
        '~' after a note ties it into the next bar's first note of the same pitch."""
        tie_from = None
        for b, cell in enumerate(text.split("|")):
            beat = 0.0
            for tok in cell.split():
                tie = tok.endswith("~")
                tok = tok.rstrip("~")
                name, _, ln = tok.partition(":")
                ln = float(ln) if ln else 1.0
                if name != "r":
                    p = midi(name) + shift
                    if tie_from is not None and beat == 0 and tie_from[0] == p:
                        self.parts[part]["notes"][tie_from[1]][1] += round(ln * TICKS)
                    else:
                        v = vel + 10 if beat == 0 else vel
                        self.note(part, bar + b, beat, ln, p, min(v, 127))
                    tie_from = (p, len(self.parts[part]["notes"]) - 1) if tie else None
                beat += ln
            assert abs(beat - BAR) < 1e-6, f"{part} bar {bar + b}: {beat} beats in '{cell}'"

    def section(self, name, bar, bars, key):
        self.sections.append({"name": name, "bar": bar, "bars": bars, "key": key})


s = Score()
# the band: Vector37's factory sounds (presets.c), gain in thousandths, pan, and how much a player hears each one
# as something to tap (the chart's weight: 0 for a pad, which has no hit)
s.part("Ocarina", 29, 1000, 0, 10, lead=True)       # FLUTE LEAD: "a hollow ocarina"
s.part("Music box", 23, 420, 35, 2)
s.part("Harp", 21, 520, -30, 3, clef="bass")
s.part("Marimba", 22, 620, -65, 5)                  # the left hand
s.part("Kalimba", 12, 560, 65, 5)                   # the right hand
s.part("Organ", 19, 300, 20, 2)                     # DRAWBAR ORGAN: the waltz's pah-pah
s.part("Choir", 18, 260, 0, 0)
s.part("Strings", 16, 300, -10, 0)
s.part("Brass", 28, 330, 10, 4)
s.part("Bass", 25, 700, 0, 6, clef="bass")          # FM BASS
s.part("Drums", -1, 600, 0, 0, clef="perc")         # weights per drum below

THEME_A = ("D5:2 B4 | G4 A4 B4 | C5:2 E5 | D5:3 | B4 C5 D5 | E5 F#5:.5 G5:.5 E5 | D5 B4 A4 | A4:3 | "
           "D5:2 B4 | G4 A4 B4 | C5:2 A5 | G5:3 | F#5 E5 D5 | C5 B4:.5 C5:.5 A4 | B4 A4 F#4 | G4:3")
CHORDS_A = "G G C G G C D D G Em C G D Am D7 G".split()
# the juggler's flourish: where the theme holds, a run
THEME_A2 = ("D5:2 B4 | G4 A4 B4 | C5:2 E5 | D5 E5:.5 D5:.5 C5:.5 B4:.5 | B4 C5 D5 | E5 F#5:.5 G5:.5 E5 | D5 B4 A4 | "
            "A4 B4:.5 C5:.5 D5:.5 E5:.5 | D5:2 B4 | G4 A4 B4 | C5:2 A5 | G5 F#5:.5 E5:.5 D5:.5 E5:.5 | "
            "F#5 E5 D5 | C5 B4:.5 C5:.5 A4 | B4 A4 F#4 | G4:3")
THEME_B = ("E5:3 | D5:3 | C5:2 B4 | B4:3 | A4:3 | B4 C5 D5 | E5:3 | D#5:3 | "
           "E5:3 | F#5:3 | G5:2 E5 | E5:3 | C5:3 | A4 B4 C5 | B4:3~ | B4:3")
CHORDS_B = "Em Bm/D C G Am G/B C B7 Em D C Em Am Am B7 B7".split()
BRIDGE = "D5:2 B4 | A4:3 | C5:2 A4 | G4:3 | B4:2 G4 | E4:3 | F#4:2 A4 | D5:3"
CHORDS_BRIDGE = "G D Am G Em C D D".split()
CODA = "E5:2 C#5 | A4 B4 C#5 | D5:2 F#5 | E5:3 | C#5 D5 E5 | F#5 G#5 A5 | A5:3~ | A5:3"
CHORDS_CODA = "A A D A A E A A".split()

plan = [  # name, first bar, bars, key, chords
    ("Intro", 0, 8, "G major", "G G C G G C D D".split()),
    ("A", 8, 16, "G major", CHORDS_A),
    ("A'", 24, 16, "G major", CHORDS_A),
    ("B: the cascade", 40, 16, "E minor", CHORDS_B),
    ("A''", 56, 16, "G major", CHORDS_A),
    ("Bridge", 72, 8, "G major", CHORDS_BRIDGE),
    ("C", 80, 16, "A major", [transpose(c, 2) for c in CHORDS_A]),
    ("Coda", 96, 8, "A major", CHORDS_CODA),
]
for name, first, bars, key, chords in plan:
    s.section(name, first, bars, key)
    s.chords.extend(chords)
TOTAL_BARS = 104
assert len(s.chords) == TOTAL_BARS


def tones(sym, low):
    pc, iv = chord(sym)
    root = near(pc, low)
    return [root + i for i in iv]


def harp(bar, sym, vel=62):
    """The waltz's arpeggio, eighths: root, fifth, octave, tenth, octave, fifth."""
    pc, iv = chord(sym)
    r = near(pc, 43)
    third = iv[1]
    for k, step in enumerate([0, 7, 12, 12 + third, 12, 7]):
        s.note("Harp", bar, k * 0.5, 0.5, r + step, vel + (8 if k == 0 else 0))


def pah(bar, sym, vel=58):
    """Beats two and three: the chord, short."""
    for beat in (1, 2):
        for p in tones(sym, 55)[:3]:
            s.note("Organ", bar, beat, 0.5, p, vel)


def bass(bar, sym, vel=96, third_beat=False):
    r = near(bass_pc(sym), 36)
    s.note("Bass", bar, 0, 2 if not third_beat else 1.5, r, vel)
    if third_beat:
        s.note("Bass", bar, 2, 1, near((chord(sym)[0] + 7) % 12, 36), vel - 16)


def cascade(bar, sym, vel=70, hands=("Marimba", "Kalimba")):
    """The juggling act: a ball a beat, three notes a ball (triplets), tossed up from the left hand, falling
    in the right, back to the left."""
    t = tones(sym, 55)
    t3 = t[:3]
    up = [t3[0], t3[1], t3[2]]
    top = [t3[2], t3[0] + 12, t3[1] + 12]
    down = [t3[1] + 12, t3[2], t3[1]]
    for beat, (hand, cell) in enumerate([(hands[0], up), (hands[1], top), (hands[0], down)]):
        for k, p in enumerate(cell):
            s.note(hand, bar, beat + k / 3, 1 / 3, p, vel + (12 if k == 0 else 0))


def pad(part, bar, sym, vel=60, bars=1):
    for p in tones(sym, 52)[:3]:
        s.note(part, bar, 0, 3 * bars, p, vel)


def drums(bar, kick=True, shaker="beats", clap=False, triangle=False, vel=96):
    if kick:
        s.note("Drums", bar, 0, 0.5, 36, vel)
    if shaker == "beats":
        for b in (1, 2):
            s.note("Drums", bar, b, 0.5, 42, vel - 30)
    elif shaker == "eighths":
        for k in range(6):
            s.note("Drums", bar, k * 0.5, 0.5, 42, vel - (24 if k % 2 == 0 else 44))
    if clap:
        for b in (1, 2):
            s.note("Drums", bar, b, 0.5, 39, vel - 36)
    if triangle:
        s.note("Drums", bar, 0, 1, 81, vel - 20)


ch = s.chords
# Intro: the music box alone, then the harp, the bass; the ocarina's first call
for b in range(8):
    if b < 4 or b >= 6:
        r = near(chord(ch[b])[0], 67)
        pc, iv = chord(ch[b])
        for k, step in enumerate([0, iv[1], 7, 12, 7, iv[1]]):
            s.note("Music box", b, k * 0.5, 0.5, r + step, 64 if k == 0 else 52)
    if b >= 4:
        harp(b, ch[b], 56)
        bass(b, ch[b], 80)
s.note("Drums", 0, 0, 1, 81, 70)
s.note("Drums", 4, 0, 1, 81, 70)
s.line("Ocarina", 6, "D5:2 B4 | A4:3", vel=84)

# A: the theme, the café band
s.line("Ocarina", 8, THEME_A, vel=92)
for b in range(8, 24):
    harp(b, ch[b])
    pah(b, ch[b])
    bass(b, ch[b])
    drums(b, triangle=(b % 8 == 0))

# A': the flourishes; the marimba starts to juggle on beats two and three; the music box joins halfway
s.line("Ocarina", 24, THEME_A2, vel=94)
s.line("Music box", 32, "|".join(THEME_A2.split("|")[8:]), vel=50, shift=12)
for b in range(24, 40):
    harp(b, ch[b], 56)
    pah(b, ch[b])
    bass(b, ch[b])
    drums(b, triangle=(b % 8 == 0))
    t = tones(ch[b], 55)
    for k, p in enumerate([t[0] + 12, t[2], t[1], t[2]]):
        s.note("Marimba", b, 1 + k * 0.5, 0.5, p, 60 + (8 if k % 2 == 0 else 0))

# B: the cascade. The ocarina holds while three balls go round
s.line("Ocarina", 40, THEME_B, vel=96)
for b in range(40, 56):
    cascade(b, ch[b])
    bass(b, ch[b], 92)
    if b % 2 == 0:
        pad("Strings", b, ch[b], 54)
        pad("Strings", b + 1, ch[b + 1], 50)
    drums(b, shaker=None, triangle=(b % 4 == 0))

# A'': the whole band, the music box an octave up with the ocarina, the choir behind
s.line("Ocarina", 56, THEME_A, vel=100)
s.line("Music box", 56, THEME_A, vel=50, shift=12)
for b in range(56, 72):
    harp(b, ch[b], 62)
    pah(b, ch[b], 62)
    bass(b, ch[b], 100, third_beat=(b % 2 == 1))
    pad("Choir", b, ch[b], 56)
    drums(b, shaker="eighths", clap=(b % 2 == 1), triangle=(b % 8 == 0))

# Bridge: the ocarina and the harp; the kalimba keeps one ball up
s.line("Ocarina", 72, BRIDGE, vel=84)
for b in range(72, 80):
    harp(b, ch[b], 50)
    t = tones(ch[b], 67)
    for k, p in enumerate([t[0], t[1], t[2]]):
        s.note("Kalimba", b, 2 + k / 3, 1 / 3, p, 50)
    if b >= 76:
        bass(b, ch[b], 72)

# C: up a tone; brass on the downbeats, everyone juggling
s.line("Ocarina", 80, THEME_A2, vel=104, shift=2)
s.line("Music box", 80, THEME_A2, vel=50, shift=14)
for b in range(80, 96):
    cascade(b, ch[b], 64)
    pah(b, ch[b], 60)
    bass(b, ch[b], 104, third_beat=True)
    pad("Choir", b, ch[b], 58)
    if b % 2 == 0:
        for p in tones(ch[b], 55)[:3]:
            s.note("Brass", b, 0, 2, p, 92)
    drums(b, shaker="eighths", clap=True, triangle=(b % 4 == 0), vel=100)

# Coda: the last throws, the long note, one last catch
s.line("Ocarina", 96, CODA, vel=100)
for b in range(96, 102):
    cascade(b, ch[b], 62)
    bass(b, ch[b], 96)
    pad("Choir", b, ch[b], 56)
    drums(b, shaker="beats", triangle=(b % 2 == 0))
for p in tones("A", 45) + tones("A", 57):
    s.note("Strings", 102, 0, 6, p, 70)
for part, p in [("Harp", 45), ("Harp", 57), ("Harp", 64), ("Marimba", 57), ("Kalimba", 69), ("Music box", 81),
                ("Bass", 45)]:
    s.note(part, 102, 0, 3, p, 100)
s.note("Drums", 102, 0, 0.5, 36, 110)
s.note("Drums", 102, 0, 3, 81, 100)

for p in s.parts.values():
    p["notes"].sort()
score = {
    "id": "jugglers-of-vienna",
    "title": "Jugglers of Vienna",
    "bpm": BPM,
    "bar_beats": BAR,
    "ticks_per_beat": TICKS,
    "start_us": START_US,
    "bars": TOTAL_BARS,
    "sections": s.sections,
    "chords": s.chords,
    # how loud each drum is to a player tapping along: the kick most, the shaker least
    "drum_weights": {"36": 6, "39": 3, "42": 2, "81": 3},
    "parts": list(s.parts.values()),
}
out = os.path.join(os.path.dirname(__file__), "..", "..", "data", "scores", "jugglers-of-vienna.json")
os.makedirs(os.path.dirname(out), exist_ok=True)
with open(out, "w") as f:
    f.write("{\n")
    keys = [k for k in score if k != "parts"]
    for k in keys:
        f.write(f" {json.dumps(k)}: {json.dumps(score[k])},\n")
    f.write(' "parts": [\n')
    for i, p in enumerate(score["parts"]):
        head = {k: v for k, v in p.items() if k != "notes"}
        body = json.dumps(head)[:-1]
        notes = ",\n   ".join(json.dumps(n) for n in p["notes"])
        f.write(f'  {body}, "notes": [\n   {notes}\n  ]}}' + ("," if i < len(score["parts"]) - 1 else "") + "\n")
    f.write(" ]\n}\n")
print(f"{out}: {sum(len(p['notes']) for p in score['parts'])} notes in {len(score['parts'])} parts")
