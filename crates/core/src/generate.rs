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

/// Beats a phrase in a song of four-beat bars: the combo number starts
/// again at each phrase.
pub const PHRASE_BEATS: i64 = 8;

/// Beats a phrase: `PHRASE_BEATS` in whole bars, the bars a power of two,
/// so a waltz's phrase is four bars of three (R8.2).
pub fn phrase_beats(a: &Analysis) -> i64 {
    let bar = a.bar_beats.max(1);
    bar * (((PHRASE_BEATS + bar - 1) / bar).max(1) as u64).next_power_of_two() as i64
}
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
/// Hard's divisor in a song with no triplets, where a third of a beat
/// would leave Hard no closer than Normal (R7.8).
const STRAIGHT_HARD_DIVISOR: i64 = 4;
/// Easy, Normal and Hard follow the groove from the true chart.
const GROOVE_LEVELS: usize = 3;
/// Bars before and after a bar whose hits decide its groove: eight in all.
const GROOVE_BACK: usize = 3;
const GROOVE_AHEAD: usize = 4;
/// The bars a groove is summed over, for its mean.
const GROOVE_BARS: u64 = (GROOVE_BACK + GROOVE_AHEAD + 1) as u64;
/// A place in the bar is part of a level's groove when stressed at least
/// this share (percent) of the most stressed place: Easy, Normal, Hard.
const GROOVE_PCT: [u64; GROOVE_LEVELS] = [25, 25, 18];
/// The share a quarter beat of a straight song needs to join Hard.
/// The share a half beat of a straight song needs to join Normal.
const STRAIGHT_HALF_PCT: u64 = 45;
const STRAIGHT_QUARTER_PCT: u64 = 35;
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

/// As `choose_slots`; with `from_true`, charted from the true chart
/// (`from_true_chart`).
pub fn choose_slots_with(a: &Analysis, difficulty: usize, from_true: bool) -> Vec<usize> {
    if from_true {
        return from_true_chart(a, difficulty);
    }
    let mut pool = heard(a);
    let mut above = 100;
    for k in (difficulty..DIFFICULTIES.len()).rev() {
        let d = &DIFFICULTIES[k];
        pool = thin(a, &pool, d, above, false, d.min_gap_us, &[]);
        above = d.keep_pct;
    }
    pool
}

/// The hardest difficulty is the true chart, a target on every hit the
/// song has, at any spacing (R7.6). The difficulties a player taps along
/// to, Easy to Hard, follow the song's groove (`groove`), so a beat is not
/// dropped because it is a little quieter than the one before (R7.7).
/// Expert keeps all of Hard and adds the most stressed of the rest (R7.6).
fn from_true_chart(a: &Analysis, difficulty: usize) -> Vec<usize> {
    let hits = heard_above(a, TRUE_FLOOR_PCT);
    let hardest = DIFFICULTIES.len() - 1;
    if difficulty == hardest {
        return hits;
    }
    let gap = |k: usize| {
        // a straight song has no thirds of a beat: its Hard takes the quarters
        let divisor = match (k, a.division % 3) {
            (2, d) if d != 0 => STRAIGHT_HARD_DIVISOR,
            _ => TRUE_GAP_DIVISORS[k],
        };
        DIFFICULTIES[k].min_gap_us.min(a.beat_us / divisor)
    };
    let levels = groove(a, &hits, [gap(0), gap(1), gap(2)]);
    // a score's featured parts join from the level they name, before the
    // groove fills in around them; each level starts from the one below
    // (R8.4)
    let mut below: Vec<usize> = Vec::new();
    for (k, level) in levels.iter().enumerate().take(difficulty + 1) {
        below = featured(a, &hits, &below, k, gap(k), level);
    }
    if difficulty < GROOVE_LEVELS {
        return below;
    }
    let must = featured(a, &hits, &below, difficulty, gap(difficulty), &[]);
    thin(
        a,
        &hits,
        &DIFFICULTIES[difficulty],
        100,
        true,
        gap(difficulty),
        &must,
    )
}

/// Level `k` of a chart from the true chart: all of `below`, then each hit
/// a score part asks level `k` to play, then the `groove`'s places, each
/// only when `gap` from every target already chosen.
fn featured(
    a: &Analysis,
    hits: &[usize],
    below: &[usize],
    k: usize,
    gap: i64,
    groove: &[usize],
) -> Vec<usize> {
    let mut taken: std::collections::BTreeSet<i64> = below.iter().map(|&q| a.slot_time(q)).collect();
    let mut out: Vec<usize> = below.to_vec();
    let asked = hits
        .iter()
        .copied()
        .filter(|&q| a.play_from.get(q).is_some_and(|&f| f as usize <= k));
    for q in asked.chain(groove.iter().copied()) {
        let t = a.slot_time(q);
        let clear = taken.range(t - gap + 1..t + gap).next().is_none();
        if clear {
            taken.insert(t);
            out.push(q);
        }
    }
    out.sort();
    out
}

/// How stressed the hit on line `q` is: its accent (or, in an analysis
/// without accents, its onset strength) times its place in the beat.
fn stress(a: &Analysis, q: usize) -> u64 {
    a.accents.get(q).copied().unwrap_or(a.slots[q]) as u64 * weight(q, a.division)
}

/// Easy, Normal and Hard from the true chart's `hits`. Each bar's groove is
/// the places in a bar (its lines, four beats long) that the bars around it
/// stress most, summed over those bars, so a place the song keeps coming
/// back to counts and one loud stray hit does not. Easy takes the beats
/// among them, Normal adds places at least half a beat from the rest, Hard
/// places at least a third of a beat (`gaps`), each only where the song
/// has a hit, in every bar. A place joins only when stressed at least its
/// level's `GROOVE_PCT` of the bar's most stressed (R7.7).
fn groove(a: &Analysis, hits: &[usize], gaps: [i64; GROOVE_LEVELS]) -> [Vec<usize>; GROOVE_LEVELS] {
    let d = a.division as usize;
    let bar = a.bar_beats.max(1) as usize * d;
    let n = a.slots.len();
    let bars = n.div_ceil(bar);
    let mut on = vec![0u64; bars * bar];
    for &q in hits {
        on[q] = stress(a, q).max(1);
    }
    // and in how many bars each place is inside the song, so a bar near
    // the start or the end is not judged by the silence past it
    let (first, last) = (
        hits.first().copied().unwrap_or(0),
        hits.last().copied().unwrap_or(0),
    );
    let mut prefix = vec![vec![0u64; bar]; bars + 1];
    let mut inside = vec![vec![0u64; bar]; bars + 1];
    for b in 0..bars {
        for p in 0..bar {
            let q = b * bar + p;
            prefix[b + 1][p] = prefix[b][p] + on[q];
            inside[b + 1][p] = inside[b][p] + u64::from(q >= first && q <= last);
        }
    }
    let span = |lines: usize| lines as i64 * a.beat_us / a.division;
    let mut out: [Vec<usize>; GROOVE_LEVELS] = Default::default();
    for b in 0..bars {
        let (lo, hi) = (b.saturating_sub(GROOVE_BACK), (b + GROOVE_AHEAD + 1).min(bars));
        let sal: Vec<u64> = (0..bar)
            .map(|p| {
                let bars_in = inside[hi][p] - inside[lo][p];
                (prefix[hi][p] - prefix[lo][p]) * GROOVE_BARS / bars_in.max(1)
            })
            .collect();
        let mut order: Vec<usize> = (0..bar).collect();
        order.sort_by(|&x, &y| sal[y].cmp(&sal[x]).then(x.cmp(&y)));
        let top = sal.iter().copied().max().unwrap_or(0);
        let top_beat = (0..bar).step_by(d).map(|p| sal[p]).max().unwrap_or(0);
        let mut places: Vec<usize> = Vec::new();
        for (level, &gap) in gaps.iter().enumerate() {
            let base = if level == 0 { top_beat } else { top };
            for &p in &order {
                // in a straight song, a half beat joins Normal and a
                // quarter beat Hard only when the song leans on it, so
                // each level adds to the one below (R7.8)
                let straight = a.division % 3 != 0;
                let pct = match level {
                    1 if straight && p % d != 0 => STRAIGHT_HALF_PCT,
                    2 if straight && !on_grid(p, a.division, 2) => STRAIGHT_QUARTER_PCT,
                    _ => GROOVE_PCT[level],
                };
                let stressed = sal[p] > 0 && sal[p] * 100 >= base * pct;
                let clear = places.iter().all(|&c| {
                    let apart = p.abs_diff(c);
                    span(apart.min(bar - apart)) >= gap
                });
                if stressed && clear && (level > 0 || p % d == 0) && !places.contains(&p) {
                    places.push(p);
                }
            }
            for &p in &places {
                let q = b * bar + p;
                if q < n && on[q] > 0 {
                    out[level].push(q);
                }
            }
        }
    }
    // where the groove changes between bars, two targets can meet across
    // the bar line: keep what the level below has, else the earlier
    let mut below: Vec<usize> = Vec::new();
    for (level, gap) in gaps.iter().enumerate() {
        out[level].sort();
        let mut kept: Vec<usize> = Vec::new();
        for &q in &out[level] {
            match kept.last() {
                Some(&last) if a.slot_time(q) - a.slot_time(last) < *gap => {
                    if below.binary_search(&q).is_ok() && below.binary_search(&last).is_err() {
                        kept.pop();
                        kept.push(q);
                    }
                }
                _ => kept.push(q),
            }
        }
        out[level] = kept.clone();
        below = kept;
    }
    out
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
    must: &[usize],
) -> Vec<usize> {
    let stress = |q: usize| match (by_accent, a.accents.get(q)) {
        (true, Some(&v)) => v as u64,
        _ => a.slots[q] as u64,
    };
    let score = |q: usize| stress(q) * weight(q, a.division);
    let by_score = |p: &usize, q: &usize| score(*q).cmp(&score(*p)).then(p.cmp(q));
    let phrase_lines = (phrase_beats(a) * a.division) as usize;
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
    // `must` (sorted) is kept whole, and the rest fit around it
    let mut chosen: Vec<usize> = must.to_vec();
    for &q in kept.iter().filter(|q| must.binary_search(q).is_err()) {
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
        let this_phrase = beat_index / phrase_beats(a);
        if this_phrase != phrase {
            phrase = this_phrase;
            combo = 0;
        }
        combo += 1;
        // a hold, when the next target is far enough off
        let next_t = times.get(k + 1).copied().unwrap_or(a.length_us - LEAD_OUT_US);
        let mut hold = None;
        // from a score, a hold is the lead's long note, held to its end or
        // until the next target needs the hand; otherwise a long gap
        // sometimes becomes one (R8.2)
        let held = match a.sustains.get(slots[k]) {
            _ if a.sustains.is_empty() => (next_t - t >= HOLD_MIN_BEATS * a.beat_us
                && rng.chance(d.hold_pct as u32, 100))
            .then(|| next_t - a.beat_us.max(d.min_gap_us)),
            Some(&lines) if lines as i64 >= HOLD_MIN_BEATS * a.division => {
                Some((a.slot_time(slots[k] + lines as usize)).min(next_t - d.min_gap_us))
            }
            _ => None,
        };
        if let Some(end) = held {
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
    use crate::beats::{analyse, NOT_FEATURED, US};
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
        let phrase_of = |t: i64| (t - a.first_beat_us) / a.beat_us / phrase_beats(&a);
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
        // however close, the barely stressed quarters too (R7.6)
        let a = straight(41);
        let c = chart_with("test", &a, 4, 1, true);
        let hits = heard_above(&a, TRUE_FLOOR_PCT);
        assert_eq!(c.targets.len(), hits.len());
        let times: Vec<i64> = hits.iter().map(|&q| a.slot_time(q)).collect();
        assert_eq!(c.targets.iter().map(|t| t.t).collect::<Vec<_>>(), times);
    }

    #[test]
    fn from_the_true_chart_the_most_stressed_hits_stay_on_expert() {
        // one hit a quarter of a beat off the beat in each bar, made more
        // stressed than anything else, stays on Expert, where accent
        // decides. The song's other quarter beats are barely stressed, and
        // the loud one moves among the bar's quarter places, so no place
        // is the groove and Hard leaves it (R7.6)
        let mut a = busy_song(128, 40, 42);
        let d = a.division as usize;
        assert_eq!(d, 4);
        let bar = 4 * d;
        let top = a.accents.iter().copied().max().unwrap_or(0);
        for q in 0..a.slots.len() {
            match q % d {
                0 => a.accents[q] = top,
                1 | 3 => a.accents[q] = 1,
                _ => {}
            }
        }
        let quarters: Vec<usize> = (0..bar)
            .filter(|p| p % 2 == 1 && a.slots[*p + 4 * bar] > 0)
            .collect();
        assert!(quarters.len() >= 4, "{quarters:?}");
        let mut loud = Vec::new();
        for b in 0..a.slots.len() / bar {
            let q = b * bar + quarters[b % quarters.len()];
            if a.slots[q] > 0 && a.slot_time(q) > 2_000_000 && a.slot_time(q) < a.length_us - 3_000_000 {
                a.accents[q] = top * 2;
                loud.push(a.slot_time(q));
            }
        }
        let hard = chart_with("test", &a, 2, 1, true);
        assert!(
            loud.iter()
                .filter(|t| hard.targets.iter().any(|x| x.t == **t))
                .count()
                * 2
                < loud.len(),
            "Hard keeps most of the moving hits"
        );
        assert!(loud.len() > 10, "{} stressed off-beats", loud.len());
        let expert = chart_with("test", &a, 3, 1, true);
        let kept = loud
            .iter()
            .filter(|t| expert.targets.iter().any(|x| x.t == **t))
            .count();
        assert!(
            kept * 10 >= loud.len() * 9,
            "{kept} of {} stressed off-beats on Expert",
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
                assert!(pair[0].targets.len() <= pair[1].targets.len());
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

    /// An analysis whose accents are set by hand: 1000 on each beat and on
    /// the half beat after the second beat of every bar, 50 elsewhere.
    fn grooved(seed: u64) -> Analysis {
        let mut a = busy_song(120, 40, seed);
        let d = a.division as usize;
        for q in 0..a.slots.len() {
            a.slots[q] = 5000;
            a.accents[q] = if q % d == 0 || q % (4 * d) == d + d / 2 {
                1000
            } else {
                50
            };
        }
        a
    }

    #[test]
    fn from_the_true_chart_easy_keeps_every_beat() {
        // a player taps the beat: no beat with a hit on it is left out of
        // Easy for being a little quieter than the others (R7.7)
        let mut a = grooved(51);
        let d = a.division as usize;
        let mut rng = Rng::new(51);
        for q in (0..a.slots.len()).step_by(d) {
            a.accents[q] = rng.between(600, 1000) as u32;
        }
        let easy = chart_with("test", &a, 0, 1, true);
        let hits = heard_above(&a, TRUE_FLOOR_PCT);
        let beats: Vec<i64> = hits
            .iter()
            .filter(|&&q| q % d == 0)
            .map(|&q| a.slot_time(q))
            .collect();
        let times: Vec<i64> = easy.targets.iter().map(|t| t.t).collect();
        assert_eq!(times, beats);
    }

    #[test]
    fn from_the_true_chart_normal_follows_the_groove_not_one_stray_hit() {
        // the half beat after beat two, stressed in every bar, is on Normal
        // in every bar; the half beat after beat four, stressed in one bar
        // only, is not (R7.7)
        let mut a = grooved(52);
        let d = a.division as usize;
        let bar = 4 * d;
        let stray = 6 * bar + 3 * d + d / 2;
        a.accents[stray] = 1000;
        let normal = chart_with("test", &a, 1, 1, true);
        let times: Vec<i64> = normal.targets.iter().map(|t| t.t).collect();
        assert!(!times.contains(&a.slot_time(stray)), "the stray hit is on Normal");
        let grooves: Vec<usize> = heard_above(&a, TRUE_FLOOR_PCT)
            .into_iter()
            .filter(|&q| q % bar == d + d / 2)
            .collect();
        for q in &grooves {
            assert!(
                times.contains(&a.slot_time(*q)),
                "the groove's hit at {} is missing",
                a.slot_time(*q)
            );
        }
        assert!(grooves.len() > 10);
    }

    /// A straight song, a bar of four beats in quarter lines: beats
    /// stressed most, one half beat a bar nearly as much, the others
    /// lightly; the first quarter of each beat stressed, the last a little
    /// (above Hard's usual share, below a straight quarter's).
    fn straight(seed: u64) -> Analysis {
        let mut a = busy_song(120, 40, seed);
        assert_eq!(a.division, 4);
        for q in 0..a.slots.len() {
            a.slots[q] = 5000;
            a.accents[q] = match (q % 16, q % 4) {
                (_, 0) => 1000,
                (6, _) => 800,
                (_, 2) => 400,
                (_, 1) => 600,
                _ => 350,
            };
        }
        a
    }

    fn times_where(a: &Analysis, keep: impl Fn(usize) -> bool) -> Vec<i64> {
        heard_above(a, TRUE_FLOOR_PCT)
            .into_iter()
            .filter(|&q| keep(q))
            .map(|q| a.slot_time(q))
            .collect()
    }

    #[test]
    fn a_straight_songs_normal_leaves_its_lightly_stressed_half_beats() {
        // the light half beats (a third of the beat's stress) stay out of
        // Normal, the one the song leans on comes in (R7.8)
        let a = straight(61);
        let normal: Vec<i64> = chart_with("test", &a, 1, 1, true)
            .targets
            .iter()
            .map(|t| t.t)
            .collect();
        assert_eq!(normal, times_where(&a, |q| q % 4 == 0 || q % 16 == 6));
    }

    #[test]
    fn a_straight_songs_hard_adds_the_quarter_beats_it_leans_on() {
        // Hard is closer than Normal in a straight song: the stressed
        // quarter beats come in, the barely-there ones do not (R7.8)
        let a = straight(62);
        let hard: Vec<i64> = chart_with("test", &a, 2, 1, true)
            .targets
            .iter()
            .map(|t| t.t)
            .collect();
        let leaned = times_where(&a, |q| q % 4 == 1);
        let light = times_where(&a, |q| q % 4 == 3);
        assert!(leaned.len() > 30);
        assert!(
            leaned.iter().all(|t| hard.contains(t)),
            "a stressed quarter is missing: {:?}",
            leaned.iter().filter(|t| !hard.contains(t)).collect::<Vec<_>>()
        );
        assert!(
            light.iter().all(|t| !hard.contains(t)),
            "a barely-there quarter is in"
        );
    }

    /// A waltz at 120: bars of three beats, two lines a beat; each bar's
    /// first beat stressed, its other beats lightly, its half beats barely.
    fn waltz() -> Analysis {
        let n = 160;
        Analysis {
            beat_us: 500_000,
            first_beat_us: 0,
            length_us: 40_000_000,
            division: 2,
            slots: vec![5000; n],
            accents: (0..n)
                .map(|q| match (q % 6, q % 2) {
                    (0, _) => 1000,
                    (_, 0) => 150,
                    _ => 50,
                })
                .collect(),
            first_sound_us: 0,
            bar_beats: 3,
            sustains: Vec::new(),
            play_from: Vec::new(),
        }
    }

    #[test]
    fn a_waltzs_easy_is_its_downbeats() {
        // the groove is found in bars of three: every first beat, and no
        // other beat, stressed a sixth as much (R8.2)
        let a = waltz();
        let easy: Vec<i64> = chart_with("test", &a, 0, 1, true)
            .targets
            .iter()
            .map(|t| t.t)
            .collect();
        let downbeats: Vec<i64> = (0..160usize)
            .filter(|q| q % 6 == 0)
            .map(|q| q as i64 * 250_000)
            .filter(|t| (1_000_000..=38_500_000).contains(t))
            .collect();
        assert_eq!(easy, downbeats);
    }

    #[test]
    fn a_waltzs_combo_starts_again_every_four_bars() {
        let a = waltz();
        let easy = chart_with("test", &a, 0, 1, true);
        // after the first phrase (whose first bar is in the lead-in)
        for t in easy.targets.iter().filter(|t| t.t >= 6_000_000) {
            let bar = t.t / 1_500_000;
            assert_eq!(t.combo as i64, bar % 4 + 1, "the target at {}", t.t);
        }
    }

    #[test]
    fn from_a_score_the_leads_long_notes_are_held() {
        // the lead holds two beats (1 s) at the start of every fourth bar,
        // and the next bar's downbeat is silent: on Easy, a downbeat a bar,
        // each such note is a hold to its end, 1 s on, sooner than 0.6 s
        // (Easy's gap) before the next target, 3 s on. In the other bars,
        // no hold (R8.2)
        let mut a = waltz();
        a.sustains = (0..160).map(|q| if q % 24 == 0 { 4 } else { 0 }).collect();
        for q in (6..160).step_by(24) {
            a.slots[q] = 0;
        }
        let easy = chart_with("test", &a, 0, 1, true);
        let mut held = 0;
        for t in &easy.targets {
            let end = t.hold.as_ref().map(|h| h.end);
            if t.t % 6_000_000 == 0 && t.t + 3_000_000 <= 38_500_000 {
                assert_eq!(end, Some(t.t + 1_000_000), "the target at {}", t.t);
                held += 1;
            } else {
                assert_eq!(end, None, "the target at {}", t.t);
            }
        }
        assert!(held >= 5, "{held} held");
    }

    #[test]
    fn a_scores_featured_part_is_played_from_the_level_it_names() {
        // a part on the half beat after each downbeat, barely stressed, so
        // the groove alone would leave it, asks to be played from Normal:
        // every one of its notes is on Normal, Hard and Expert, none on Easy
        // (R8.4)
        let mut a = waltz();
        a.play_from = (0..160)
            .map(|q| if q % 6 == 1 { 1 } else { NOT_FEATURED })
            .collect();
        let part = times_where(&a, |q| q % 6 == 1);
        assert!(part.len() > 20);
        let groove_only = chart_with("test", &waltz(), 1, 1, true);
        assert!(part.iter().all(|t| groove_only.targets.iter().all(|x| x.t != *t)));
        for (difficulty, want) in [(0, false), (1, true), (2, true), (3, true)] {
            let c = chart_with("test", &a, difficulty, 1, true);
            for t in &part {
                assert_eq!(
                    c.targets.iter().any(|x| x.t == *t),
                    want,
                    "difficulty {difficulty}, {t}"
                );
            }
        }
    }
}
