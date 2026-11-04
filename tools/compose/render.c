/* SPDX-License-Identifier: GPL-3.0-only
 * Renders a score's parts through the Vector37 engine (genkigenki/Vector37, GPL-3.0-only, by M. Geisler) and mixes
 * them to one stereo WAV. This file includes the engine's sources, so it is under the engine's licence, not the
 * repository's (LICENSES.md); the audio it writes is not.
 *   render EVENTS OUT.wav      EVENTS: the lines tools/compose/score.py writes:
 *     length FRAMES
 *     part NAME PRESET GAIN_MILLI PAN    PRESET: a Vector37 factory sound 0..31, or -1 the drums below; PAN -100..100
 *     tempo BPM                          the part's tempo (its orbit turns in time)
 *     on FRAME NOTE VEL | off FRAME NOTE (frames ascending within a part) */
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include "mk_tables.h"
#define RING_PUBLISH() ((void)0)
#include "firmware/src/synth.h"
#include "firmware/src/orbit.c"
#include "firmware/src/engine.c"
#include "firmware/src/analog.c"
#include "firmware/src/fx.c"
#include "firmware/src/mix.c"
#include "firmware/src/params.c"
#include "firmware/src/presets.c"

typedef struct { uint32_t at; int on; uint32_t note, vel; } ev_t;

static float *mixl, *mixr;
static uint32_t frames;

static void reset(const patch_t *p)
{
    memset(vs, 0, sizeof vs);
    memset(&down, 0, sizeof down);
    memset(&lat, 0, sizeof lat);
    memset(dl_buf, 0, sizeof dl_buf);
    memset(pl_mem, 0, sizeof pl_mem);
    memset(ch_buf, 0, sizeof ch_buf);
    memset(&pls, 0, sizeof pls);
    memset(&dls, 0, sizeof dls);
    ev_r = ev_w = 0;
    syn.oph = syn.sph = 0;
    syn.arp_note = -1, syn.arp_run = 0, syn.latched = 0;
    focus = solo = latch = 0;
    joy_x = joy_y = 0;
    vmax = NVOICE;
    pt = *p;
}

static void add(uint32_t i, double l, double r, double gain, int pan)
{
    double pl = pan <= 0 ? 1.0 : 1.0 - pan / 100.0, pr = pan >= 0 ? 1.0 : 1.0 + pan / 100.0;
    if (i < frames)
        mixl[i] += (float)(l * gain * pl), mixr[i] += (float)(r * gain * pr);
}

/* one part through the engine: its events at their control blocks */
static void engine_part(int preset, uint32_t bpm, const ev_t *e, uint32_t n, double gain, int pan)
{
    patch_t p;
    int32_t out[2 * CTL];
    uint32_t b, i, k = 0, nb = frames / CTL;
    preset_make((uint32_t)preset, &p);
    p.g[G_ARP] = ARP_OFF;
    p.g[G_TEMPO] = (int16_t)bpm;
    reset(&p);
    for (b = 0; b < nb; b++) {
        while (k < n && e[k].at < (b + 1) * CTL) {
            /* the engine's queue holds 64: push in small groups */
            ev_push(e[k].on ? EV_ON : EV_OFF, e[k].note, e[k].vel);
            k++;
        }
        synth_block(out, CTL);
        for (i = 0; i < CTL; i++)
            add(b * CTL + i, out[2 * i] / 32768.0, out[2 * i + 1] / 32768.0, gain, pan);
    }
}

/* the drums: 36 a kick, 42 a shaker, 81 a triangle, 39 a clap; a fixed seed, so every render is the same */
static uint32_t seed = 12345;
static double noise(void) { seed = seed * 1664525u + 1013904223u; return (int32_t)seed / 2147483648.0; }
static void drum(uint32_t at, uint32_t note, uint32_t vel, double gain, int pan)
{
    double v = vel / 127.0, lp = 0, hp_prev = 0, hp = 0;
    uint32_t i, len;
    switch (note) {
    case 36:   /* a soft kick: a sine falling from 110 to 48 Hz */
        len = FS / 3;
        for (i = 0; i < len; i++) {
            double t = (double)i / FS, f = 48 + 62 * exp(-t * 30), ph = 2 * M_PI * (48 * t + 62 * (1 - exp(-t * 30)) / 30);
            double s = sin(ph) * exp(-t * 9) * v;
            (void)f;
            add(at + i, s, s, gain, pan);
        }
        break;
    case 42:   /* a shaker: high-passed noise, a short swell and fall */
        len = FS / 12;
        for (i = 0; i < len; i++) {
            double t = (double)i / FS, x = noise(), env = (1 - exp(-t * 400)) * exp(-t * 55);
            hp = 0.85 * (hp + x - hp_prev), hp_prev = x;
            add(at + i, hp * env * v * 0.5, hp * env * v * 0.5, gain, pan);
        }
        break;
    case 81:   /* a triangle: inharmonic partials ringing on */
        len = FS * 3 / 2;
        for (i = 0; i < len; i++) {
            double t = (double)i / FS, s = 0;
            s += sin(2 * M_PI * 1760 * t) * 0.5 + sin(2 * M_PI * 4290 * t) * 0.3 + sin(2 * M_PI * 6810 * t) * 0.2;
            s *= exp(-t * 2.8) * v * 0.35;
            add(at + i, s, s, gain, pan);
        }
        break;
    default:   /* a clap: three quick bursts of band-passed noise, then a tail */
        len = FS / 5;
        for (i = 0; i < len; i++) {
            double t = (double)i / FS, x = noise(), env;
            double burst = fmod(t, 0.011) < 0.004 && t < 0.033 ? 1 : 0;
            lp += 0.3 * (x - lp);
            hp = 0.7 * (hp + lp - hp_prev), hp_prev = lp;
            env = burst + (t >= 0.022 ? exp(-(t - 0.022) * 22) : 0);
            add(at + i, hp * env * v * 0.6, hp * env * v * 0.6, gain, pan);
        }
        break;
    }
}

static void wav(const char *path)
{
    FILE *f = fopen(path, "wb");
    uint32_t v, sr = FS, br = FS * 4, i;
    uint16_t a = 1, ch = 2, ba = 4, bits = 16;
    double peak = 1e-9, scale;
    if (!f) { perror(path); exit(1); }
    for (i = 0; i < frames; i++) {
        if (fabs(mixl[i]) > peak) peak = fabs(mixl[i]);
        if (fabs(mixr[i]) > peak) peak = fabs(mixr[i]);
    }
    scale = 0.89 / peak;   /* the loudest sample at -1 dBFS */
    fwrite("RIFF", 1, 4, f); v = 36 + frames * 4; fwrite(&v, 4, 1, f);
    fwrite("WAVEfmt ", 1, 8, f); v = 16; fwrite(&v, 4, 1, f);
    fwrite(&a, 2, 1, f); fwrite(&ch, 2, 1, f); fwrite(&sr, 4, 1, f); fwrite(&br, 4, 1, f);
    fwrite(&ba, 2, 1, f); fwrite(&bits, 2, 1, f);
    fwrite("data", 1, 4, f); v = frames * 4; fwrite(&v, 4, 1, f);
    for (i = 0; i < frames; i++) {
        int16_t s[2] = {(int16_t)lrint(mixl[i] * scale * 32767), (int16_t)lrint(mixr[i] * scale * 32767)};
        fwrite(s, 2, 2, f);
    }
    fclose(f);
    fprintf(stderr, "render: %u frames, peak %.2f before scaling\n", frames, peak);
}

int main(int argc, char **argv)
{
    FILE *in;
    char line[256], name[64];
    static ev_t ev[200000];
    uint32_t n = 0, bpm = 120;
    int preset = 0, pan = 0, have = 0;
    double gain = 1;
    if (argc != 3) { fprintf(stderr, "usage: render EVENTS OUT.wav\n"); return 2; }
    if (!(in = fopen(argv[1], "r"))) { perror(argv[1]); return 1; }
#define FLUSH()                                                                                    \
    do {                                                                                           \
        if (have) {                                                                                \
            fprintf(stderr, "render: %-10s %u events\n", name, n);                                 \
            if (preset < 0) { for (uint32_t j = 0; j < n; j++) if (ev[j].on) drum(ev[j].at, ev[j].note, ev[j].vel, gain, pan); } \
            else engine_part(preset, bpm, ev, n, gain, pan);                                       \
        }                                                                                          \
        n = 0;                                                                                     \
    } while (0)
    while (fgets(line, sizeof line, in)) {
        uint32_t at, note, vel, len;
        int g;
        if (sscanf(line, "length %u", &len) == 1) {
            frames = len / CTL * CTL + CTL;
            mixl = calloc(frames, sizeof *mixl), mixr = calloc(frames, sizeof *mixr);
        } else if (sscanf(line, "part %63s %d %d %d", name, &preset, &g, &pan) == 4) {
            FLUSH();
            have = 1, gain = g / 1000.0;
        } else if (sscanf(line, "tempo %u", &bpm) == 1) {
        } else if (sscanf(line, "on %u %u %u", &at, &note, &vel) == 3) {
            ev[n++] = (ev_t){at, 1, note, vel};
        } else if (sscanf(line, "off %u %u", &at, &note) == 2) {
            ev[n++] = (ev_t){at, 0, note, 0};
        }
    }
    FLUSH();
    if (!frames) { fprintf(stderr, "render: no length\n"); return 1; }
    wav(argv[2]);
    return 0;
}
