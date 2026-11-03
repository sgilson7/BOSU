# Second-order notebook

Stage 6, kept by the agent as the work happens, not afterwards. `B3 Build in self-questioning`.

The code and tests are the work; this file is the record about the work. It is where a decision the agent made alone, an assumption that turned out wrong, or something only the person can do becomes visible to the person. With the plan approved in advance, it is the main place those decisions surface before the person uses the tool. [`docs/procedure.md`](https://github.com/sgilson7/builder-setting/blob/main/docs/procedure.md#the-second-order-notebook) says why.

One row per assumption, surprise or open item. **Kind** is *divergence* (what was assumed, what turned out to be true, what changed), *finding*, or *worklist* (the person's to do). Number rows by round (R1.1, R1.2, then R2.1 after the first triage) so a later round does not renumber an earlier one. **Rule** is the rule the row produced for later sessions, and where it now lives (`CLAUDE.md`, a test, `tests/breaks.json`), or blank. **Status** is *done*, *open*, or *person*; every *person* row is named in `HANDOFF.md`. `scripts/notebook_check.py` checks the form of each row and that the notebook is not empty once `PLAN.md` holds a decision, not what the rows say.

| # | Kind | Assumed or found | What is true | What changed | Rule | Status |
| --- | --- | --- | --- | --- | --- | --- |
| R1.1 | divergence | The description asks for "a 2d browser version of OSU". | Builder's agent builds games in a genre, not a recreation of one existing game. The click-on-the-beat genre is open; osu!'s name, skins, sounds, file format and scoring tables are not this game's. | The brief describes an original game in the genre; the title is BO-Q1. The repository keeps the name BOSU, as granted. | BO-3, journal; CLAUDE.md once agreed | open |
| R1.2 | worklist | The songs can be taken from the SoundCloud set countless-days-in-ableton by gearmaster. | On 2026-11-02 no public set by that name answered at soundcloud.com/gearmaster (that address belongs to another artist, Honkmester), /gear-master or /gearmaster-1. Downloading a stream SoundCloud does not offer for download is against its terms, even for the owner. | The five audio files and their titles come from the person (BO-Q2). The person supplied ten on 2026-11-02 (R6). | | done |
| R1.3 | worklist | The album's makers agree to its use in a public game. | The brief says so in the person's words; the repository needs it written down. | `LICENSES.md` will quote the person and name each track and its makers. | D14, `LICENSES.md` | person |
| R1.4 | divergence | Online play uses kits/online's rollback session (D23). | Players in one lobby each play their own copy of the same chart; nobody's input changes anyone else's game. Rollback has nothing to roll back. | Proposed: the lobby agrees the chart and a start time, then each browser sends its running score; no shared world (BO-Q4). A change to D23, recorded once agreed. | BUILDER-SPEC.md change table | open |
| R1.5 | finding | The core can take time from the page as a frame timestamp (kits/realtime). | A rhythm game's clock is the audio's, not the frame's: judging hits by frame time moves every judgement by the audio output latency and by any dropped frame. | The page passes the audio clock's time (in microseconds, converted in the shim as the realtime kit does) with each input; the core judges against it. | BO-4, a test to write | open |
| R1.6 | worklist | The questions in the opening entry. | Eleven answer chips, BO-Q1 to BO-Q11. | | | person |
| R1.7 | finding | The baseline check runs on an untouched template. | `BRIEF.md` was rewritten while the setup and check ran, so the baseline is the template plus this brief, as in Idle Game. | Recorded in the measured part of the opening entry. | | done |
| R2.1 | worklist | BO-Q2: B, the songs come from the playlist on gearmaster (T1). | The address still answers SoundCloud's not-found page on 2026-11-02 (checked by page and by oEmbed). | The exact link is BO-Q12; rounds 1 to 3 build on five placeholder songs I generate. The link came in the BO-Q12 note and works; what is left is R3.1. | | done |
| R2.2 | finding | The build machine can decode MP3. | ffmpeg is not installed; Homebrew is. | Round 2 installs ffmpeg with Homebrew for `tools/chart`; the game never needs it. | | open |
| R2.3 | divergence | T2 asks for "some form of blockchain". | Spreading scores needs a merge-only set of records with ids, not mining or one agreed chain. | Planned as the ledger (PLAN.md 17 to 20); trust, size and names are BO-Q13 to BO-Q15. | | open |
| R2.4 | worklist | The plan's questions. | BO-Q12 to BO-Q17, on the journal's plan page. Answered 2026-11-02; BO-Q13 B is T3. | | | done |
| R3.1 | worklist | With the right link, the songs can be downloaded from SoundCloud (T1). | soundcloud.com/leefy5/sets/countless-days-in-ableton lists BoneCereal and Monke Walkin' (Leefy5), bunko carnival (boyetano), Grunge Dome and Lime Cruncher (user-449148713). All five are marked not downloadable, so I took nothing. | The files come by downloads turned on or by the person's own copies (BO-Q18). The person put ten tracks in ~/Music/countless-days on 2026-11-02. | | done |
| R3.2 | finding | Target centres may sit between y 120 and 840 (PLAN.md's table). | With Easy's radius of 76, a target at y 840 reaches 916, past the 900-unit field. | `Y_MAX` is 800; the mockups' numbers.json follows. | `windows_nest_and_targets_fit_the_field` in `tuning.rs` | done |
| R3.3 | finding | An input at a tick's exact time could be applied after the tick. | `advance(t)` ran the tick before the move at the same time, so a move made at the tick did not count for it. | An input first runs time up to t − 1 µs, then applies, then the tick at t follows. | `leaving_the_path_breaks_the_combo` | done |
| R3.4 | divergence | The beat grid by dynamic programming (PLAN.md decision 9). | Songs made in Ableton keep one tempo; one grid fitted to the whole song to 10 µs puts every target on the song's own beat, where a grid made by dynamic programming over hops is only good to a hop (5.8 ms). Placeholders came out within 0.01 beats a minute. A song that changes tempo would need the plan's way. | `beats.rs` fits period and phase over the whole song. | `beats.rs` tests | open |
| R3.5 | divergence | The canvas kit draws at a fixed size with shapes at three scales. | The playfield must fill any window, and targets are circles of a size per difficulty that fade in. | `render2d.js` scales the field to the canvas's displayed size; `draw.rs` takes a raw size and adds `circle` and `alpha`. | BUILDER-SPEC.md change table | done |
| R3.6 | finding | Every file under `web/` is text for the page scan. | The songs are MP3; the scan stopped on the first one. | The scan skips media files by extension and still requires every other file to be text. | `MEDIA` in `boundary.rs`; BUILDER-SPEC.md | done |
| R3.7 | finding | The template's check covers touch with `webkit-iphone` when asked. | BO-Q7 wants tablets and phones checked every time. | `check.py` runs `webkit-iphone` and `webkit-ipad` by default. | BUILDER-SPEC.md | done |
| R3.8 | divergence | `kits/realtime`'s `input.js` (PLAN.md decision 1). | This game needs no pointer lock and no tick bits: every event goes to the core with its own timestamp. | `app.js` forwards Pointer Events and Z/X keys itself; codes come from the core's assets. | | done |
| R3.9 | finding | The WebKit console error "Refused to apply a stylesheet" came from the page. | Playwright's WebKit screenshot injects a stylesheet; the page's CSP refuses it, correctly. The check takes no screenshots, so it never sees it. | Nothing in the page; screenshots for the journal are taken in Chromium. | | done |
| R3.10 | finding | The game decodes MP3 the way the chart tool does. | ffmpeg removes the encoder's delay at the start of an MP3 using its LAME header; Chrome and Firefox do too, Safari may not. A constant shift is what the audio offset and calibration absorb. | Charts are analysed from the same MP3 the page plays. | | open |
| R3.11 | worklist | Easy has about one target every two to three seconds on the placeholders (27 in 70 s). | Whether that is too sparse is a matter of feel. | Play it and file a triage row if Easy should be busier; `keep_pct` in `tuning.rs` is the number. | | person |
| R3.12 | worklist | The placeholder songs' titles ("Placeholder 1" to "5", "by BOSU"). | Names a player reads are the person's (BO-10); they go when the real songs arrive. | Renamed on the person's request (R6.1). | | done |
| R3.13 | worklist | The repository is public with no LICENSE file. | Without one, the code is all rights reserved, the safe default; choosing a licence is the person's. | The person chose MIT on 2026-11-02 ("please add mit license"); `LICENSE`. | LICENSE, LICENSES.md | done |
| R4.1 | divergence | Press keys are Z and X, fixed in the page (`assets.keys`). | The person plays with W, A, S and D in one hand and aims with the other (T4). | Every key goes to the core by its `KeyboardEvent.code`; the press keys, up to eight, live in the save (version 2, migrated from 1), and the mouse can be set to aim only. Codes, not letters, so a key keeps its place on any keyboard layout. | `game.rs` key tests; `tests/saves/v2-wasd.json` | done |
| R4.2 | divergence | The host's seat passes to the earliest joiner when the host leaves (PLAN.md decision 16). | The transport is a star: joiners link only to the host, so when the host goes there is nobody to hand the lobby to without a new connection. | The lobby closes for everyone with "The host left"; a player mid-round finishes the song alone. | `the_host_leaving_ends_the_lobby_for_joiners_and_a_joiner_leaving_frees_a_seat` | open |
| R4.3 | finding | Two browsers on different builds could play together. | What must agree is the charts. | The hello carries a hash of the chart index; a browser with other charts is told to reload. | `another_build_or_protocol_is_turned_away_with_a_reason` | done |
| R4.4 | worklist | The room code over real relays works between two browsers on this machine (`tests/browser/relays.py`, 2026-11-02). | Two people on different networks, and phones on mobile data, have not been tried; two strict NATs would need TURN, which this design has not. | Try it with a friend on another network and say what happens. | | person |
| R5.1 | divergence | The core keeps `serde` and `serde_json` only (D1); the plan meant to write SHA-256 itself. | BO-Q13: B wants signatures, and a signature scheme is not code to write by hand. | `ed25519-dalek` 2 (no default features; no randomness: the key comes from the page's bytes) and `sha2` 0.10 join the allowlist. The WebAssembly grew from 393 KB to 864 KB. | `ALLOWED_DEPENDENCIES` in `boundary.rs`; BUILDER-SPEC.md | done |
| R5.2 | divergence | Replays kept for the top 100 per chart (BO-Q14: A). | A replay of a dense chart is about 10 KB; 100 for each of 25 charts is 25 MB, and the browser keeps about 5 MB for this whole site, shared with the other games under sgilson7.github.io. | Replays for the top 100 per chart while all of them fit in 1.5 MB, the best first; past that a record keeps its signature and shows "checked by another player". Only a large group would reach it. | `the_ledger_drops_replays_below_the_top_100_and_records_below_200` | open |
| R5.3 | divergence | Every player's best per chart (BO-Q14: A). | Without a cap the save outgrows the browser's storage at a few thousand players. | The best 200 players per chart are kept. | same test | open |
| R5.4 | finding | The online kit's fence allows no page JavaScript in a subdirectory. | The saves kit puts `store.js` in `web/save/`, and the page scan reads it. | The fence names it in `OTHER_KITS_JS`, and skips media files as the page scan does. | `online_fence.rs` | done |
| R5.5 | finding | A record can be checked on arrival. | Its chart may not be loaded; checking needs it. | A record with a replay waits while the page fetches its chart, then is checked; a record for a chart this build lacks is kept, marked, and never shown on this build's boards. | `check_pending` in `together.rs`; `tests/browser/online.py` | done |
| R6.1 | worklist | The placeholders' titles are the person's to choose (BO-10, R3.12). | The person asked for "funky retro names like \"Sunrise on the Ganges\" or \"3000 Jelly\"". | Their two, and three of mine in that style: Disco Satellite, Velvet Moped, Neon Cassette Shuffle. The five stay as extra songs beside the album's ten. Rename any in `data/songs.json`. | | person |
| R6.2 | finding | A slot under 8% of the loudest is never a target. | One huge hit in a real song set that bar so high that Grunge Dome kept 2 targets on Easy and Space Tsunami 4. | The bar is 8% of the 95th percentile. | `one_huge_hit_does_not_empty_the_chart` | done |
| R6.3 | finding | Rewriting an MP3's tags with ffmpeg leaves its sound as it was. | It dropped the LAME header that tells decoders how much encoder delay to trim, which moved the sound 11 ms, and that changed Space Tsunami's tempo from 109.8 to 152.5. | `tools/songs/strip_tags.py` removes the ID3 blocks and keeps every frame; the decode is byte-identical to the original's. | | done |
| R6.4 | finding | One fitted grid finds each song's beat. | Halves of each song analysed alone agree for most, but Grunge Dome and Space Tsunami have no beat their halves agree on, and Monke Walkin', :) and Ballad Of Mr Nasty move between 120 and 160 (a 3:4 feel). A fit a hair off a whole tempo (119.955) drifts 46 ms over a song. | A whole-number tempo is taken when it scores within 97% of the best; `data/songs.json` may give a song's tempo (`bpm_milli`) from its Ableton project; `chart_tool stability` prints the halves. Grunge Dome and Space Tsunami need the person's ear, or their tempo. The beat finder was rebuilt in round 7 (R7.1); what is left for the person is R7.5. | `a_whole_number_tempo_is_found_exactly`, `a_given_tempo_is_kept_and_only_the_first_beat_is_found` | done |
| R6.5 | finding | Track makers appear as their SoundCloud names. | The person asked that a friend's real name not be in the game. | That friend is credited as "boyetano" everywhere the game shows a name, and the songs carry no ID3 tags, so no name rides inside the files. Older commits in the public repository still hold the friend's SoundCloud handle in records (R3.1, BO-Q18). | `no_name_the_person_asked_to_keep_out_is_in_the_game` | person |
| R6.6 | finding | Every browser decodes an MP3 from the same first sample (R3.10). | Measured: Chromium decodes as ffmpeg does; Firefox and WebKit trim 11.0 ms more from the album's tracks, and WebKit adds 13.1 ms to our own. Players there would hit every target 11 ms early. | The page hands the core the first 3 s of its own decode; the core finds the first strong attack and moves song time by the difference from the build's. `tests/browser/decode.py` shows each engine's shift. | `a_browser_that_trims_the_song_s_start_is_moved_back_onto_the_chart` | done |
| R6.7 | finding | The strongest share of a song's onsets make a good Easy chart. | A loud chorus took them all: bunko carnival, Monke Walkin' and :) had nearly 30 s with no target. The browser check caught it. | The share is taken within each eight-beat phrase. | `a_quiet_opening_still_gets_targets` | done |
| R6.8 | divergence | The art would come in round 6 with the rest of the polish. | The person asked for "a major spruce up with some tikz art in the lowballer style" on the landing page. | A Lowballer style pack (`styles/lowballer`), the landing picture and the tab icon (`tools/art/pictures.py`), and Lowballer's palette across the page. The playfield's targets and marks keep their shapes. | `art.py --check-only` (calm upper left) | done |
| R7.1 | finding | The envelope's strongest repeat, refined, is a song's beat, and every song is straight (four lines a beat). | Checked against an independent beat tracker (librosa) and its onsets: Grunge Dome, Monke Walkin' and Space Tsunami are 120, not 150, 160 and 110; Vamos's Den and Ballad Of Mr Nasty mix straight half beats with triplets, which a four-line grid cannot hold. 120 straight and 160 in triplets share every line, so the hits alone cannot say which is the beat. | Grids of 2, 4, 3 and 6 lines a beat are fitted to the song's onset peaks; the beat is the candidate whose own lines are hit most clearly, weighted by a bell curve on tempo centred on 120 a minute, half an octave wide (the usual prior in beat trackers); the phase is held on the beat's line while the grid's division is chosen. All fifteen songs now get the tempo the independent tracker hears; the slow-tempo doubling (COMFORT_BPM) is gone, the prior does it. Scoring the beat by its own lines rather than the whole grid's fit has no synthetic test that fails without it; the evidence is Space Tsunami, whose whole-grid fit preferred a spurious 156.6. | `a_dotted_rhythm_at_120_is_found_as_120_not_160`, `a_song_in_triplets_gets_lines_on_its_triplets`, `the_tempo_prior_favours_120_and_falls_away_by_octaves` | done |
| R7.2 | finding | A grid line's onset-envelope value says whether a target belongs there. | The envelope is rarely zero, so lines with no hit on them became targets: 45% of Master's targets and a third of Easy's sat more than 25 ms from any onset the independent detector found. Easy, Normal and Hard each picked their own targets, so a pattern learnt on Hard did not carry down. | A line's strength is the strongest onset peak within 12 ms of it, else 0, and a line at 0 is never a target. Beats weigh 125%, half beats 100%, the rest 80%. Each difficulty chooses from the one above's targets, so Easy's are all in Normal, Normal's in Hard. On every onset the independent detector finds, targets within 25 ms rose from 79/79/77/68/63% (Easy to Master) to 88/86/86/80/75%. | `every_target_sits_on_an_onset`, `each_difficulty_s_targets_are_all_in_the_one_above`, `a_line_with_no_onset_on_it_has_no_strength` | done |
| R7.3 | divergence | Circle size is a difficulty's alone. | The person asked that "the size of the circles you hit should differ from map to map". Sized chart by chart, a harder chart could get bigger circles than an easier one on the same song (Monke Walkin' Normal 67 against Easy 66). | Each song gets one size: the square root of its onsets a minute against 348 (`HEARD_PER_MINUTE`, the median of the fifteen), held between 80% and 120%, times each difficulty's radius; each chart carries the result. Easy now runs 63 (Grunge Dome, Sunrise on the Ganges) to 88 units (Vamos's Den), and every song's circles shrink from Easy to Master. Older charts without a radius keep the difficulty's. | `a_busier_song_gets_bigger_circles_within_bounds`, `each_difficulty_s_circles_are_smaller_than_the_one_below_on_a_song`, `a_chart_with_its_own_circle_size_is_judged_by_it`, `a_chart_with_a_circle_size_out_of_range_is_refused` | done |
| R7.4 | finding | A regenerated chart keeps its leaderboard. | A board is keyed by the chart's hash, so every board starts empty with these charts; records for the old charts stay in each ledger but match no chart. | Nothing; said to the person. | | done |
| R7.5 | worklist | Every song's grid can be found from its sound. | Grunge Dome and Space Tsunami still fit poorly: a third and a half of the independent detector's onsets sit on a line, against three quarters or more for the other album tracks, and the two detectors disagree on Grunge Dome's phase by about 20 ms. Vamos's Den reads equally well as 124.6 with triplets or 187 straight; 124.6 is what the independent tracker hears. | A play from the person, or each song's tempo from its Ableton project into `bpm_milli` in `data/songs.json`. | | person |
| R7.6 | divergence | Each difficulty picks its targets from a grid by onset strength (R7.2). | The person: "still the beats dont really map well to the songs when i am playing them, like stylistically the emphasis of the notes seems off", and asked for a "true" version with "every single beat of the song" mapped, the easier ones made from it by removing notes, Ballad Of Mr Nasty first. Measured on Ballad: onset strength is a sum of each band's own rise, so a soft hi-hat after quiet scores as high as a kick; and the fixed grids and gaps cut its triplet eighths (167 ms apart at 120) from every level below Master. | A song with `"true_chart": true` in `data/songs.json` (Ballad only, for now) is charted from its true chart: Master is a target on every hit (373), and each level below keeps the most stressed hits of the one above. Stress is the accent: how much louder the whole mix gets at the hit (`beats::accent`), times its place in the beat. No fixed grid; the minimum gap is at most a beat, a half, a third and a quarter for Easy to Expert. Ballad: 71 / 111 / 172 / 203 / 373 targets. The other fourteen songs are unchanged. | `the_true_chart_has_a_target_on_every_hit`, `from_the_true_chart_the_most_stressed_hits_stay`, `from_the_true_chart_triplets_at_120_stay_on_hard`, `from_the_true_chart_each_difficulty_is_within_the_one_above`, `a_hit_that_lifts_the_whole_mix_is_stressed_more_than_one_that_does_not` | person |

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read.

| Key | String |
| --- | --- |
| `title` | Countless Days |
| `intro` | (was) Press each target as its ring closes. Mouse or pen to aim; click, Z or X to press. On a touch screen, tap the target. |
| `menu.play` | Play |
| `menu.settings` | Settings |
| `menu.back` | Back |
| `select.heading` | Songs |
| `select.difficulty` | Difficulty |
| `select.start` | Start |
| `select.by` | by {by} |
| `select.facts` | {bpm} beats a minute, {length} |
| `select.targets` | {n} targets |
| `select.best` | Best {score}, {accuracy} |
| `select.unplayed` | Not played yet |
| `difficulty.easy` | Easy |
| `difficulty.normal` | Normal |
| `difficulty.hard` | Hard |
| `difficulty.expert` | Expert |
| `difficulty.master` | Master |
| `loading` | Loading {song} |
| `loading.failed` | {song} could not be loaded: {why}. Check the connection and press Start again. |
| `play.score` | {score} |
| `play.accuracy` | {accuracy} |
| `play.combo` | {n} combo |
| `judgement.perfect` | Perfect |
| `judgement.great` | Great |
| `judgement.good` | Good |
| `judgement.miss` | Miss |
| `pause.heading` | Paused |
| `pause.resume` | Resume |
| `pause.retry` | Start again |
| `pause.quit` | Back to songs |
| `results.heading` | {song}, {difficulty} |
| `results.score` | Score {score} |
| `results.accuracy` | Accuracy {accuracy} |
| `results.combo` | Longest combo {n} of {full} |
| `results.counts` | Perfect {perfect}, Great {great}, Good {good}, Miss {miss} |
| `results.holds` | Hold ticks {hit} of {total} |
| `results.timing.early` | Your hits came {ms} ms early on average. |
| `results.timing.late` | Your hits came {ms} ms late on average. |
| `results.timing.none` | No hits to time. |
| `results.best` | A new best for this chart. |
| `results.retry` | Play again |
| `results.songs` | Back to songs |
| `settings.heading` | Settings |
| `settings.audio` | Audio offset: {ms} ms |
| `settings.visual` | Visual offset: {ms} ms |
| `settings.volume` | Volume: {pct}% |
| `settings.name` | Your name |
| `settings.name.note` | Your name travels with your scores to everyone you play with, and on to the people they play with. |
| `settings.less` | Less |
| `settings.more` | More |
| `settings.calibrate` | Calibrate |
| `calibrate.heading` | Calibrate the audio offset |
| `calibrate.intro` | Press Z, X or click on every click you hear. After {n} presses the game sets your audio offset. |
| `calibrate.count` | {n} of {total} presses |
| `calibrate.result` | Your presses came {ms} ms after the clicks. The audio offset is now {offset} ms. |
| `calibrate.done` | Done |
| `calibrate.again` | Again |
| `save.unreadable` | Your saved scores and settings could not be read. They are kept in this browser; a new save has started. |
| `save.newer` | Your saved scores come from a newer version of the game. They are kept in this browser; a new save has started. |
| `save.no_step` | Your saved scores come from a version this one cannot update. They are kept in this browser; a new save has started. |
| `settings.keys` | Press keys: {keys} |
| `settings.keys.none` | No press keys. Add one, or press with the mouse. |
| `settings.keys.add` | Add a key |
| `settings.keys.remove` | Remove {key} |
| `settings.keys.reset` | Back to Z and X |
| `settings.keys.listening` | Press the key to add. Escape cancels. |
| `settings.keys.taken` | {key} is a press key already. |
| `settings.keys.full` | You have {n} press keys. Remove one to add another. |
| `settings.mouse.on` | A click presses as well as aiming. |
| `settings.mouse.off` | The mouse only aims; press with your keys. |
| `settings.mouse.turn_off` | Mouse aims only |
| `settings.mouse.turn_on` | Mouse presses too |
| `menu.host` | Host a lobby |
| `menu.join` | Join a lobby |
| `play.standing` | {score} · {n} combo |
| `results.lobby` | Back to the lobby |
| `results.standings` | This round |
| `results.standing.done` | {rank}. {name}: {score}, {accuracy} |
| `results.standing.playing` | {rank}. {name}: {score}, still playing |
| `board.heading` | Leaderboard |
| `board.empty` | No scores yet on this chart. Finish a play, or play with friends to bring theirs here. |
| `board.row` | {rank}. {name}: {score}, {accuracy} · {mark} |
| `board.you` | you |
| `board.replayed` | replayed here |
| `board.signed` | checked by another player |
| `board.count` | Players on this chart in your ledger: {n}. |
| `online.anonymous` | Player {id} |
| `online.connecting` | Connecting to the host. |
| `online.waiting` | Waiting for players. Send them the invite link. |
| `online.in_lobby` | Connected. |
| `online.reload` | The other player has a different version of the game. Reload the page, then join again. |
| `online.garbled` | A message from another player could not be read; play goes on. |
| `online.full` | That lobby is full. |
| `online.host_left` | The host left, so the lobby has closed. |
| `online.left` | You left the lobby. |
| `online.no_relay` | No relay answered, so no one could be found. Try again, or invite by code. |
| `online.no_path` | Your browsers could not reach each other directly. Try another network, or invite by code. |
| `online.signal_failed` | The connection could not be set up. Try again. |
| `online.bad_code` | That is not an invite code from this game. Copy the whole code and paste it again. |
| `online.bad_room` | A room code is six letters and digits, as in the invite link. |
| `online.copied` | Invite link copied. |
| `join.heading` | Play with friends |
| `join.invited` | You have an invite to lobby {code}. Nothing connects until you press Join. |
| `join.button` | Join {code} |
| `join.code` | Room code |
| `join.note` | Type the six-character code, then press Enter. |
| `lobby.heading` | Lobby {code} |
| `lobby.heading.code` | Lobby by invite code |
| `lobby.link` | Invite link: {link} |
| `lobby.copy` | Copy the invite link |
| `lobby.members` | Players: {n} of {seats} |
| `lobby.member` | {name} |
| `lobby.member.host` | {name} (host) |
| `lobby.member.ready` | {name}, ready |
| `lobby.chosen` | Next: {song}, {difficulty} |
| `lobby.start` | Start |
| `lobby.waiting_ready` | Start opens when every player is ready. |
| `lobby.ready` | Ready |
| `lobby.unready` | Not ready |
| `lobby.leave` | Leave the lobby |
| `paste.heading` | No relay? Invite by code |
| `paste.host_note` | Make a code, send it to one friend however you like, and paste back the reply they send you. |
| `paste.make` | Make an invite code |
| `paste.your_invitation` | Your invite code |
| `paste.their_reply` | Their reply code |
| `paste.invitation` | Invite code from the host |
| `paste.joiner_note` | Paste the whole code, then press Enter. |
| `paste.your_reply` | Your reply code |
| `paste.reply_note` | Send this back to the host. |
| `paste.host_instead` | Host by invite code instead |
| `title.songs` | {n} songs, five difficulties each, by {makers}, and a few of our own. |
| `title.and` | and |
| `intro` | Press each circle as its ring closes in, right on the beat. Aim with the mouse; press with a click or your keys (Z and X to start, any keys you like in Settings). On a touch screen, tap. Host a lobby to play with friends. |
| `title.tagline` | A rhythm game on the album countless days in ableton. |
| `title.how` | How to play |
| `ChartError::BadRadius` (core) | The chart's circle radius {r} is outside 20 to 99. |
