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

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read.

| Key | String |
| --- | --- |
