# Licences

Every file in this repository that is not code written for it: its source, its author, its terms. The code is under the repository's licence, MIT (`LICENSE`, chosen by the person on 2026-11-02). The album's songs are not covered by it: their terms are in the row below.

## Assets

| Path | Source | Terms |
| --- | --- | --- |
| `web/art/` | drawn by `tools/art/art.py` from TikZ in this repository | this repository's licence |
| `web/audio/` for sunrise-on-the-ganges, 3000-jelly, disco-satellite, velvet-moped, neon-cassette-shuffle | synthesised by `tools/placeholders/make.sh` with ffmpeg's `aevalsrc` (kick, snare, hats, bass); made for this project | this repository's licence; first placeholders, now five extra songs (R6.1) |
| `data/analysis/`, `data/charts/` | written by `crates/chart_tool` from the songs above | this repository's licence |
| The album *countless days in ableton*: `web/audio/` for bunko-carnival, bonecereal, grunge-dome, lime-cruncher, monke-walkin, vamoss-den, ballad-of-mr-nasty, space-tsunami, smiley, baped-out | the person's files in `~/Music/countless-days` (2026-11-02), copied without their ID3 tags by `tools/songs/strip_tags.py`; made by Leefy5, Emracool and boyetano (a name the person chose so a friend's real name is not in the game, R6.5) | the person's words in the brief: "copyright free songs my friends and i made so there are no reprecussions to using them" |

## Vendored code

| Path | Project and version | Author | Licence | Checksum |
| --- | --- | --- | --- | --- |
| `web/net/vendor/trystero-nostr-0.25.4.js` | Trystero 0.25.4, Nostr strategy, bundled | Dan Motzenbecker | MIT | `web/net/vendor/SHA256SUMS` |

## Code taken from other repositories

| From | What | Licence |
| --- | --- | --- |
| Raiders of the Lost Mart (sgilson7), via Vagrancy and Floodline | `web/net/rtc.js`: the star transport, repeated role, pasted codes | the person's own code |
| crates.io: `ed25519-dalek` 2, `sha2` 0.10 and what they pull in | signing and hashing in the core | BSD-3-Clause; MIT or Apache-2.0 |

A file with no licence, a likeness, a recording or any third-party picture is a worklist row in `SECOND-ORDER.md` for the person to decide, and stays out of git until they do.
