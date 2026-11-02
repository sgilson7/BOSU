//! The game: which screen is showing, the settings, the bests, the play in
//! progress, and what the page should do next (PLAN.md decisions 2, 12, 13).
//!
//! The page talks to it through four calls: `act(name, value)` for every
//! button, setting and loaded file; `input(...)` for every pointer and key
//! event with its own timestamp; `frame(...)` once a display frame, which
//! returns the canvas's draw list; and `commands()`, the audio and loading
//! the page should carry out. The page decides none of these.
//!
//! Time. The page passes the audio clock as the Web Audio API gives it:
//! `getOutputTimestamp()`'s `contextTime` (the context time of the sound
//! being heard now) with its `performanceTime`, and the event's own
//! `timeStamp`. All three arrive in microseconds. The context time heard at
//! the event is `context + (stamp - performance)`; song time is that minus
//! the time the song was started at, minus the audio offset (BO-2).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::catalog::Index;
use crate::chart::Chart;
use crate::draw::{Align, Color, DrawList, Shape, UNIT};
use crate::page::{self, fill, grouped, minutes, percent, Strings};
use crate::play::{Input, Judgement, Mark, Play, JUDGEMENTS};
use crate::tuning::{DIFFICULTIES, FIELD_H, FIELD_W};

/// The shortest wait between pressing Start and the song's first sound.
pub const LEAD_US: i64 = 1_000_000;
/// The first target never appears sooner than this after the song starts.
pub const FIRST_TARGET_MARGIN_US: i64 = 500_000;
/// A play ends this long after its last target, if the song runs on.
pub const OUTRO_US: i64 = 2_000_000;

pub const AUDIO_OFFSET_MS: (i64, i64, i64) = (-300, 300, 5);
pub const VISUAL_OFFSET_MS: (i64, i64, i64) = (-100, 100, 5);
pub const VOLUME_PCT: (i64, i64, i64) = (0, 100, 10);
pub const NAME_MAX: usize = 16;

/// Calibration: a click every half second, the median of this many presses.
pub const CALIBRATION_BEAT_US: i64 = 500_000;
pub const CALIBRATION_PRESSES: usize = 16;
pub const CALIBRATION_CLICKS: i64 = 64;

const MS: i64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Screen {
    Title,
    Select,
    Loading,
    Play,
    Results,
    Settings,
    Calibrate,
}

impl Screen {
    fn from_name(name: &str) -> Option<Screen> {
        Some(match name {
            "title" => Screen::Title,
            "select" => Screen::Select,
            "settings" => Screen::Settings,
            "calibrate" => Screen::Calibrate,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub audio_offset_ms: i64,
    pub visual_offset_ms: i64,
    pub volume_pct: i64,
    pub name: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { audio_offset_ms: 0, visual_offset_ms: 0, volume_pct: 80, name: String::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Best {
    pub score: i64,
    pub accuracy: i64,
}

/// What the page should do, in order. Sent as JSON from `commands()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    /// Fetch this file under `data/` and pass its text to `chart.loaded`.
    LoadChart { url: String },
    /// Fetch and decode this audio file, then call `audio.ready` with the url.
    LoadAudio { url: String },
    /// Start the loaded song at this audio context time.
    StartAudio { url: String, at_us: i64 },
    StopAudio,
    Suspend,
    Resume,
    /// Schedule `count` clicks, `every_us` apart, from `at_us`.
    Clicks { at_us: i64, every_us: i64, count: i64 },
    Volume { pct: i64 },
    /// The screen changed: draw `ui()` again.
    Ui,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Session {
    play: Play,
    /// The audio context time the song starts at, once scheduled.
    start_us: Option<i64>,
    song_now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Calibration {
    start_us: Option<i64>,
    offsets: Vec<i64>,
    now: i64,
    result: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Outcome {
    song: usize,
    difficulty: usize,
    score: i64,
    accuracy: i64,
    max_combo: u32,
    full_combo: u32,
    counts: [u32; 4],
    ticks: (u32, u32),
    mean_offset_us: Option<i64>,
    new_best: bool,
}

/// The part of the game a save keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Kept {
    pub settings: Settings,
    pub bests: BTreeMap<String, Best>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    pub name: String,
    pub value: String,
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub detail: String,
    pub enabled: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub action: String,
    pub label: String,
    pub value: String,
    pub max: usize,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Panel {
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub heading: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hint {
    pub term: String,
    pub definition: String,
}

/// Everything the page draws in HTML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ui {
    pub screen: Screen,
    pub title: String,
    pub canvas: bool,
    pub paused: bool,
    pub panels: Vec<Panel>,
    pub hints: Vec<Hint>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    strings: Strings,
    hints: Vec<Hint>,
    index: Index,
    pub screen: Screen,
    song: usize,
    difficulty: usize,
    pub kept: Kept,
    chart: Option<(String, Chart)>,
    audio_ready: Option<String>,
    session: Option<Session>,
    calibration: Option<Calibration>,
    outcome: Option<Outcome>,
    paused: bool,
    commands: Vec<Command>,
    message: String,
    /// The last pointer position, in playfield units.
    pointer: (i32, i32),
    /// A still scene asked for by `?tile=`: the time to hold, once its
    /// chart arrives.
    tile_at_ms: Option<i64>,
}

fn action(name: &str, value: impl ToString, label: String) -> Action {
    Action { name: name.into(), value: value.to_string(), label, detail: String::new(), enabled: true, selected: false }
}

fn panel(id: &str) -> Panel {
    Panel { id: id.into(), heading: String::new(), lines: Vec::new(), actions: Vec::new(), field: None }
}

fn signed_ms(ms: i64) -> String {
    if ms > 0 {
        format!("+{ms}")
    } else {
        ms.to_string()
    }
}

impl Game {
    pub fn new(strings_json: &str, glossary_md: &str, index_json: &str) -> Result<Game, String> {
        let strings = page::parse_strings(strings_json).map_err(|e| e.message())?;
        let hints = crate::glossary::parse(glossary_md)
            .map_err(|e| e.message())?
            .into_iter()
            .map(|e| Hint { term: e.term, definition: e.definition })
            .collect();
        let index = crate::catalog::parse_index(index_json)?;
        if index.songs.is_empty() {
            return Err("The chart index lists no songs.".into());
        }
        Ok(Game {
            strings,
            hints,
            index,
            screen: Screen::Title,
            song: 0,
            difficulty: 0,
            kept: Kept::default(),
            chart: None,
            audio_ready: None,
            session: None,
            calibration: None,
            outcome: None,
            paused: false,
            commands: Vec::new(),
            message: String::new(),
            pointer: (FIELD_W / 2, FIELD_H / 2),
            tile_at_ms: None,
        })
    }

    fn s(&self, key: &str) -> String {
        fill(&self.strings, key, &[])
    }

    fn f(&self, key: &str, values: &[(&str, String)]) -> String {
        fill(&self.strings, key, values)
    }

    fn best_key(&self, song: usize, difficulty: usize) -> String {
        format!("{}:{}", self.index.songs[song].id, DIFFICULTIES[difficulty].key)
    }

    fn chart_url(&self) -> String {
        let s = &self.index.songs[self.song];
        let c = &s.charts[self.difficulty];
        format!("data/{}?h={}", c.file, c.hash)
    }

    fn audio_url(&self) -> String {
        let s = &self.index.songs[self.song];
        format!("{}?h={}", s.audio, s.audio_hash)
    }

    fn show(&mut self, screen: Screen) {
        self.screen = screen;
        self.message.clear();
        self.commands.push(Command::Ui);
    }

    /// Loads what the chosen chart needs, then waits for the first frame to
    /// start it.
    fn begin(&mut self) {
        self.stop();
        self.outcome = None;
        let chart_url = self.chart_url();
        if self.chart.as_ref().map(|(u, _)| u != &chart_url).unwrap_or(true) {
            self.chart = None;
            self.commands.push(Command::LoadChart { url: chart_url });
        }
        let audio_url = self.audio_url();
        if self.audio_ready.as_ref() != Some(&audio_url) {
            self.audio_ready = None;
            self.commands.push(Command::LoadAudio { url: audio_url });
        }
        self.commands.push(Command::Volume { pct: self.kept.settings.volume_pct });
        self.show(Screen::Loading);
    }

    fn stop(&mut self) {
        if self.session.take().is_some() || self.calibration.take().is_some() {
            self.commands.push(Command::StopAudio);
        }
        if self.paused {
            self.paused = false;
            self.commands.push(Command::Resume);
        }
    }

    fn step(value: &mut i64, range: (i64, i64, i64), up: bool) {
        let next = if up { *value + range.2 } else { *value - range.2 };
        *value = next.clamp(range.0, range.1);
    }

    /// The one door for buttons, settings and loaded files.
    pub fn act(&mut self, name: &str, value: &str) -> Result<(), String> {
        match name {
            "screen.show" => {
                let screen = Screen::from_name(value).ok_or_else(|| format!("There is no screen \"{value}\"."))?;
                self.stop();
                if screen == Screen::Calibrate {
                    self.calibration = Some(Calibration { start_us: None, offsets: Vec::new(), now: 0, result: None });
                    self.commands.push(Command::Volume { pct: self.kept.settings.volume_pct });
                }
                self.show(screen);
            }
            "song.pick" => {
                let k: usize = value.parse().map_err(|_| format!("There is no song \"{value}\"."))?;
                if k >= self.index.songs.len() {
                    return Err(format!("There is no song \"{value}\"."));
                }
                self.song = k;
                self.commands.push(Command::Ui);
            }
            "difficulty.pick" => {
                let k = DIFFICULTIES
                    .iter()
                    .position(|d| d.key == value)
                    .ok_or_else(|| format!("There is no difficulty \"{value}\"."))?;
                self.difficulty = k;
                self.commands.push(Command::Ui);
            }
            "play.start" => self.begin(),
            "chart.loaded" => {
                if let Some(at) = self.tile_at_ms.take() {
                    return self.tile(value, at);
                }
                let chart = Chart::parse(value).map_err(|e| e.message())?;
                self.chart = Some((self.chart_url(), chart));
            }
            "tile.spec" => {
                // song id, difficulty key and a time in ms: "placeholder-3:hard:12000"
                let parts: Vec<&str> = value.split(':').collect();
                let bad = || format!("The scene \"{value}\" is not song:difficulty:ms.");
                let [song, diff, ms] = parts[..] else {
                    return Err(bad());
                };
                self.song = self.index.songs.iter().position(|s| s.id == song).ok_or_else(bad)?;
                self.difficulty = DIFFICULTIES.iter().position(|d| d.key == diff).ok_or_else(bad)?;
                self.tile_at_ms = Some(ms.parse().map_err(|_| bad())?);
                self.commands.push(Command::LoadChart { url: self.chart_url() });
            }
            "audio.ready" => self.audio_ready = Some(value.to_string()),
            "load.failed" => {
                let song = self.index.songs[self.song].title.clone();
                self.message = self.f("loading.failed", &[("song", song), ("why", value.to_string())]);
                self.screen = Screen::Select;
                self.commands.push(Command::Ui);
            }
            "play.pause" | "page.hidden" => {
                let running = self.session.as_ref().map(|s| s.start_us.is_some()).unwrap_or(false);
                if self.screen == Screen::Play && running && !self.paused {
                    self.paused = true;
                    self.commands.push(Command::Suspend);
                    self.commands.push(Command::Ui);
                }
            }
            "key.escape" => {
                if self.paused {
                    return self.act("play.resume", "");
                }
                match self.screen {
                    Screen::Play => return self.act("play.pause", ""),
                    Screen::Select | Screen::Settings => self.show(Screen::Title),
                    Screen::Results => self.show(Screen::Select),
                    Screen::Calibrate => return self.act("screen.show", "settings"),
                    _ => {}
                }
            }
            "play.resume" => {
                if self.paused {
                    self.paused = false;
                    self.commands.push(Command::Resume);
                    self.commands.push(Command::Ui);
                }
            }
            "play.retry" => self.begin(),
            "play.quit" => {
                self.stop();
                self.show(Screen::Select);
            }
            "audio.less" | "audio.more" => {
                Self::step(&mut self.kept.settings.audio_offset_ms, AUDIO_OFFSET_MS, name.ends_with("more"));
                self.commands.push(Command::Ui);
            }
            "visual.less" | "visual.more" => {
                Self::step(&mut self.kept.settings.visual_offset_ms, VISUAL_OFFSET_MS, name.ends_with("more"));
                self.commands.push(Command::Ui);
            }
            "volume.less" | "volume.more" => {
                Self::step(&mut self.kept.settings.volume_pct, VOLUME_PCT, name.ends_with("more"));
                self.commands.push(Command::Volume { pct: self.kept.settings.volume_pct });
                self.commands.push(Command::Ui);
            }
            "name.set" => {
                self.kept.settings.name = clean_name(value);
                self.commands.push(Command::Ui);
            }
            "calibrate.again" => return self.act("screen.show", "calibrate"),
            other => return Err(format!("Unknown action \"{other}\".")),
        }
        Ok(())
    }

    /// The context time heard at the moment `stamp_us`, from the audio
    /// clock's output timestamp.
    fn heard(ctx_us: i64, perf_us: i64, stamp_us: i64) -> i64 {
        ctx_us + (stamp_us - perf_us)
    }

    /// One pointer or key event. `kind` is 0 for a move, 1 for a press, 2 for
    /// a release. `x` and `y` are in the canvas's CSS pixels, `w` and `h` its
    /// size, so the playfield maps to any window.
    #[allow(clippy::too_many_arguments)]
    pub fn input(&mut self, kind: u8, button: u8, x: i32, y: i32, w: i32, h: i32, ctx_us: i64, perf_us: i64, stamp_us: i64) {
        if w > 0 && h > 0 {
            self.pointer = ((x as i64 * FIELD_W as i64 / w as i64) as i32, (y as i64 * FIELD_H as i64 / h as i64) as i32);
        }
        let heard = Self::heard(ctx_us, perf_us, stamp_us);
        let offset = self.kept.settings.audio_offset_ms * MS;
        if let Some(cal) = self.calibration.as_mut() {
            if let (1, Some(start)) = (kind, cal.start_us) {
                if cal.result.is_none() {
                    let t = heard - start;
                    // the nearest click
                    let k = ((t + CALIBRATION_BEAT_US / 2).div_euclid(CALIBRATION_BEAT_US)).clamp(0, CALIBRATION_CLICKS - 1);
                    cal.offsets.push(t - k * CALIBRATION_BEAT_US);
                    if cal.offsets.len() >= CALIBRATION_PRESSES {
                        let ms = median(&cal.offsets) / MS;
                        cal.result = Some(ms);
                        self.kept.settings.audio_offset_ms = ms.clamp(AUDIO_OFFSET_MS.0, AUDIO_OFFSET_MS.1);
                        self.commands.push(Command::StopAudio);
                    }
                    self.commands.push(Command::Ui);
                }
            }
            return;
        }
        if self.paused {
            return;
        }
        let pointer = self.pointer;
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let Some(start) = session.start_us else {
            return;
        };
        let t = heard - start - offset;
        session.play.input(Input::Move { t, x: pointer.0, y: pointer.1 });
        match kind {
            1 => session.play.input(Input::Down { t, button }),
            2 => session.play.input(Input::Up { t, button }),
            _ => {}
        }
    }

    /// One display frame: runs the play up to now and returns the draw list.
    pub fn frame(&mut self, ctx_us: i64, perf_us: i64, now_us: i64) -> Vec<i32> {
        let heard = Self::heard(ctx_us, perf_us, now_us);
        match self.screen {
            Screen::Loading => self.try_start(heard),
            Screen::Play if !self.paused => self.run(heard),
            Screen::Calibrate => self.run_calibration(heard),
            _ => {}
        }
        self.draw()
    }

    fn try_start(&mut self, heard: i64) {
        let Some((url, chart)) = self.chart.clone() else {
            return;
        };
        if url != self.chart_url() || self.audio_ready.as_deref() != Some(self.audio_url().as_str()) {
            return;
        }
        let d = DIFFICULTIES[chart.difficulty];
        let first = chart.targets.first().map(|t| t.t).unwrap_or(0);
        let lead = LEAD_US.max(d.approach_us + FIRST_TARGET_MARGIN_US - first);
        let start = heard + lead;
        self.session = Some(Session { play: Play::new(chart), start_us: Some(start), song_now: -lead });
        self.commands.push(Command::StartAudio { url: self.audio_url(), at_us: start });
        self.show(Screen::Play);
    }

    fn run(&mut self, heard: i64) {
        let offset = self.kept.settings.audio_offset_ms * MS;
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let Some(start) = session.start_us else {
            return;
        };
        let now = heard - start - offset;
        session.song_now = now;
        session.play.advance(now);
        let p = &session.play;
        let last_end = p.chart.targets.iter().map(|t| t.end()).max().unwrap_or(0);
        if now >= p.chart.length_us || (p.over() && now >= last_end + OUTRO_US) {
            self.finish();
        }
    }

    fn finish(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };
        self.commands.push(Command::StopAudio);
        let p = session.play;
        let key = self.best_key(self.song, self.difficulty);
        let old = self.kept.bests.get(&key).copied();
        let new_best = old.map(|b| p.score > b.score).unwrap_or(true);
        if new_best {
            self.kept.bests.insert(key, Best { score: p.score, accuracy: p.accuracy() });
        }
        self.outcome = Some(Outcome {
            song: self.song,
            difficulty: self.difficulty,
            score: p.score,
            accuracy: p.accuracy(),
            max_combo: p.max_combo,
            full_combo: p.full_combo(),
            counts: p.counts,
            ticks: (p.ticks_hit, p.ticks_total),
            mean_offset_us: p.mean_offset_us(),
            new_best,
        });
        self.show(Screen::Results);
    }

    fn run_calibration(&mut self, heard: i64) {
        let Some(cal) = self.calibration.as_mut() else {
            return;
        };
        match cal.start_us {
            None => {
                let start = heard + LEAD_US;
                cal.start_us = Some(start);
                self.commands.push(Command::Clicks { at_us: start, every_us: CALIBRATION_BEAT_US, count: CALIBRATION_CLICKS });
            }
            Some(start) => cal.now = heard - start,
        }
    }

    /// The commands since the last call, as JSON.
    pub fn commands(&mut self) -> String {
        let out = serde_json::to_string(&self.commands).unwrap_or_else(|_| "[]".into());
        self.commands.clear();
        out
    }

    pub fn ui(&self) -> Ui {
        let mut panels = Vec::new();
        let st = &self.kept.settings;
        match self.screen {
            Screen::Title => {
                let mut p = panel("intro");
                p.heading = self.s("title");
                p.lines.push(self.s("intro"));
                p.actions.push(action("screen.show", "select", self.s("menu.play")));
                p.actions.push(action("screen.show", "settings", self.s("menu.settings")));
                panels.push(p);
            }
            Screen::Select | Screen::Loading => {
                let mut songs = panel("songs");
                songs.heading = self.s("select.heading");
                for (k, s) in self.index.songs.iter().enumerate() {
                    let mut a = action("song.pick", k, s.title.clone());
                    let bpm = format!("{}", (s.bpm_milli + 500) / 1000);
                    a.detail = format!(
                        "{} · {}",
                        self.f("select.by", &[("by", s.by.clone())]),
                        self.f("select.facts", &[("bpm", bpm), ("length", minutes(s.length_us))])
                    );
                    a.selected = k == self.song;
                    songs.actions.push(a);
                }
                panels.push(songs);
                let mut diff = panel("difficulty");
                diff.heading = self.s("select.difficulty");
                let song = &self.index.songs[self.song];
                for (k, d) in DIFFICULTIES.iter().enumerate() {
                    let mut a = action("difficulty.pick", d.key, self.s(&format!("difficulty.{}", d.key)));
                    let targets = song.charts.get(k).map(|c| c.targets).unwrap_or(0);
                    let best = match self.kept.bests.get(&self.best_key(self.song, k)) {
                        Some(b) => self.f("select.best", &[("score", grouped(b.score)), ("accuracy", percent(b.accuracy))]),
                        None => self.s("select.unplayed"),
                    };
                    a.detail = format!("{} · {}", self.f("select.targets", &[("n", grouped(targets as i64))]), best);
                    a.selected = k == self.difficulty;
                    diff.actions.push(a);
                }
                panels.push(diff);
                let mut go = panel("go");
                if self.screen == Screen::Loading {
                    go.lines.push(self.f("loading", &[("song", song.title.clone())]));
                }
                let mut start = action("play.start", "", self.s("select.start"));
                start.enabled = self.screen == Screen::Select;
                go.actions.push(start);
                go.actions.push(action("screen.show", "title", self.s("menu.back")));
                panels.push(go);
            }
            Screen::Play => {
                if self.paused {
                    let mut p = panel("pause");
                    p.heading = self.s("pause.heading");
                    p.actions.push(action("play.resume", "", self.s("pause.resume")));
                    p.actions.push(action("play.retry", "", self.s("pause.retry")));
                    p.actions.push(action("play.quit", "", self.s("pause.quit")));
                    panels.push(p);
                }
            }
            Screen::Results => {
                if let Some(o) = &self.outcome {
                    let mut p = panel("results");
                    let song = &self.index.songs[o.song];
                    let diff = self.s(&format!("difficulty.{}", DIFFICULTIES[o.difficulty].key));
                    p.heading = self.f("results.heading", &[("song", song.title.clone()), ("difficulty", diff)]);
                    p.lines.push(self.f("results.score", &[("score", grouped(o.score))]));
                    p.lines.push(self.f("results.accuracy", &[("accuracy", percent(o.accuracy))]));
                    p.lines.push(self.f(
                        "results.combo",
                        &[("n", grouped(o.max_combo as i64)), ("full", grouped(o.full_combo as i64))],
                    ));
                    let counts: Vec<(&str, String)> = JUDGEMENTS
                        .iter()
                        .map(|j| (j.key(), grouped(o.counts[*j as usize] as i64)))
                        .collect();
                    p.lines.push(self.f("results.counts", &counts));
                    if o.ticks.1 > 0 {
                        p.lines.push(self.f(
                            "results.holds",
                            &[("hit", grouped(o.ticks.0 as i64)), ("total", grouped(o.ticks.1 as i64))],
                        ));
                    }
                    p.lines.push(match o.mean_offset_us {
                        None => self.s("results.timing.none"),
                        Some(us) if us < 0 => self.f("results.timing.early", &[("ms", ((-us + 500) / MS).to_string())]),
                        Some(us) => self.f("results.timing.late", &[("ms", ((us + 500) / MS).to_string())]),
                    });
                    if o.new_best {
                        p.lines.push(self.s("results.best"));
                    }
                    p.actions.push(action("play.retry", "", self.s("results.retry")));
                    p.actions.push(action("play.quit", "", self.s("results.songs")));
                    panels.push(p);
                }
            }
            Screen::Settings => {
                let mut head = panel("settings");
                head.heading = self.s("settings.heading");
                panels.push(head);
                let rows = [
                    ("audio", self.f("settings.audio", &[("ms", signed_ms(st.audio_offset_ms))])),
                    ("visual", self.f("settings.visual", &[("ms", signed_ms(st.visual_offset_ms))])),
                    ("volume", self.f("settings.volume", &[("pct", st.volume_pct.to_string())])),
                ];
                for (id, line) in rows {
                    let mut p = panel(id);
                    p.lines.push(line);
                    p.actions.push(action(&format!("{id}.less"), "", self.s("settings.less")));
                    p.actions.push(action(&format!("{id}.more"), "", self.s("settings.more")));
                    if id == "audio" {
                        p.actions.push(action("screen.show", "calibrate", self.s("settings.calibrate")));
                    }
                    panels.push(p);
                }
                let mut name = panel("name");
                name.field = Some(Field {
                    action: "name.set".into(),
                    label: self.s("settings.name"),
                    value: st.name.clone(),
                    max: NAME_MAX,
                    note: self.s("settings.name.note"),
                });
                panels.push(name);
                let mut back = panel("back");
                back.actions.push(action("screen.show", "title", self.s("menu.back")));
                panels.push(back);
            }
            Screen::Calibrate => {
                let mut p = panel("calibrate");
                p.heading = self.s("calibrate.heading");
                if let Some(cal) = &self.calibration {
                    match cal.result {
                        None => {
                            p.lines.push(self.f("calibrate.intro", &[("n", CALIBRATION_PRESSES.to_string())]));
                            p.lines.push(self.f(
                                "calibrate.count",
                                &[("n", cal.offsets.len().to_string()), ("total", CALIBRATION_PRESSES.to_string())],
                            ));
                        }
                        Some(ms) => {
                            p.lines.push(self.f(
                                "calibrate.result",
                                &[("ms", ms.to_string()), ("offset", signed_ms(st.audio_offset_ms))],
                            ));
                            p.actions.push(action("calibrate.again", "", self.s("calibrate.again")));
                        }
                    }
                }
                p.actions.push(action("screen.show", "settings", self.s("calibrate.done")));
                panels.push(p);
            }
        }
        Ui {
            screen: self.screen,
            title: self.s("title"),
            canvas: matches!(self.screen, Screen::Play | Screen::Calibrate),
            paused: self.paused,
            panels,
            hints: self.hints.clone(),
            message: self.message.clone(),
        }
    }

    pub fn ui_json(&self) -> String {
        serde_json::to_string(&self.ui()).unwrap_or_default()
    }

    /// The canvas for this frame.
    pub fn draw(&self) -> Vec<i32> {
        let mut list = DrawList::new();
        match self.screen {
            Screen::Play => {
                if let Some(s) = &self.session {
                    let t = s.song_now + self.kept.settings.visual_offset_ms * MS;
                    draw_play(&mut list, &s.play, t, s.song_now, &self.strings, self.pointer);
                }
            }
            Screen::Calibrate => {
                if let Some(c) = &self.calibration {
                    draw_calibration(&mut list, c.now, c.result.is_some());
                }
            }
            _ => {}
        }
        list.finish()
    }

    /// Sets up a still scene for screenshots (`?tile=` on the page): the
    /// chart given, with time held at `at_ms` and the perfect bot's presses
    /// up to then.
    pub fn tile(&mut self, chart_json: &str, at_ms: i64) -> Result<(), String> {
        let chart = Chart::parse(chart_json).map_err(|e| e.message())?;
        let t = at_ms * MS;
        let mut play = Play::new(chart.clone());
        for i in crate::play::perfect_inputs(&chart) {
            if i.at() <= t - 200 * MS {
                play.input(i);
            }
        }
        play.advance(t);
        self.pointer = play.cursor;
        self.session = Some(Session { play, start_us: None, song_now: t });
        self.screen = Screen::Play;
        self.commands.push(Command::Ui);
        Ok(())
    }

    /// How many targets of the play under way have a judgement, for the
    /// page's `data-judged` (the browser check reads it).
    pub fn judged(&self) -> u32 {
        self.session.as_ref().map(|s| s.play.counts.iter().sum()).unwrap_or(0)
    }

    pub fn kept_json(&self) -> String {
        serde_json::to_string(&self.kept).unwrap_or_default()
    }

    /// Takes settings and bests from a save; a save that cannot be read is
    /// ignored, and the message says so.
    pub fn restore(&mut self, kept_json: &str) {
        match serde_json::from_str::<Kept>(kept_json) {
            Ok(mut k) => {
                k.settings.audio_offset_ms = k.settings.audio_offset_ms.clamp(AUDIO_OFFSET_MS.0, AUDIO_OFFSET_MS.1);
                k.settings.visual_offset_ms = k.settings.visual_offset_ms.clamp(VISUAL_OFFSET_MS.0, VISUAL_OFFSET_MS.1);
                k.settings.volume_pct = k.settings.volume_pct.clamp(VOLUME_PCT.0, VOLUME_PCT.1);
                k.settings.name = clean_name(&k.settings.name);
                self.kept = k;
            }
            Err(_) => self.kept = Kept::default(),
        }
    }
}

/// A name with control characters taken out, trimmed, at most `NAME_MAX`
/// characters.
pub fn clean_name(raw: &str) -> String {
    let kept: String = raw.chars().filter(|c| !c.is_control()).collect();
    kept.trim().chars().take(NAME_MAX).collect::<String>().trim_end().to_string()
}

fn median(values: &[i64]) -> i64 {
    let mut v = values.to_vec();
    v.sort();
    if v.is_empty() {
        return 0;
    }
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2
    }
}

fn judgement_look(j: Judgement) -> (Shape, Color) {
    match j {
        Judgement::Perfect => (Shape::Star, Color::Perfect),
        Judgement::Great => (Shape::Diamond, Color::Great),
        Judgement::Good => (Shape::Triangle, Color::Good),
        Judgement::Miss => (Shape::Cross, Color::Miss),
    }
}

/// The playfield at song time `t` (moved by the visual offset); `heard` is
/// the unmoved time, for the progress bar.
pub fn draw_play(list: &mut DrawList, p: &Play, t: i64, heard: i64, strings: &Strings, pointer: (i32, i32)) {
    let d = p.d;
    let r = d.radius;
    list.clear(Color::Paper);
    list.rect(0, 0, FIELD_W, FIELD_H, Color::Field);
    // progress along the top
    let length = p.chart.length_us.max(1);
    let done = (heard.clamp(0, length) * FIELD_W as i64 / length) as i32;
    list.rect(0, 0, done, 6, Color::Muted);
    // targets on screen, latest first so the next one is on top
    let visible: Vec<usize> = (0..p.chart.targets.len())
        .filter(|&k| {
            let target = &p.chart.targets[k];
            let shown = t >= target.t - d.approach_us;
            let open = p.judged[k].is_none() && t <= target.t + d.good_us;
            let holding = p.judged[k].map(|j| j != Judgement::Miss).unwrap_or(false) && target.hold.is_some() && t <= target.end();
            shown && (open || holding)
        })
        .collect();
    for &k in visible.iter().rev() {
        let target = &p.chart.targets[k];
        let fade_in = ((t - (target.t - d.approach_us)) * UNIT as i64 / (d.approach_us / 3).max(1)).clamp(0, UNIT as i64) as i32;
        list.alpha(fade_in);
        if let Some(h) = &target.hold {
            let pts: Vec<(i32, i32)> = h.path.iter().map(|q| (q[0], q[1])).collect();
            list.alpha(fade_in * 45 / 100);
            list.line(&pts, Color::Hold, 2 * r);
            list.alpha(fade_in);
            list.line(&pts, Color::Ink, 3);
            let (ex, ey) = target.point_at(h.end);
            list.circle(ex, ey, r * 7 / 10, None, Some(Color::Ink), 3);
        }
        let head_open = p.judged[k].is_none();
        if head_open {
            list.circle(target.x, target.y, r, Some(Color::Accent), Some(Color::Ink), 5);
            list.text(target.x, target.y, r, Color::Paper, Align::Center, &target.combo.to_string());
            // the approach ring meets the edge at the target's time
            let left = (target.t - t).max(0);
            let ring = r + (2 * r as i64 * left / d.approach_us) as i32;
            list.circle(target.x, target.y, ring, None, Some(Color::Ink), 4);
        } else {
            // a hold under way: the point to follow, and how far it may stray
            let (fx, fy) = target.point_at(t);
            list.circle(fx, fy, r * crate::tuning::HOLD_FOLLOW_PCT / 100, None, Some(Color::Muted), 2);
            list.circle(fx, fy, r * 8 / 10, Some(Color::Accent), Some(Color::Ink), 4);
        }
    }
    list.alpha(UNIT);
    // marks where things were judged, fading over half a second
    for f in &p.feed {
        let age = t - f.t;
        if !(0..500_000).contains(&age) {
            continue;
        }
        let a = (UNIT as i64 - age * UNIT as i64 / 500_000) as i32;
        list.alpha(a);
        let rise = (age / 20_000) as i32;
        match f.mark {
            Mark::Judged(j) => {
                let (shape, color) = judgement_look(j);
                list.shape(shape, f.x, f.y - rise, r * UNIT * 9 / 10, color, Color::Ink, 2);
                let word = fill(strings, &format!("judgement.{}", j.key()), &[]);
                list.text(f.x, f.y + r + 22 - rise, 30, color, Align::Center, &word);
            }
            Mark::Tick => {
                list.circle(f.x, f.y, 10, Some(Color::Perfect), None, 0);
            }
            Mark::HoldDone => {
                list.shape(Shape::Star, f.x, f.y - rise, r * UNIT * 7 / 10, Color::Perfect, Color::Ink, 2);
            }
            Mark::HoldBroken => {
                list.shape(Shape::Cross, f.x, f.y - rise, r * UNIT * 7 / 10, Color::Miss, Color::Ink, 2);
            }
        }
    }
    list.alpha(UNIT);
    // score, accuracy and combo
    list.text(FIELD_W - 40, 50, 44, Color::Ink, Align::Right, &fill(strings, "play.score", &[("score", grouped(p.score))]));
    list.text(FIELD_W - 40, 96, 28, Color::Muted, Align::Right, &fill(strings, "play.accuracy", &[("accuracy", percent(p.accuracy()))]));
    if p.combo > 0 {
        list.text(40, FIELD_H - 50, 44, Color::Ink, Align::Left, &fill(strings, "play.combo", &[("n", grouped(p.combo as i64))]));
    }
    // the cursor
    list.circle(pointer.0, pointer.1, 12, None, Some(Color::Ink), 3);
    list.circle(pointer.0, pointer.1, 4, Some(Color::Ink), None, 0);
}

fn draw_calibration(list: &mut DrawList, now: i64, done: bool) {
    list.clear(Color::Paper);
    list.rect(0, 0, FIELD_W, FIELD_H, Color::Field);
    let (cx, cy) = (FIELD_W / 2, FIELD_H / 3);
    let into = now.rem_euclid(CALIBRATION_BEAT_US);
    let pulse = if now >= 0 && !done { (120 - into * 120 / CALIBRATION_BEAT_US) as i32 } else { 0 };
    list.circle(cx, cy, 60 + pulse / 2, Some(Color::Accent), Some(Color::Ink), 5);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{ChartEntry, IndexSong};
    use crate::chart::tests::{chart, tap};

    const GLOSSARY: &str = "### Target\n\nA circle.\n";

    fn index() -> String {
        let song = IndexSong {
            id: "s".into(),
            title: "Song".into(),
            by: "Us".into(),
            audio: "audio/s.mp3".into(),
            audio_hash: "a1".into(),
            bpm_milli: 120_000,
            length_us: 60_000_000,
            charts: (0..5)
                .map(|d| ChartEntry {
                    difficulty: d,
                    file: format!("charts/s-{}.json", DIFFICULTIES[d].key),
                    hash: format!("h{d}"),
                    targets: 3,
                })
                .collect(),
        };
        serde_json::to_string(&Index { songs: vec![song] }).unwrap()
    }

    fn game() -> Game {
        Game::new(include_str!("../../../data/strings.json"), GLOSSARY, &index()).expect("game")
    }

    fn commands(g: &mut Game) -> serde_json::Value {
        serde_json::from_str(&g.commands()).unwrap()
    }

    /// A game playing a three-target Easy chart, the song started at audio
    /// context time 10 s.
    fn playing() -> Game {
        let mut g = game();
        g.act("play.start", "").unwrap();
        let c = commands(&mut g);
        assert_eq!(c[0]["load_chart"]["url"], "data/charts/s-easy.json?h=h0");
        assert_eq!(c[1]["load_audio"]["url"], "audio/s.mp3?h=a1");
        let ch = chart(0, vec![tap(2000, 400, 400, 1), tap(3000, 600, 400, 2), tap(4000, 800, 400, 3)]);
        g.act("chart.loaded", &ch.to_json()).unwrap();
        g.frame(9_000_000, 0, 0);
        assert_eq!(g.screen, Screen::Loading, "no start before the audio is ready");
        g.act("audio.ready", "audio/s.mp3?h=a1").unwrap();
        commands(&mut g);
        // heard now: 8.0 s; lead: max(1 s, 1.5 s + 0.5 s - 2 s) = 1 s
        g.frame(8_000_000, 0, 0);
        assert_eq!(g.screen, Screen::Play);
        let c = commands(&mut g);
        assert_eq!(c[0]["start_audio"]["at_us"], 9_000_000);
        g
    }

    #[test]
    fn song_time_is_the_heard_time_less_the_start_and_the_offset() {
        let mut g = playing();
        // output timestamp: context 10.5 s heard at performance 100 s; the
        // press was 1.5 s later, so heard at 12.0 s: song time 3.0 s
        g.input(0, 0, 600, 400, FIELD_W, FIELD_H, 10_500_000, 100_000_000, 101_500_000);
        g.input(1, 0, 600, 400, FIELD_W, FIELD_H, 10_500_000, 100_000_000, 101_500_000);
        let p = &g.session.as_ref().unwrap().play;
        assert_eq!(p.judged[1], Some(Judgement::Perfect));
        assert_eq!(p.feed.last().unwrap().offset_us, 0);
        // with a +20 ms audio offset, the same press counts 20 ms earlier
        let mut g = playing();
        g.kept.settings.audio_offset_ms = 20;
        g.input(1, 0, 600, 400, FIELD_W, FIELD_H, 10_500_000, 100_000_000, 101_500_000);
        assert_eq!(g.session.as_ref().unwrap().play.feed.last().unwrap().offset_us, -20_000);
    }

    #[test]
    fn the_pointer_maps_from_any_canvas_size_to_the_playfield() {
        let mut g = playing();
        g.input(0, 0, 400, 225, 800, 450, 0, 0, 0);
        assert_eq!(g.pointer, (800, 450));
        g.input(0, 0, 1920, 0, 1920, 1080, 0, 0, 0);
        assert_eq!(g.pointer, (FIELD_W, 0));
    }

    #[test]
    fn a_play_ends_on_the_results_screen_with_a_best_kept() {
        let mut g = playing();
        // no presses at all; the song runs past the outro
        g.frame(9_000_000 + 7_000_000, 0, 0);
        assert_eq!(g.screen, Screen::Results);
        let ui = g.ui();
        assert_eq!(ui.panels[0].lines[0], "Score 0");
        assert!(ui.panels[0].lines.contains(&"Perfect 0, Great 0, Good 0, Miss 3".to_string()));
        assert_eq!(g.kept.bests.get("s:easy"), Some(&Best { score: 0, accuracy: 0 }));
    }

    #[test]
    fn pausing_suspends_the_audio_and_freezes_input() {
        let mut g = playing();
        g.act("key.escape", "").unwrap();
        assert!(g.paused);
        assert_eq!(commands(&mut g)[0], "suspend");
        g.input(1, 0, 400, 400, FIELD_W, FIELD_H, 11_000_000, 0, 0);
        assert_eq!(g.session.as_ref().unwrap().play.judged[0], None);
        g.act("key.escape", "").unwrap();
        assert!(!g.paused);
        assert_eq!(commands(&mut g)[0], "resume");
    }

    #[test]
    fn calibration_of_taps_20_ms_late_sets_an_offset_of_20_ms() {
        let mut g = game();
        g.act("screen.show", "calibrate").unwrap();
        g.frame(5_000_000, 0, 0);
        let c = commands(&mut g);
        let clicks = c.as_array().unwrap().iter().find(|c| c.get("clicks").is_some()).unwrap().clone();
        assert_eq!(clicks["clicks"]["at_us"], 6_000_000);
        for k in 0..CALIBRATION_PRESSES as i64 {
            // late by 20 ms, give or take 3
            let wobble = [0, 3, -3, 1][k as usize % 4];
            let at = 6_000_000 + (k + 2) * CALIBRATION_BEAT_US + 20_000 + wobble * 1000;
            g.input(1, 1, 0, 0, 0, 0, at, 0, 0);
        }
        assert_eq!(g.kept.settings.audio_offset_ms, 20);
        assert!(g.ui().panels[0].lines[0].contains("The audio offset is now +20 ms."));
    }

    #[test]
    fn settings_step_and_stop_at_their_limits() {
        let mut g = game();
        for _ in 0..100 {
            g.act("audio.more", "").unwrap();
            g.act("volume.less", "").unwrap();
        }
        assert_eq!(g.kept.settings.audio_offset_ms, AUDIO_OFFSET_MS.1);
        assert_eq!(g.kept.settings.volume_pct, 0);
        g.act("visual.less", "").unwrap();
        assert_eq!(g.kept.settings.visual_offset_ms, -5);
    }

    #[test]
    fn a_name_is_trimmed_cleaned_and_cut_to_sixteen() {
        assert_eq!(clean_name("  Ana \n"), "Ana");
        assert_eq!(clean_name("a\u{7}b"), "ab");
        assert_eq!(clean_name("abcdefghijklmnopqrstu"), "abcdefghijklmnop");
        assert_eq!(clean_name("abcdefghijklmno qrs"), "abcdefghijklmno");
    }

    #[test]
    fn a_save_from_any_build_restores_within_the_limits() {
        let mut g = game();
        g.restore(r#"{"settings": {"audio_offset_ms": 900, "visual_offset_ms": 0, "volume_pct": 50, "name": "Cy"}, "bests": {"s:easy": {"score": 10, "accuracy": 9000}}}"#);
        assert_eq!(g.kept.settings.audio_offset_ms, 300);
        assert_eq!(g.kept.settings.name, "Cy");
        assert_eq!(g.kept.bests.len(), 1);
        g.restore("not json");
        assert_eq!(g.kept, Kept::default());
    }

    #[test]
    fn unknown_actions_and_screens_are_refused_by_name() {
        let mut g = game();
        assert_eq!(g.act("screen.jump", ""), Err("Unknown action \"screen.jump\".".into()));
        assert_eq!(g.act("screen.show", "nowhere"), Err("There is no screen \"nowhere\".".into()));
        assert_eq!(g.act("song.pick", "7"), Err("There is no song \"7\".".into()));
    }

    #[test]
    fn a_tile_spec_loads_its_chart_and_holds_the_scene() {
        let mut g = game();
        g.act("tile.spec", "s:hard:2500").unwrap();
        assert_eq!(commands(&mut g)[0]["load_chart"]["url"], "data/charts/s-hard.json?h=h2");
        let ch = chart(2, vec![tap(2000, 400, 400, 1), tap(3000, 600, 400, 2)]);
        g.act("chart.loaded", &ch.to_json()).unwrap();
        assert_eq!(g.screen, Screen::Play);
        let p = &g.session.as_ref().unwrap().play;
        assert_eq!(p.judged, vec![Some(Judgement::Perfect), None]);
        assert!(g.act("tile.spec", "s:hard").is_err());
    }

    #[test]
    fn the_play_screen_draws_a_list_the_page_can_walk() {
        let mut g = playing();
        // song time 1.5 s: the first target is on screen
        let list = g.frame(10_500_000, 0, 0);
        assert!(crate::draw::walk(&list).unwrap() > 10);
    }
}

/// The shim's answer to an action: `{"ok":true}` or `{"error":"..."}`.
pub fn result_json(r: Result<(), String>) -> String {
    match r {
        Ok(()) => serde_json::json!({ "ok": true }).to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}
