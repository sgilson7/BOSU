//! A composed song's score (R8.1): every note of every part, with when it
//! starts and how long it lasts in ticks, so its chart comes from the notes
//! themselves rather than from what the beat finder hears. Written by
//! `tools/compose/<id>.py` into `data/scores/<id>.json`; the same file is
//! played into the song's audio (`tools/compose/make.sh`) and drawn as its
//! score (`tools/compose/score_tikz.py`).
//!
//! What the chart takes from it (R8.2): the beat and the first beat exactly;
//! the bar's length; on each grid line, how strongly a player hears a hit
//! (each note's velocity times its part's weight, the loudest) and how
//! stressed it is (the same, summed over the parts); and how long the lead's
//! note lasts, for holds.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::beats::Analysis;

/// Grid lines a beat a score may use, the fewest first: straight halves,
/// triplets, straight quarters, halves and triplets at once.
const DIVISIONS: [i64; 4] = [2, 3, 4, 6];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub name: String,
    pub bar: u32,
    pub bars: u32,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub name: String,
    /// The Vector37 factory sound that plays it, or -1 for the drums.
    pub preset: i32,
    pub gain: i32,
    pub pan: i32,
    /// How much a player hears this part as something to tap: 0 for a pad.
    pub chart_weight: u32,
    pub clef: String,
    /// The part whose long notes may become holds.
    pub lead: bool,
    /// Each note: start tick, length in ticks, MIDI pitch, velocity.
    pub notes: Vec<[i64; 4]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Score {
    pub id: String,
    pub title: String,
    pub bpm: i64,
    pub bar_beats: i64,
    pub ticks_per_beat: i64,
    /// Where tick 0 is in the audio.
    pub start_us: i64,
    pub bars: u32,
    pub sections: Vec<Section>,
    pub chords: Vec<String>,
    /// The drums' weights by pitch, as `chart_weight` is a part's.
    pub drum_weights: BTreeMap<String, u32>,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScoreError {
    NotJson(String),
    NoTempo,
    OffGrid(String, i64),
}

impl ScoreError {
    pub fn message(&self) -> String {
        match self {
            ScoreError::NotJson(d) => format!("The score could not be read: {d}"),
            ScoreError::NoTempo => "The score's tempo, bar or ticks a beat is not above zero.".into(),
            ScoreError::OffGrid(part, tick) => {
                format!("{part}'s note at tick {tick} is on no grid of halves, thirds or quarters of a beat.")
            }
        }
    }
}

impl Score {
    pub fn parse(json: &str) -> Result<Score, ScoreError> {
        let s: Score = serde_json::from_str(json).map_err(|e| ScoreError::NotJson(e.to_string()))?;
        if s.bpm <= 0 || s.bar_beats <= 0 || s.ticks_per_beat <= 0 {
            return Err(ScoreError::NoTempo);
        }
        Ok(s)
    }

    fn weight(&self, part: &Part, pitch: i64) -> u32 {
        if part.preset < 0 {
            self.drum_weights.get(&pitch.to_string()).copied().unwrap_or(0)
        } else {
            part.chart_weight
        }
    }

    /// The grid: the fewest lines a beat that every heard note starts on.
    fn division(&self) -> Result<i64, ScoreError> {
        let tpb = self.ticks_per_beat;
        let heard: Vec<(&Part, i64)> = self
            .parts
            .iter()
            .flat_map(|p| p.notes.iter().map(move |n| (p, n)))
            .filter(|(p, n)| self.weight(p, n[2]) > 0)
            .map(|(p, n)| (p, n[0]))
            .collect();
        DIVISIONS
            .iter()
            .copied()
            .find(|&d| tpb % d == 0 && heard.iter().all(|(_, t)| t % (tpb / d) == 0))
            .ok_or_else(|| {
                let (p, t) = heard
                    .iter()
                    .find(|(_, t)| DIVISIONS.iter().all(|&d| tpb % d != 0 || t % (tpb / d) != 0))
                    .map(|(p, t)| (p.name.clone(), *t))
                    .unwrap_or_default();
                ScoreError::OffGrid(p, t)
            })
    }

    /// The analysis a chart is made from, for audio `length_us` long.
    pub fn analysis(&self, length_us: i64) -> Result<Analysis, ScoreError> {
        let division = self.division()?;
        let beat_us = 60_000_000 / self.bpm;
        let tick_lines = self.ticks_per_beat / division;
        let count = ((length_us - self.start_us) * division / beat_us).max(0) as usize;
        let mut slots = vec![0u32; count];
        let mut accents = vec![0u32; count];
        let mut sustains = vec![0u32; count];
        for part in &self.parts {
            for n in &part.notes {
                let w = self.weight(part, n[2]);
                let line = (n[0] / tick_lines) as usize;
                if line >= count {
                    continue;
                }
                let v = (n[3] as u32).saturating_mul(w);
                slots[line] = slots[line].max(v);
                accents[line] = accents[line].saturating_add(v);
                if part.lead {
                    sustains[line] = sustains[line].max((n[1] / tick_lines) as u32);
                }
            }
        }
        Ok(Analysis {
            beat_us,
            first_beat_us: self.start_us,
            length_us,
            division,
            slots,
            accents,
            first_sound_us: 0,
            bar_beats: self.bar_beats,
            sustains,
        })
    }
}

/// How near (microseconds) an onset the audio's beat finder hears must be
/// to a score's note for the two to agree.
pub const AGREE_US: i64 = 25_000;
/// The share (percent) of a score's heard lines the audio must agree with,
/// or the audio was not made from the score (R8.1).
pub const AGREE_PCT: i64 = 60;

/// The share (percent) of the analysis's heard lines (its strongest
/// half, the notes a player most hears) with one of the audio's `peaks`
/// within `AGREE_US`.
pub fn agreement(a: &Analysis, peaks: &[(i64, i64)]) -> i64 {
    let mut strong: Vec<u32> = a.slots.iter().copied().filter(|&v| v > 0).collect();
    strong.sort_unstable();
    let median = strong.get(strong.len() / 2).copied().unwrap_or(0);
    let lines: Vec<i64> = (0..a.slots.len())
        .filter(|&q| a.slots[q] > 0 && a.slots[q] >= median)
        .map(|q| a.slot_time(q))
        .collect();
    let times: Vec<i64> = peaks.iter().map(|p| p.0).collect();
    let near = lines
        .iter()
        .filter(|&&t| {
            let k = times.partition_point(|&p| p < t - AGREE_US);
            times.get(k).is_some_and(|&p| p <= t + AGREE_US)
        })
        .count() as i64;
    near * 100 / (lines.len() as i64).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(notes_a: &str, notes_b: &str) -> String {
        format!(
            r#"{{"id":"t","title":"T","bpm":120,"bar_beats":3,"ticks_per_beat":12,"start_us":500000,"bars":2,
            "sections":[],"chords":[],"drum_weights":{{"36":6}},"parts":[
            {{"name":"Lead","preset":29,"gain":1000,"pan":0,"chart_weight":10,"clef":"treble","lead":true,"notes":[{notes_a}]}},
            {{"name":"Drums","preset":-1,"gain":1000,"pan":0,"chart_weight":0,"clef":"perc","lead":false,"notes":[{notes_b}]}}]}}"#
        )
    }

    #[test]
    fn a_score_gives_its_beat_bar_and_hits_exactly() {
        // 120 a minute: a beat of 500 ms; eighths (6 ticks) and triplets
        // (4 ticks) need six lines a beat, a line 2 ticks
        let s = Score::parse(&score(
            "[0,36,74,100],[6,6,76,80],[16,4,77,90]",
            "[0,6,36,90],[24,6,38,100]",
        ))
        .expect("parses");
        let a = s.analysis(3_000_000).expect("on a grid");
        assert_eq!(a.beat_us, 500_000);
        assert_eq!(a.first_beat_us, 500_000);
        assert_eq!(a.division, 6);
        assert_eq!(a.bar_beats, 3);
        // (3 s - 0.5 s) at 12 lines a second
        assert_eq!(a.slots.len(), 30);
        // line 0: the lead 100 x 10 and the kick 90 x 6; the loudest, and
        // the sum
        assert_eq!(a.slots[0], 1000);
        assert_eq!(a.accents[0], 1540);
        // line 3 (tick 6): the lead's eighth; line 8 (tick 16): its triplet
        assert_eq!(a.slots[3], 800);
        assert_eq!(a.slots[8], 900);
        // tick 24, pitch 38: a drum with no weight is not heard as a hit
        assert_eq!(a.slots[12], 0);
        // the lead's first note lasts 36 ticks: 18 lines
        assert_eq!(a.sustains[0], 18);
        assert_eq!(a.sustains[3], 3);
        assert_eq!(a.slot_time(3), 750_000);
    }

    #[test]
    fn a_score_in_straight_eighths_is_charted_on_half_beats() {
        let s = Score::parse(&score("[0,6,74,100],[6,6,76,80],[12,12,77,90]", "")).expect("parses");
        assert_eq!(s.analysis(3_000_000).expect("on a grid").division, 2);
    }

    #[test]
    fn a_score_note_off_every_grid_is_refused() {
        // tick 1 of 12 is a twelfth of a beat
        let s = Score::parse(&score("[0,6,74,100],[1,5,76,80]", "")).expect("parses");
        assert_eq!(s.analysis(3_000_000), Err(ScoreError::OffGrid("Lead".into(), 1)));
        assert_eq!(
            ScoreError::OffGrid("Lead".into(), 1).message(),
            "Lead's note at tick 1 is on no grid of halves, thirds or quarters of a beat."
        );
    }

    #[test]
    fn the_audio_agrees_with_a_score_when_its_onsets_are_on_the_notes() {
        // the lead's notes are 1000 at 0.5 s, 800 at 0.75 s and 900 at
        // 1.1667 s: the median is 900, so the strong lines are 0.5 s and
        // 1.1667 s, and the 0.75 s note does not count
        let s = Score::parse(&score("[0,36,74,100],[6,6,76,80],[16,4,77,90]", "")).expect("parses");
        let a = s.analysis(3_000_000).expect("on a grid");
        assert_eq!(agreement(&a, &[(510_000, 9), (1_300_000, 9)]), 50);
        assert_eq!(agreement(&a, &[(476_000, 9), (1_190_000, 9)]), 100);
        // 26 ms early, 26 ms late, and on the note that does not count
        assert_eq!(agreement(&a, &[(474_000, 9), (526_000, 9), (750_000, 9)]), 0);
        assert_eq!(agreement(&a, &[]), 0);
    }
}
