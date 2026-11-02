//! Every number that tunes play, in one place (PLAN.md, "The tuning table").
//! The journal's mockups read the same numbers from
//! `../bosu-journal/mockups/numbers.json`; `the_table_matches_the_mockups`
//! in `tests/tuning.rs` keeps the two equal.
//!
//! Times are whole microseconds, positions are playfield units on a
//! 1600 by 900 field.

/// The playfield, in units. The canvas draws it at this size and the page
/// scales it to the window.
pub const FIELD_W: i32 = 1600;
pub const FIELD_H: i32 = 900;

/// Where a target's centre may sit. The margin keeps room for the score at
/// the top and the scoreboard at the side.
pub const X_MIN: i32 = 300;
pub const X_MAX: i32 = 1500;
pub const Y_MIN: i32 = 120;
pub const Y_MAX: i32 = 800;

/// While a hold is held, the cursor must stay within this share of the
/// radius (in percent) of the point moving along the path.
pub const HOLD_FOLLOW_PCT: i32 = 240;
/// Points for each beat tick of a hold followed through.
pub const TICK_POINTS: i64 = 10;
/// Points for finishing a hold, before the combo multiplier.
pub const END_POINTS: i64 = 30;
/// The combo multiplier: points times (100 + combo, at most this) / 100.
pub const COMBO_CAP: i64 = 100;

/// How many players a lobby holds (BO-Q5).
pub const LOBBY_SEATS: usize = 8;

/// One difficulty's numbers. `grid` is the number of grid steps per beat
/// that targets snap to; `keep_pct` the share of onsets that become
/// targets, strongest first; `spacing` units of distance per 100 ms
/// between targets; `max_jump` the longest move between two targets;
/// `hold_pct` the share of long gaps that become holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Difficulty {
    pub key: &'static str,
    pub approach_us: i64,
    pub radius: i32,
    pub perfect_us: i64,
    pub great_us: i64,
    pub good_us: i64,
    pub min_gap_us: i64,
    pub grid: i64,
    pub keep_pct: i64,
    pub spacing: i64,
    pub max_jump: i64,
    pub hold_pct: i64,
}

const MS: i64 = 1000;

/// Easy, Normal, Hard, Expert, Master (BO-Q8), in that order. The names a
/// player reads are in `data/strings.json` under `difficulty.<key>`.
pub const DIFFICULTIES: [Difficulty; 5] = [
    Difficulty {
        key: "easy",
        approach_us: 1500 * MS,
        radius: 76,
        perfect_us: 50 * MS,
        great_us: 95 * MS,
        good_us: 140 * MS,
        min_gap_us: 600 * MS,
        grid: 1,
        keep_pct: 35,
        spacing: 40,
        max_jump: 300,
        hold_pct: 25,
    },
    Difficulty {
        key: "normal",
        approach_us: 1200 * MS,
        radius: 66,
        perfect_us: 45 * MS,
        great_us: 85 * MS,
        good_us: 125 * MS,
        min_gap_us: 400 * MS,
        grid: 2,
        keep_pct: 50,
        spacing: 55,
        max_jump: 420,
        hold_pct: 25,
    },
    Difficulty {
        key: "hard",
        approach_us: 950 * MS,
        radius: 58,
        perfect_us: 38 * MS,
        great_us: 75 * MS,
        good_us: 110 * MS,
        min_gap_us: 260 * MS,
        grid: 2,
        keep_pct: 65,
        spacing: 70,
        max_jump: 560,
        hold_pct: 20,
    },
    Difficulty {
        key: "expert",
        approach_us: 750 * MS,
        radius: 50,
        perfect_us: 32 * MS,
        great_us: 65 * MS,
        good_us: 100 * MS,
        min_gap_us: 170 * MS,
        grid: 4,
        keep_pct: 80,
        spacing: 85,
        max_jump: 700,
        hold_pct: 20,
    },
    Difficulty {
        key: "master",
        approach_us: 600 * MS,
        radius: 44,
        perfect_us: 26 * MS,
        great_us: 55 * MS,
        good_us: 90 * MS,
        min_gap_us: 120 * MS,
        grid: 4,
        keep_pct: 92,
        spacing: 100,
        max_jump: 820,
        hold_pct: 15,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_difficulty_is_stricter_than_the_one_below() {
        for pair in DIFFICULTIES.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(b.approach_us < a.approach_us, "{} approach", b.key);
            assert!(b.radius < a.radius, "{} radius", b.key);
            assert!(b.perfect_us < a.perfect_us, "{} perfect", b.key);
            assert!(b.great_us < a.great_us, "{} great", b.key);
            assert!(b.good_us < a.good_us, "{} good", b.key);
            assert!(b.min_gap_us < a.min_gap_us, "{} gap", b.key);
            assert!(b.keep_pct > a.keep_pct, "{} keep", b.key);
        }
    }

    #[test]
    fn windows_nest_and_targets_fit_the_field() {
        for d in DIFFICULTIES {
            assert!(0 < d.perfect_us && d.perfect_us < d.great_us && d.great_us < d.good_us, "{}", d.key);
            // two targets a minimum gap apart are never both inside one Good window
            assert!(2 * d.good_us < d.min_gap_us + d.good_us, "{}", d.key);
            assert!(X_MIN - d.radius > 0 && X_MAX + d.radius < FIELD_W, "{}", d.key);
            assert!(Y_MIN - d.radius > 0 && Y_MAX + d.radius < FIELD_H, "{}", d.key);
        }
    }
}
