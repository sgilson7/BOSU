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

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("cannot read {path}: {e}")))
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    exit(1)
}

fn decode(path: &str) -> Vec<i16> {
    let out = Command::new("ffmpeg")
        .args(["-loglevel", "error", "-i", path, "-ac", "1", "-ar"])
        .arg(beats::RATE.to_string())
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
                let a = beats::analyse(&samples).unwrap_or_else(|| fail(&format!("{}: no beat found", s.id)));
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
        _ => fail("usage: chart analyse | charts | check"),
    }
}
