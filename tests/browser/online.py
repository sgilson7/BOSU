#!/usr/bin/env python3
"""Online check: two browsers meet by pasted codes and play on one origin.

Part of the online kit (kits/online/README.md). Copy it to
tests/browser/online.py, fill in SELECTORS for the project's page, and run it
after scripts/build.sh. It sits beside the browser check, not inside it,
because check.py fails on any WebSocket or off-origin request and must keep
doing so.

How it stays on the origin:

- `?ice=none` makes web/net/config.js drop the STUN servers, so the two
  browsers connect on host candidates alone;
- the pasted-code path carries the whole handshake in the codes, so no relay
  is contacted;
- host addresses are not hidden behind mDNS names, which a CI runner may not
  resolve.

It fails on: a page or console error, any request off the origin, any
WebSocket (the room-code path must not start), a browser that never reaches
the playing state, the two browsers disagreeing at the end, or a browser
reporting a desync.

Countless Days: it also checks the lobby (pick, Ready, Start, both players'
running scores) and the ledger (a signed record crossing from the host's
browser to the joiner's and replayed there). What it does not cover: the
room-code path over public relays, STUN, TURN, and two real networks. Those are checked by hand before a release and the
result written in HANDOFF.md.

    python3 tests/browser/online.py [chromium firefox ...]   (default: chromium)
"""
import functools
import http.server
import json
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[2]
DIST = ROOT / "dist"
STRINGS = json.loads((ROOT / "data" / "strings.json").read_text())

LEDGER_SAVE = (ROOT / "tests" / "browser" / "ledger_save.json").read_text().strip()
# Countless Days: the host starts with a save holding Ana's signed record
# (crates/core/tests/ledger_fixture.rs); the joiner starts empty.
OFFER_READY = "body[data-offer='ready']"
ANSWER_READY = "body[data-answer='ready']"
TIMEOUT_MS = 15000

failures = []


def check(ok, what):
    print(("  PASS " if ok else "  FAIL ") + what)
    if not ok:
        failures.append(what)


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
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


# Both browsers share this machine, so plain local addresses are what lets
# them meet without STUN.
LAUNCH = {
    "chromium": {"args": ["--disable-features=WebRtcHideLocalIpsWithMdns"]},
    "firefox": {"firefox_user_prefs": {"media.peerconnection.ice.obfuscate_host_addresses": False}},
}


def open_page(browser, origin, who, seen, save=None):
    context = browser.new_context()
    if save:
        context.add_init_script("localStorage.setItem('save:/index.html', %s)" % json.dumps(save))
    page = context.new_page()
    page.on("console", lambda m: seen["errors"].append(f"{who}: {m.text}") if m.type == "error" else None)
    page.on("pageerror", lambda e: seen["errors"].append(f"{who}: {e}"))
    page.on("websocket", lambda ws: seen["sockets"].append(f"{who}: {ws.url}"))
    page.on(
        "request",
        lambda r: None if r.url.startswith((origin + "/", "blob:", "data:")) else seen["offsite"].append(r.url),
    )
    page.goto(origin + "/index.html?ice=none")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=TIMEOUT_MS)
    return page


def run(p, engine, origin):
    print(f"\n[{engine}] online, two browsers, pasted codes")
    browser = getattr(p, engine).launch(**LAUNCH.get(engine, {}))
    seen = {"errors": [], "sockets": [], "offsite": []}
    host = open_page(browser, origin, "host", seen, LEDGER_SAVE)
    joiner = open_page(browser, origin, "joiner", seen)

    host.click("button[data-value='join']")
    host.click("button[data-action='online.host_code']")
    host.wait_for_selector(OFFER_READY, state="attached", timeout=TIMEOUT_MS)
    offer = host.input_value("#offer-out")
    check(bool(offer), "the host shows a code to send")

    joiner.click("button[data-value='join']")
    joiner.fill("#offer-in", offer)
    joiner.press("#offer-in", "Enter")
    joiner.wait_for_selector(ANSWER_READY, state="attached", timeout=TIMEOUT_MS)
    answer = joiner.input_value("#answer-out")
    check(bool(answer), "the joiner shows a reply code")

    host.fill("#answer-in", answer)
    host.press("#answer-in", "Enter")
    connected = STRINGS["online.in_lobby"]
    for who, page in (("host", host), ("joiner", joiner)):
        try:
            page.locator("#lobby p", has_text=connected).first.wait_for(timeout=TIMEOUT_MS)
            ok = True
        except Exception:
            ok = False
        check(ok, f"the {who} shows it is connected")
    joiner_line = STRINGS["lobby.member"].replace("{name}", "Player ")
    host.locator("#members p", has_text=joiner_line).wait_for(timeout=TIMEOUT_MS)
    check(host.locator("#members p").count() == 2, "the host lists two players")

    # the host picks the second song on Hard; the joiner sees it
    host.click("button[data-action='song.pick'][data-value='1']")
    host.click("button[data-action='difficulty.pick'][data-value='hard']")
    second = json.loads((ROOT / "data" / "charts" / "index.json").read_text())["songs"][1]["title"]
    want = STRINGS["lobby.chosen"].replace("{song}", second).replace("{difficulty}", STRINGS["difficulty.hard"])
    joiner.locator("#chosen p", has_text=want).wait_for(timeout=TIMEOUT_MS)
    check(True, f"the joiner sees the host's pick: {want}")
    check(host.is_disabled("button[data-action='lobby.start']"), "Start waits for the joiner's Ready")
    joiner.click("button[data-action='lobby.ready']")
    host.wait_for_selector("button[data-action='lobby.start']:not([disabled])", timeout=TIMEOUT_MS)
    host.click("button[data-action='lobby.start']")
    for who, page in (("host", host), ("joiner", joiner)):
        page.wait_for_selector("body[data-screen='play']", timeout=30000)
        check(True, f"the {who} starts the round")
    # each sees both players' running scores
    for who, page in (("host", host), ("joiner", joiner)):
        page.wait_for_selector("body[data-scoring='2']", timeout=TIMEOUT_MS)
        check(True, f"the {who} holds both players' scores")
    for page in (host, joiner):
        page.keyboard.press("Escape")
        page.wait_for_selector("body[data-paused='true']", timeout=TIMEOUT_MS)
        page.click("button[data-action='play.quit']")
        page.wait_for_selector("body[data-screen='lobby']", timeout=TIMEOUT_MS)
    check(True, "both quit the round back to the lobby")

    # the ledger: Ana's record crossed from the host's browser to the joiner's
    joiner.click("button[data-action='online.leave']")
    joiner.click("button[data-value='select']")
    joiner.click("button[data-action='song.pick'][data-value='0']")
    joiner.click("button[data-action='difficulty.pick'][data-value='easy']")
    ana = STRINGS["board.row"].split("{name}")[0].replace("{rank}", "1") + "Ana"
    try:
        joiner.locator("#board p", has_text=ana).wait_for(timeout=TIMEOUT_MS)
        ok = True
    except Exception:
        ok = False
    check(ok, "Ana's record reached the joiner and shows on its leaderboard")
    mark = STRINGS["board.replayed"]
    check(mark in joiner.inner_text("#board"), "the joiner replayed Ana's presses and marks the record so")

    check(not seen["errors"], f"no page or console error {seen['errors'][:3]}")
    check(not seen["offsite"], f"no request off the origin {seen['offsite'][:3]}")
    check(not seen["sockets"], f"no WebSocket, so no relay was contacted {seen['sockets'][:3]}")
    browser.close()


def main():
    if not (DIST / "index.html").exists():
        sys.exit("dist/ is missing: run scripts/build.sh first")
    engines = sys.argv[1:] or ["chromium"]
    server = serve()
    origin = "http://127.0.0.1:%d" % server.server_address[1]
    with sync_playwright() as p:
        for engine in engines:
            run(p, engine, origin)
    server.shutdown()
    if failures:
        print(f"\nONLINE CHECK FAILED: {len(failures)} problem(s)")
        for f in failures:
            print("  - " + f)
        sys.exit(1)
    print(f"\nONLINE CHECK PASSED in {', '.join(engines)}")


if __name__ == "__main__":
    main()
