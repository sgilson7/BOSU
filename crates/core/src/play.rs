//! One play of one chart: judging presses, holds, score, combo and
//! accuracy (PLAN.md decisions 2, 4, 5 and 6).
//!
//! Every input carries its own time in microseconds of song time, already
//! moved by the player's audio offset. The page's frames only say "time has
//! reached t"; they never decide a judgement. So the same inputs give the
//! same play at any frame rate (BO-2): `advance` processes misses and hold
//! ticks in time order up to each input's own time before applying it.

use serde::{Deserialize, Serialize};

use crate::chart::{Chart, Target};
use crate::tuning::{Difficulty, COMBO_CAP, DIFFICULTIES, END_POINTS, HOLD_FOLLOW_PCT, TICK_POINTS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Judgement {
    Perfect = 0,
    Great = 1,
    Good = 2,
    Miss = 3,
}

/// Points for each judgement before the combo multiplier (BO-Q16).
pub const POINTS: [i64; 4] = [100, 60, 25, 0];
pub const JUDGEMENTS: [Judgement; 4] = [
    Judgement::Perfect,
    Judgement::Great,
    Judgement::Good,
    Judgement::Miss,
];

impl Judgement {
    pub fn key(self) -> &'static str {
        ["perfect", "great", "good", "miss"][self as usize]
    }
}

/// What the player did, at what time. Buttons are numbered: the mouse or a
/// touch is 0, Z is 1, X is 2. A press lands where the cursor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Input {
    Move { t: i64, x: i32, y: i32 },
    Down { t: i64, button: u8 },
    Up { t: i64, button: u8 },
}

impl Input {
    pub fn at(&self) -> i64 {
        match *self {
            Input::Move { t, .. } | Input::Down { t, .. } | Input::Up { t, .. } => t,
        }
    }
}

/// Something to show where it happened: a judgement on a target, a hold
/// tick, a hold's end, or a broken hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Judged(Judgement),
    Tick,
    HoldDone,
    HoldBroken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Feedback {
    pub t: i64,
    pub x: i32,
    pub y: i32,
    pub mark: Mark,
    pub offset_us: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveHold {
    index: usize,
    next_tick: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Play {
    pub chart: Chart,
    pub d: Difficulty,
    pub now: i64,
    pub cursor: (i32, i32),
    held: u32,
    /// Each target's head judgement, once it has one.
    pub judged: Vec<Option<Judgement>>,
    /// The first target that may still be unjudged.
    first_open: usize,
    hold: Option<ActiveHold>,
    pub score: i64,
    pub combo: u32,
    pub max_combo: u32,
    pub counts: [u32; 4],
    pub ticks_hit: u32,
    pub ticks_total: u32,
    pub holds_done: u32,
    /// The sum and count of hit timings, for the mean early or late.
    offset_sum: i64,
    offset_n: i64,
    pub feed: Vec<Feedback>,
    /// Every input, in order: the replay a ledger record carries.
    pub replay: Vec<Input>,
}

fn within(a: (i32, i32), b: (i32, i32), r: i32) -> bool {
    let dx = (a.0 - b.0) as i64;
    let dy = (a.1 - b.1) as i64;
    dx * dx + dy * dy <= (r as i64) * (r as i64)
}

/// The times of a hold's beat ticks: each beat after the press, while it is
/// more than a Good window before the end, so letting go inside the last
/// window never skips a tick.
pub fn tick_times(target: &Target, beat_us: i64, good_us: i64) -> Vec<i64> {
    let mut out = Vec::new();
    if let Some(h) = &target.hold {
        let mut t = target.t + beat_us;
        while t + good_us < h.end {
            out.push(t);
            t += beat_us;
        }
    }
    out
}

impl Play {
    pub fn new(chart: Chart) -> Play {
        let d = DIFFICULTIES[chart.difficulty.min(DIFFICULTIES.len() - 1)];
        let n = chart.targets.len();
        Play {
            chart,
            d,
            now: i64::MIN,
            cursor: (crate::tuning::FIELD_W / 2, crate::tuning::FIELD_H / 2),
            held: 0,
            judged: vec![None; n],
            first_open: 0,
            hold: None,
            score: 0,
            combo: 0,
            max_combo: 0,
            counts: [0; 4],
            ticks_hit: 0,
            ticks_total: 0,
            holds_done: 0,
            offset_sum: 0,
            offset_n: 0,
            feed: Vec::new(),
            replay: Vec::new(),
        }
    }

    fn multiplied(&self, points: i64) -> i64 {
        points * (100 + (self.combo as i64).min(COMBO_CAP)) / 100
    }

    fn add_combo(&mut self) {
        self.combo += 1;
        self.max_combo = self.max_combo.max(self.combo);
    }

    fn judge(&mut self, index: usize, j: Judgement, at: i64, offset_us: i64) {
        self.judged[index] = Some(j);
        self.counts[j as usize] += 1;
        let target = &self.chart.targets[index];
        let (x, y) = (target.x, target.y);
        let is_hold = target.hold.is_some();
        if j == Judgement::Miss {
            self.combo = 0;
        } else {
            self.score += self.multiplied(POINTS[j as usize]);
            self.add_combo();
            self.offset_sum += offset_us;
            self.offset_n += 1;
            if is_hold {
                self.hold = Some(ActiveHold { index, next_tick: 0 });
            }
        }
        self.feed.push(Feedback {
            t: at,
            x,
            y,
            mark: Mark::Judged(j),
            offset_us,
        });
        while self.first_open < self.judged.len() && self.judged[self.first_open].is_some() {
            self.first_open += 1;
        }
    }

    /// The next moment something happens without input: a hold tick, a
    /// hold's end, or the first open target running out of time.
    fn next_event(&self) -> Option<i64> {
        let hold_event = self.hold.map(|h| {
            let target = &self.chart.targets[h.index];
            let ticks = tick_times(target, self.chart.beat_us, self.d.good_us);
            ticks.get(h.next_tick).copied().unwrap_or(target.end())
        });
        let miss = self
            .chart
            .targets
            .get(self.first_open)
            .map(|t| t.t + self.d.good_us + 1);
        match (hold_event, miss) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn on_path(&self, index: usize, at: i64) -> bool {
        let target = &self.chart.targets[index];
        let r = self.d.radius * HOLD_FOLLOW_PCT / 100;
        within(self.cursor, target.point_at(at), r)
    }

    fn following(&self, index: usize, at: i64) -> bool {
        self.held != 0 && self.on_path(index, at)
    }

    fn finish_hold(&mut self, h: ActiveHold, at: i64, done: bool) {
        let target = &self.chart.targets[h.index];
        let ticks = tick_times(target, self.chart.beat_us, self.d.good_us);
        let (x, y) = target.point_at(at);
        self.hold = None;
        if done {
            self.score += self.multiplied(END_POINTS);
            self.add_combo();
            self.holds_done += 1;
            self.feed.push(Feedback {
                t: at,
                x,
                y,
                mark: Mark::HoldDone,
                offset_us: 0,
            });
        } else {
            self.ticks_total += (ticks.len() - h.next_tick) as u32;
            self.combo = 0;
            self.feed.push(Feedback {
                t: at,
                x,
                y,
                mark: Mark::HoldBroken,
                offset_us: 0,
            });
        }
    }

    /// Runs everything that happens by itself up to and including time `t`.
    pub fn advance(&mut self, t: i64) {
        while let Some(at) = self.next_event() {
            if at > t {
                break;
            }
            if let Some(h) = self.hold {
                let target = &self.chart.targets[h.index];
                let ticks = tick_times(target, self.chart.beat_us, self.d.good_us);
                let hold_at = ticks.get(h.next_tick).copied().unwrap_or(target.end());
                if hold_at == at {
                    let following = self.following(h.index, at);
                    if h.next_tick < ticks.len() {
                        self.ticks_total += 1;
                        self.hold = Some(ActiveHold {
                            next_tick: h.next_tick + 1,
                            ..h
                        });
                        let (x, y) = target.point_at(at);
                        if following {
                            self.ticks_hit += 1;
                            self.score += TICK_POINTS;
                            self.add_combo();
                            self.feed.push(Feedback {
                                t: at,
                                x,
                                y,
                                mark: Mark::Tick,
                                offset_us: 0,
                            });
                        } else {
                            self.combo = 0;
                        }
                    } else {
                        self.finish_hold(h, at, following);
                    }
                    continue;
                }
            }
            self.judge(self.first_open, Judgement::Miss, at, 0);
        }
        self.now = self.now.max(t);
        let keep_from = self.now - 1_000_000;
        self.feed.retain(|f| f.t >= keep_from);
    }

    /// Applies one input at its own time. An input earlier than one already
    /// applied is taken as happening now: time never runs backwards.
    pub fn input(&mut self, input: Input) {
        // what happens by itself at this very moment (a tick, a hold's end)
        // comes after the input, so a move made at a tick's time counts
        let t = input.at().max(self.now);
        self.advance(t - 1);
        self.replay.push(input);
        match input {
            Input::Move { x, y, .. } => self.cursor = (x, y),
            Input::Down { button, .. } => self.press(t, button),
            Input::Up { button, .. } => self.release(t, button),
        }
    }

    fn press(&mut self, t: i64, button: u8) {
        let bit = 1u32 << (button & 31);
        if self.held & bit != 0 {
            return;
        }
        self.held |= bit;
        if self.hold.is_some() {
            return;
        }
        let d = self.d;
        let hit = (self.first_open..self.chart.targets.len())
            .take_while(|&k| self.chart.targets[k].t <= t + d.good_us)
            .find(|&k| {
                let target = &self.chart.targets[k];
                self.judged[k].is_none()
                    && (t - target.t).abs() <= d.good_us
                    && within(self.cursor, (target.x, target.y), d.radius)
            });
        if let Some(k) = hit {
            let off = t - self.chart.targets[k].t;
            let j = if off.abs() <= d.perfect_us {
                Judgement::Perfect
            } else if off.abs() <= d.great_us {
                Judgement::Great
            } else {
                Judgement::Good
            };
            self.judge(k, j, t, off);
        }
    }

    fn release(&mut self, t: i64, button: u8) {
        self.held &= !(1u32 << (button & 31));
        if self.held != 0 {
            return;
        }
        if let Some(h) = self.hold {
            let end = self.chart.targets[h.index].end();
            let in_window = t >= end - self.d.good_us;
            let done = in_window && self.on_path(h.index, t);
            self.finish_hold(h, t, done);
        }
    }

    /// The mean timing of every hit, negative when early; none before a hit.
    pub fn mean_offset_us(&self) -> Option<i64> {
        (self.offset_n > 0).then(|| self.offset_sum / self.offset_n)
    }

    /// Whether every target has been judged and no hold is running.
    pub fn over(&self) -> bool {
        self.first_open >= self.judged.len() && self.hold.is_none()
    }

    /// Accuracy in hundredths of a percent: head points earned over 100 per
    /// judged target. Before any judgement it is 100.00%.
    pub fn accuracy(&self) -> i64 {
        let judged: i64 = self.counts.iter().map(|&c| c as i64).sum();
        if judged == 0 {
            return 10_000;
        }
        let earned: i64 = JUDGEMENTS
            .iter()
            .map(|j| POINTS[*j as usize] * self.counts[*j as usize] as i64)
            .sum();
        earned * 100 / judged
    }

    /// The largest combo the chart allows: one for each target, tick and
    /// hold end.
    pub fn full_combo(&self) -> u32 {
        self.chart
            .targets
            .iter()
            .map(|t| {
                let ticks = tick_times(t, self.chart.beat_us, self.d.good_us).len() as u32;
                1 + ticks + t.hold.as_ref().map(|_| 1).unwrap_or(0)
            })
            .sum()
    }
}

/// The inputs of a player who presses every target at its exact time and
/// centre and follows every hold to its end (BO-4).
pub fn perfect_inputs(chart: &Chart) -> Vec<Input> {
    let d = DIFFICULTIES[chart.difficulty];
    let mut out = Vec::new();
    for target in &chart.targets {
        out.push(Input::Move {
            t: target.t,
            x: target.x,
            y: target.y,
        });
        out.push(Input::Down {
            t: target.t,
            button: 0,
        });
        for tick in tick_times(target, chart.beat_us, d.good_us) {
            let (x, y) = target.point_at(tick);
            out.push(Input::Move { t: tick, x, y });
        }
        let end = target.end();
        let (x, y) = target.point_at(end);
        out.push(Input::Move { t: end, x, y });
        out.push(Input::Up { t: end, button: 0 });
    }
    out
}

/// Plays a list of inputs through to the song's end.
pub fn replay(chart: Chart, inputs: &[Input]) -> Play {
    let mut p = Play::new(chart);
    for i in inputs {
        p.input(*i);
    }
    let end = p.chart.length_us;
    p.advance(end);
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::tests::{chart, tap};
    use crate::chart::Hold;

    const EASY: usize = 0;
    const MASTER: usize = 4;

    fn press_at(c: &Chart, t_us: i64, x: i32, y: i32) -> Play {
        let mut p = Play::new(c.clone());
        p.input(Input::Move { t: t_us, x, y });
        p.input(Input::Down { t: t_us, button: 0 });
        p.advance(t_us);
        p
    }

    fn one(difficulty: usize) -> Chart {
        chart(difficulty, vec![tap(2000, 800, 450, 1)])
    }

    // Window edges worked out by hand from the tuning table: Easy is
    // 50 / 95 / 140 ms, Master 26 / 55 / 90 ms, both inclusive, either side.
    #[test]
    fn easy_window_edges_on_both_sides() {
        let c = one(EASY);
        let cases = [
            (0, Some(Judgement::Perfect)),
            (50_000, Some(Judgement::Perfect)),
            (-50_000, Some(Judgement::Perfect)),
            (50_001, Some(Judgement::Great)),
            (-95_000, Some(Judgement::Great)),
            (95_001, Some(Judgement::Good)),
            (-140_000, Some(Judgement::Good)),
            (140_000, Some(Judgement::Good)),
            (-140_001, None),
        ];
        for (off, want) in cases {
            let p = press_at(&c, 2_000_000 + off, 800, 450);
            assert_eq!(p.judged[0], want, "offset {off}");
        }
        // 140.001 ms late: the target ran out of time before the press
        let p = press_at(&c, 2_140_001, 800, 450);
        assert_eq!(p.judged[0], Some(Judgement::Miss));
    }

    #[test]
    fn master_window_edges_on_both_sides() {
        let c = one(MASTER);
        let cases = [
            (26_000, Some(Judgement::Perfect)),
            (-26_001, Some(Judgement::Great)),
            (55_000, Some(Judgement::Great)),
            (-55_001, Some(Judgement::Good)),
            (90_000, Some(Judgement::Good)),
            (-90_001, None),
            (90_001, Some(Judgement::Miss)),
        ];
        for (off, want) in cases {
            let p = press_at(&c, 2_000_000 + off, 800, 450);
            assert_eq!(p.judged[0], want, "offset {off}");
        }
    }

    #[test]
    fn a_press_must_land_inside_the_circle() {
        // Easy radius 76: 76 to the right counts, 77 does not
        let c = one(EASY);
        assert_eq!(
            press_at(&c, 2_000_000, 876, 450).judged[0],
            Some(Judgement::Perfect)
        );
        assert_eq!(press_at(&c, 2_000_000, 877, 450).judged[0], None);
        // 3-4-5: (45, 60) is 75 away
        assert_eq!(
            press_at(&c, 2_000_000, 845, 510).judged[0],
            Some(Judgement::Perfect)
        );
    }

    #[test]
    fn a_press_on_no_target_does_nothing() {
        let c = one(EASY);
        let p = press_at(&c, 1_000_000, 800, 450);
        assert_eq!(p.judged[0], None);
        assert_eq!((p.score, p.combo, p.counts), (0, 0, [0; 4]));
    }

    #[test]
    fn two_overlapping_targets_take_the_earlier_first() {
        let c = chart(EASY, vec![tap(2000, 800, 450, 1), tap(2100, 810, 450, 2)]);
        let p = press_at(&c, 2_050_000, 805, 450);
        assert_eq!(p.judged, vec![Some(Judgement::Perfect), None]);
    }

    #[test]
    fn holding_a_key_down_presses_once() {
        let c = chart(EASY, vec![tap(2000, 800, 450, 1), tap(2100, 800, 450, 2)]);
        let mut p = press_at(&c, 2_000_000, 800, 450);
        p.input(Input::Down {
            t: 2_100_000,
            button: 0,
        });
        assert_eq!(p.judged[1], None, "a repeat of a held button");
        p.input(Input::Down {
            t: 2_100_000,
            button: 1,
        });
        assert_eq!(p.judged[1], Some(Judgement::Perfect), "the other key");
    }

    // Ten targets one second apart on Easy; all Perfect except a Miss at
    // target 6. Worked by hand: targets 1-5 score 100 x (100 + 0..4)/100 =
    // 100, 101, 102, 103, 104 = 510. Target 6 misses (combo to 0). Targets
    // 7-10 score 100, 101, 102, 103 = 406. Total 916. Accuracy 900/10 =
    // 90.00%. Max combo 5.
    #[test]
    fn hand_worked_score_for_ten_targets_with_one_miss() {
        let targets = (1..=10)
            .map(|k| tap(k * 1000, 400 + 50 * k as i32, 450, k as u32))
            .collect();
        let c = chart(EASY, targets);
        let inputs: Vec<Input> = perfect_inputs(&c)
            .into_iter()
            .filter(|i| i.at() != 6_000_000)
            .collect();
        let p = replay(c, &inputs);
        assert_eq!(p.score, 916);
        assert_eq!(p.accuracy(), 9000);
        assert_eq!(p.max_combo, 5);
        assert_eq!(p.counts, [9, 0, 0, 1]);
    }

    #[test]
    fn the_multiplier_stops_growing_at_a_combo_of_100() {
        let targets = (0..102)
            .map(|k| tap(1000 + k * 700, 400 + (k as i32 % 10) * 100, 450, 1))
            .collect();
        let c = chart(EASY, targets);
        let p = replay(c.clone(), &perfect_inputs(&c));
        // 100 x (100 + k)/100 for k = 0..99 is 100 + k, summing to 14950;
        // the 101st and 102nd are capped at 200 each
        assert_eq!(p.score, 14_950 + 200 + 200);
    }

    fn hold_chart() -> Chart {
        // Easy, beat 500 ms: a hold from 2.0 s to 4.0 s along a straight
        // path. Ticks at 2.5, 3.0 and 3.5 s (3.5 + 0.14 < 4.0).
        let mut c = chart(EASY, vec![tap(2000, 400, 450, 1)]);
        c.targets[0].hold = Some(Hold {
            end: 4_000_000,
            path: vec![[400, 450], [800, 450]],
        });
        c.length_us = 6_000_000;
        c
    }

    #[test]
    fn a_hold_followed_to_the_end_scores_every_tick() {
        let c = hold_chart();
        assert_eq!(
            tick_times(&c.targets[0], c.beat_us, DIFFICULTIES[0].good_us),
            vec![2_500_000, 3_000_000, 3_500_000]
        );
        let p = replay(c.clone(), &perfect_inputs(&c));
        // head 100 x 1.00, ticks 10 each, end 30 x (100 + 4)/100 = 31
        assert_eq!(p.score, 100 + 30 + 31);
        assert_eq!((p.ticks_hit, p.ticks_total, p.holds_done), (3, 3, 1));
        assert_eq!(p.max_combo, 5);
        assert_eq!(p.full_combo(), 5);
    }

    #[test]
    fn leaving_the_path_breaks_the_combo() {
        let c = hold_chart();
        let mut inputs = perfect_inputs(&c);
        // at the 3.0 s tick the point is at x 600; Easy follows within
        // 76 x 240% = 182, so x 783 is outside
        for i in inputs.iter_mut() {
            if let Input::Move { t: 3_000_000, x, .. } = i {
                *x = 783;
            }
        }
        let p = replay(c, &inputs);
        assert_eq!((p.ticks_hit, p.ticks_total), (2, 3));
        assert_eq!(p.max_combo, 2);
        assert_eq!(p.holds_done, 1, "back on the path by the end");
    }

    #[test]
    fn letting_go_early_misses_the_end() {
        let c = hold_chart();
        let mut p = Play::new(c.clone());
        p.input(Input::Move {
            t: 2_000_000,
            x: 400,
            y: 450,
        });
        p.input(Input::Down {
            t: 2_000_000,
            button: 0,
        });
        p.input(Input::Move {
            t: 2_500_000,
            x: 500,
            y: 450,
        });
        // 3.85 s is 150 ms before the end, outside Easy's 140 ms, with the
        // cursor on the point (x 770)
        p.input(Input::Move {
            t: 3_850_000,
            x: 770,
            y: 450,
        });
        p.input(Input::Up {
            t: 3_850_000,
            button: 0,
        });
        p.advance(6_000_000);
        assert_eq!(p.holds_done, 0);
        assert_eq!(p.combo, 0);
        assert_eq!(p.ticks_total, 3);
        // letting go 140 ms before the end still completes it
        let mut inputs = perfect_inputs(&c);
        let last = inputs.len() - 1;
        inputs[last] = Input::Up {
            t: 3_860_000,
            button: 0,
        };
        inputs[last - 1] = Input::Move {
            t: 3_860_000,
            x: 772,
            y: 450,
        };
        assert_eq!(replay(c, &inputs).holds_done, 1);
    }

    #[test]
    fn an_input_later_than_its_frame_counts_from_the_frame() {
        let c = one(EASY);
        let mut p = Play::new(c);
        p.advance(1_900_000);
        p.input(Input::Move {
            t: 1_990_000,
            x: 800,
            y: 450,
        });
        // the frame arrives 30 ms after the press was made
        p.advance(2_020_000);
        p.input(Input::Down {
            t: 1_990_000,
            button: 0,
        });
        // the press is taken as happening at the latest time, 2.02 s
        assert_eq!(p.judged[0], Some(Judgement::Perfect));
        assert_eq!(p.feed[0].offset_us, 20_000);
    }

    #[test]
    fn the_same_presses_judge_the_same_at_30_60_and_144_frames() {
        let targets = (1..=8)
            .map(|k| tap(k * 450, 400 + 100 * k as i32, 450, k as u32))
            .collect();
        let c = chart(MASTER, targets);
        // presses off by a different amount each, some missing
        let mut inputs = Vec::new();
        for (k, t) in c.targets.iter().enumerate() {
            let off = [0, 30_000, -60_000, 85_000, -10, 95_000, 20_000, -91_000][k];
            inputs.push(Input::Move {
                t: t.t + off,
                x: t.x,
                y: t.y,
            });
            inputs.push(Input::Down {
                t: t.t + off,
                button: 0,
            });
            inputs.push(Input::Up {
                t: t.t + off + 1,
                button: 0,
            });
        }
        let run = |fps: i64| {
            let mut p = Play::new(c.clone());
            let frame = 1_000_000 / fps;
            let mut next = 0;
            let mut now = 0;
            while now < c.length_us {
                // every input up to this frame, then the frame
                while next < inputs.len() && inputs[next].at() <= now {
                    p.input(inputs[next]);
                    next += 1;
                }
                p.advance(now);
                now += frame;
            }
            (p.judged.clone(), p.score, p.max_combo)
        };
        let at30 = run(30);
        assert_eq!(at30, run(60));
        assert_eq!(at30, run(144));
        let judged: Vec<Judgement> = at30.0.iter().map(|j| j.expect("judged")).collect();
        use Judgement::*;
        assert_eq!(
            judged,
            vec![Perfect, Great, Good, Good, Perfect, Miss, Perfect, Miss]
        );
    }
}
