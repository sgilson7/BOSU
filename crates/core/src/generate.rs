//! Laying out a chart from a song's analysis (PLAN.md decision 10).
//!
//! Which grid lines become targets: only lines with an onset on them
//! (R7.2), and of those the strongest share (`keep_pct`) of each phrase on
//! the difficulty's grid, beats counting for more than half beats and half
//! beats for more than the rest, never closer than the minimum gap. Each
//! difficulty chooses from the targets of the one above it, so Easy's
//! targets are all in Normal, Normal's in Hard, and so on (R7.2).
//! Circle size: a song busier than the usual gets bigger targets at every
//! difficulty, a sparser one smaller (R7.3).
//! Where each goes: as far from the last as the time between them allows
//! (`spacing` units a 100 ms, at most `max_jump`), turned by a seeded
//! angle, kept inside the playfield and off every target still on screen.
//! Combo numbers start again every eight beats. Some long gaps become holds.
//! The same analysis, difficulty and seed give the same chart (BO-6).

use crate::beats::Analysis;
use crate::chart::{Chart, Hold, Target};
use crate::fx;
use crate::rng::Rng;
use crate::tuning::{
    Difficulty, DIFFICULTIES, HEARD_PER_MINUTE, SIZE_MAX_PCT, SIZE_MIN_PCT, X_MAX, X_MIN, Y_MAX, Y_MIN,
};

/// Beats a phrase: the combo number starts again at each phrase.
pub const PHRASE_BEATS: i64 = 8;
/// Targets start this long after the song's first onset strong enough to
/// keep, and stop this long before its end.
const LEAD_OUT_US: i64 = 1_500_000;
/// A slot quieter than this share (percent) of the song's strong slots is
/// never a target. "Strong" is the 95th percentile, not the loudest: one
/// huge hit in a real song set the bar so high that Grunge Dome kept two
/// targets on Easy (R6.2).
const FLOOR_PCT: u64 = 8;
/// From the true chart, each difficulty's minimum gap is at most a beat
/// divided by this (Easy a beat, Normal a half, Hard a third, Expert a
/// quarter), so a song's triplets fit where a fixed time would cut them by
/// a few ms (R7.6). The hardest has none.
const TRUE_GAP_DIVISORS: [i64; 5] = [1, 2, 3, 4, 1];
/// The true chart's floor, as a share (percent) of the strong slots: low,
/// to leave out only what is barely there.
const TRUE_FLOOR_PCT: u64 = 3;
/// The percentile that counts as strong.
const STRONG_PCT: usize = 95;
/// Tries at placing one target before taking the least-bad spot.
const TRIES: u32 = 96;
/// The spacing of the whole-field search when every try is crowded.
const FALLBACK_STEP: usize = 20;
/// No target in the first second.
const LEAD_IN_US: i64 = 1_000_000;
/// The gap before the next target that may hold, in beats.
const HOLD_MIN_BEATS: i64 = 2;

fn inside(x: i32, y: i32) -> bool {
    (X_MIN..=X_MAX).contains(&x) && (Y_MIN..=Y_MAX).contains(&y)
}

fn dist2(a: (i32, i32), b: (i32, i32)) -> i64 {
    let dx = (a.0 - b.0) as i64;
    let dy = (a.1 - b.1) as i64;
    dx * dx + dy * dy
}

/// Whether grid line `q` of a song with `division` lines a beat is on a
/// difficulty's grid of `grid` steps a beat. In triplets the half beat is
/// the third line, the swung one; with six lines both half beats count.
pub fn on_grid(q: usize, division: i64, grid: i64) -> bool {
    let p = q as i64 % division;
    match (grid, division) {
        (g, _) if g >= 4 => true,
        (g, 4) if g >= 2 => p % 2 == 0,
        (g, 3) if g >= 2 => p != 1,
        (g, 6) if g >= 2 => p % 3 == 0 || p == 4,
        (g, 2) if g >= 2 => true,
        (g, 1) if g >= 2 => true,
        _ => p == 0,
    }
}

/// Percent weights by a line's place in the beat: on the beat, on the half
/// beat, elsewhere.
const WEIGHT_BEAT: u64 = 125;
const WEIGHT_HALF: u64 = 100;
const WEIGHT_REST: u64 = 80;

fn weight(q: usize, division: i64) -> u64 {
    if on_grid(q, division, 1) {
        WEIGHT_BEAT
    } else if on_grid(q, division, 2) {
        WEIGHT_HALF
    } else {
        WEIGHT_REST
    }
}

/// The grid lines with an onset strong enough to be a target, in time
/// order.
pub fn heard(a: &Analysis) -> Vec<usize> {
    heard_above(a, FLOOR_PCT)
}

fn heard_above(a: &Analysis, floor_pct: u64) -> Vec<usize> {
    let mut sorted: Vec<u32> = a.slots.iter().copied().filter(|&v| v > 0).collect();
    sorted.sort_unstable();
    let strong = sorted.get(sorted.len() * STRONG_PCT / 100).copied().unwrap_or(0) as u64;
    let floor = strong * floor_pct / 100;
    let last_t = a.length_us - LEAD_OUT_US;
    (0..a.slots.len())
        .filter(|&q| a.slots[q] as u64 > floor && a.slot_time(q) <= last_t && a.slot_time(q) >= LEAD_IN_US)
        .collect()
}

/// The grid lines that become targets at a difficulty, in time order.
pub fn choose_slots(a: &Analysis, difficulty: usize) -> Vec<usize> {
    choose_slots_with(a, difficulty, false)
}

/// As `choose_slots`; with `from_true`, the hardest difficulty is the true
/// chart, a target on every hit the song has, at any spacing, and each
/// difficulty below drops the hits a player hears least, by their accent,
/// from the one above (R7.6).
pub fn choose_slots_with(a: &Analysis, difficulty: usize, from_true: bool) -> Vec<usize> {
    let hardest = DIFFICULTIES.len() - 1;
    let mut pool = if from_true {
        heard_above(a, TRUE_FLOOR_PCT)
    } else {
        heard(a)
    };
    let mut above = 100;
    for k in (difficulty..DIFFICULTIES.len()).rev() {
        if from_true && k == hardest {
            continue;
        }
        let d = &DIFFICULTIES[k];
        let gap = if from_true {
            d.min_gap_us.min(a.beat_us / TRUE_GAP_DIVISORS[k])
        } else {
            d.min_gap_us
        };
        pool = thin(a, &pool, d, above, from_true, gap);
        above = DIFFICULTIES[k].keep_pct;
    }
    pool
}

/// The targets one difficulty keeps from `pool`, the targets of the
/// difficulty above it (or every heard line, for the hardest), whose share
/// was `above` percent: each phrase keeps `keep_pct / above` of its pool,
/// so the shares multiply out to `keep_pct` of what is heard.
fn thin(
    a: &Analysis,
    pool: &[usize],
    d: &Difficulty,
    above: i64,
    by_accent: bool,
    min_gap_us: i64,
) -> Vec<usize> {
    let stress = |q: usize| match (by_accent, a.accents.get(q)) {
        (true, Some(&v)) => v as u64,
        _ => a.slots[q] as u64,
    };
    let score = |q: usize| stress(q) * weight(q, a.division);
    let by_score = |p: &usize, q: &usize| score(*q).cmp(&score(*p)).then(p.cmp(q));
    let phrase_lines = (PHRASE_BEATS * a.division) as usize;
    // from the true chart any line may stay, its accent and place in the
    // beat deciding; otherwise only the difficulty's grid
    let on: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&q| by_accent || on_grid(q, a.division, d.grid))
        .collect();
    // The strongest share within each phrase, so a quiet verse gets targets
    // in proportion and a loud chorus does not take them all (R6.7).
    let mut kept: Vec<usize> = Vec::new();
    let mut start = 0;
    while start < on.len() {
        let phrase = on[start] / phrase_lines;
        let end = start
            + on[start..]
                .iter()
                .take_while(|&&q| q / phrase_lines == phrase)
                .count();
        let mut group = on[start..end].to_vec();
        group.sort_by(by_score);
        let keep = ((group.len() as i64 * d.keep_pct + above - 1) / above) as usize;
        kept.extend(group.into_iter().take(keep));
        start = end;
    }
    kept.sort_by(by_score);
    let mut chosen: Vec<usize> = Vec::new();
    for &q in kept.iter() {
        let t = a.slot_time(q);
        if chosen.iter().all(|&c| (a.slot_time(c) - t).abs() >= min_gap_us) {
            chosen.push(q);
        }
    }
    chosen.sort();
    chosen
}

/// A song's circle size in percent of each difficulty's radius: the square
/// root of how busy it is (`heard` lines a minute) against
/// `HEARD_PER_MINUTE`, within `SIZE_MIN_PCT` and `SIZE_MAX_PCT` (R7.3).
pub fn size_pct(heard: usize, span_us: i64) -> i64 {
    let per_minute_milli = heard as i64 * 60_000_000_000 / span_us.max(1);
    isqrt(per_minute_milli * 10 / HEARD_PER_MINUTE).clamp(SIZE_MIN_PCT, SIZE_MAX_PCT)
}

fn isqrt(v: i64) -> i64 {
    if v <= 0 {
        return 0;
    }
    let mut x = v;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + v / x) / 2;
    }
    x
}

/// Generates one chart.
pub fn chart(song: &str, a: &Analysis, difficulty: usize, seed: u64) -> Chart {
    chart_with(song, a, difficulty, seed, false)
}

/// As `chart`; `from_true` as in `choose_slots_with`.
pub fn chart_with(song: &str, a: &Analysis, difficulty: usize, seed: u64, from_true: bool) -> Chart {
    let d = DIFFICULTIES[difficulty];
    let slots = choose_slots_with(a, difficulty, from_true);
    let times: Vec<i64> = slots.iter().map(|&q| a.slot_time(q)).collect();
    let lines = heard(a);
    let span = match (lines.first(), lines.last()) {
        (Some(&f), Some(&l)) => a.slot_time(l) - a.slot_time(f),
        _ => 0,
    };
    let radius = (d.radius as i64 * size_pct(lines.len(), span) / 100) as i32;
    let mut rng = Rng::new(seed ^ ((difficulty as u64 + 1) << 32));
    let mut targets: Vec<Target> = Vec::new();
    let mut at = ((X_MIN + X_MAX) / 2, (Y_MIN + Y_MAX) / 2);
    let mut heading: i32 = rng.between(0, fx::TURN - 1);
    let mut combo = 0u32;
    let mut phrase = -1i64;
    let min_sep = 2 * radius as i64 + 8;
    let mut k = 0;
    while k < times.len() {
        let t = times[k];
        // where the previous target ends: its own spot, or its hold's end
        let (from, from_t) = match targets.last() {
            Some(p) => (p.point_at(p.end()), p.end()),
            None => (at, t - 1_000_000),
        };
        let gap_ms = (t - from_t) / 1000;
        let want = (d.spacing * gap_ms / 100).clamp(min_sep, d.max_jump);
        // targets on screen when this one appears
        let visible: Vec<(i32, i32)> = targets
            .iter()
            .filter(|p| p.end() + d.good_us >= t - d.approach_us)
            .flat_map(|p| {
                let mut v = vec![(p.x, p.y)];
                if p.hold.is_some() {
                    v.push(p.point_at(p.end()));
                }
                v
            })
            .collect();
        let mut best: Option<((i32, i32), i32, i64)> = None;
        for attempt in 0..TRIES {
            let swing = fx::TURN / 3 + (attempt as i32) * fx::TURN / (2 * TRIES as i32);
            let turn = rng.between(-swing, swing);
            let angle = heading + turn;
            // the wanted distance first; then any distance the difficulty allows
            let len = if attempt < TRIES / 3 {
                want
            } else {
                rng.between(min_sep as i32, d.max_jump as i32) as i64
            };
            let x = from.0 + fx::round(fx::mul(fx::cos(angle), fx::from_int(len as i32)));
            let y = from.1 + fx::round(fx::mul(fx::sin(angle), fx::from_int(len as i32)));
            if !inside(x, y) {
                continue;
            }
            let room = visible
                .iter()
                .map(|&v| dist2(v, (x, y)))
                .min()
                .unwrap_or(i64::MAX);
            if best.map(|b| room > b.2).unwrap_or(true) {
                best = Some(((x, y), angle, room));
            }
            if room >= min_sep * min_sep {
                break;
            }
        }
        // crowded: search the whole field for a clear spot, nearest the
        // wanted distance, before taking the least-bad one
        if best.map(|b| b.2 < min_sep * min_sep).unwrap_or(true) {
            let mut clear: Option<((i32, i32), i64)> = None;
            for x in (X_MIN..=X_MAX).step_by(FALLBACK_STEP) {
                for y in (Y_MIN..=Y_MAX).step_by(FALLBACK_STEP) {
                    let room = visible
                        .iter()
                        .map(|&v| dist2(v, (x, y)))
                        .min()
                        .unwrap_or(i64::MAX);
                    let off = (dist2(from, (x, y)) - want * want).abs();
                    if room >= min_sep * min_sep && clear.map(|c| off < c.1).unwrap_or(true) {
                        clear = Some(((x, y), off));
                    }
                }
            }
            if let Some((spot, _)) = clear {
                best = Some((spot, heading, min_sep * min_sep));
            }
        }
        let (spot, angle, _) = best.unwrap_or((((X_MIN + X_MAX) / 2, (Y_MIN + Y_MAX) / 2), heading, 0));
        heading = angle;
        at = spot;
        let beat_index = (t - a.first_beat_us) / a.beat_us;
        let this_phrase = beat_index / PHRASE_BEATS;
        if this_phrase != phrase {
            phrase = this_phrase;
            combo = 0;
        }
        combo += 1;
        // a hold, when the next target is far enough off
        let next_t = times.get(k + 1).copied().unwrap_or(a.length_us - LEAD_OUT_US);
        let mut hold = None;
        if next_t - t >= HOLD_MIN_BEATS * a.beat_us && rng.chance(d.hold_pct as u32, 100) {
            let end = next_t - a.beat_us.max(d.min_gap_us);
            if end - t >= a.beat_us {
                let len_ms = (end - t) / 1000;
                let reach = (d.spacing * len_ms / 200).clamp(radius as i64 * 2, d.max_jump);
                let bend = rng.between(-fx::TURN / 8, fx::TURN / 8);
                let mut path = vec![[spot.0, spot.1]];
                let mut p = spot;
                let mut dir = heading + rng.between(-fx::TURN / 4, fx::TURN / 4);
                for _ in 0..2 {
                    let step = (reach / 2) as i32;
                    let nx = p.0 + fx::round(fx::mul(fx::cos(dir), fx::from_int(step)));
                    let ny = p.1 + fx::round(fx::mul(fx::sin(dir), fx::from_int(step)));
                    let (nx, ny) = if inside(nx, ny) {
                        (nx, ny)
                    } else {
                        (p.0 - (nx - p.0), p.1 - (ny - p.1))
                    };
                    let (nx, ny) = (nx.clamp(X_MIN, X_MAX), ny.clamp(Y_MIN, Y_MAX));
                    path.push([nx, ny]);
                    p = (nx, ny);
                    dir += bend;
                }
                hold = Some(Hold { end, path });
            }
        }
        targets.push(Target {
            t,
            x: spot.0,
            y: spot.1,
            combo,
            hold,
        });
        k += 1;
    }
    Chart {
        song: song.to_string(),
        difficulty,
        beat_us: a.beat_us,
        first_beat_us: a.first_beat_us,
        length_us: a.length_us,
        radius,
        targets,
    }
}

/// How hard a chart is, as numbers to compare: targets a minute, and the
/// mean distance moved a second (units).
pub fn load(c: &Chart) -> (i64, i64) {
    let minutes_milli = (c.length_us / 60_000).max(1);
    let per_minute = c.targets.len() as i64 * 1000 / minutes_milli;
    let mut moved = 0i64;
    for pair in c.targets.windows(2) {
        let a = pair[0].point_at(pair[0].end());
        let b = (pair[1].x, pair[1].y);
        moved += fx::floor(fx::length(fx::from_int(a.0 - b.0), fx::from_int(a.1 - b.1))) as i64;
    }
    (per_minute, moved * 1_000_000 / c.length_us.max(1))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::beats::tests::{clicks_at, song};
    use crate::beats::{analyse, US};
    use crate::play::{perfect_inputs, replay, Judgement};

    /// A song with beats, offbeats and sixteenths at three strengths.
    pub fn busy_song(bpm: i64, seconds: i64, seed: u64) -> Analysis {
        let beat = 60 * US / bpm;
        let mut c = clicks_at(bpm, 400_000, seconds);
        let mut extra = Vec::new();
        for (k, &(t, _)) in c.iter().enumerate() {
            extra.push((t + beat / 2, 7000));
            if k % 2 == 1 {
                extra.push((t + beat / 4, 4000));
                extra.push((t + 3 * beat / 4, 3500));
            }
        }
        c.extend(extra);
        c.sort();
        analyse(&song(seconds, &c, seed)).expect("a beat")
    }

    fn all(a: &Analysis) -> Vec<Chart> {
        (0..5).map(|d| chart("test", a, d, 7)).collect()
    }

    #[test]
    fn generation_is_the_same_every_time() {
        let a = busy_song(128, 40, 11);
        assert_eq!(all(&a), all(&busy_song(128, 40, 11)));
        assert_ne!(
            chart("test", &a, 2, 7),
            chart("test", &a, 2, 8),
            "the seed matters"
        );
    }

    #[test]
    fn every_generated_chart_passes_the_chart_rules() {
        for (bpm, seed) in [(96, 1), (128, 2), (174, 3)] {
            for c in all(&busy_song(bpm, 40, seed)) {
                assert_eq!(c.check(), Ok(()), "{bpm} bpm, difficulty {}", c.difficulty);
            }
        }
    }

    #[test]
    fn targets_stay_inside_the_playfield() {
        for c in all(&busy_song(150, 40, 4)) {
            for t in &c.targets {
                assert!(inside(t.x, t.y));
                for p in t.hold.iter().flat_map(|h| h.path.iter()) {
                    assert!(inside(p[0], p[1]));
                }
            }
        }
    }

    #[test]
    fn no_two_visible_targets_overlap() {
        for (bpm, seed) in [(110, 5), (140, 6), (174, 7)] {
            for c in all(&busy_song(bpm, 40, seed)) {
                let d = DIFFICULTIES[c.difficulty];
                let sep = (2 * c.radius) as i64;
                for (k, t) in c.targets.iter().enumerate() {
                    for p in &c.targets[..k] {
                        if p.end() + d.good_us >= t.t - d.approach_us {
                            assert!(
                                dist2((p.x, p.y), (t.x, t.y)) >= sep * sep,
                                "{bpm} bpm {}: targets at {} and {} us overlap",
                                d.key,
                                p.t,
                                t.t
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn targets_sit_on_the_grid_and_never_closer_than_the_gap() {
        let a = busy_song(128, 40, 8);
        for c in all(&a) {
            let d = DIFFICULTIES[c.difficulty];
            let step = a.beat_us / d.grid;
            for t in &c.targets {
                let off = (t.t - a.first_beat_us) * d.grid % a.beat_us;
                assert!(
                    off * 1000 / a.beat_us < 5 || (a.beat_us - off) * 1000 / a.beat_us < 5,
                    "{} off grid by {off}",
                    d.key
                );
                let _ = step;
            }
            for pair in c.targets.windows(2) {
                assert!(
                    pair[1].t - pair[0].end() >= d.min_gap_us.min(pair[1].t - pair[0].t),
                    "{}",
                    d.key
                );
                assert!(pair[1].t - pair[0].t >= d.min_gap_us, "{}", d.key);
            }
        }
    }

    #[test]
    fn each_difficulty_is_harder_than_the_one_below() {
        for (bpm, seed) in [(100, 9), (128, 10), (170, 11)] {
            let charts = all(&busy_song(bpm, 60, seed));
            for pair in charts.windows(2) {
                let (lo, hi) = (load(&pair[0]), load(&pair[1]));
                assert!(
                    hi.0 > lo.0,
                    "{bpm} bpm: {} has {} targets a minute, {} has {}",
                    DIFFICULTIES[pair[1].difficulty].key,
                    hi.0,
                    DIFFICULTIES[pair[0].difficulty].key,
                    lo.0
                );
                assert!(
                    hi.1 > lo.1,
                    "{bpm} bpm: {} moves {} a second, below {}",
                    DIFFICULTIES[pair[1].difficulty].key,
                    hi.1,
                    lo.1
                );
            }
        }
    }

    #[test]
    fn one_huge_hit_does_not_empty_the_chart() {
        let mut a = busy_song(128, 40, 14);
        let plain = choose_slots(&a, 0).len();
        // one slot forty times as loud as anything else
        let top = *a.slots.iter().max().unwrap();
        a.slots[40] = top * 40;
        let with_hit = choose_slots(&a, 0).len();
        assert!(with_hit * 10 >= plain * 9, "{plain} targets fell to {with_hit}");
    }

    #[test]
    fn a_quiet_opening_still_gets_targets() {
        // the first half at a tenth of the loudness of the second
        let mut a = busy_song(128, 60, 15);
        let half = a.slots.len() / 2;
        for v in a.slots[..half].iter_mut() {
            *v /= 10;
        }
        let c = chart("test", &a, 0, 1);
        let first = c.targets[0].t;
        assert!(first < 4_000_000, "Easy's first target waits until {first} us");
        let early = c.targets.iter().filter(|t| t.t < a.length_us / 2).count();
        assert!(
            early * 3 >= c.targets.len(),
            "{early} of {} targets in the quiet half",
            c.targets.len()
        );
    }

    #[test]
    fn combo_numbers_start_again_each_phrase() {
        let a = busy_song(120, 40, 12);
        let c = chart("test", &a, 3, 1);
        let phrase_of = |t: i64| (t - a.first_beat_us) / a.beat_us / PHRASE_BEATS;
        for pair in c.targets.windows(2) {
            if phrase_of(pair[1].t) == phrase_of(pair[0].t) {
                assert_eq!(pair[1].combo, pair[0].combo + 1);
            } else {
                assert_eq!(pair[1].combo, 1);
            }
        }
    }

    #[test]
    fn some_charts_have_holds_and_the_bot_clears_them() {
        let a = busy_song(128, 60, 13);
        let mut holds = 0;
        for c in all(&a) {
            holds += c.targets.iter().filter(|t| t.hold.is_some()).count();
            let p = replay(c.clone(), &perfect_inputs(&c));
            assert_eq!(p.counts[Judgement::Perfect as usize] as usize, c.targets.len());
            assert_eq!(p.max_combo, p.full_combo());
        }
        assert!(holds > 0, "no holds at all");
    }

    #[test]
    fn each_difficulty_s_targets_are_all_in_the_one_above() {
        // Easy's in Normal's, Normal's in Hard's, and so on (R7.2)
        for (bpm, seed) in [(100, 31), (128, 32), (170, 33)] {
            let charts = all(&busy_song(bpm, 40, seed));
            for pair in charts.windows(2) {
                let above: Vec<i64> = pair[1].targets.iter().map(|t| t.t).collect();
                for t in &pair[0].targets {
                    assert!(
                        above.contains(&t.t),
                        "{bpm} bpm: {} has a target at {} that {} lacks",
                        DIFFICULTIES[pair[0].difficulty].key,
                        t.t,
                        DIFFICULTIES[pair[1].difficulty].key
                    );
                }
            }
        }
    }

    #[test]
    fn every_target_sits_on_an_onset() {
        // a line with no onset on it is never a target, even where a whole
        // stretch has none (R7.2)
        let mut a = busy_song(128, 40, 34);
        let half = a.slots.len() / 2;
        for v in a.slots[half..].iter_mut() {
            *v = 0;
        }
        for c in all(&a) {
            for t in &c.targets {
                let q = ((t.t - a.first_beat_us) * a.division / a.beat_us) as usize;
                assert_eq!(a.slot_time(q), t.t);
                assert!(
                    a.slots[q] > 0,
                    "{} target at {} us",
                    DIFFICULTIES[c.difficulty].key,
                    t.t
                );
            }
        }
    }

    #[test]
    fn a_busier_song_gets_bigger_circles_within_bounds() {
        // sized for 348 onsets a minute (R7.3)
        let minute = 60_000_000;
        assert_eq!(size_pct(348, minute), 100);
        // a quarter as busy: half by the square root, held at 80%
        assert_eq!(size_pct(87, minute), 80);
        // four times as busy: twice, held at 120%
        assert_eq!(size_pct(1392, minute), 120);
        // 500 is 1.4368 times as busy; its square root is 1.1987: 119%
        assert_eq!(size_pct(500, minute), 119);
        // 250 is 0.7184; its square root is 0.8476: 84%
        assert_eq!(size_pct(250, minute), 84);
    }

    #[test]
    fn each_difficulty_s_circles_are_smaller_than_the_one_below_on_a_song() {
        for (bpm, seed) in [(100, 35), (128, 36), (170, 37)] {
            let charts = all(&busy_song(bpm, 40, seed));
            for pair in charts.windows(2) {
                assert!(
                    pair[1].radius < pair[0].radius,
                    "{bpm} bpm: {} then {}",
                    pair[0].radius,
                    pair[1].radius
                );
            }
        }
    }

    #[test]
    fn the_true_chart_has_a_target_on_every_hit() {
        // Master from the true chart: every heard line above the true floor,
        // however close (R7.6)
        let a = busy_song(128, 40, 41);
        let c = chart_with("test", &a, 4, 1, true);
        let hits = heard_above(&a, TRUE_FLOOR_PCT);
        assert_eq!(c.targets.len(), hits.len());
        let times: Vec<i64> = hits.iter().map(|&q| a.slot_time(q)).collect();
        assert_eq!(c.targets.iter().map(|t| t.t).collect::<Vec<_>>(), times);
    }

    #[test]
    fn from_the_true_chart_the_most_stressed_hits_stay() {
        // one off-beat hit a bar made far more stressed than anything else
        // stays on Easy, over the beats beside it (R7.6)
        let mut a = busy_song(128, 40, 42);
        let d = a.division as usize;
        let bar = 4 * d;
        let mut loud = Vec::new();
        for q in (d / 2..a.slots.len()).step_by(bar) {
            if a.slots[q] > 0 && a.slot_time(q) > 2_000_000 && a.slot_time(q) < a.length_us - 3_000_000 {
                a.accents[q] = 100_000;
                loud.push(a.slot_time(q));
            }
        }
        assert!(loud.len() > 10, "{} stressed off-beats", loud.len());
        let easy = chart_with("test", &a, 0, 1, true);
        let kept = loud
            .iter()
            .filter(|t| easy.targets.iter().any(|x| x.t == **t))
            .count();
        assert!(
            kept * 10 >= loud.len() * 9,
            "{kept} of {} stressed off-beats on Easy",
            loud.len()
        );
    }

    #[test]
    fn from_the_true_chart_triplets_at_120_stay_on_hard() {
        // a third of a beat at 120 is 166.7 ms, under Expert's 170 ms and
        // Hard's 260: from the true chart Hard's gap is at most a third of a
        // beat, so Hard keeps two targets a third of a beat apart (R7.6)
        let beat = 500_000;
        let mut c: Vec<(i64, i32)> = Vec::new();
        let mut t = 300_000;
        while t < 30 * US {
            c.push((t, 20_000));
            c.push((t + beat / 3, 12_000));
            c.push((t + 2 * beat / 3, 12_000));
            t += beat;
        }
        let a = analyse(&song(30, &c, 43)).expect("a beat");
        let hard = chart_with("test", &a, 2, 1, true);
        let close = hard
            .targets
            .windows(2)
            .filter(|p| p[1].t - p[0].t < 170_000)
            .count();
        assert!(close > 10, "{close} pairs a third of a beat apart on Hard");
        for p in hard.targets.windows(2) {
            assert!(p[1].t - p[0].t >= beat / 3 - 1, "{} and {}", p[0].t, p[1].t);
        }
    }

    #[test]
    fn from_the_true_chart_each_difficulty_is_within_the_one_above() {
        for (bpm, seed) in [(100, 44), (128, 45), (170, 46)] {
            // accents that do not follow the onsets' strength, as in a mix
            let mut a = busy_song(bpm, 40, seed);
            let mut rng = Rng::new(seed);
            for v in a.accents.iter_mut().filter(|v| **v > 0) {
                *v = rng.between(1, 3000) as u32;
            }
            let charts: Vec<Chart> = (0..5).map(|d| chart_with("test", &a, d, 7, true)).collect();
            for pair in charts.windows(2) {
                let above: Vec<i64> = pair[1].targets.iter().map(|t| t.t).collect();
                assert!(pair[0].targets.len() < pair[1].targets.len());
                for t in &pair[0].targets {
                    assert!(
                        above.contains(&t.t),
                        "{bpm} bpm: {} at {}",
                        DIFFICULTIES[pair[0].difficulty].key,
                        t.t
                    );
                }
            }
            for c in &charts {
                let p = replay(c.clone(), &perfect_inputs(c));
                assert_eq!(p.counts[Judgement::Perfect as usize] as usize, c.targets.len());
            }
        }
    }
}
