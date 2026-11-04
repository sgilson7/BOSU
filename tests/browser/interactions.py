"""The project's own browser interactions, run by check.py in every engine.

check.py has already loaded the page, waited for it to report ready, and let
it idle. Each step waits on a data-* attribute the page sets, never a fixed
time. Expected words come from data/strings.json.
"""

import json
import pathlib

STRINGS = json.loads((pathlib.Path(__file__).resolve().parents[2] / "data" / "strings.json").read_text())


def run(page, check, engine):
    check(page.inner_text("#title") == STRINGS["title"], "the title from data/strings.json is drawn")
    check(page.get_attribute("body", "data-screen") == "title", "the title screen is shown")
    check(page.inner_text("#message") == "", "no error message is shown")

    page.click("button[data-value='select']")
    page.wait_for_selector("body[data-screen='select']")
    check(page.locator("button[data-action='song.pick']").count() == 16, "sixteen songs to choose from: the album's ten and six of our own")
    check(page.locator("button[data-action='difficulty.pick']").count() == 5, "five difficulties to choose from")
    names = [page.inner_text(f"button[data-value='{k}'] > span") for k in ["easy", "normal", "hard", "expert", "master"]]
    check(names == [STRINGS[f"difficulty.{k}"] for k in ["easy", "normal", "hard", "expert", "master"]], f"the difficulty names {names}")

    # play: the song loads, starts on the audio clock, and with no presses
    # the first target runs out of time and is judged a Miss
    page.click("button[data-value='easy']")
    page.click("button[data-action='play.start']")
    page.wait_for_selector("body[data-screen='play']", timeout=30000)
    check(page.is_visible("#field"), "the playfield shows")
    page.wait_for_selector("body:not([data-judged='0'])", timeout=30000)
    check(True, "the audio clock ran and the first target was judged")

    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-paused='true']")
    check(page.inner_text("#pause h2") == STRINGS["pause.heading"], "Escape pauses")
    page.click("button[data-action='play.quit']")
    page.wait_for_selector("body[data-screen='select']")
    check(not page.is_visible("#field"), "back on the song list, the playfield is gone")

    # settings: a step of the audio offset shows in its line
    page.click("button[data-value='title']")
    page.click("button[data-value='settings']")
    page.wait_for_selector("body[data-screen='settings']")
    page.click("button[data-action='audio.more']")
    want = STRINGS["settings.audio"].replace("{ms}", "+5")
    check(page.inner_text("#audio p") == want, f"the audio offset reads {want}")

    # press keys: add W by pressing it, then the line lists it
    page.click("button[data-action='keys.listen']")
    page.wait_for_selector("#keys p:nth-of-type(2)")
    page.keyboard.press("w")
    want = STRINGS["settings.keys"].replace("{keys}", "Z, X, W")
    page.wait_for_selector(f"#keys p:text-is('{want}')")
    check(True, f"pressing W while listening adds it: {want}")
    page.click("button[data-action='keys.reset']")
