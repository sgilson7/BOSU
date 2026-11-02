#!/usr/bin/env python3
"""Copy an MP3 without its ID3 tags: the ID3v2 block at the start and the
ID3v1 block at the end go, every MPEG frame (the LAME/Xing header frame
included, which tells decoders how much encoder delay to trim) stays byte
for byte. So the file carries no artist name, title or picture, and decodes
exactly as the original (R6.3).

    python3 tools/songs/strip_tags.py SOURCE.mp3 DEST.mp3
"""
import sys

data = open(sys.argv[1], "rb").read()
start = 0
while data[start:start + 3] == b"ID3":
    size = 0
    for b in data[start + 6:start + 10]:
        size = (size << 7) | (b & 0x7F)
    footer = 10 if data[start + 5] & 0x10 else 0
    start += 10 + size + footer
end = len(data)
if data[end - 128:end - 125] == b"TAG":
    end -= 128
assert data[start] == 0xFF and data[start + 1] & 0xE0 == 0xE0, "no MPEG frame where the tags end"
open(sys.argv[2], "wb").write(data[start:end])
