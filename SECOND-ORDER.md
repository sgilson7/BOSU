# Second-order notebook

Stage 6, kept by the agent as the work happens, not afterwards. `B3 Build in self-questioning`.

The code and tests are the work; this file is the record about the work. It is where a decision the agent made alone, an assumption that turned out wrong, or something only the person can do becomes visible to the person. With the plan approved in advance, it is the main place those decisions surface before the person uses the tool. [`docs/procedure.md`](https://github.com/sgilson7/builder-setting/blob/main/docs/procedure.md#the-second-order-notebook) says why.

One row per assumption, surprise or open item. **Kind** is *divergence* (what was assumed, what turned out to be true, what changed), *finding*, or *worklist* (the person's to do). Number rows by round (R1.1, R1.2, then R2.1 after the first triage) so a later round does not renumber an earlier one. **Rule** is the rule the row produced for later sessions, and where it now lives (`CLAUDE.md`, a test, `tests/breaks.json`), or blank. **Status** is *done*, *open*, or *person*; every *person* row is named in `HANDOFF.md`. `scripts/notebook_check.py` checks the form of each row and that the notebook is not empty once `PLAN.md` holds a decision, not what the rows say.

| # | Kind | Assumed or found | What is true | What changed | Rule | Status |
| --- | --- | --- | --- | --- | --- | --- |
| R1.1 | divergence | The description asks for "a 2d browser version of OSU". | Builder's agent builds games in a genre, not a recreation of one existing game. The click-on-the-beat genre is open; osu!'s name, skins, sounds, file format and scoring tables are not this game's. | The brief describes an original game in the genre; the title is BO-Q1. The repository keeps the name BOSU, as granted. | BO-3, journal; CLAUDE.md once agreed | open |
| R1.2 | worklist | The songs can be taken from the SoundCloud set countless-days-in-ableton by gearmaster. | On 2026-11-02 no public set by that name answered at soundcloud.com/gearmaster (that address belongs to another artist, Honkmester), /gear-master or /gearmaster-1. Downloading a stream SoundCloud does not offer for download is against its terms, even for the owner. | The five audio files and their titles come from the person (BO-Q2). | | person |
| R1.3 | worklist | The album's makers agree to its use in a public game. | The brief says so in the person's words; the repository needs it written down. | `LICENSES.md` will quote the person and name each track and its makers. | D14, `LICENSES.md` | person |
| R1.4 | divergence | Online play uses kits/online's rollback session (D23). | Players in one lobby each play their own copy of the same chart; nobody's input changes anyone else's game. Rollback has nothing to roll back. | Proposed: the lobby agrees the chart and a start time, then each browser sends its running score; no shared world (BO-Q4). A change to D23, recorded once agreed. | BUILDER-SPEC.md change table | open |
| R1.5 | finding | The core can take time from the page as a frame timestamp (kits/realtime). | A rhythm game's clock is the audio's, not the frame's: judging hits by frame time moves every judgement by the audio output latency and by any dropped frame. | The page passes the audio clock's time (in microseconds, converted in the shim as the realtime kit does) with each input; the core judges against it. | BO-4, a test to write | open |
| R1.6 | worklist | The questions in the opening entry. | Eleven answer chips, BO-Q1 to BO-Q11. | | | person |
| R1.7 | finding | The baseline check runs on an untouched template. | `BRIEF.md` was rewritten while the setup and check ran, so the baseline is the template plus this brief, as in Idle Game. | Recorded in the measured part of the opening entry. | | done |
| R2.1 | worklist | BO-Q2: B, the songs come from the playlist on gearmaster (T1). | The address still answers SoundCloud's not-found page on 2026-11-02 (checked by page and by oEmbed). | The exact link is BO-Q12; rounds 1 to 3 build on five placeholder songs I generate. The link came in the BO-Q12 note and works; what is left is R3.1. | | done |
| R2.2 | finding | The build machine can decode MP3. | ffmpeg is not installed; Homebrew is. | Round 2 installs ffmpeg with Homebrew for `tools/chart`; the game never needs it. | | open |
| R2.3 | divergence | T2 asks for "some form of blockchain". | Spreading scores needs a merge-only set of records with ids, not mining or one agreed chain. | Planned as the ledger (PLAN.md 17 to 20); trust, size and names are BO-Q13 to BO-Q15. | | open |
| R2.4 | worklist | The plan's questions. | BO-Q12 to BO-Q17, on the journal's plan page. Answered 2026-11-02; BO-Q13 B is T3. | | | done |
| R3.1 | worklist | With the right link, the songs can be downloaded from SoundCloud (T1). | soundcloud.com/leefy5/sets/countless-days-in-ableton lists BoneCereal and Monke Walkin' (Leefy5), bunko carnival (cay-wagner), Grunge Dome and Lime Cruncher (user-449148713). All five are marked not downloadable, so I took nothing. | The files come by downloads turned on or by the person's own copies (BO-Q18). Placeholders until then. | | person |
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
| R3.12 | worklist | The placeholder songs' titles ("Placeholder 1" to "5", "by BOSU"). | Names a player reads are the person's (BO-10); they go when the real songs arrive. | | | person |
| R3.13 | worklist | The repository is public with no LICENSE file. | Without one, the code is all rights reserved, the safe default; choosing a licence is the person's. | | | person |

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read.

| Key | String |
| --- | --- |
| `title` | Countless Days |
| `intro` | Press each target as its ring closes. Mouse or pen to aim; click, Z or X to press. On a touch screen, tap the target. |
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
