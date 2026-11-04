//! The chart tool (PLAN.md decision 11). Input and output only: it decodes
//! each song with ffmpeg and hands the samples to tool_core, which finds the
//! beat and lays out the charts. Run from the repository root:
//!
//!     cargo run --release -p chart_tool -- analyse   decode every song, write data/analysis/
//!     cargo run --release -p chart_tool -- charts    write data/charts/ from data/analysis/
//!     cargo run --release -p chart_tool -- check     exit 1 if data/charts/ differs from what `charts` would write
//!
//! Charts are generated here, at build time, and committed; the page never
//! generates one (BO-6).
use std::fs;
use std::process::{exit, Command};

use tool_core::beats::{self, Analysis};
use tool_core::catalog;
use tool_core::score;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("cannot read {path}: {e}")))
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    exit(1)
}

fn decode(path: &str) -> Vec<i16> {
    decode_with(path, &["-ac", "1"], beats::RATE)
}

/// The opening of the left channel alone, at the page's usual rate: what the
/// page hands the core from its own decode (`game::audio_head`, R6.6).
const HEAD_RATE: i64 = 44_100;
fn decode_head(path: &str) -> Vec<i16> {
    let secs = beats::HEAD_SECONDS.to_string();
    decode_with(path, &["-af", "pan=mono|c0=c0", "-t", &secs], HEAD_RATE)
}

fn decode_with(path: &str, args: &[&str], rate: i64) -> Vec<i16> {
    let out = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-i", path])
        .args(args)
        .arg("-ar")
        .arg(rate.to_string())
        .args(["-f", "s16le", "-"])
        .output()
        .unwrap_or_else(|e| fail(&format!("cannot run ffmpeg (brew install ffmpeg): {e}")));
    if !out.status.success() {
        fail(&format!(
            "ffmpeg could not decode {path}: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    out.stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b))
        .collect()
}

fn analysis_path(id: &str) -> String {
    format!("data/analysis/{id}.json")
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let songs = catalog::parse_songs(&read("data/songs.json")).unwrap_or_else(|e| fail(&e));
    match mode.as_str() {
        "analyse" => {
            fs::create_dir_all("data/analysis").unwrap_or_else(|e| fail(&e.to_string()));
            for s in &songs.songs {
                let samples = decode(&format!("web/{}", s.audio));
                let mut a = match &s.score {
                    // a composed song: the notes, checked against what the
                    // audio's onsets say (R8.1)
                    Some(path) => {
                        let sc = score::Score::parse(&read(&format!("data/{path}")))
                            .unwrap_or_else(|e| fail(&format!("{}: {}", s.id, e.message())));
                        let a = sc
                            .analysis(samples.len() as i64 * beats::US / beats::RATE)
                            .unwrap_or_else(|e| fail(&format!("{}: {}", s.id, e.message())));
                        let agree = score::agreement(&a, &beats::peaks(&beats::envelope(&samples)));
                        println!(
                            "{}: {agree}% of the score's strong notes have an onset in the audio",
                            s.id
                        );
                        if agree < score::AGREE_PCT {
                            fail(&format!(
                                "{}: the audio does not follow its score ({agree}% against {}%)",
                                s.id,
                                score::AGREE_PCT
                            ));
                        }
                        a
                    }
                    None => beats::analyse_with(&samples, s.bpm_milli)
                        .unwrap_or_else(|| fail(&format!("{}: no beat found", s.id))),
                };
                a.first_sound_us =
                    beats::first_sound_us(&decode_head(&format!("web/{}", s.audio)), HEAD_RATE).unwrap_or(0);
                let text = serde_json::to_string(&a).unwrap_or_default() + "\n";
                fs::write(analysis_path(&s.id), text).unwrap_or_else(|e| fail(&e.to_string()));
                println!(
                    "{}: {}.{:03} bpm, first beat {} us, {} s",
                    s.id,
                    a.bpm_milli() / 1000,
                    a.bpm_milli() % 1000,
                    a.first_beat_us,
                    a.length_us / 1_000_000
                );
            }
        }
        "charts" | "check" => {
            let analyses: Vec<Analysis> = songs
                .songs
                .iter()
                .map(|s| {
                    serde_json::from_str(&read(&analysis_path(&s.id)))
                        .unwrap_or_else(|e| fail(&format!("{}: {e}", s.id)))
                })
                .collect();
            let charts: Vec<Vec<(String, String)>> = songs
                .songs
                .iter()
                .zip(&analyses)
                .map(|(s, a)| catalog::charts_for(s, a))
                .collect();
            let audio_hashes: Vec<String> = songs
                .songs
                .iter()
                .map(|s| {
                    catalog::fnv(
                        &fs::read(format!("web/{}", s.audio))
                            .unwrap_or_else(|e| fail(&format!("{}: {e}", s.audio))),
                    )
                })
                .collect();
            let index = catalog::index(&songs, &analyses, &charts, &audio_hashes);
            let mut files: Vec<(String, String)> = charts.into_iter().flatten().collect();
            files.push(("charts/index.json".into(), index));
            let mut stale = 0;
            for (path, text) in &files {
                let full = format!("data/{path}");
                if mode == "check" {
                    if fs::read_to_string(&full).ok().as_deref() != Some(text.as_str()) {
                        eprintln!("{full} differs from what the generator writes");
                        stale += 1;
                    }
                } else {
                    fs::create_dir_all("data/charts").unwrap_or_else(|e| fail(&e.to_string()));
                    fs::write(&full, text).unwrap_or_else(|e| fail(&e.to_string()));
                }
            }
            if stale > 0 {
                fail(&format!(
                    "{stale} chart files are stale: run cargo run --release -p chart_tool -- charts"
                ));
            }
            println!(
                "{} files {}",
                files.len(),
                if mode == "check" {
                    "match the generator"
                } else {
                    "written"
                }
            );
        }
        "peaks" => {
            // each song's onset peaks, time and strength a line, for checking
            // the beat finder against another onset detector (R7.1)
            fs::create_dir_all("target/peaks").unwrap_or_else(|e| fail(&e.to_string()));
            for s in &songs.songs {
                let samples = decode(&format!("web/{}", s.audio));
                let lines: Vec<String> = beats::peaks(&beats::envelope(&samples))
                    .iter()
                    .map(|(t, v)| format!("{t} {v}"))
                    .collect();
                fs::write(format!("target/peaks/{}.txt", s.id), lines.join("\n") + "\n")
                    .unwrap_or_else(|e| fail(&e.to_string()));
            }
        }
        "stability" => {
            // each song's grid from its first and second halves alone, set
            // against the whole song's: a song whose halves disagree has no
            // one steady beat, and its charts need a person's ear (R6.4)
            for s in &songs.songs {
                let samples = decode(&format!("web/{}", s.audio));
                let whole = beats::analyse(&samples).unwrap_or_else(|| fail("no beat"));
                let half = samples.len() / 2;
                let offset_us = half as i64 * 1_000_000 / beats::RATE;
                let (a, b) = (beats::analyse(&samples[..half]), beats::analyse(&samples[half..]));
                let off = |first: i64| {
                    let q = whole.beat_us / 4;
                    let d = (first - whole.first_beat_us).rem_euclid(q);
                    d.min(q - d) / 1000
                };
                match (a, b) {
                    (Some(a), Some(b)) => println!(
                        "{:24} whole {:>8.3} | first half {:>8.3}, {:>3} ms off the grid | second half {:>8.3}, {:>3} ms off",
                        s.id,
                        whole.bpm_milli() as f64 / 1000.0,
                        a.bpm_milli() as f64 / 1000.0,
                        off(a.first_beat_us),
                        b.bpm_milli() as f64 / 1000.0,
                        off(b.first_beat_us + offset_us)
                    ),
                    _ => println!("{:24} a half has no beat", s.id),
                }
            }
        }
        _ => fail("usage: chart analyse | charts | check | peaks | stability"),
    }
}
