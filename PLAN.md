# Plan

Stage 2, written by the agent from `BRIEF.md` and the answers BO-Q1 to BO-Q11 (journal, agreed entry of 2026-11-02). `B2 Require step-by-step justification`.

The game is **Countless Days** (BO-Q1), an original rhythm game in the click-on-the-beat genre, in the repository BOSU. Every number below is proposed; it lives in one tuning table in the core (`crates/core/src/tuning.rs`), and the journal's mockups are drawn from the same numbers (`../bosu-journal/mockups/numbers.json`).

## Build order

Each round ends with `./scripts/check.sh` passing and a journal entry with pictures. Rounds 1 to 3 build a whole solo game; 4 adds the lobby; 5 the ledger (T2); 6 polishes and deploys.

| # | Decision | Reason | Test that shows it holds |
| --- | --- | --- | --- |
| **Round 1: the clock, the chart, the judgement** | | | |
| 1 | Copy `kits/canvas` (`draw.rs`, `render2d.js`, `tiles.py`) and from `kits/realtime` `rng.rs` and `input.js` without pointer lock. | A playfield drawn every frame from a list of integers keeps the page free of numbers (D3). | the kits' own tests, run in this project |
| 2 | Time enters the core only as audio time in whole microseconds. The page reads `AudioContext.currentTime` and `outputLatency` once a frame and with every press; the shim converts seconds to microseconds with one multiplication and a cast. The core schedules the song's start ("start at audio time T") and the page calls `source.start(T)` with the value the core gave. | A judgement must not move with the frame rate or with a dropped frame (BO-2, R1.5). | `the_same_presses_judge_the_same_at_30_60_and_144_frames`; `a_press_is_judged_at_its_own_time_not_the_frames` |
| 3 | A chart is data: song id, difficulty, offset, beat map (tempo and first beat), and a list of targets, each a tap or a hold with time (µs), position on a 1600×900 playfield, combo number, and for a hold a path of points and an end time. Stored as JSON in `data/charts/<song>-<difficulty>.json`, read by the core, hand-editable. | BO-Q3: generated, but a person can fix one target without regenerating. | `every_chart_round_trips`; `a_chart_with_a_target_outside_the_playfield_is_refused` |
| 4 | Judgement: a press within the window of the nearest unjudged target whose circle holds the cursor is Perfect, Great or Good by distance in time; a target never pressed by the end of its Good window is a Miss. Windows, approach time and radius per difficulty are in the table below. | The genre's core rule, in this game's own numbers (BO-3). | hand-worked cases at each window edge, both sides, for Easy and Master; `a_press_on_no_target_does_nothing`; `two_overlapping_targets_take_the_earlier_first` |
| 5 | Holds (BO-Q6): the head is judged as a tap; while held, the cursor must stay within 240% of the radius of the point moving along the path; each beat tick inside gives 10 points, leaving it breaks combo; letting go within the Good window of the end completes it. | Holds add variety without a new kind of input. | `a_hold_followed_to_the_end_scores_every_tick`; `leaving_the_path_breaks_the_combo`; `letting_go_early_misses_the_end` |
| 6 | Score: each judgement's points (Perfect 100, Great 60, Good 25, Miss 0) times (100 + combo, capped at 100) / 100. Accuracy in hundredths of a percent: points earned over 100 per target. The core formats every number shown. | Integer maths (D2); one place for every number on screen. | hand-worked score for a 10-target chart with one Miss at target 6 |
| 7 | A bot that presses every target at its exact time and centre clears every chart with all Perfect and a full combo. | BO-4: a chart that cannot be cleared is a broken chart. | `every_chart_is_cleared_by_a_perfect_bot` over all 25 |
| **Round 2: the songs and the charts** | | | |
| 8 | Songs: the five tracks of the SoundCloud playlist, once the exact link works and downloads are on (T1, BO-Q12). Until then the build runs on five placeholder songs I generate (click and bass patterns at five tempos), so nothing waits. Each song is served as MP3 from `web/audio/`. | BO-Q2: B; the link given does not answer yet. | `LICENSES.md` names each file; the browser check loads one |
| 9 | Beat finding lives in the core (`beats.rs`): the decoded song (16-bit mono samples, from a build-time tool using ffmpeg) goes in; energy in three bands per 512-sample hop, onset strength from the rise in each band, tempo from the onset envelope's autocorrelation between 70 and 190 beats a minute, and the beat grid by dynamic programming over that tempo. All integer. | A rule, so it belongs in the core and is tested there (BO-1). The tool only decodes. | synthetic songs built in the test: clicks at 120 and 174 bpm found within ±1 bpm and beats within one hop; a click track with one swung beat keeps the grid |
| 10 | The chart generator (`generate.rs`) lays out targets from the beat grid and onsets, seeded per song: which onsets become targets (strongest first, snapped to the difficulty's grid, never closer than its minimum gap), where each goes (distance from the last scaled by the time between them, turned by a seeded angle, kept inside the playfield and off the targets still on screen), where combos break (on phrase boundaries every 4 or 8 beats), and which long notes become holds. | BO-Q3: A. | `generation_is_the_same_every_time` (BO-6); `no_two_visible_targets_overlap`; `targets_stay_inside_the_playfield`; `each_difficulty_is_harder_than_the_one_below` (BO-5) |
| 11 | `tools/chart` (a binary in the core's workspace) decodes each song and writes its five charts to `data/charts/`. Charts are committed; the game never generates in the browser. | Browsers decode MP3 slightly differently, so charts made in the browser could differ between players in one lobby. | `the_committed_charts_match_the_generator` (regenerating changes nothing) |
| **Round 3: the solo game** | | | |
| 12 | Screens from `ui()`: title, song select (mockup *select*), play (mockup *play*), results, settings (audio offset, visual offset, volume, player name). Input: mouse or pen moves the cursor; a click, Z or X, or a touch presses (BO-Q7: kits/mobile for touch). | The brief's "done means". | `tests/browser/interactions.py` plays the first 10 s of one chart with scripted presses and reads the score from `data-*` |
| 13 | Offsets: a calibration screen plays a click every beat and the player taps along; the core takes the average early or late as the audio offset. | Audio output latency differs on every machine. | `calibration_of_taps_20_ms_late_sets_an_offset_of_20_ms` |
| 14 | Saves (BO-Q10, kits/saves): settings, the player's id and name, best per chart, and the ledger, in one versioned save; a save from every deployed build loads on every check. | Bests and offsets would be lost on every visit; Idle Game's lost save. | the kit's tests, `old_saves.rs` |
| **Round 4: the lobby** | | | |
| 15 | `kits/online` with Trystero over Nostr relays, vendored and checksummed; the relays start from Level Twenty's list, each checked before release. Host makes a room code; the invite link is the page address with `#room=CODE` (Raiders). Opening it shows the lobby with a Join button: nothing connects before the press (BO-7). Pasted codes are the fallback. | BO-Q4: A, BO-Q5: A (eight seats). | `online_fence.rs`; `tests/browser/online.py` with two browsers |
| 16 | No rollback (R1.4, a change to D23): the host chooses song and difficulty and sends them; each member presses Ready; the host's Start sends a start message and each browser starts its own copy after a 3-second count-in. Every 500 ms of song time each sends its score, combo and accuracy; at the end, its result and replay. The host's seat passes to the earliest joiner if the host leaves. | Each player plays their own copy; only scores are shared. | `a_lobby_of_eight_agrees_on_the_chart`; `a_ninth_is_told_the_lobby_is_full`; `the_host_leaving_hands_the_lobby_on`; `garbage_and_another_build_change_only_the_status` (from the kit) |
| **Round 5: the ledger (T2)** | | | |
| 17 | A record is one finished play: player id, name, chart, score, accuracy, max combo, judgement counts, the build's chart hash, and the replay (every press as a time and position). Its id is the SHA-256 of its text, computed in the core (a short, tested implementation; no new dependency). | T2: records that spread need a name that cannot collide. | the FIPS 180-4 test vectors; `a_record_round_trips_and_keeps_its_id` |
| 18 | A record is kept only if the core replays its presses against the chart and gets the same score (BO-Q13). This stops a typed-in score; it does not stop a bot that plays perfectly. | A merge-only list anyone can add to needs some floor. | `a_record_whose_score_does_not_match_its_replay_is_refused`; `a_record_for_a_chart_this_build_lacks_is_kept_unchecked_and_marked` |
| 19 | Merge: when browsers meet in a lobby, each sends the ids it holds; each asks for the ones it lacks; each checks and adds them. Merging only adds, and a better record for the same player and chart replaces a worse one, so merging in any order gives the same ledger. The leaderboard is the best record per player per chart. | T2: "the more you play with other people the bigger your leaderboard gets". | `merging_in_any_order_gives_the_same_ledger`; `ana_s_score_reaches_cy_through_ben` (mockup *ledger*); `a_worse_record_never_replaces_a_better_one` |
| 20 | Size: the ledger keeps every player's best per chart; the replay is kept for the top 100 per chart and dropped below (BO-Q14). A record without its replay is still shown and passed on, marked as checked by someone else. | Browser storage holds about 5 MB; 25 charts × 1,000 players without replays is about 2.5 MB. | `the_ledger_drops_replays_below_the_top_100`; a probe of the save's size at 1,000 players |
| **Round 6: art, sound, deploy** | | | |
| 21 | Art in Lowballer's flat TikZ style (BO-Q9): title, song cards, target and ring shapes, judgement marks; each judgement has its own shape as well as its colour (BO-9). Hit sounds made in the core as bytes. | BO-Q9: A. | `art_check.py`; `each_judgement_has_a_shape_of_its_own` |
| 22 | Create the public repository BOSU and publish to GitHub Pages with `./scripts/publish.sh` once `./scripts/check.sh` passes; then the browser check against the live address, and two players over real relays by hand, dated in `HANDOFF.md`. A first solo deploy after round 3 (BO-Q17). | The brief's grant. | `BUILDER_ORIGIN=… tests/browser/check.py` |

## The tuning table (proposed)

| Difficulty | Approach (ms) | Radius | Perfect (±ms) | Great (±ms) | Good (±ms) | Minimum gap (ms) | Grid | Onsets kept | Spacing (units per 100 ms) | Longest jump | Holds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Easy | 1500 | 76 | 50 | 95 | 140 | 600 | beats | strongest 35% | 40 | 300 | 25% |
| Normal | 1200 | 66 | 45 | 85 | 125 | 400 | half beats | 50% | 55 | 420 | 25% |
| Hard | 950 | 58 | 38 | 75 | 110 | 260 | half beats | 65% | 70 | 560 | 20% |
| Expert | 750 | 50 | 32 | 65 | 100 | 170 | quarter beats | 80% | 85 | 700 | 20% |
| Master | 600 | 44 | 26 | 55 | 90 | 120 | quarter beats | 92% | 100 | 820 | 15% |

Playfield 1600×900; targets stay inside x 300–1500, y 120–840, leaving room for the scoreboard. Lobby: 8 seats, scores every 500 ms, 3-second count-in.

## Changes to Builder's defaults this plan makes

- `kits/online`: D3, D4 and the page scan, as the kit states.
- D23: no rollback session; lobby messages and scores only (R1.4).
- `kits/saves`: D17.
- `kits/mobile`: D4 (manifest) and D7 (iPad pass).
- Audio files in `web/audio/`, listed in `LICENSES.md`.

Each goes in `BUILDER-SPEC.md`'s change table when its round lands.

## Open questions for the person

In the journal as answer chips, BO-Q12 to BO-Q17.

## Status

- [ ] Read by the person on:
