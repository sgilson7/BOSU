#!/usr/bin/env bash
# Makes the five songs of our own (first placeholders, R2.1; named in R6.1) with ffmpeg's own
# synthesiser: kick, snare, hats and a bass line at five tempos. They stand
# in until the album's tracks arrive (R3.1) and are this project's own work
# (LICENSES.md). Run from the repository root:  tools/placeholders/make.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
command -v ffmpeg >/dev/null || { echo "needs ffmpeg (brew install ffmpeg)" >&2; exit 1; }

# id, beats a minute, seconds, root note in Hz
songs=(
  "sunrise-on-the-ganges 100 70 55"
  "3000-jelly 118 75 49"
  "disco-satellite 132 80 61.74"
  "velvet-moped 150 80 58.27"
  "neon-cassette-shuffle 172 85 46.25"
)
for s in "${songs[@]}"; do
  read -r id bpm secs root <<<"$s"
  B=$(echo "scale=8; 60/$bpm" | bc)
  # p: position in the beat; e: position in the eighth; bar: which bar
  p="mod(t,$B)"; e="mod(t,$B/2)"; s16="mod(t,$B/4)"
  beat="floor(t/$B)"; bar="floor(t/(4*$B))"
  intro="gte(t,4*4*$B)"
  kick="0.9*sin(2*PI*(45+150*exp(-$p*35))*$p)*exp(-$p*7)*$intro"
  snare="0.45*(random(0)*2-1)*exp(-$p*16)*eq(mod($beat,2),1)*$intro"
  hats="0.12*(random(1)*2-1)*exp(-$e*70)"
  sixteenths="0.07*(random(2)*2-1)*exp(-$s16*90)*gte(mod($bar,8),4)"
  step="mod(floor(t/($B/2)),8)"
  semis="if(eq($step,3),3,if(eq($step,6),7,if(eq($step,7),5,0)))+if(eq(mod($bar,4),2),5,if(eq(mod($bar,4),3),-2,0))"
  bass="0.32*sin(2*PI*$root*pow(2,($semis)/12)*t)*exp(-$e*5)*$intro"
  fade="min(1,($secs-t)/2)"
  ffmpeg -loglevel error -y -f lavfi \
    -i "aevalsrc=exprs='($kick+$snare+$hats+$sixteenths+$bass)*0.7*$fade':s=44100:d=$secs" \
    -ac 1 -codec:a libmp3lame -b:a 128k "web/audio/$id.tagged.mp3"
  python3 tools/songs/strip_tags.py "web/audio/$id.tagged.mp3" "web/audio/$id.mp3"
  rm "web/audio/$id.tagged.mp3"
  echo "web/audio/$id.mp3  $bpm bpm  $secs s"
done
