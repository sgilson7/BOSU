"""Probe for BO-11: frame times while playing the densest chart.

    .venv/bin/python tests/browser/probe_frames.py [engine ...]

Serves dist/, plays the chart with the most targets for 15 s with presses on
the beat from a script, and prints the frame-time percentiles the page saw
(requestAnimationFrame timestamps, recorded by an init script, so the page
itself is unchanged). A probe measures and prints; it asserts nothing. Copy
the line into MEASUREMENTS.md with the machine it ran on.
"""
import functools
import http.server
import json
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import serve  # noqa: E402

index = json.loads((ROOT / "data" / "charts" / "index.json").read_text())
song, chart = max(((s, c) for s in index["songs"] for c in s["charts"]), key=lambda sc: sc[1]["targets"])
DIFF = ["easy", "normal", "hard", "expert", "master"][chart["difficulty"]]

srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(serve.Handler, directory=str(ROOT / "dist")))
threading.Thread(target=srv.serve_forever, daemon=True).start()
url = f"http://127.0.0.1:{srv.server_port}/index.html"
RECORD = """
window.__frames = [];
const raf = window.requestAnimationFrame.bind(window);
window.requestAnimationFrame = (f) => raf((t) => { window.__frames.push(t); f(t); });
"""
with sync_playwright() as p:
    for engine in sys.argv[1:] or ["chromium"]:
        b = getattr(p, engine).launch(args=["--disable-gpu"] if engine == "chromium" else [])
        pg = b.new_page(viewport={"width": 1600, "height": 900})
        pg.add_init_script(RECORD)
        pg.goto(url)
        pg.wait_for_selector("body[data-ready='true']")
        pg.click("button[data-value='select']")
        pg.click(f"button[data-action='song.pick'] >> text={song['title']}")
        pg.click(f"button[data-value='{DIFF}']")
        pg.click("button[data-action='play.start']")
        pg.wait_for_selector("body[data-screen='play']", timeout=30000)
        pg.evaluate("window.__frames = []")
        pg.mouse.move(800, 450)
        for _ in range(60):
            pg.mouse.down()
            pg.mouse.up()
            pg.wait_for_timeout(250)
        frames = pg.evaluate("window.__frames")
        gaps = sorted(b - a for a, b in zip(frames, frames[1:]))
        pct = lambda q: gaps[min(len(gaps) - 1, int(len(gaps) * q))]
        slow = sum(1 for g in gaps if g > 20)
        print(f"{engine}: {song['id']} {DIFF} ({chart['targets']} targets), {len(gaps)} frames in "
              f"{(frames[-1] - frames[0]) / 1000:.1f} s: median {pct(0.5):.1f} ms, 95th {pct(0.95):.1f} ms, "
              f"99th {pct(0.99):.1f} ms, {slow} frames over 20 ms")
        b.close()
