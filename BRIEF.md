# Brief

Stage 1. The person's own words come first; the rest is written by the agent from them, for the person to read. `C1 Frame the problem and set the rules`, `B1 Establish context`.

## The description, word for word

> the game is as follows: a 2d browser version of OSU, but the songs are taken from countless-days-in-ableton from soundcloud from gearmaster, as they are copyright free songs my friends and i made so there are no reprecussions to using them. the mvp should have all 5 maps modeled with 5 difficulties each, and the game should be playable online in a lobby with friends, with an invite link

The opening, also word for word: "Use Builder to constrain and direct your work as the game's co-designer and developer. The goal is a page on GitHub Pages that anyone can open in a browser, so put running well in the browser first."

## The tool

A 2D rhythm game in the browser: targets appear on a playfield in time with the music, and the player hits each one on its beat with the mouse (or keys, or a finger). Five songs from the person's and their friends' album *countless days in ableton* (SoundCloud, account gearmaster), each with five difficulties. Friends play the same song together from a lobby they reach by an invite link. Runs on a desktop browser first; phones are a question (BO-Q7).

The genre comes from click-on-the-beat rhythm games, osu! the best known of them. This game is an original one in that genre: its own name, art, sounds, scoring and object set. It does not use osu!'s name, skins, sounds, file format or scoring tables (BO-3).

## Done means

- Five songs, each with five difficulties (25 charts), all playable from start to end, and each chart shown to be clearable by a bot that hits every object (a core test).
- A difficulty is measurably harder than the one below it on each song: more objects per minute, shorter approach time, or tighter hit windows (a core test over the 25 charts).
- A player can open the page, pick a song and difficulty, play, and see a result screen, in Chromium, Firefox and WebKit.
- A player can press Host, copy an invite link, send it to a friend, and both play the same chart together and see each other's scores; checked by hand over real relays, with the date in `HANDOFF.md`, and between two browsers on one machine by `tests/browser/online.py`.
- Hits are judged against the audio clock, not the frame clock, so a dropped frame does not move a judgement (a core test feeding the same hits at 30, 60 and 144 frames a second).
- The page loads and idles without any off-origin request; the lobby reaches relays only after Host or Join.
- Deployed to GitHub Pages as the public repository BOSU.

## Decisions that stay with the person

- The five songs, their audio files, and confirmation that the album's makers agree to their use (the brief says they do; it goes in `LICENSES.md` in the person's words).
- Every chart's feel: the agent generates and tunes them, the person plays them and files triage rows.
- The game's name (BO-Q1), difficulty names, and any other list a player reads.
- Whether online play goes ahead with public Nostr relays, which see that two browsers met.

## Accessibility

Each judgement is carried by a word and a shape as well as a colour. Targets are numbered within a combo. A visual offset and an audio offset the player can set. Mouse, keyboard taps, and (if BO-Q7) touch.

## Style

Flat 2D drawn from TikZ, in the manner of Lowballer, unless BO-Q9 says otherwise.

## References

- **Raiders of the Lost Mart** (~/Documents/RaidersOfTheLostMart): the invite link (`#room=` on the page address), the Host/Join lobby, `web/net/` transport and relay list
- **Level Twenty** (~/Documents/LevelTwenty): the pasted-code invitation as the fallback when relays fail, garbage and wrong-build handling in the online core
- **Lowballer** (https://github.com/sgilson7/lowballer): flat 2D drawn from TikZ art
- **Idle Game** (~/Documents/IdleGame): the journal's shape and answer chips; versioned saves, if best scores are kept

## Words

Terms from the genre the person may already use: map (a song's set of charts), difficulty, beat, combo, accuracy. They go into `GLOSSARY.md` with this game's own definitions before any string uses them.

## Autonomy

The person's grant, word for word: "You may create a public repository named BOSU and deploy it to GitHub Pages when the checks pass." "Do not use subagents."

The plan is not approved in advance: the agent writes the opening entry, then `PLAN.md`, and stops for the person each time. Where this brief is silent during the build, the agent decides, records the decision in `SECOND-ORDER.md`, and continues. The agent may not loosen a Builder requirement or a boundary test, and may not add a server, an account or analytics.

- Deploys: `when the checks pass`
- Subagents: `none`

## Journal

`default`

## Allowed core dependencies

`serde`, `serde_json` (reference default).
