#!/bin/sh
# A composed song from its score: the score from its composition, the audio from the score through the Vector37
# engine (fetched at a fixed commit into target/), the MP3 into web/audio/, the score drawn into docs/scores/.
#   tools/compose/make.sh jugglers-of-vienna       (needs cc, git, ffmpeg, python3; pdflatex for the drawing)
set -e
cd "$(dirname "$0")/../.."
ID="${1:?a song id}"
V37_COMMIT=08742a4198f99e2db2846b00b70ed6808239c447
V37=target/vector37
if [ ! -d "$V37/.git" ]; then
    git clone -q https://github.com/genkigenki/Vector37 "$V37"
fi
git -C "$V37" fetch -q origin "$V37_COMMIT" 2>/dev/null || true
git -C "$V37" checkout -q "$V37_COMMIT"
mkdir -p target/compose
python3 "$V37/tools/gen_tables.py" target/compose/mk_tables.h >/dev/null
cc -O2 -w -Itarget/compose -I"$V37" -o target/compose/render tools/compose/render.c -lm
python3 "tools/compose/$(echo "$ID" | tr - _).py"
python3 tools/compose/score.py "data/scores/$ID.json" "target/compose/$ID.events"
target/compose/render "target/compose/$ID.events" "target/compose/$ID.wav"
ffmpeg -loglevel error -y -i "target/compose/$ID.wav" -map_metadata -1 -codec:a libmp3lame -q:a 2 "web/audio/$ID.mp3"
python3 tools/compose/score_tikz.py "data/scores/$ID.json" "docs/scores/$ID"
echo "made web/audio/$ID.mp3 and docs/scores/$ID.pdf"
