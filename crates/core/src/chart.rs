//! A chart: the targets of one song at one difficulty (PLAN.md decision 3).
//! Stored as JSON in `data/charts/<song>-<difficulty>.json`, written by
//! `tools/chart` and editable by hand; the core refuses one that breaks a
//! rule below before anything is drawn.

use serde::{Deserialize, Serialize};

use crate::tuning::{DIFFICULTIES, X_MAX, X_MIN, Y_MAX, Y_MIN};

/// A hold: press on the target, follow the path, let go at `end`. The path's
/// points are evenly spaced in time from the target's time to `end`, and
/// the first is the target's own position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hold {
    pub end: i64,
    pub path: Vec<[i32; 2]>,
}

/// One target: its time in microseconds of song time, its centre on the
/// playfield, and the number drawn on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub t: i64,
    pub x: i32,
    pub y: i32,
    pub combo: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<Hold>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chart {
    pub song: String,
    /// Index into `tuning::DIFFICULTIES`.
    pub difficulty: usize,
    /// The beat's length and the first beat's time, in microseconds.
    pub beat_us: i64,
    pub first_beat_us: i64,
    /// The song's length; play ends here.
    pub length_us: i64,
    /// The circles' radius in units; 0 (an older chart) is the
    /// difficulty's own (R7.3).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub radius: i32,
    pub targets: Vec<Target>,
}

fn is_zero(r: &i32) -> bool {
    *r == 0
}

/// The smallest and largest radius a chart may give its circles.
pub const RADIUS_MIN: i32 = 20;
pub const RADIUS_MAX: i32 = 99;

/// Why a chart cannot be played. `message` is shown to the reader as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChartError {
    NotJson(String),
    NoDifficulty(usize),
    NoBeat,
    BadRadius(i32),
    Empty,
    OutOfOrder(usize),
    Outside(usize),
    BadCombo(usize),
    BadHold(usize),
    InsideHold(usize),
    PastEnd(usize),
}

impl ChartError {
    pub fn message(&self) -> String {
        match self {
            ChartError::NotJson(d) => format!("The chart could not be read: {d}"),
            ChartError::NoDifficulty(d) => format!("The chart names difficulty {d}, which does not exist."),
            ChartError::NoBeat => "The chart's beat length is not above zero.".into(),
            ChartError::BadRadius(r) => {
                format!("The chart's circle radius {r} is outside {RADIUS_MIN} to {RADIUS_MAX}.")
            }
            ChartError::Empty => "The chart has no targets.".into(),
            ChartError::OutOfOrder(k) => format!("Target {k} is not later than the one before it."),
            ChartError::Outside(k) => format!("Target {k} lies outside the playfield."),
            ChartError::BadCombo(k) => format!("Target {k} has no combo number."),
            ChartError::BadHold(k) => {
                format!("Target {k}'s hold has a path that does not start on it or ends before it.")
            }
            ChartError::InsideHold(k) => format!("Target {k} starts before the hold before it ends."),
            ChartError::PastEnd(k) => format!("Target {k} ends after the song."),
        }
    }
}

fn inside(x: i32, y: i32) -> bool {
    (X_MIN..=X_MAX).contains(&x) && (Y_MIN..=Y_MAX).contains(&y)
}

impl Chart {
    /// Reads and checks a chart.
    pub fn parse(json: &str) -> Result<Chart, ChartError> {
        let chart: Chart = serde_json::from_str(json).map_err(|e| ChartError::NotJson(e.to_string()))?;
        chart.check()?;
        Ok(chart)
    }

    pub fn check(&self) -> Result<(), ChartError> {
        if self.difficulty >= DIFFICULTIES.len() {
            return Err(ChartError::NoDifficulty(self.difficulty));
        }
        if self.beat_us <= 0 {
            return Err(ChartError::NoBeat);
        }
        if self.radius != 0 && !(RADIUS_MIN..=RADIUS_MAX).contains(&self.radius) {
            return Err(ChartError::BadRadius(self.radius));
        }
        if self.targets.is_empty() {
            return Err(ChartError::Empty);
        }
        let mut busy_until = i64::MIN;
        for (k, t) in self.targets.iter().enumerate() {
            if t.t <= busy_until || t.t < 0 {
                return Err(
                    if k > 0 && self.targets[k - 1].hold.is_some() && t.t > self.targets[k - 1].t {
                        ChartError::InsideHold(k)
                    } else {
                        ChartError::OutOfOrder(k)
                    },
                );
            }
            if !inside(t.x, t.y) {
                return Err(ChartError::Outside(k));
            }
            if t.combo == 0 {
                return Err(ChartError::BadCombo(k));
            }
            busy_until = t.t;
            if let Some(h) = &t.hold {
                let starts_here = h.path.first() == Some(&[t.x, t.y]);
                if h.end <= t.t || h.path.len() < 2 || !starts_here {
                    return Err(ChartError::BadHold(k));
                }
                if h.path.iter().any(|p| !inside(p[0], p[1])) {
                    return Err(ChartError::Outside(k));
                }
                busy_until = h.end;
            }
            if busy_until > self.length_us {
                return Err(ChartError::PastEnd(k));
            }
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

impl Target {
    /// Where the hold's moving point is at time `t`, clamped to the hold.
    /// A tap's point is its centre.
    pub fn point_at(&self, t: i64) -> (i32, i32) {
        let Some(h) = &self.hold else {
            return (self.x, self.y);
        };
        let span = h.end - self.t;
        let last = (h.path.len() - 1) as i64;
        let along = (t.clamp(self.t, h.end) - self.t) * last;
        let i = (along / span).min(last - 1) as usize;
        let rest = along - i as i64 * span;
        let (a, b) = (h.path[i], h.path[i + 1]);
        let lerp = |p: i32, q: i32| p + ((q - p) as i64 * rest / span) as i32;
        (lerp(a[0], b[0]), lerp(a[1], b[1]))
    }

    /// The time of a hold's last moment, or the target's own time.
    pub fn end(&self) -> i64 {
        self.hold.as_ref().map(|h| h.end).unwrap_or(self.t)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn tap(t_ms: i64, x: i32, y: i32, combo: u32) -> Target {
        Target {
            t: t_ms * 1000,
            x,
            y,
            combo,
            hold: None,
        }
    }

    pub fn chart(difficulty: usize, targets: Vec<Target>) -> Chart {
        let length_us = targets.last().map(|t| t.end()).unwrap_or(0) + 2_000_000;
        Chart {
            song: "test".into(),
            difficulty,
            beat_us: 500_000,
            first_beat_us: 0,
            length_us,
            radius: 0,
            targets,
        }
    }

    #[test]
    fn every_chart_round_trips() {
        let mut c = chart(0, vec![tap(1000, 400, 400, 1), tap(2000, 600, 400, 2)]);
        c.targets.push(Target {
            t: 3_000_000,
            x: 800,
            y: 500,
            combo: 3,
            hold: Some(Hold {
                end: 4_000_000,
                path: vec![[800, 500], [1000, 500]],
            }),
        });
        c.length_us = 6_000_000;
        let back = Chart::parse(&c.to_json()).expect("parses");
        assert_eq!(back, c);
    }

    #[test]
    fn a_chart_with_a_target_outside_the_playfield_is_refused() {
        let c = chart(0, vec![tap(1000, 400, 400, 1), tap(2000, 299, 400, 2)]);
        assert_eq!(c.check(), Err(ChartError::Outside(1)));
        let c = chart(0, vec![tap(1000, 400, 841, 1)]);
        assert_eq!(c.check(), Err(ChartError::Outside(0)));
        assert_eq!(
            ChartError::Outside(1).message(),
            "Target 1 lies outside the playfield."
        );
    }

    #[test]
    fn targets_out_of_order_or_inside_a_hold_are_refused() {
        let c = chart(0, vec![tap(2000, 400, 400, 1), tap(2000, 600, 400, 2)]);
        assert_eq!(c.check(), Err(ChartError::OutOfOrder(1)));
        let mut c = chart(0, vec![tap(1000, 400, 400, 1), tap(1500, 600, 400, 2)]);
        c.targets[0].hold = Some(Hold {
            end: 1_600_000,
            path: vec![[400, 400], [500, 400]],
        });
        assert_eq!(c.check(), Err(ChartError::InsideHold(1)));
    }

    #[test]
    fn a_hold_whose_path_does_not_start_on_its_target_is_refused() {
        let mut c = chart(0, vec![tap(1000, 400, 400, 1)]);
        c.targets[0].hold = Some(Hold {
            end: 2_000_000,
            path: vec![[410, 400], [500, 400]],
        });
        assert_eq!(c.check(), Err(ChartError::BadHold(0)));
        c.targets[0].hold = Some(Hold {
            end: 1_000_000,
            path: vec![[400, 400], [500, 400]],
        });
        assert_eq!(c.check(), Err(ChartError::BadHold(0)));
    }

    #[test]
    fn a_hold_point_moves_evenly_along_its_path() {
        let t = Target {
            t: 1_000_000,
            x: 400,
            y: 400,
            combo: 1,
            hold: Some(Hold {
                end: 3_000_000,
                path: vec![[400, 400], [600, 400], [600, 600]],
            }),
        };
        assert_eq!(t.point_at(0), (400, 400));
        assert_eq!(t.point_at(1_500_000), (500, 400));
        assert_eq!(t.point_at(2_000_000), (600, 400));
        assert_eq!(t.point_at(2_500_000), (600, 500));
        assert_eq!(t.point_at(3_000_000), (600, 600));
        assert_eq!(t.point_at(9_000_000), (600, 600));
    }

    #[test]
    fn a_chart_with_a_circle_size_out_of_range_is_refused() {
        let mut c = chart(0, vec![tap(1000, 800, 450, 1)]);
        c.radius = 19;
        assert_eq!(c.check(), Err(ChartError::BadRadius(19)));
        c.radius = 100;
        assert_eq!(c.check(), Err(ChartError::BadRadius(100)));
        c.radius = 20;
        assert_eq!(c.check(), Ok(()));
        c.radius = 0;
        assert_eq!(c.check(), Ok(()));
    }
}
