# Licences

Every file in this repository that is not code written for it: its source, its author, its terms. The code is under the repository's licence, MIT (`LICENSE`, chosen by the person on 2026-11-02). The album's songs are not covered by it: their terms are in the row below.

## Assets

| Path | Source | Terms |
| --- | --- | --- |
| `web/art/` | drawn by `tools/art/art.py` from TikZ in this repository | this repository's licence |
| `web/audio/placeholder-1.mp3` to `placeholder-5.mp3` | synthesised by `tools/placeholders/make.sh` with ffmpeg's `aevalsrc` (kick, snare, hats, bass); made for this project | this repository's licence; they stand in until the album's tracks arrive (R3.1) |
| `data/analysis/`, `data/charts/` | written by `crates/chart_tool` from the songs above | this repository's licence |
| The album *countless days in ableton* (not yet in the repository) | soundcloud.com/leefy5/sets/countless-days-in-ableton: Leefy5, cay-wagner, user-449148713 | the person's words in the brief: "copyright free songs my friends and i made so there are no reprecussions to using them". Each track's makers to be named here when the files arrive (R1.3) |

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
