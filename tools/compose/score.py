"""A score (data/scores/<id>.json) as the renderer's events: each part's notes on and off at their frames.

    python3 tools/compose/score.py data/scores/ID.json OUT.events
"""
import json
import sys

FS = 44100      # the Vector37 engine's rate
TAIL_US = 4_000_000


def frames(score, tick):
    beat_us = 60_000_000 // score["bpm"]
    us = score["start_us"] + tick * beat_us // score["ticks_per_beat"]
    return us * FS // 1_000_000


def main(src, dst):
    score = json.load(open(src))
    end = max(n[0] + n[1] for p in score["parts"] for n in p["notes"])
    out = [f"length {frames(score, end) + TAIL_US * FS // 1_000_000}"]
    for p in score["parts"]:
        out.append(f"part {p['name'].replace(' ', '_')} {p['preset']} {p['gain']} {p['pan']}")
        out.append(f"tempo {score['bpm']}")
        ev = []
        for t, n, pitch, vel in p["notes"]:
            ev.append((frames(score, t), 1, f"on {frames(score, t)} {pitch} {vel}"))
            if p["preset"] >= 0:
                # off a little before the next note's on, so a repeated note sounds again
                ev.append((frames(score, t + n) - 64, 0, f"off {frames(score, t + n) - 64} {pitch}"))
        ev.sort(key=lambda e: (e[0], e[1]))
        out.extend(e[2] for e in ev)
    open(dst, "w").write("\n".join(out) + "\n")


if __name__ == "__main__":
    main(*sys.argv[1:])
