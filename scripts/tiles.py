#!/usr/bin/env python3
"""Screenshots of still scenes the core sets up, one per tile spec.

Part of the canvas kit (kits/canvas/README.md), after Peter's tiles script
for his games' home page.

    python3 scripts/tiles.py OUTDIR SPEC [SPEC ...]
    python3 scripts/tiles.py OUTDIR --from tiles.txt     (one spec a line)

For each spec it opens index.html?tile=<spec> on a local server, waits for
the page to report <body data-tile="drawn">, and saves the canvas as
OUTDIR/<spec>.png. The page passes the spec to the core, which builds the
scene and holds it still; the page only draws. A spec the core does not know
should come back as an error the page shows, and this script then fails.

It is a tool for looking, not a check: check.sh does not run it. It fails
on a page or console error, a request off the origin, or a tile that never
reports drawn.

Environment:
    BUILDER_ENGINE   chromium (default), firefox or webkit
    TILE_SIZE        viewport as WIDTHxHEIGHT (default 1280x720)
    TILE_SCALE       device scale factor (default 2)
"""
import functools
import http.server
import os
import pathlib
import re
import sys
import threading
import urllib.parse

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
DIST = ROOT / "dist"
ENGINE = os.environ.get("BUILDER_ENGINE", "chromium")
WIDTH, HEIGHT = (int(n) for n in os.environ.get("TILE_SIZE", "1280x720").split("x"))
SCALE = float(os.environ.get("TILE_SCALE", "2"))
TIMEOUT_MS = 20000


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
    }

    def log_message(self, *args):
        pass


def serve():
    handler = functools.partial(Quiet, directory=str(DIST))
    # Port 0: the system picks a free port, so two runs never collide.
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def file_name(spec):
    return re.sub(r"[^A-Za-z0-9._-]+", "_", spec).strip("_") or "tile"


def read_specs(args):
    if args[:1] == ["--from"]:
        lines = pathlib.Path(args[1]).read_text().splitlines()
        return [s.strip() for s in lines if s.strip() and not s.startswith("#")]
    return args


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    out = pathlib.Path(sys.argv[1])
    specs = read_specs(sys.argv[2:])
    if not specs:
        sys.exit("no tile specs given")
    if not (DIST / "index.html").exists():
        sys.exit("dist/ is missing: run scripts/build.sh first")
    out.mkdir(parents=True, exist_ok=True)
    server = serve()
    origin = "http://127.0.0.1:%d" % server.server_address[1]
    problems = []
    with sync_playwright() as p:
        browser = getattr(p, ENGINE).launch()
        context = browser.new_context(viewport={"width": WIDTH, "height": HEIGHT}, device_scale_factor=SCALE)
        context.on(
            "request",
            lambda r: None if r.url.startswith((origin + "/", "blob:", "data:")) else problems.append(f"off-origin: {r.url}"),
        )
        for spec in specs:
            page = context.new_page()
            page.on("console", lambda m, s=spec: problems.append(f"{s}: {m.text}") if m.type == "error" else None)
            page.on("pageerror", lambda e, s=spec: problems.append(f"{s}: {e}"))
            page.goto(origin + "/index.html?tile=" + urllib.parse.quote(spec))
            try:
                page.wait_for_selector("body[data-tile='drawn']", state="attached", timeout=TIMEOUT_MS)
            except Exception:
                problems.append(f"{spec}: never reported drawn")
                page.close()
                continue
            target = out / (file_name(spec) + ".png")
            page.locator("canvas").first.screenshot(path=str(target))
            print(f"  {spec} -> {target}")
            page.close()
        browser.close()
    server.shutdown()
    if problems:
        print("\nTILES FAILED")
        for line in problems:
            print("  - " + line)
        sys.exit(1)
    print(f"\n{len(specs)} tile(s) saved in {out}")


if __name__ == "__main__":
    main()
