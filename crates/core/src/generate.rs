//! Laying out a chart from a song's analysis (PLAN.md decision 10).
//!
//! Which slots become targets: the strongest share (`keep_pct`) of the
//! difficulty's grid, strongest first, never closer than its minimum gap.
//! Where each goes: as far from the last as the time between them allows
//! (`spacing` units a 100 ms, at most `max_jump`), turned by a seeded
//! angle, kept inside the playfield and off every target still on screen.
//! Combo numbers start again every eight beats. Some long gaps become holds.
//! The same analysis, difficulty and seed give the same chart (BO-6).

use crate::beats::{Analysis, SLOTS_PER_BEAT};
use crate::chart::{Chart, Hold, Target};
use crate::fx;
use crate::rng::Rng;
use crate::tuning::{Difficulty, DIFFICULTIES, X_MAX, X_MIN, Y_MAX, Y_MIN};

/// Beats a phrase: the combo number starts again at each phrase.
pub const PHRASE_BEATS: i64 = 8;
/// Targets start this long after the song's first onset strong enough to
/// keep, and stop this long before its end.
const LEAD_OUT_US: i64 = 1_500_000;
/// A slot quieter than this share (percent) of the song's loudest slot is
/// never a target.
const FLOOR_PCT: u64 = 8;
/// Tries at placing one target before taking the least-bad spot.
const TRIES: u32 = 96;
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

/// The slots that become targets, in time order.
pub fn choose_slots(a: &Analysis, d: &Difficulty) -> Vec<usize> {
    let step = (SLOTS_PER_BEAT / d.grid) as usize;
    let loudest = a.slots.iter().copied().max().unwrap_or(0) as u64;
    let floor = loudest * FLOOR_PCT / 100;
    let last_t = a.length_us - LEAD_OUT_US;
    let mut candidates: Vec<usize> = (0..a.slots.len())
        .step_by(step)
        .filter(|&q| a.slots[q] as u64 > floor && a.slot_time(q) <= last_t && a.slot_time(q) >= 1_000_000)
        .collect();
    // strongest first; earlier first among equals
    candidates.sort_by(|&p, &q| a.slots[q].cmp(&a.slots[p]).then(p.cmp(&q)));
    let keep = (candidates.len() as i64 * d.keep_pct / 100) as usize;
    let mut chosen: Vec<usize> = Vec::new();
    for &q in candidates.iter().take(keep) {
        let t = a.slot_time(q);
        let clear = chosen.iter().all(|&c| (a.slot_time(c) - t).abs() >= d.min_gap_us);
        if clear {
            chosen.push(q);
        }
    }
    chosen.sort();
    chosen
}

/// Generates one chart.
pub fn chart(song: &str, a: &Analysis, difficulty: usize, seed: u64) -> Chart {
    let d = DIFFICULTIES[difficulty];
    let slots = choose_slots(a, &d);
    let times: Vec<i64> = slots.iter().map(|&q| a.slot_time(q)).collect();
    let mut rng = Rng::new(seed ^ ((difficulty as u64 + 1) << 32));
    let mut targets: Vec<Target> = Vec::new();
    let mut at = ((X_MIN + X_MAX) / 2, (Y_MIN + Y_MAX) / 2);
    let mut heading: i32 = rng.between(0, fx::TURN - 1);
    let mut combo = 0u32;
    let mut phrase = -1i64;
    let min_sep = 2 * d.radius as i64 + 8;
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
                let reach = (d.spacing * len_ms / 200).clamp(d.radius as i64 * 2, d.max_jump);
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
                let sep = (2 * d.radius) as i64;
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
}
