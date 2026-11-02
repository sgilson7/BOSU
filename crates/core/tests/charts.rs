//! The committed charts (BO-4, BO-5, BO-6). Reads data/songs.json,
//! data/analysis/ and data/charts/ and checks every chart against the rules.

use std::fs;
use std::path::{Path, PathBuf};

use tool_core::beats::Analysis;
use tool_core::catalog;
use tool_core::chart::Chart;
use tool_core::generate;
use tool_core::play::{perfect_inputs, replay, Judgement};
use tool_core::tuning::DIFFICULTIES;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn songs() -> Vec<(catalog::Song, Analysis, Vec<Chart>)> {
    let songs = catalog::parse_songs(&read("data/songs.json")).expect("song list");
    songs
        .songs
        .into_iter()
        .map(|s| {
            let a: Analysis =
                serde_json::from_str(&read(&format!("data/analysis/{}.json", s.id))).expect("analysis");
            let charts = (0..DIFFICULTIES.len())
                .map(|d| {
                    Chart::parse(&read(&format!("data/{}", catalog::chart_file(&s.id, d)))).expect("chart")
                })
                .collect();
            (s, a, charts)
        })
        .collect()
}

#[test]
fn there_are_five_songs_with_five_charts_each() {
    let all = songs();
    assert_eq!(all.len(), 5);
    let index = catalog::parse_index(&read("data/charts/index.json")).expect("index");
    assert_eq!(index.songs.len(), 5);
    for s in &index.songs {
        assert_eq!(s.charts.len(), 5, "{}", s.id);
    }
}

#[test]
fn the_committed_charts_match_the_generator() {
    for (s, a, charts) in songs() {
        for (d, (path, text)) in catalog::charts_for(&s, &a).into_iter().enumerate() {
            assert_eq!(
                read(&format!("data/{path}")),
                text,
                "{path}: run cargo run --release -p chart_tool -- charts"
            );
            assert_eq!(charts[d], Chart::parse(&text).expect("parses"));
        }
    }
}

#[test]
fn every_chart_is_cleared_by_a_perfect_bot() {
    for (s, _, charts) in songs() {
        for c in charts {
            let p = replay(c.clone(), &perfect_inputs(&c));
            let key = DIFFICULTIES[c.difficulty].key;
            assert_eq!(
                p.counts[Judgement::Perfect as usize] as usize,
                c.targets.len(),
                "{} {key}",
                s.id
            );
            assert_eq!(p.max_combo, p.full_combo(), "{} {key}: combo broken", s.id);
        }
    }
}

#[test]
fn each_committed_difficulty_is_harder_than_the_one_below() {
    for (s, _, charts) in songs() {
        for pair in charts.windows(2) {
            let (lo, hi) = (generate::load(&pair[0]), generate::load(&pair[1]));
            let key = DIFFICULTIES[pair[1].difficulty].key;
            assert!(
                hi.0 > lo.0,
                "{} {key}: {} targets a minute against {}",
                s.id,
                hi.0,
                lo.0
            );
            assert!(
                hi.1 > lo.1,
                "{} {key}: moves {} a second against {}",
                s.id,
                hi.1,
                lo.1
            );
        }
    }
}

#[test]
fn no_two_visible_targets_overlap_in_a_committed_chart() {
    for (s, _, charts) in songs() {
        for c in charts {
            let d = DIFFICULTIES[c.difficulty];
            let sep = 2 * d.radius as i64;
            for (k, t) in c.targets.iter().enumerate() {
                for p in &c.targets[..k] {
                    if p.end() + d.good_us >= t.t - d.approach_us {
                        let (dx, dy) = ((p.x - t.x) as i64, (p.y - t.y) as i64);
                        assert!(
                            dx * dx + dy * dy >= sep * sep,
                            "{} {}: {} and {} us",
                            s.id,
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
