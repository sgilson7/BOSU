# Handoff

Stage 9, written by the agent. `C5 Consolidate in your own words`. A fresh session starts from this file, not from memory.

## What this is

Countless Days: an original browser rhythm game, press each target as its ring closes, for the person and their friends, on five songs from their album at five difficulties. Rounds 1 to 3 (solo play) are built on five placeholder songs; the lobby (round 4), the ledger (round 5) and the art (round 6) are not. Repository BOSU; deployed to GitHub Pages from `dist/`.

## Rules that hold it up

| Rule | Guarded by |
| --- | --- |
| Judgements use each input's own audio time, never the frame's (BO-2) | `the_same_presses_judge_the_same_at_30_60_and_144_frames`, `song_time_is_the_heard_time_less_the_start_and_the_offset` |
| Committed charts are what the generator writes (BO-6) | `the_committed_charts_match_the_generator` |
| A perfect bot clears every chart (BO-4) | `every_chart_is_cleared_by_a_perfect_bot` |
| Each difficulty is harder (BO-5) | `each_committed_difficulty_is_harder_than_the_one_below` |
| Every deployed save loads | `old_saves.rs`, with `tests/saves/v1.json` |

## Shape of the code

| Where | What |
| --- | --- |
| `crates/core/src/play.rs` | judging, holds, score, combo, accuracy, the perfect bot |
| `crates/core/src/game.rs` | screens, settings, the audio clock, calibration, the play screen's draw list |
| `crates/core/src/beats.rs`, `generate.rs`, `catalog.rs` | beat finding, chart layout, the chart index |
| `crates/core/src/tuning.rs` | every number of play |
| `crates/chart_tool` | build-time ffmpeg decoding and file writing |
| `web/app.js` | carries out the core's commands; forwards events with timestamps |
| `data/songs.json` | the song list people edit; `data/charts/` is generated |
| `crates/core/src/online.rs` | the lobby's protocol: hello, members, Ready, Start, scores |
| `crates/core/src/ledger.rs` | records: signing, checking, merging, trimming |
| `crates/core/src/together.rs` | the lobby and ledger joined to screens, play and save |
| `web/online.js`, `web/net/` | the transport (kits/online); nothing loads until Host or Join |

## Commands

- `./scripts/check.sh`: prints `BUILDER CHECK PASSED`.
- New songs: put the MP3 in `web/audio/`, list it in `data/songs.json`, then `cargo run --release -p chart_tool -- analyse` and `-- charts`. `-- check` says whether the charts are stale.
- `tools/placeholders/make.sh`: remakes our five songs (needs ffmpeg).
- `python3 tools/songs/strip_tags.py SRC.mp3 web/audio/ID.mp3`: a new song without its tags; then add it to `data/songs.json` and run the chart tool.
- `./target/release/chart stability`: each song's tempo from its halves (R6.4).
- `./target/release/chart peaks`: each song's onset peaks, time and strength a line, under `target/peaks/`, for checking the beat finder against another onset detector (R7.1).
- `python3 tools/art/art.py`: redraws the landing picture and icon (needs TeX).
- `tools/compose/make.sh ID`: a composed song: its score from `tools/compose/ID.py` into `data/scores/`, its audio through the Vector37 engine (fetched at a fixed commit into `target/`, GPL-3.0) into `web/audio/`, its score drawn into `docs/scores/ID.pdf`. Then list it in `data/songs.json` with `"score"` and run the chart tool (R8.1).
- `.venv/bin/python tests/browser/decode.py`: every song decodes in each engine, and the shift the core applies there.
- `.venv/bin/python tests/browser/probe_frames.py chromium firefox webkit`: frame times (BO-11).
- `python3 scripts/break_tests.py`: every break caught.
- `.venv/bin/python tests/browser/online.py chromium firefox webkit`: two browsers by pasted codes, on the origin: lobby, scores and a ledger record crossing.
- `.venv/bin/python tests/browser/relays.py [ORIGIN]`: two browsers by room code over the real relays (leaves the origin).

## What will bite within the hour

- Changing `beats.rs`, `generate.rs` or `tuning.rs` changes the charts: regenerate them, or `the_committed_charts_match_the_generator` fails.
- The core may not name `time`, `env` or `f64`; use `at()` and other names.
- A change to the save's fields needs a version bump and a step in `save.rs`'s `migrate` (kits/saves).

## The journal

`../bosu-journal/`, published privately at https://claude.ai/artifact/2RGq94P6vWrVUS8mksQgHR.

## Open for the person

Every `SECOND-ORDER.md` row with status *person*, by number, with what the person has to decide or supply. `scripts/notebook_check.py` fails while one is missing.

- R1.2: the five audio files and their titles (BO-Q2).
- R1.3: the album makers' agreement, in your words, for `LICENSES.md`.
- R1.6: the opening entry's questions, BO-Q1 to BO-Q11. Answered 2026-11-02.
- R2.1: the exact SoundCloud playlist link (BO-Q12, T1). Given 2026-11-02.
- R2.4: the plan's questions, BO-Q12 to BO-Q17. Answered 2026-11-02.
- R3.1: the five audio files: downloads turned on for each track, or the files in BrowserOSU/songs/ (BO-Q18).
- R3.11: whether Easy is busy enough, after playing it.
- R6.1: the names of our five extra songs, if you want others.
- R7.5: Grunge Dome and Space Tsunami: play them; if the beat is off, give their tempo from Ableton in `data/songs.json` (`bpm_milli`). Say whether Vamos's Den feels like 124.6 with triplets or 187 straight.
- R6.5: older commits in the public repository still hold the friend's SoundCloud handle; history can be rewritten if you want it gone.
- R4.4: a lobby with a friend on another network, by hand.

## Mistakes so far

By system, from `SECOND-ORDER.md`, with the rule each one produced.

## What passed

The last lines of `./scripts/check.sh` and `./scripts/count-tests.sh`, with the date and commit.

2026-11-03, round 7 (T11, R7.1 to R7.5), uncommitted on top of de0b244: `./scripts/check.sh` ended `BROWSER CHECK PASSED in chromium, firefox, webkit, webkit-iphone, webkit-ipad` and `BUILDER CHECK PASSED`; `./scripts/count-tests.sh`: 167 passed, 0 failed, 0 ignored; `python3 scripts/break_tests.py`: 71 of 71 breaks caught; `chart_tool check`: 76 files match the generator.

2026-11-04, round 8 (T14, T15), 4308304: `./scripts/check.sh` ended `BROWSER CHECK PASSED in chromium, firefox, webkit, webkit-iphone, webkit-ipad` and `BUILDER CHECK PASSED`; `python3 scripts/break_tests.py`: 91 of 91 breaks caught.

2026-11-04, round 8 (T16), 7209779: `./scripts/check.sh` ended `BROWSER CHECK PASSED in chromium, firefox, webkit, webkit-iphone, webkit-ipad` and `BUILDER CHECK PASSED`; `python3 scripts/break_tests.py`: 94 of 94 breaks caught.

## The next action

One action.

## Deploys

| Date | Commit | Address | Checks |
| --- | --- | --- | --- |
| 2026-11-02 | cb035fb (build e6d82e8f9f77) | https://sgilson7.github.io/BOSU/ | `./scripts/check.sh` passed before publishing; `BUILDER_ORIGIN=https://sgilson7.github.io/BOSU tests/browser/check.py` passed in chromium, firefox, webkit, webkit-iphone, webkit-ipad. Solo game on placeholder songs (BO-Q17: A). Not checked: real audio latency on a person's machine, a real phone. |
| 2026-11-02 | e30d0c4 | https://sgilson7.github.io/BOSU/ | check.sh passed; live browser check passed in all five profiles. Press keys (T4), MIT licence (T5). |
| 2026-11-02 | b24de0a | https://sgilson7.github.io/BOSU/ | check.sh passed; live browser check passed in all five profiles; `tests/browser/online.py` passed in chromium, firefox, webkit before publishing; `tests/browser/relays.py https://sgilson7.github.io/BOSU` connected two browsers by the live invite link over the real relays. Lobby and ledger (T2, T3, T6). Not checked: two different networks (R4.4). |
| 2026-11-02 | dcebb48 | https://sgilson7.github.io/BOSU/ | check.sh passed; `online.py` and `decode.py` passed in chromium, firefox, webkit before publishing; live browser check passed in all five profiles; a live invite link connected two browsers over the real relays. The album (T7-T9) and the landing page (T10). |
| 2026-11-03 | edc9a31 | https://sgilson7.github.io/BOSU/ | check.sh passed (publish.sh ran it); break_tests 71 of 71; live browser check passed in all five profiles; `relays.py` against the live site connected two browsers over the real relays. Charts on each song's hits, nested difficulties, one circle size a song (T11, R7.1 to R7.3). Every board starts empty (R7.4). Not checked: Grunge Dome and Space Tsunami by ear (R7.5). |
| 2026-11-03 | 639beb1 | https://sgilson7.github.io/BOSU/ | check.sh passed (publish.sh ran it); the new and repaired break rows caught; live browser check passed in all five profiles. Ballad Of Mr Nasty from its true chart (T12, R7.6); the other songs unchanged. |
| 2026-11-03 | eb95dbb | https://sgilson7.github.io/BOSU/ | check.sh passed (publish.sh ran it); the true-chart and groove break rows caught; live browser check passed in all five profiles. Ballad's Easy to Hard follow the groove (T13, R7.7); the other songs unchanged. |
| 2026-11-04 | 4308304 | https://sgilson7.github.io/BOSU/ | check.sh passed (publish.sh ran it); break_tests 91 of 91; live browser check passed in all five profiles. Every song by its groove (T14, R7.8); composed songs from a score (R8.1, R8.2); Jugglers of Vienna not listed until its render (R8.3). |
| 2026-11-04 | 7209779 | https://sgilson7.github.io/BOSU/ | check.sh passed (publish.sh ran it); break_tests 94 of 94; live browser check passed in all five profiles, sixteen songs in each. Jugglers of Vienna rendered, listed and charted from its score (T15, T16, R8.3, R8.4). |
