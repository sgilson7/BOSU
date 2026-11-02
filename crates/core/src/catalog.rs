//! The song list and the chart index the page loads (PLAN.md decisions 8
//! and 11). `data/songs.json` is written by people: each song's id, title,
//! who made it, its audio file and its chart seed. The chart tool writes
//! `data/analysis/<id>.json` from the audio and `data/charts/` from the
//! analysis; every rule it applies is here or in `beats` and `generate`.

use serde::{Deserialize, Serialize};

use crate::beats::Analysis;
use crate::chart::Chart;
use crate::generate;
use crate::tuning::DIFFICULTIES;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub by: String,
    /// The audio file, relative to the page.
    pub audio: String,
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Songs {
    pub songs: Vec<Song>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartEntry {
    pub difficulty: usize,
    pub file: String,
    /// Changes whenever the chart does, so the page never reads a stale copy.
    pub hash: String,
    pub targets: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexSong {
    pub id: String,
    pub title: String,
    pub by: String,
    pub audio: String,
    pub audio_hash: String,
    pub bpm_milli: i64,
    pub length_us: i64,
    pub charts: Vec<ChartEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
    pub songs: Vec<IndexSong>,
}

pub fn parse_songs(json: &str) -> Result<Songs, String> {
    let s: Songs = serde_json::from_str(json).map_err(|e| format!("The song list could not be read: {e}"))?;
    if s.songs.is_empty() {
        return Err("The song list is empty.".into());
    }
    for (k, song) in s.songs.iter().enumerate() {
        let ok_id = !song.id.is_empty() && song.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !ok_id {
            return Err(format!("Song {k} has an id that is not lower-case letters, digits and hyphens."));
        }
        if s.songs[..k].iter().any(|o| o.id == song.id) {
            return Err(format!("Song id \"{}\" appears twice.", song.id));
        }
    }
    Ok(s)
}

pub fn parse_index(json: &str) -> Result<Index, String> {
    serde_json::from_str(json).map_err(|e| format!("The chart index could not be read: {e}"))
}

/// FNV-1a, 64 bits, as 16 hex digits: a name for a file's contents, for
/// cache busting only.
pub fn fnv(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub fn chart_file(song: &str, difficulty: usize) -> String {
    format!("charts/{song}-{}.json", DIFFICULTIES[difficulty].key)
}

/// The five charts of one song, as (path under data/, text).
pub fn charts_for(song: &Song, a: &Analysis) -> Vec<(String, String)> {
    (0..DIFFICULTIES.len())
        .map(|d| {
            let c = generate::chart(&song.id, a, d, song.seed);
            (chart_file(&song.id, d), pretty(&c))
        })
        .collect()
}

/// A chart as the committed file holds it: one target a line, so a person
/// can edit one and a diff shows which moved.
pub fn pretty(c: &Chart) -> String {
    let mut out = format!(
        "{{\"song\": {}, \"difficulty\": {}, \"beat_us\": {}, \"first_beat_us\": {}, \"length_us\": {},\n \"targets\": [\n",
        serde_json::to_string(&c.song).unwrap_or_default(),
        c.difficulty,
        c.beat_us,
        c.first_beat_us,
        c.length_us
    );
    let lines: Vec<String> = c
        .targets
        .iter()
        .map(|t| format!("  {}", serde_json::to_string(t).unwrap_or_default()))
        .collect();
    out.push_str(&lines.join(",\n"));
    out.push_str("\n]}\n");
    out
}

/// The index of every song and chart.
pub fn index(songs: &Songs, analyses: &[Analysis], charts: &[Vec<(String, String)>], audio_hashes: &[String]) -> String {
    let index = Index {
        songs: songs
            .songs
            .iter()
            .zip(analyses)
            .zip(charts)
            .zip(audio_hashes)
            .map(|(((s, a), cs), ah)| IndexSong {
                id: s.id.clone(),
                title: s.title.clone(),
                by: s.by.clone(),
                audio: s.audio.clone(),
                audio_hash: ah.clone(),
                bpm_milli: a.bpm_milli(),
                length_us: a.length_us,
                charts: cs
                    .iter()
                    .enumerate()
                    .map(|(d, (file, text))| ChartEntry {
                        difficulty: d,
                        file: file.clone(),
                        hash: fnv(text.as_bytes()),
                        targets: Chart::parse(text).map(|c| c.targets.len()).unwrap_or(0),
                    })
                    .collect(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&index).unwrap_or_default() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_matches_its_published_vectors() {
        assert_eq!(fnv(b""), "cbf29ce484222325");
        assert_eq!(fnv(b"a"), "af63dc4c8601ec8c");
        assert_eq!(fnv(b"foobar"), "85944171f73967e8");
    }

    #[test]
    fn a_song_list_with_a_repeated_or_bad_id_is_refused() {
        let one = r#"{"id": "a-1", "title": "A", "by": "B", "audio": "audio/a.mp3", "seed": 1}"#;
        assert!(parse_songs(&format!(r#"{{"songs": [{one}]}}"#)).is_ok());
        assert_eq!(
            parse_songs(&format!(r#"{{"songs": [{one}, {one}]}}"#)),
            Err("Song id \"a-1\" appears twice.".into())
        );
        let bad = one.replace("a-1", "A 1");
        assert!(parse_songs(&format!(r#"{{"songs": [{bad}]}}"#)).is_err());
    }

    #[test]
    fn a_pretty_chart_reads_back_the_same() {
        let a = crate::generate::tests::busy_song(120, 20, 1);
        let c = generate::chart("x", &a, 2, 3);
        assert_eq!(Chart::parse(&pretty(&c)), Ok(c));
    }
}
