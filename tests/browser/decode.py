#!/usr/bin/env python3
"""Every song decodes in each engine, to the length the charts were made
for (within 60 ms), and its first sound falls within the 60 ms the core
corrects for (game::audio_head, R6.6). Prints each engine's difference from
the build's decode: that is the shift the game applies in that engine.

    .venv/bin/python tests/browser/decode.py [chromium firefox webkit]
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
srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(serve.Handler, directory=str(ROOT / "dist")))
threading.Thread(target=srv.serve_forever, daemon=True).start()
base = f"http://127.0.0.1:{srv.server_port}/"
bad = []
JS = """async (url) => {
  const ctx = new OfflineAudioContext(1, 1, 44100);
  const buf = await ctx.decodeAudioData(await (await fetch(url)).arrayBuffer());
  const d = buf.getChannelData(0);
  // as beats::first_sound_us: the first sample at a quarter of the loudest
  // in the opening three seconds
  const head = d.subarray(0, 3 * buf.sampleRate);
  let peak = 0;
  for (const v of head) peak = Math.max(peak, Math.abs(v));
  let first = 0;
  while (first < head.length && Math.abs(head[first]) * 4 < peak) first++;
  return [buf.duration, first / buf.sampleRate];
}"""
with sync_playwright() as p:
    for engine in sys.argv[1:] or ["chromium", "firefox", "webkit"]:
        b = getattr(p, engine).launch()
        pg = b.new_page()
        pg.goto(base + "build.txt")
        starts = []
        for s in index["songs"]:
            dur, first = pg.evaluate(JS, base + s["audio"])
            want = s["length_us"] / 1e6
            ok = abs(dur - want) < 0.06
            shift_ms = s["first_sound_us"] / 1000 - first * 1000
            starts.append((s["id"], shift_ms))
            if not ok:
                bad.append(f"{engine} {s['id']}: {dur:.3f} s, the charts assume {want:.3f} s")
            if abs(shift_ms) > 60:
                bad.append(f"{engine} {s['id']}: first sound {shift_ms:.1f} ms from the build's, past what the core corrects")
        print(engine, "decoded", len(index["songs"]), "songs; shift the core applies (ms):",
              ", ".join(f"{i} {t:+.1f}" for i, t in starts))
        b.close()
print("DECODE CHECK", "FAILED:\n  " + "\n  ".join(bad) if bad else "PASSED")
sys.exit(1 if bad else 0)
