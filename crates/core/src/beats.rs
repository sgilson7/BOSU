//! Finding the beat in a song (PLAN.md decision 9). Integer only.
//!
//! The song comes in as 16-bit mono samples at `RATE` (the chart tool
//! decodes it; the core only reads numbers). Three bands are split with
//! one-pole filters; each band's energy per hop is put on a log scale; an
//! onset is a rise in that energy. The tempo comes from the onset
//! envelope's autocorrelation between 70 and 190 beats a minute, and is then
//! refined with the phase, to the microsecond, by fitting one fixed grid to
//! the whole song: songs made in Ableton keep one tempo, and a fitted grid
//! puts every target on the song's own beat rather than on the nearest hop
//! (R3.4, a change from the plan's dynamic programming).

use serde::{Deserialize, Serialize};

/// Samples a second.
pub const RATE: i64 = 22_050;
/// Samples a hop: 128 at 22,050 a second is about 5.8 ms.
pub const HOP: i64 = 128;
pub const MIN_BPM: i64 = 70;
pub const MAX_BPM: i64 = 190;
/// A tempo below this is doubled when the double still fits.
pub const COMFORT_BPM: i64 = 90;
/// A whole-number tempo is taken when its grid scores at least this share
/// (percent) of the best fit's.
pub const WHOLE_BPM_PCT: i64 = 97;
/// Grid slots a beat: strengths are kept on quarter beats.
pub const SLOTS_PER_BEAT: i64 = 4;

pub const US: i64 = 1_000_000;

/// What the chart generator needs from a song.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Analysis {
    pub beat_us: i64,
    /// The first beat at or after 0.
    pub first_beat_us: i64,
    pub length_us: i64,
    /// Onset strength at each quarter beat from `first_beat_us`.
    pub slots: Vec<u32>,
    /// Where the first sound is in this decode. A browser that trims the
    /// start of an MP3 differently finds it elsewhere, and the game moves
    /// song time by the difference (R6.6).
    #[serde(default)]
    pub first_sound_us: i64,
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
        self.first_beat_us + slot as i64 * self.beat_us / SLOTS_PER_BEAT
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

/// The envelope near time `t`: the larger of the two hops around it.
fn env_at(onsets: &[i64], t: i64) -> i64 {
    let i = (t * RATE / (HOP * US)) as usize;
    let a = onsets.get(i).copied().unwrap_or(0);
    let b = onsets.get(i + 1).copied().unwrap_or(0);
    a.max(b)
}

fn grid_score(onsets: &[i64], period: i64, phase: i64, length: i64) -> i64 {
    let mut s = 0;
    let mut t = phase;
    while t < length {
        s += env_at(onsets, t);
        t += period;
    }
    s
}

/// The best (period, phase) with period in `periods` stepping by `p_step`
/// and phase stepping by `f_step` across one period.
fn fit(onsets: &[i64], periods: (i64, i64), p_step: i64, f_step: i64, length: i64) -> (i64, i64) {
    let mut best = (0, periods.0, 0);
    let mut p = periods.0;
    while p <= periods.1 {
        let mut f = 0;
        while f < p {
            let s = grid_score(onsets, p, f, length);
            if s > best.0 {
                best = (s, p, f);
            }
            f += f_step;
        }
        p += p_step;
    }
    (best.1, best.2)
}

/// Finds the tempo, the phase and the quarter-beat strengths of a song.
/// Returns `None` for a song too short or too quiet to have a beat.
pub fn analyse(samples: &[i16]) -> Option<Analysis> {
    analyse_with(samples, None)
}

/// As `analyse`, with the tempo given in thousandths of a beat a minute
/// (from the song's project, in `data/songs.json`): only the first beat is
/// found (R6.4).
pub fn analyse_with(samples: &[i16], bpm_milli: Option<i64>) -> Option<Analysis> {
    let onsets = envelope(samples);
    let length = samples.len() as i64 * US / RATE;
    let hop = hop_us();
    // coarse tempo: autocorrelation over lags in hops
    let min_lag = (60 * US / MAX_BPM) / hop;
    let max_lag = (60 * US / MIN_BPM) / hop + 1;
    if (onsets.len() as i64) < max_lag * 4 {
        return None;
    }
    let mut best = (0i64, 0i64);
    for lag in min_lag..=max_lag {
        let l = lag as usize;
        let s: i64 = (0..onsets.len() - l).map(|n| onsets[n] * onsets[n + l]).sum();
        if s > best.0 {
            best = (s, lag);
        }
    }
    if best.0 == 0 {
        return None;
    }
    let first = first_sound_us(samples, RATE).unwrap_or(0);
    if let Some(bpm) = bpm_milli.filter(|b| *b > 0) {
        let period = 60 * US * 1000 / bpm;
        let (_, phase) = fit(&onsets, (period, period), 1, hop / 8, length);
        return Some(Analysis {
            first_sound_us: first,
            ..slots_for(&onsets, period, phase, length)
        });
    }
    let mut lag_us = best.1 * hop;
    if 60 * US / lag_us < COMFORT_BPM && 60 * US / (lag_us / 2) <= MAX_BPM {
        lag_us /= 2;
    }
    // refine the period and phase together, coarse then fine
    let (p, _) = fit(
        &onsets,
        (lag_us - 2 * hop, lag_us + 2 * hop),
        200,
        hop / 2,
        length,
    );
    let (fitted, fitted_phase) = fit(&onsets, (p - 400, p + 400), 10, hop / 8, length);
    // songs made in Ableton almost always sit on a whole number of beats a
    // minute; a fit a hair off one drifts tens of ms by the end (R6.4). Take
    // the whole number when its grid scores nearly as well.
    let best = grid_score(&onsets, fitted, fitted_phase, length);
    let whole_bpm = (60 * US * 1000 / fitted + 500) / 1000;
    let whole_period = 60 * US / whole_bpm.max(1);
    let (_, whole_phase) = fit(&onsets, (whole_period, whole_period), 1, hop / 8, length);
    let whole_score = grid_score(&onsets, whole_period, whole_phase, length);
    let (beat_us, phase) = if whole_score * 100 >= best * WHOLE_BPM_PCT {
        (whole_period, whole_phase)
    } else {
        (fitted, fitted_phase)
    };
    Some(Analysis {
        first_sound_us: first,
        ..slots_for(&onsets, beat_us, phase, length)
    })
}

fn slots_for(onsets: &[i64], beat_us: i64, phase: i64, length: i64) -> Analysis {
    let hop = hop_us();
    let first_beat_us = phase % beat_us;
    let count = ((length - first_beat_us) * SLOTS_PER_BEAT / beat_us).max(0) as usize;
    let slots = (0..count)
        .map(|q| {
            let t = first_beat_us + q as i64 * beat_us / SLOTS_PER_BEAT;
            env_at(onsets, t)
                .max(env_at(onsets, t - hop))
                .min(u32::MAX as i64) as u32
        })
        .collect();
    Analysis {
        beat_us,
        first_beat_us,
        length_us: length,
        slots,
        first_sound_us: 0,
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
        // slot 0, 4, 8 are beats; 2, 6 are half beats with nothing on them
        for beat in 1..20usize {
            assert!(
                a.slots[beat * 4] > 4 * a.slots[beat * 4 + 2].max(1),
                "beat {beat}: {:?}",
                &a.slots[beat * 4..beat * 4 + 4]
            );
        }
    }

    #[test]
    fn a_held_note_is_an_onset_where_it_starts_only() {
        // a square tone held 300 ms from each beat at 120 bpm: 125 ms in it
        // is still loud, and 250 ms in it is louder than the silence after
        // it, but neither is a rise
        let n = (20 * RATE) as usize;
        let mut s = vec![0i16; n];
        let beat = (RATE / 2) as usize;
        for b in 0..40 {
            for k in 0..(RATE as usize * 3 / 10) {
                if let Some(v) = s.get_mut(b * beat + k) {
                    *v = if (k / 50) % 2 == 0 { 9000 } else { -9000 };
                }
            }
        }
        let a = analyse(&s).expect("a beat");
        assert!((a.bpm_milli() - 120_000).abs() <= 1000);
        for beat in 2..30usize {
            let later = a.slots[beat * 4 + 1].max(a.slots[beat * 4 + 2]).max(1);
            assert!(
                a.slots[beat * 4] > 4 * later,
                "beat {beat}: {:?}",
                &a.slots[beat * 4..beat * 4 + 4]
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
}
