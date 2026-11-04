//! Finding the beat in a song (PLAN.md decision 9). Integer only.
//!
//! The song comes in as 16-bit mono samples at `RATE` (the chart tool
//! decodes it; the core only reads numbers). Three bands are split with
//! one-pole filters; each band's energy per hop is put on a log scale; an
//! onset is a rise in that energy, and a peak in the rises is a hit a
//! player hears. The envelope's autocorrelation between 70 and 190 beats a
//! minute gives candidate tempos; each, and the tempos a half, a third and a
//! quarter away, is fitted to the microsecond as one fixed grid, straight
//! (four lines a beat) and in triplets (three), by how many peaks lie on its
//! lines beyond chance. Grids that fit about as well are told apart by
//! which line of the beat the peaks favour: 120 straight and 160 in
//! triplets share every line, and only the beat says which it is (R7.1).
//! Songs made in Ableton keep one tempo, so one grid serves the whole song
//! (R3.4, a change from the plan's dynamic programming).

use serde::{Deserialize, Serialize};

/// Samples a second.
pub const RATE: i64 = 22_050;
/// Samples a hop: 128 at 22,050 a second is about 5.8 ms.
pub const HOP: i64 = 128;
pub const MIN_BPM: i64 = 70;
pub const MAX_BPM: i64 = 190;
/// A whole-number tempo is taken when its grid scores at least this share
/// (percent) of the best fit's.
pub const WHOLE_BPM_PCT: i64 = 97;
/// Grid lines a beat in a straight song.
pub const STRAIGHT: i64 = 4;

fn straight() -> i64 {
    STRAIGHT
}

pub const US: i64 = 1_000_000;

/// What the chart generator needs from a song.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Analysis {
    pub beat_us: i64,
    /// The first beat at or after 0.
    pub first_beat_us: i64,
    pub length_us: i64,
    /// Grid lines a beat: 4 for a straight song with quarter beats, 2 for
    /// one with half beats only, 3 for one in triplets, 6 for one with both
    /// straight half beats and triplets (R7.1).
    #[serde(default = "straight")]
    pub division: i64,
    /// The strongest onset on each grid line from `first_beat_us`, or 0
    /// where no onset is (R7.2).
    pub slots: Vec<u32>,
    /// How much louder the whole mix gets at each grid line's onset (see
    /// `accent`), or 0 where no onset is: which hits a player hears as
    /// stressed (R7.6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accents: Vec<u32>,
    /// Where the first sound is in this decode. A browser that trims the
    /// start of an MP3 differently finds it elsewhere, and the game moves
    /// song time by the difference (R6.6).
    #[serde(default)]
    pub first_sound_us: i64,
    /// Beats a bar, for the groove and the combo's phrases: 4 unless a
    /// score says otherwise (R8.2).
    #[serde(default = "four", skip_serializing_if = "is_four")]
    pub bar_beats: i64,
    /// From a score: how many grid lines the lead's note starting on each
    /// line lasts, 0 where none starts; a hold may follow it (R8.2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sustains: Vec<u32>,
}

fn four() -> i64 {
    4
}

fn is_four(v: &i64) -> bool {
    *v == 4
}

/// How much of a song's opening is searched for its first sound, in
/// seconds. The page hands the core this much of its own decode.
pub const HEAD_SECONDS: i64 = 3;
/// The first sound is the first sample at least this share (1 in N) of the
/// opening's loudest: a strong attack, which every decoder and sample rate
/// places within a twentieth of a millisecond, where a fixed low level
/// wanders by ten on a song that fades in (R6.6).
pub const FIRST_SOUND_SHARE: i32 = 4;

/// The time of the first sound in the opening `HEAD_SECONDS`, in
/// microseconds at `rate` samples a second.
pub fn first_sound_us(samples: &[i16], rate: i64) -> Option<i64> {
    let head = &samples[..samples.len().min((HEAD_SECONDS * rate) as usize)];
    let peak = head.iter().map(|s| (*s as i32).abs()).max()?;
    if peak == 0 {
        return None;
    }
    let k = head
        .iter()
        .position(|s| (*s as i32).abs() * FIRST_SOUND_SHARE >= peak)?;
    Some(k as i64 * US / rate)
}

impl Analysis {
    pub fn bpm_milli(&self) -> i64 {
        60 * US * 1000 / self.beat_us
    }

    pub fn slot_time(&self, slot: usize) -> i64 {
        self.first_beat_us + slot as i64 * self.beat_us / self.division
    }
}

/// A log scale in 256ths of a doubling.
fn log2_256(v: i64) -> i64 {
    if v <= 0 {
        return 0;
    }
    let l = 63 - v.leading_zeros() as i64;
    let frac = if l >= 8 {
        (v >> (l - 8)) & 255
    } else {
        (v << (8 - l)) & 255
    };
    l * 256 + frac
}

/// One onset strength per hop.
pub fn envelope(samples: &[i16]) -> Vec<i64> {
    envelope_of(&bands(samples))
}

/// Each hop's energy in three bands: below about 200 Hz, 200 Hz to 2 kHz,
/// and above.
fn bands(samples: &[i16]) -> Vec<[i64; 3]> {
    // one-pole low-pass filters, in 1024ths: about 200 Hz and 2 kHz
    const LOW_A: i64 = 58;
    const MID_A: i64 = 480;
    let (mut lp_low, mut lp_mid) = (0i64, 0i64);
    let hops = samples.len() / HOP as usize;
    let mut bands = vec![[0i64; 3]; hops];
    for (k, &s) in samples.iter().take(hops * HOP as usize).enumerate() {
        let x = (s as i64) << 10;
        lp_low += (x - lp_low) * LOW_A / 1024;
        lp_mid += (x - lp_mid) * MID_A / 1024;
        let hop = &mut bands[k / HOP as usize];
        hop[0] += (lp_low >> 10).abs();
        hop[1] += ((lp_mid - lp_low) >> 10).abs();
        hop[2] += ((x - lp_mid) >> 10).abs();
    }
    bands
}

/// Hops before a hit over which the quiet it rises from is found, and hops
/// from it over which its loudest is found.
const ACCENT_BEFORE: usize = 4;
const ACCENT_AFTER: usize = 3;

/// How much louder the whole mix gets at the hit in hop `k`, in 256ths of a
/// doubling: its loudest against the quietest just before it. A kick or a
/// snare lifts the whole mix; a hi-hat over a held chord barely does, where
/// its own band's rise can be as large as a kick's (R7.6).
fn accent(bands: &[[i64; 3]], k: usize) -> i64 {
    let total = |i: usize| bands.get(i).map(|b| b[0] + b[1] + b[2]).unwrap_or(0);
    let loud = (k..k + ACCENT_AFTER).map(total).max().unwrap_or(0);
    let quiet = (k.saturating_sub(ACCENT_BEFORE)..k).map(total).min().unwrap_or(0);
    (log2_256(loud) - log2_256(quiet.max(1))).max(0)
}

fn envelope_of(bands: &[[i64; 3]]) -> Vec<i64> {
    let hops = bands.len();
    let logs: Vec<[i64; 3]> = bands
        .iter()
        .map(|b| [log2_256(b[0]), log2_256(b[1]), log2_256(b[2])])
        .collect();
    let mut rise = vec![0i64; hops];
    for n in 1..hops {
        rise[n] = (0..3).map(|b| (logs[n][b] - logs[n - 1][b]).max(0)).sum();
    }
    // take away the local mean, so steady noise is not an onset
    const W: usize = 16;
    let mut prefix = vec![0i64; hops + 1];
    for n in 0..hops {
        prefix[n + 1] = prefix[n] + rise[n];
    }
    (0..hops)
        .map(|n| {
            let (a, b) = (n.saturating_sub(W), (n + W + 1).min(hops));
            let mean = (prefix[b] - prefix[a]) / (b - a) as i64;
            (rise[n] - mean).max(0)
        })
        .collect()
}

fn hop_us() -> i64 {
    HOP * US / RATE
}

/// The middle of hop `k`, in microseconds, without the rounding of
/// `hop_us` adding up over a song: an onset found in a hop lies somewhere
/// in it.
fn hop_time(k: usize) -> i64 {
    (2 * k as i64 + 1) * HOP * US / (2 * RATE)
}

/// Hops either side that a peak must top.
const PEAK_NEAR: usize = 3;
/// Hops either side over which a peak's surroundings are averaged: about
/// 0.37 s.
const PEAK_LOCAL: usize = 64;
/// A peak stands at least this share (percent) above its surroundings.
const PEAK_OVER_PCT: i64 = 200;
/// A peak lies on a grid line when it is this close to it: about two hops.
pub const ON_LINE_US: i64 = 12_000;
/// The weakest share (percent) of the peaks is left out of the tempo
/// search, so the grid follows the hits a player hears.
const FIT_SKIP_PCT: usize = 40;
/// Each peak counts by its strength, up to this percentile's, so a loud
/// hit counts for more than a faint one and one huge hit no more than a
/// strong one.
const FIT_CAP_PCT: usize = 95;
/// Peaks weaker than this share (1 in N) of the capped strength are left
/// out of the tempo search.
const FIT_FLOOR_SHARE: i64 = 8;
/// Phase is searched in steps of this many microseconds of a unit.
const PHASE_STEP_US: i64 = 700;
/// The grids tried on a beat: straight half beats, straight quarters,
/// triplets, and half beats and triplets at once.
const DIVISIONS: [i64; 4] = [2, 4, 3, 6];
/// Tempos are favoured by how near they are to this many beats a minute...
const PRIOR_BPM: i64 = 120;
/// ...falling to about 60% half an octave away (in 256ths of an octave).
const PRIOR_WIDTH: i64 = 128;

/// The onsets a player hears: hops whose envelope tops its neighbours and
/// stands well above its surroundings. Each is (time in microseconds,
/// strength).
pub fn peaks(onsets: &[i64]) -> Vec<(i64, i64)> {
    let n = onsets.len();
    let mut prefix = vec![0i64; n + 1];
    for k in 0..n {
        prefix[k + 1] = prefix[k] + onsets[k];
    }
    (0..n)
        .filter_map(|k| {
            let v = onsets[k];
            let (a, b) = (k.saturating_sub(PEAK_NEAR), (k + PEAK_NEAR + 1).min(n));
            // the first of equal tops
            let top =
                v > 0 && onsets[a..k].iter().all(|&w| w < v) && onsets[k + 1..b].iter().all(|&w| w <= v);
            let (la, lb) = (k.saturating_sub(PEAK_LOCAL), (k + PEAK_LOCAL + 1).min(n));
            let mean = (prefix[lb] - prefix[la]) / (lb - la) as i64;
            (top && v * 100 > mean * PEAK_OVER_PCT).then(|| (hop_time(k), v))
        })
        .collect()
}

/// How one grid fits: lines every `period / division` from `phase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fit {
    period: i64,
    division: i64,
    /// The first line at or after 0, on a beat.
    phase: i64,
    /// The strength of the peaks on a line.
    hits: i64,
    /// Strength on a line beyond what lines this close would catch by chance,
    /// per mille of what is left after chance.
    fit: i64,
}

/// The best phase for a grid of `division` lines a `period`, and how well
/// it fits `times`. Positions are taken as `t * division mod period`, so a
/// period not divisible by the division loses nothing.
fn fit_grid(times: &[(i64, i64)], period: i64, division: i64) -> Fit {
    let span = period; // one unit, scaled by `division`
    let bins = (span / (PHASE_STEP_US * division)).max(1);
    let mut hist = vec![0i64; bins as usize];
    for &(t, w) in times {
        let x = (t * division).rem_euclid(span);
        hist[(x * bins / span) as usize] += w;
    }
    let half = (ON_LINE_US * division * bins / span).min((bins - 1) / 2) as usize;
    let nb = bins as usize;
    let mut sum: i64 = (0..=2 * half).map(|j| hist[(nb + j - half) % nb]).sum();
    let mut best = (sum, 0usize);
    for b in 1..nb {
        sum += hist[(b + half) % nb] - hist[(nb + b - half - 1) % nb];
        if sum > best.0 {
            best = (sum, b);
        }
    }
    // every centre on a plateau of the window catches the same peaks; take
    // the mean of the peaks it catches, not the plateau's first centre
    let centre = best.1 as i64 * span / bins + span / (2 * bins);
    let reach = ON_LINE_US * division;
    let (mut moved, mut caught) = (0i64, 0i64);
    for &(t, w) in times {
        let d = ((t * division).rem_euclid(span) - centre + span / 2).rem_euclid(span) - span / 2;
        if d.abs() <= reach {
            moved += d * w;
            caught += w;
        }
    }
    let unit_phase = (centre + moved / caught.max(1)).rem_euclid(span) / division;
    let n = times.iter().map(|p| p.1).sum::<i64>().max(1);
    let chance = 2 * ON_LINE_US * division * 1000 / period;
    let fit = (best.0 * 1000 - n * chance) * 1000 / (n * (1000 - chance).max(1));
    // which of the `division` lines in a beat is hit most: the beat
    let mut counts = vec![0i64; division as usize];
    for &(t, w) in times {
        let line = ((t - unit_phase) * division + period / 2).div_euclid(period);
        let at = unit_phase + line * period / division;
        if (t - at).abs() <= ON_LINE_US {
            counts[line.rem_euclid(division) as usize] += w;
        }
    }
    let r = counts
        .iter()
        .enumerate()
        .max_by_key(|&(r, c)| (*c, -(r as i64)))
        .map(|p| p.0)
        .unwrap_or(0);
    Fit {
        period,
        division,
        phase: (unit_phase + r as i64 * period / division).rem_euclid(period),
        hits: counts.iter().sum(),
        fit,
    }
}

/// The best fit with a period within `reach` of `around`, in steps small
/// enough that the grid moves less than a line's width over the song.
fn refine(times: &[(i64, i64)], around: i64, reach: i64, division: i64, length: i64) -> Fit {
    let step = (ON_LINE_US * around / (2 * length.max(1))).max(1);
    let mut best = fit_grid(times, around, division);
    let mut p = around - reach;
    while p <= around + reach {
        let f = fit_grid(times, p, division);
        if f.hits > best.hits {
            best = f;
        }
        p += step;
    }
    best
}

/// How much a beat of `period` is favoured, per mille: 1000 at
/// `PRIOR_BPM`, falling away by octaves like a bell curve. Listeners and
/// beat trackers alike hear a beat near 120 a minute, so of 80, 120 and 160,
/// which share most of their lines, 120 is taken unless the song's hits
/// say otherwise (R7.1).
fn tempo_prior(period: i64) -> i64 {
    let l = log2_256(60 * US * 1000 / period) - log2_256(PRIOR_BPM * 1000);
    let s = PRIOR_WIDTH * PRIOR_WIDTH;
    // 1 / (1 + z^2/2 + z^4/8) for z = l / width, close to exp(-z^2/2)
    1000 * 8 * s * s / (8 * s * s + 4 * l * l * s + l * l * l * l)
}

/// The beat, of each candidate's grids, by how clearly its own lines are
/// hit, weighted by `tempo_prior`; then `settle` on its best-fitting
/// period (R7.1).
fn choose(times: &[(i64, i64)], beats: &[Vec<Fit>]) -> Option<Fit> {
    let score = |fits: &Vec<Fit>| {
        fits.iter()
            .map(|f| fit_grid(times, f.period, 1).fit.max(0) * tempo_prior(f.period))
            .max()
            .unwrap_or(0)
    };
    let fits = beats.iter().max_by_key(|fits| score(fits))?;
    let period = fits.iter().max_by_key(|f| f.fit)?.period;
    Some(settle(times, period))
}

/// The grid on a beat of `period`: the beat's line where its strongest
/// hits fall, then the division that fits best with that line held, so a
/// finer grid cannot slide off the beat to catch hits between its lines
/// (R7.1). Of equal fits, the coarser.
fn settle(times: &[(i64, i64)], period: i64) -> Fit {
    let beat = fit_grid(times, period, 1);
    DIVISIONS
        .iter()
        .map(|&d| fit_at(times, period, d, beat.phase))
        .max_by_key(|f| (f.fit, -f.division))
        .unwrap_or(beat)
}

/// How well a grid with its lines fixed at `phase` fits `times`.
fn fit_at(times: &[(i64, i64)], period: i64, division: i64, phase: i64) -> Fit {
    let mut hits = 0;
    for &(t, w) in times {
        let line = ((t - phase) * division + period / 2).div_euclid(period);
        if (t - phase - line * period / division).abs() <= ON_LINE_US {
            hits += w;
        }
    }
    let n = times.iter().map(|p| p.1).sum::<i64>().max(1);
    let chance = 2 * ON_LINE_US * division * 1000 / period;
    Fit {
        period,
        division,
        phase,
        hits,
        fit: (hits * 1000 - n * chance) * 1000 / (n * (1000 - chance).max(1)),
    }
}

/// Finds the tempo, the phase and the quarter-beat strengths of a song.
/// Returns `None` for a song too short or too quiet to have a beat.
pub fn analyse(samples: &[i16]) -> Option<Analysis> {
    analyse_with(samples, None)
}

/// As `analyse`, with the tempo given in thousandths of a beat a minute
/// (from the song's project, in `data/songs.json`): only the first beat
/// and whether the song is straight or in triplets are found (R6.4).
pub fn analyse_with(samples: &[i16], bpm_milli: Option<i64>) -> Option<Analysis> {
    let energy = bands(samples);
    let onsets = envelope_of(&energy);
    let length = samples.len() as i64 * US / RATE;
    let hop = hop_us();
    let min_lag = (60 * US / MAX_BPM) / hop;
    let max_lag = (60 * US / MIN_BPM) / hop + 1;
    if (onsets.len() as i64) < max_lag * 4 {
        return None;
    }
    let found = peaks(&onsets);
    let accents: Vec<i64> = found
        .iter()
        .map(|p| accent(&energy, (p.0 * RATE / (HOP * US)) as usize))
        .collect();
    let mut by_strength: Vec<i64> = found.iter().map(|p| p.1).collect();
    by_strength.sort_unstable();
    let cut = by_strength
        .get(by_strength.len() * FIT_SKIP_PCT / 100)
        .copied()
        .unwrap_or(0);
    let cap = by_strength
        .get(by_strength.len() * FIT_CAP_PCT / 100)
        .copied()
        .unwrap_or(0);
    let times: Vec<(i64, i64)> = found
        .iter()
        .filter(|p| p.1 >= cut.max(cap / FIT_FLOOR_SHARE))
        .map(|p| (p.0, p.1.min(cap)))
        .collect();
    if times.len() < 8 {
        return None;
    }
    let first = first_sound_us(samples, RATE).unwrap_or(0);
    let min_period = 60 * US / MAX_BPM;
    let max_period = 60 * US / MIN_BPM;
    let beats: Vec<Vec<Fit>> = if let Some(bpm) = bpm_milli.filter(|b| *b > 0) {
        let period = 60 * US * 1000 / bpm;
        vec![DIVISIONS.iter().map(|&d| fit_grid(&times, period, d)).collect()]
    } else {
        // candidate beats: the envelope's strongest repeats and the tempos
        // a half, a third or a quarter away from them
        let mut lags: Vec<(i64, i64)> = (min_lag..=max_lag)
            .map(|lag| {
                let l = lag as usize;
                let s: i64 = (0..onsets.len() - l).map(|n| onsets[n] * onsets[n + l]).sum();
                (s, lag)
            })
            .collect();
        let peaks_at: Vec<i64> = (1..lags.len() - 1)
            .filter(|&k| lags[k].0 >= lags[k - 1].0 && lags[k].0 > lags[k + 1].0)
            .map(|k| lags[k].1)
            .collect();
        lags.retain(|l| peaks_at.contains(&l.1));
        lags.sort_by_key(|l| (-l.0, l.1));
        if lags.first().map(|l| l.0).unwrap_or(0) <= 0 {
            return None;
        }
        let mut periods: Vec<i64> = Vec::new();
        for &(_, lag) in lags.iter().take(4) {
            let base = lag * HOP * US / RATE;
            for (num, den) in [(1, 1), (2, 1), (1, 2), (2, 3), (3, 2), (3, 4), (4, 3)] {
                let p = base * num / den;
                if (min_period..=max_period).contains(&p) && periods.iter().all(|&q| (q - p).abs() > hop) {
                    periods.push(p);
                }
            }
        }
        periods
            .iter()
            .map(|&p| {
                DIVISIONS
                    .iter()
                    .map(|&d| refine(&times, p, 2 * hop, d, length))
                    .collect()
            })
            .collect()
    };
    let mut best = choose(&times, &beats)?;
    if bpm_milli.is_none() {
        // songs made in Ableton almost always sit on a whole number of beats
        // a minute; a fit a hair off one drifts tens of ms by the end (R6.4)
        let whole_bpm = (60 * US * 1000 / best.period + 500) / 1000;
        let whole_period = 60 * US / whole_bpm.max(1);
        let beat = fit_grid(&times, whole_period, 1);
        let whole = fit_at(&times, whole_period, best.division, beat.phase);
        if whole.hits * 100 >= best.hits * WHOLE_BPM_PCT {
            best = whole;
        }
    }
    Some(Analysis {
        first_sound_us: first,
        ..slots_for(&found, &accents, best.period, best.division, best.phase, length)
    })
}

/// Each grid line's strength and accent: the strongest of the peaks on it,
/// or 0.
fn slots_for(
    found: &[(i64, i64)],
    accents: &[i64],
    beat_us: i64,
    division: i64,
    phase: i64,
    length: i64,
) -> Analysis {
    let first_beat_us = phase.rem_euclid(beat_us);
    let count = ((length - first_beat_us) * division / beat_us).max(0) as usize;
    let mut slots = vec![0u32; count];
    let mut stress = vec![0u32; count];
    for (&(t, s), &acc) in found.iter().zip(accents) {
        let line = ((t - first_beat_us) * division + beat_us / 2).div_euclid(beat_us);
        let at = first_beat_us + line * beat_us / division;
        if line >= 0 && (line as usize) < count && (t - at).abs() <= ON_LINE_US {
            let v = &mut slots[line as usize];
            *v = (*v).max(s.min(u32::MAX as i64) as u32);
            let w = &mut stress[line as usize];
            *w = (*w).max(acc.min(u32::MAX as i64) as u32);
        }
    }
    Analysis {
        beat_us,
        first_beat_us,
        length_us: length,
        division,
        slots,
        accents: stress,
        first_sound_us: 0,
        bar_beats: 4,
        sustains: Vec::new(),
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::rng::Rng;

    /// A synthetic song: a quiet noise floor and a decaying click at each
    /// time given, louder where `loud` says.
    pub fn song(seconds: i64, clicks: &[(i64, i32)], seed: u64) -> Vec<i16> {
        let n = (seconds * RATE) as usize;
        let mut rng = Rng::new(seed);
        let mut out: Vec<i32> = (0..n).map(|_| rng.between(-300, 300)).collect();
        for &(t_us, amp) in clicks {
            let start = (t_us * RATE / US) as usize;
            for k in 0..1200usize {
                if start + k >= n {
                    break;
                }
                // a 1/8-sample square buzz that halves every 200 samples
                let sign = if (k / 6) % 2 == 0 { 1 } else { -1 };
                let decay = amp >> (k / 200);
                out[start + k] += sign * decay + rng.between(-decay / 4 - 1, decay / 4 + 1);
            }
        }
        out.iter().map(|&v| v.clamp(-32768, 32767) as i16).collect()
    }

    pub fn clicks_at(bpm: i64, first_us: i64, seconds: i64) -> Vec<(i64, i32)> {
        let beat = 60 * US / bpm;
        let mut out = Vec::new();
        let mut t = first_us;
        let mut k = 0;
        while t < seconds * US {
            // a stronger click on the first beat of every four
            out.push((t, if k % 4 == 0 { 16000 } else { 9000 }));
            t += beat;
            k += 1;
        }
        out
    }

    fn assert_grid(a: &Analysis, bpm: i64, first_us: i64) {
        let beat = 60 * US / bpm;
        let found = a.bpm_milli();
        assert!(
            (found - bpm * 1000).abs() <= 1000,
            "{bpm} bpm found as {found} milli"
        );
        // each beat lies within one hop of a found grid line, to the end
        let hop = hop_us();
        let mut t = first_us;
        while t < a.length_us - beat {
            let k = ((t - a.first_beat_us) + a.beat_us / 2).div_euclid(a.beat_us);
            let line = a.first_beat_us + k * a.beat_us;
            assert!(
                (line - t).abs() <= hop,
                "beat at {t} us is {} us off the grid",
                line - t
            );
            t += beat;
        }
    }

    #[test]
    fn a_click_track_at_120_is_found_within_a_beat_a_minute_and_a_hop() {
        let s = song(30, &clicks_at(120, 250_000, 30), 1);
        let a = analyse(&s).expect("a beat");
        assert_grid(&a, 120, 250_000);
    }

    #[test]
    fn a_click_track_at_174_is_found_within_a_beat_a_minute_and_a_hop() {
        let s = song(30, &clicks_at(174, 100_000, 30), 2);
        let a = analyse(&s).expect("a beat");
        assert_grid(&a, 174, 100_000);
    }

    #[test]
    fn a_slow_song_is_doubled_into_the_comfortable_range() {
        // 80 bpm with an offbeat at half the strength: found as 160
        let mut c = clicks_at(80, 300_000, 30);
        let beat = 60 * US / 80;
        let off: Vec<(i64, i32)> = c.iter().map(|&(t, _)| (t + beat / 2, 6000)).collect();
        c.extend(off);
        c.sort();
        let a = analyse(&song(30, &c, 3)).expect("a beat");
        assert_grid(&a, 160, 300_000);
    }

    #[test]
    fn a_click_track_with_one_swung_beat_keeps_the_grid() {
        let mut c = clicks_at(128, 200_000, 30);
        c[20].0 += 60_000;
        let a = analyse(&song(30, &c, 4)).expect("a beat");
        assert_grid(&a, 128, 200_000);
    }

    #[test]
    fn strong_slots_are_the_clicks_and_weak_ones_between() {
        let a = analyse(&song(20, &clicks_at(120, 0, 20), 5)).expect("a beat");
        // every beat's line is strong; the half beat between has nothing
        let d = a.division as usize;
        assert_eq!(d % 2, 0, "a straight song has half beats");
        for beat in 1..19usize {
            assert!(
                a.slots[beat * d] > 4 * a.slots[beat * d + d / 2].max(1),
                "beat {beat}: {:?}",
                &a.slots[beat * d..beat * d + d]
            );
        }
    }

    #[test]
    fn a_held_note_is_an_onset_where_it_starts_only() {
        // a square tone held 245 ms from each beat at 120 bpm: 125 ms in it
        // is still loud, and its last moments, at the half beat, are louder
        // than the silence after them, but neither is a rise
        let n = (20 * RATE) as usize;
        let mut s = vec![0i16; n];
        let beat = (RATE / 2) as usize;
        for b in 0..40 {
            for k in 0..(RATE as usize * 245 / 1000) {
                if let Some(v) = s.get_mut(b * beat + k) {
                    *v = if (k / 50) % 2 == 0 { 9000 } else { -9000 };
                }
            }
        }
        let a = analyse(&s).expect("a beat");
        assert!((a.bpm_milli() - 120_000).abs() <= 1000);
        let d = a.division as usize;
        for beat in 2..30usize {
            let later = a.slots[beat * d + 1..beat * d + d]
                .iter()
                .copied()
                .max()
                .unwrap_or(0)
                .max(1);
            assert!(
                a.slots[beat * d] > 4 * later,
                "beat {beat}: {:?}",
                &a.slots[beat * d..beat * d + d]
            );
        }
    }

    #[test]
    fn a_whole_number_tempo_is_found_exactly() {
        // 120 bpm with some loud hits off the beat, which pull a free fit
        let mut c = clicks_at(120, 150_000, 60);
        for k in (5..c.len()).step_by(9) {
            c.push((c[k].0 + 70_000, 15_000));
        }
        c.sort();
        let a = analyse(&song(60, &c, 6)).expect("a beat");
        assert_eq!(a.beat_us, 500_000, "found {} milli-bpm", a.bpm_milli());
        // and a tempo that is not whole keeps its fraction: 124.6 bpm
        let beat = 60 * US * 10 / 1246;
        let mut c = Vec::new();
        let mut t = 100_000;
        while t < 60 * US {
            c.push((t, 12_000));
            t += beat;
        }
        let a = analyse(&song(60, &c, 7)).expect("a beat");
        assert!((a.bpm_milli() - 124_600).abs() <= 50, "found {}", a.bpm_milli());
    }

    #[test]
    fn a_given_tempo_is_kept_and_only_the_first_beat_is_found() {
        let s = song(30, &clicks_at(120, 210_000, 30), 8);
        // 240 is past what the search would choose, so only a kept tempo gives it
        let a = analyse_with(&s, Some(240_000)).expect("a beat");
        assert_eq!(a.beat_us, 250_000);
        assert!(
            (a.first_beat_us - 210_000).abs() <= hop_us(),
            "first beat {}",
            a.first_beat_us
        );
    }

    #[test]
    fn silence_has_no_beat() {
        assert_eq!(analyse(&vec![0i16; (10 * RATE) as usize]), None);
        // five clicks are too few hits to say where a beat is
        let few: Vec<(i64, i32)> = clicks_at(120, 0, 10).into_iter().take(5).collect();
        let mut quiet = vec![0i16; (10 * RATE) as usize];
        for (t, amp) in few {
            let start = (t * RATE / US) as usize;
            for k in 0..1200usize {
                quiet[start + k] = if (k / 6) % 2 == 0 {
                    (amp >> (k / 200)) as i16
                } else {
                    -((amp >> (k / 200)) as i16)
                };
            }
        }
        assert_eq!(analyse(&quiet), None);
        assert_eq!(analyse(&[0i16; 100]), None);
    }

    #[test]
    fn the_log_scale_is_monotone_and_exact_at_powers_of_two() {
        assert_eq!(log2_256(1), 0);
        assert_eq!(log2_256(2), 256);
        assert_eq!(log2_256(1024), 2560);
        let mut last = -1;
        for v in 1..5000 {
            let l = log2_256(v);
            assert!(l >= last);
            last = l;
        }
    }

    #[test]
    fn a_dotted_rhythm_at_120_is_found_as_120_not_160() {
        // a kick on every beat and hits every three sixteenths, the pattern
        // that read Monke Walkin' as 160 (R7.1)
        let sixteenth = 125_000;
        let mut c: Vec<(i64, i32)> = Vec::new();
        for k in 0..(40 * 8) {
            let t = 200_000 + k * sixteenth;
            if k % 4 == 0 || (k % 16) % 3 == 0 {
                c.push((t, 15_000));
            }
        }
        let a = analyse(&song(40, &c, 21)).expect("a beat");
        assert!((a.bpm_milli() - 120_000).abs() <= 1000, "found {}", a.bpm_milli());
        assert_eq!(a.division, 4);
    }

    #[test]
    fn a_song_in_triplets_gets_lines_on_its_triplets() {
        // 100 a minute, a strong beat and two much softer hits a third of a
        // beat apart: every hit on a line, read in threes (R7.1). With the
        // beat no louder than the rest, three even hits a beat read as well
        // in twos at 150, and the song's own tempo is the way to say which.
        let beat = 600_000;
        let mut c: Vec<(i64, i32)> = Vec::new();
        let mut t = 300_000;
        while t < 30 * US {
            c.push((t, 20_000));
            c.push((t + beat / 3, 2_500));
            c.push((t + 2 * beat / 3, 2_500));
            t += beat;
        }
        let a = analyse(&song(30, &c, 22)).expect("a beat");
        assert!((a.bpm_milli() - 100_000).abs() <= 1000, "found {}", a.bpm_milli());
        assert_eq!(a.division % 3, 0, "division {}", a.division);
        let per_third = (a.division / 3) as usize;
        for b in 1..40usize {
            for third in 0..3 {
                let q = b * a.division as usize + third * per_third;
                assert!(
                    a.slots[q] > 0,
                    "beat {b}, third {third}: {:?}",
                    &a.slots[q - 1..q + 2]
                );
            }
        }
    }

    #[test]
    fn a_line_with_no_onset_on_it_has_no_strength() {
        // clicks on the beat, and softer ones 140 ms after, between the
        // grid's lines, over silence: every line but the beat's is exactly 0,
        // the late click on none (R7.2)
        let mut s = vec![0i16; (20 * RATE) as usize];
        let mut c = clicks_at(120, 0, 20);
        let late: Vec<(i64, i32)> = c.iter().map(|&(t, _)| (t + 140_000, 4000)).collect();
        c.extend(late);
        for (t, amp) in c {
            let start = (t * RATE / US) as usize;
            for k in 0..1200usize {
                if let Some(v) = s.get_mut(start + k) {
                    *v = if (k / 6) % 2 == 0 {
                        (amp >> (k / 200)) as i16
                    } else {
                        -((amp >> (k / 200)) as i16)
                    };
                }
            }
        }
        let a = analyse(&s).expect("a beat");
        let d = a.division as usize;
        for (q, &v) in a.slots.iter().enumerate().take(19 * d) {
            if q % d != 0 {
                assert_eq!(v, 0, "line {q}");
            }
        }
    }

    #[test]
    fn the_tempo_prior_favours_120_and_falls_away_by_octaves() {
        // at 120 a minute: 1000. At 160: log2_256 gives 4408 and 4308, 100
        // apart; with s = 128 * 128 = 16384,
        // 8000 s^2 / (8 s^2 + 4 * 100^2 * s + 100^4) = 739
        assert_eq!(tempo_prior(500_000), 1000);
        assert_eq!(tempo_prior(375_000), 739);
        assert!(tempo_prior(750_000) < tempo_prior(500_000));
        assert!(tempo_prior(250_000) < tempo_prior(375_000));
    }

    #[test]
    fn a_hit_that_lifts_the_whole_mix_is_stressed_more_than_one_that_does_not() {
        // a low tone held under everything, a soft high tick on each half
        // beat and a loud low thump on each beat: the ticks' own band rises
        // as much as the thumps', but the whole mix barely gets louder at a
        // tick (R7.6)
        let n = (20 * RATE) as usize;
        let mut s = vec![0i32; n];
        for (k, v) in s.iter_mut().enumerate() {
            *v += if (k / 40) % 2 == 0 { 6000 } else { -6000 };
        }
        let beat = (RATE / 2) as usize;
        for b in 0..40 {
            for k in 0..600usize {
                if let Some(v) = s.get_mut(b * beat + k) {
                    *v += if (k / 30) % 2 == 0 {
                        20000 >> (k / 200)
                    } else {
                        -(20000 >> (k / 200))
                    };
                }
                if let Some(v) = s.get_mut(b * beat + beat / 2 + k) {
                    *v += if k % 2 == 0 {
                        3000 >> (k / 100)
                    } else {
                        -(3000 >> (k / 100))
                    };
                }
            }
        }
        let s: Vec<i16> = s.iter().map(|&v| v.clamp(-32768, 32767) as i16).collect();
        let a = analyse(&s).expect("a beat");
        let d = a.division as usize;
        for b in 2..36usize {
            let (on, off) = (b * d, b * d + d / 2);
            assert!(a.slots[off] > 0, "beat {b}: the tick is heard");
            assert!(
                a.accents[on] > 2 * a.accents[off],
                "beat {b}: thump {} against tick {}",
                a.accents[on],
                a.accents[off]
            );
        }
    }
}
