#!/usr/bin/env python3
"""Two browsers meet by room code over the real Nostr relays in
web/net/config.js, the way friends do: the host presses Host a lobby, the
joiner opens the invite link and presses Join.

This leaves the origin on purpose (relays, STUN), so it is not part of
check.sh. Run it before a release and write the result and date in
HANDOFF.md (kits/online, "What stays with a person"); it still says nothing
about two different networks.

    python3 tests/browser/relays.py [ORIGIN]   (default: serve dist/ locally)
"""
import functools
import http.server
import json
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[2]
STRINGS = json.loads((ROOT / "data" / "strings.json").read_text())
sys.path.insert(0, str(ROOT / "scripts"))
import serve  # noqa: E402

if len(sys.argv) > 1:
    origin = sys.argv[1].rstrip("/")
else:
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(serve.Handler, directory=str(ROOT / "dist")))
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    origin = f"http://127.0.0.1:{srv.server_port}"

with sync_playwright() as p:
    b = p.chromium.launch()
    errors = []
    host = b.new_context().new_page()
    host.on("console", lambda m: m.type == "error" and errors.append("host: " + m.text))
    host.goto(origin + "/index.html")
    host.wait_for_selector("body[data-ready='true']")
    host.click("button[data-action='online.host']")
    link_line = host.inner_text("#lobby p:nth-of-type(2)")
    link = link_line.split(": ", 1)[1]
    print("invite link:", link)
    joiner = b.new_context().new_page()
    joiner.on("console", lambda m: m.type == "error" and errors.append("joiner: " + m.text))
    joiner.goto(link)
    joiner.wait_for_selector("body[data-screen='join']")
    joiner.click("#join button[data-action='online.join']")
    connected = STRINGS["online.in_lobby"]
    for who, page in (("host", host), ("joiner", joiner)):
        page.locator("#lobby p", has_text=connected).first.wait_for(timeout=60000)
        print(f"PASS the {who} is connected over the relays")
    host.locator("#members p").nth(1).wait_for(timeout=15000)
    print("PASS the host lists", host.locator("#members p").count(), "players")
    print("console errors:", errors)
    b.close()
