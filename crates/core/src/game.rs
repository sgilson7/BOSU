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

use crate::catalog::Index;
use crate::chart::Chart;
use crate::draw::{Align, Color, DrawList, Shape, UNIT};
use crate::page::{self, fill, grouped, minutes, percent, Strings};
use crate::play::{Input, Judgement, Mark, Play, JUDGEMENTS};
use crate::save::{self, Loaded, Save};
use crate::tuning::{DIFFICULTIES, FIELD_H, FIELD_W};

/// The shortest wait between pressing Start and the song's first sound.
pub const LEAD_US: i64 = 1_000_000;
/// The first target never appears sooner than this after the song starts.
pub const FIRST_TARGET_MARGIN_US: i64 = 500_000;
/// The maker named on the songs this project made itself.
pub const OUR_OWN: &str = "BOSU";
/// The largest difference between two decodes' starts taken as real.
pub const MAX_SHIFT_US: i64 = 60_000;
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
/// The button number of the mouse or a touch.
pub const MOUSE: u8 = 0;

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
    Lobby,
    Join,
}

impl Screen {
    fn from_name(name: &str) -> Option<Screen> {
        Some(match name {
            "title" => Screen::Title,
            "select" => Screen::Select,
            "settings" => Screen::Settings,
            "calibrate" => Screen::Calibrate,
            "join" => Screen::Join,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub audio_offset_ms: i64,
    pub visual_offset_ms: i64,
    pub volume_pct: i64,
    pub name: String,
    /// The press keys, as `KeyboardEvent.code` values ("KeyZ", "KeyW"):
    /// the key's place on the keyboard, whatever its layout prints on it.
    pub keys: Vec<String>,
    /// Whether a mouse button or a touch presses as well as aiming.
    pub mouse_presses: bool,
}

/// The press keys a new player starts with.
pub fn default_keys() -> Vec<String> {
    vec!["KeyZ".into(), "KeyX".into()]
}

/// How many press keys a player may have. Button 0 is the mouse; keys are
/// buttons 1 up.
pub const MAX_KEYS: usize = 8;
/// The one key that is never a press key: it pauses and cancels.
pub const ESCAPE: &str = "Escape";

/// What a player reads for a key code: "KeyW" is "W", "Digit1" is "1",
/// "ArrowLeft" is "Left", "ShiftLeft" is "Left Shift"; others as given.
pub fn key_label(code: &str) -> String {
    if let Some(k) = code.strip_prefix("Key").or_else(|| code.strip_prefix("Digit")) {
        return k.to_string();
    }
    if let Some(k) = code.strip_prefix("Arrow") {
        return k.to_string();
    }
    if let Some(k) = code.strip_prefix("Numpad") {
        return format!("Numpad {k}");
    }
    for side in ["Left", "Right"] {
        if let Some(k) = code.strip_suffix(side) {
            if !k.is_empty() {
                return format!("{side} {k}");
            }
        }
    }
    code.to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            audio_offset_ms: 0,
            visual_offset_ms: 0,
            volume_pct: 80,
            keys: default_keys(),
            mouse_presses: true,
            name: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Best {
    pub score: i64,
    pub accuracy: i64,
}

/// What the page should do, in order. Sent as JSON from `commands()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    /// Fetch this file under `data/` and pass its text to `chart.loaded`.
    LoadChart {
        url: String,
        /// For checking a ledger record (`check.chart`), not for playing.
        check: bool,
    },
    /// Fetch and decode this audio file, then call `audio.ready` with the url.
    LoadAudio {
        url: String,
    },
    /// Start the loaded song at this audio context time.
    StartAudio {
        url: String,
        at_us: i64,
    },
    StopAudio,
    Suspend,
    Resume,
    /// Keep this text in the browser (`web/save/store.js`).
    Store {
        text: String,
    },
    /// Keep this refused save beside the new one, never overwriting it.
    KeepRefused {
        text: String,
    },
    /// Schedule `count` clicks, `every_us` apart, from `at_us`.
    Clicks {
        at_us: i64,
        every_us: i64,
        count: i64,
    },
    Volume {
        pct: i64,
    },
    /// The screen changed: draw `ui()` again.
    Ui,
    /// Open the transport: host or join, by room code or pasted codes.
    WireOpen {
        host: bool,
        mode: String,
        room: String,
    },
    /// Make a pasted-code invitation and pass it to `online.invitation`.
    WireInvitation,
    /// Answer a pasted invitation; pass the reply to `online.reply`.
    WireReply {
        text: String,
    },
    /// Take the joiner's pasted reply.
    WireAccept {
        text: String,
    },
    WireClose,
    Send {
        peer: u32,
        text: String,
    },
    /// Put this text on the clipboard.
    Copy {
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Session {
    pub(crate) play: Play,
    /// The audio context time the song starts at, once scheduled.
    pub(crate) start_us: Option<i64>,
    pub(crate) song_now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Calibration {
    start_us: Option<i64>,
    offsets: Vec<i64>,
    now: i64,
    result: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outcome {
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
    /// The element's id on the page.
    pub id: String,
    pub action: String,
    pub label: String,
    pub value: String,
    /// The most characters; 0 for no limit.
    pub max: usize,
    pub note: String,
    pub multiline: bool,
    pub readonly: bool,
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
    pub(crate) strings: Strings,
    pub(crate) hints: Vec<Hint>,
    pub(crate) index: Index,
    pub screen: Screen,
    pub(crate) song: usize,
    pub(crate) difficulty: usize,
    pub kept: Save,
    pub(crate) chart: Option<(String, Chart)>,
    pub(crate) audio_ready: Option<String>,
    pub(crate) session: Option<Session>,
    pub(crate) calibration: Option<Calibration>,
    pub(crate) outcome: Option<Outcome>,
    pub(crate) paused: bool,
    pub(crate) commands: Vec<Command>,
    pub(crate) message: String,
    /// The last pointer position, in playfield units.
    pub(crate) pointer: (i32, i32),
    /// A still scene asked for by `?tile=`: the time to hold, once its
    /// chart arrives.
    pub(crate) tile_at_ms: Option<i64>,
    /// Waiting for the next key, to add it as a press key.
    pub(crate) listening: bool,
    /// How much earlier this browser's decode of the song starts than the
    /// build's (R6.6); added to song time.
    pub(crate) audio_shift_us: i64,
    /// The lobby and the ledger's traffic (`together.rs`).
    pub(crate) together: crate::together::Together,
}

pub(crate) fn action(name: &str, value: impl ToString, label: String) -> Action {
    Action {
        name: name.into(),
        value: value.to_string(),
        label,
        detail: String::new(),
        enabled: true,
        selected: false,
    }
}

pub(crate) fn panel(id: &str) -> Panel {
    Panel {
        id: id.into(),
        heading: String::new(),
        lines: Vec::new(),
        actions: Vec::new(),
        field: None,
    }
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
            .map(|e| Hint {
                term: e.term,
                definition: e.definition,
            })
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
            kept: crate::save::new_game(),
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
            listening: false,
            audio_shift_us: 0,
            together: crate::together::Together::new(&crate::catalog::fnv(index_json.as_bytes())),
        })
    }

    pub(crate) fn s(&self, key: &str) -> String {
        fill(&self.strings, key, &[])
    }

    pub(crate) fn f(&self, key: &str, values: &[(&str, String)]) -> String {
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

    pub(crate) fn show(&mut self, screen: Screen) {
        self.screen = screen;
        self.message.clear();
        self.commands.push(Command::Ui);
    }

    /// Loads what the chosen chart needs, then waits for the first frame to
    /// start it.
    pub(crate) fn begin(&mut self) {
        self.stop();
        self.outcome = None;
        let chart_url = self.chart_url();
        if self.chart.as_ref().map(|(u, _)| u != &chart_url).unwrap_or(true) {
            self.chart = None;
            self.commands.push(Command::LoadChart {
                url: chart_url,
                check: false,
            });
        }
        let audio_url = self.audio_url();
        if self.audio_ready.as_ref() != Some(&audio_url) {
            self.audio_ready = None;
            self.commands.push(Command::LoadAudio { url: audio_url });
        }
        self.commands.push(Command::Volume {
            pct: self.kept.settings.volume_pct,
        });
        self.show(Screen::Loading);
    }

    /// The host's song and difficulty go to the lobby.
    fn pick_for_lobby(&mut self) {
        if let Some(l) = self.together.lobby.as_mut() {
            l.pick(self.song, self.difficulty);
        }
        self.after_wire();
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
                let screen =
                    Screen::from_name(value).ok_or_else(|| format!("There is no screen \"{value}\"."))?;
                self.stop();
                if screen == Screen::Calibrate {
                    self.calibration = Some(Calibration {
                        start_us: None,
                        offsets: Vec::new(),
                        now: 0,
                        result: None,
                    });
                    self.commands.push(Command::Volume {
                        pct: self.kept.settings.volume_pct,
                    });
                }
                self.show(screen);
            }
            "song.pick" => {
                let k: usize = value
                    .parse()
                    .map_err(|_| format!("There is no song \"{value}\"."))?;
                if k >= self.index.songs.len() {
                    return Err(format!("There is no song \"{value}\"."));
                }
                self.song = k;
                self.pick_for_lobby();
                self.commands.push(Command::Ui);
            }
            "difficulty.pick" => {
                let k = DIFFICULTIES
                    .iter()
                    .position(|d| d.key == value)
                    .ok_or_else(|| format!("There is no difficulty \"{value}\"."))?;
                self.difficulty = k;
                self.pick_for_lobby();
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
                self.song = self
                    .index
                    .songs
                    .iter()
                    .position(|s| s.id == song)
                    .ok_or_else(bad)?;
                self.difficulty = DIFFICULTIES.iter().position(|d| d.key == diff).ok_or_else(bad)?;
                self.tile_at_ms = Some(ms.parse().map_err(|_| bad())?);
                self.commands.push(Command::LoadChart {
                    url: self.chart_url(),
                    check: false,
                });
            }
            "audio.ready" => self.audio_ready = Some(value.to_string()),
            "load.failed" => {
                let song = self.index.songs[self.song].title.clone();
                self.message = self.f("loading.failed", &[("song", song), ("why", value.to_string())]);
                self.screen = Screen::Select;
                self.commands.push(Command::Ui);
            }
            "play.pause" | "page.hidden" => {
                let running = self
                    .session
                    .as_ref()
                    .map(|s| s.start_us.is_some())
                    .unwrap_or(false);
                if self.screen == Screen::Play && running && !self.paused {
                    self.paused = true;
                    self.commands.push(Command::Suspend);
                    self.commands.push(Command::Ui);
                }
            }
            "key.escape" => {
                if self.listening {
                    self.listening = false;
                    self.commands.push(Command::Ui);
                    return Ok(());
                }
                if self.paused {
                    return self.act("play.resume", "");
                }
                match self.screen {
                    Screen::Play => return self.act("play.pause", ""),
                    Screen::Select | Screen::Settings => self.show(Screen::Title),
                    Screen::Results if self.together.lobby.is_some() => self.show(Screen::Lobby),
                    Screen::Results => self.show(Screen::Select),
                    Screen::Join => self.show(Screen::Title),
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
                self.together.in_round = false;
                self.show(if self.together.lobby.is_some() {
                    Screen::Lobby
                } else {
                    Screen::Select
                });
            }
            "audio.less" | "audio.more" => {
                Self::step(
                    &mut self.kept.settings.audio_offset_ms,
                    AUDIO_OFFSET_MS,
                    name.ends_with("more"),
                );
                self.commands.push(Command::Ui);
            }
            "visual.less" | "visual.more" => {
                Self::step(
                    &mut self.kept.settings.visual_offset_ms,
                    VISUAL_OFFSET_MS,
                    name.ends_with("more"),
                );
                self.commands.push(Command::Ui);
            }
            "volume.less" | "volume.more" => {
                Self::step(
                    &mut self.kept.settings.volume_pct,
                    VOLUME_PCT,
                    name.ends_with("more"),
                );
                self.commands.push(Command::Volume {
                    pct: self.kept.settings.volume_pct,
                });
                self.commands.push(Command::Ui);
            }
            "name.set" => {
                self.kept.settings.name = clean_name(value);
                self.commands.push(Command::Ui);
            }
            "keys.listen" => {
                if self.kept.settings.keys.len() >= MAX_KEYS {
                    self.message = self.f("settings.keys.full", &[("n", MAX_KEYS.to_string())]);
                } else {
                    self.listening = true;
                    self.message.clear();
                }
                self.commands.push(Command::Ui);
            }
            "keys.remove" => {
                self.kept.settings.keys.retain(|k| k != value);
                self.message.clear();
                self.commands.push(Command::Ui);
            }
            "keys.reset" => {
                self.kept.settings.keys = default_keys();
                self.listening = false;
                self.message.clear();
                self.commands.push(Command::Ui);
            }
            "mouse.toggle" => {
                self.kept.settings.mouse_presses = !self.kept.settings.mouse_presses;
                self.commands.push(Command::Ui);
            }
            "calibrate.again" => return self.act("screen.show", "calibrate"),
            "save.loaded" => self.restore(value),
            other if crate::together::handles(other) => return self.together_act(other, value),
            other => return Err(format!("Unknown action \"{other}\".")),
        }
        let changes_save = ["name.set", "keys.remove", "keys.reset", "mouse.toggle"];
        if name.ends_with(".less") || name.ends_with(".more") || changes_save.contains(&name) {
            self.keep();
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
    pub fn input(
        &mut self,
        kind: u8,
        button: u8,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        ctx_us: i64,
        perf_us: i64,
        stamp_us: i64,
    ) {
        if w > 0 && h > 0 {
            self.pointer = (
                (x as i64 * FIELD_W as i64 / w as i64) as i32,
                (y as i64 * FIELD_H as i64 / h as i64) as i32,
            );
        }
        // with the mouse set to aim only, its presses are moves
        let kind = if button == MOUSE && !self.kept.settings.mouse_presses {
            0
        } else {
            kind
        };
        let heard = Self::heard(ctx_us, perf_us, stamp_us);
        let offset = self.kept.settings.audio_offset_ms * MS;
        if let Some(cal) = self.calibration.as_mut() {
            if let (1, Some(start)) = (kind, cal.start_us) {
                if cal.result.is_none() {
                    let t = heard - start;
                    // the nearest click
                    let k = ((t + CALIBRATION_BEAT_US / 2).div_euclid(CALIBRATION_BEAT_US))
                        .clamp(0, CALIBRATION_CLICKS - 1);
                    cal.offsets.push(t - k * CALIBRATION_BEAT_US);
                    if cal.offsets.len() >= CALIBRATION_PRESSES {
                        let ms = median(&cal.offsets) / MS;
                        cal.result = Some(ms);
                        self.kept.settings.audio_offset_ms = ms.clamp(AUDIO_OFFSET_MS.0, AUDIO_OFFSET_MS.1);
                        self.commands.push(Command::StopAudio);
                        self.commands.push(Command::Store {
                            text: save::store(&self.kept),
                        });
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
        let t = heard - start - offset + self.audio_shift_us;
        session.play.input(Input::Move {
            t,
            x: pointer.0,
            y: pointer.1,
        });
        match kind {
            1 => session.play.input(Input::Down { t, button }),
            2 => session.play.input(Input::Up { t, button }),
            _ => {}
        }
    }

    /// The first seconds of the song as this browser decoded it, at `rate`
    /// samples a second: where its first sound falls, against where the
    /// build's decode found it, gives the shift to song time. A shift past
    /// `MAX_SHIFT_US` is taken as a song that opens too softly to tell, and
    /// ignored.
    pub fn audio_head(&mut self, url: &str, samples: &[i16], rate: i64) {
        let song = &self.index.songs[self.song];
        let expected = song.first_sound_us;
        self.audio_shift_us = match crate::beats::first_sound_us(samples, rate.max(1)) {
            Some(here) if url == self.audio_url() && (expected - here).abs() <= MAX_SHIFT_US => {
                expected - here
            }
            _ => 0,
        };
    }

    /// One key going down (`kind` 1) or up (`kind` 2), by its code. Escape
    /// pauses or cancels; a press key presses its button; while listening,
    /// the key becomes a press key. Returns whether the key was the game's,
    /// so the page can keep the browser from acting on it too.
    pub fn key(&mut self, kind: u8, code: &str, ctx_us: i64, perf_us: i64, stamp_us: i64) -> bool {
        if code == ESCAPE {
            if kind == 1 {
                let _ = self.act("key.escape", "");
            }
            return true;
        }
        if self.listening {
            if kind != 1 {
                return true;
            }
            self.listening = false;
            let keys = &mut self.kept.settings.keys;
            if keys.iter().any(|k| k == code) {
                self.message = self.f("settings.keys.taken", &[("key", key_label(code))]);
            } else {
                keys.push(code.to_string());
                self.message.clear();
                self.keep();
            }
            self.commands.push(Command::Ui);
            return true;
        }
        let Some(k) = self.kept.settings.keys.iter().position(|c| c == code) else {
            return false;
        };
        self.input(kind, (k + 1) as u8, 0, 0, 0, 0, ctx_us, perf_us, stamp_us);
        true
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
        // a lobby's round counts in together (PLAN.md decision 16)
        let least = if self.together.in_round {
            crate::together::COUNT_IN_US
        } else {
            LEAD_US
        };
        let lead = least.max(d.approach_us + FIRST_TARGET_MARGIN_US - first);
        let start = heard + lead;
        self.session = Some(Session {
            play: Play::new(chart),
            start_us: Some(start),
            song_now: -lead,
        });
        self.commands.push(Command::StartAudio {
            url: self.audio_url(),
            at_us: start,
        });
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
        let now = heard - start - offset + self.audio_shift_us;
        session.song_now = now;
        session.play.advance(now);
        self.post_score(now, false);
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let p = &session.play;
        let last_end = p.chart.targets.iter().map(|t| t.end()).max().unwrap_or(0);
        if now >= p.chart.length_us || (p.over() && now >= last_end + OUTRO_US) {
            self.finish();
        }
    }

    fn finish(&mut self) {
        self.post_score(0, true);
        let Some(session) = self.session.take() else {
            return;
        };
        self.record_play(&session.play);
        self.commands.push(Command::StopAudio);
        let p = session.play;
        let key = self.best_key(self.song, self.difficulty);
        let old = self.kept.bests.get(&key).copied();
        // a play that scored nothing is not a best
        let new_best = p.score > old.map(|b| b.score).unwrap_or(0);
        if new_best {
            self.kept.bests.insert(
                key,
                Best {
                    score: p.score,
                    accuracy: p.accuracy(),
                },
            );
            self.keep();
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
                self.commands.push(Command::Clicks {
                    at_us: start,
                    every_us: CALIBRATION_BEAT_US,
                    count: CALIBRATION_CLICKS,
                });
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
                p.lines.push(self.s("title.tagline"));
                // who made the songs, in the order they first appear
                let mut makers: Vec<String> = Vec::new();
                for s in &self.index.songs {
                    if s.by != OUR_OWN && !makers.contains(&s.by) {
                        makers.push(s.by.clone());
                    }
                }
                let names = match makers.len() {
                    0 => String::new(),
                    1 => makers[0].clone(),
                    n => format!(
                        "{} {} {}",
                        makers[..n - 1].join(", "),
                        self.s("title.and"),
                        makers[n - 1]
                    ),
                };
                p.lines.push(self.f(
                    "title.songs",
                    &[("n", self.index.songs.len().to_string()), ("makers", names)],
                ));
                p.actions
                    .push(action("screen.show", "select", self.s("menu.play")));
                p.actions.push(action("online.host", "", self.s("menu.host")));
                p.actions.push(action("screen.show", "join", self.s("menu.join")));
                p.actions
                    .push(action("screen.show", "settings", self.s("menu.settings")));
                panels.push(p);
                let mut how = panel("how");
                how.heading = self.s("title.how");
                how.lines.push(self.s("intro"));
                panels.push(how);
            }
            Screen::Lobby => self.lobby_panels(&mut panels),
            Screen::Join => self.join_panels(&mut panels),
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
                        Some(b) => self.f(
                            "select.best",
                            &[("score", grouped(b.score)), ("accuracy", percent(b.accuracy))],
                        ),
                        None => self.s("select.unplayed"),
                    };
                    a.detail = format!(
                        "{} · {}",
                        self.f("select.targets", &[("n", grouped(targets as i64))]),
                        best
                    );
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
                go.actions
                    .push(action("screen.show", "title", self.s("menu.back")));
                panels.push(go);
                panels.push(self.board_panel());
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
                    p.heading = self.f(
                        "results.heading",
                        &[("song", song.title.clone()), ("difficulty", diff)],
                    );
                    p.lines
                        .push(self.f("results.score", &[("score", grouped(o.score))]));
                    p.lines
                        .push(self.f("results.accuracy", &[("accuracy", percent(o.accuracy))]));
                    p.lines.push(self.f(
                        "results.combo",
                        &[
                            ("n", grouped(o.max_combo as i64)),
                            ("full", grouped(o.full_combo as i64)),
                        ],
                    ));
                    let counts: Vec<(&str, String)> = JUDGEMENTS
                        .iter()
                        .map(|j| (j.key(), grouped(o.counts[*j as usize] as i64)))
                        .collect();
                    p.lines.push(self.f("results.counts", &counts));
                    if o.ticks.1 > 0 {
                        p.lines.push(self.f(
                            "results.holds",
                            &[
                                ("hit", grouped(o.ticks.0 as i64)),
                                ("total", grouped(o.ticks.1 as i64)),
                            ],
                        ));
                    }
                    p.lines.push(match o.mean_offset_us {
                        None => self.s("results.timing.none"),
                        Some(us) if us < 0 => {
                            self.f("results.timing.early", &[("ms", ((-us + 500) / MS).to_string())])
                        }
                        Some(us) => self.f("results.timing.late", &[("ms", ((us + 500) / MS).to_string())]),
                    });
                    if o.new_best {
                        p.lines.push(self.s("results.best"));
                    }
                    if self.together.lobby.is_some() {
                        p.actions.push(action("lobby.back", "", self.s("results.lobby")));
                        panels.push(p);
                        let mut all = panel("standings");
                        all.heading = self.s("results.standings");
                        for (k, (name, sc)) in self.standings().iter().enumerate() {
                            let key = if sc.done {
                                "results.standing.done"
                            } else {
                                "results.standing.playing"
                            };
                            all.lines.push(self.f(
                                key,
                                &[
                                    ("rank", (k + 1).to_string()),
                                    ("name", name.clone()),
                                    ("score", grouped(sc.score)),
                                    ("accuracy", percent(sc.accuracy)),
                                ],
                            ));
                        }
                        panels.push(all);
                    } else {
                        p.actions.push(action("play.retry", "", self.s("results.retry")));
                        p.actions.push(action("play.quit", "", self.s("results.songs")));
                        panels.push(p);
                    }
                    panels.push(self.board_panel());
                }
            }
            Screen::Settings => {
                let mut head = panel("settings");
                head.heading = self.s("settings.heading");
                panels.push(head);
                let rows = [
                    (
                        "audio",
                        self.f("settings.audio", &[("ms", signed_ms(st.audio_offset_ms))]),
                    ),
                    (
                        "visual",
                        self.f("settings.visual", &[("ms", signed_ms(st.visual_offset_ms))]),
                    ),
                    (
                        "volume",
                        self.f("settings.volume", &[("pct", st.volume_pct.to_string())]),
                    ),
                ];
                for (id, line) in rows {
                    let mut p = panel(id);
                    p.lines.push(line);
                    p.actions
                        .push(action(&format!("{id}.less"), "", self.s("settings.less")));
                    p.actions
                        .push(action(&format!("{id}.more"), "", self.s("settings.more")));
                    if id == "audio" {
                        p.actions
                            .push(action("screen.show", "calibrate", self.s("settings.calibrate")));
                    }
                    panels.push(p);
                }
                let mut keys = panel("keys");
                let labels: Vec<String> = st.keys.iter().map(|k| key_label(k)).collect();
                keys.lines.push(if labels.is_empty() {
                    self.s("settings.keys.none")
                } else {
                    self.f("settings.keys", &[("keys", labels.join(", "))])
                });
                if self.listening {
                    keys.lines.push(self.s("settings.keys.listening"));
                }
                let mut add = action("keys.listen", "", self.s("settings.keys.add"));
                add.enabled = !self.listening;
                keys.actions.push(add);
                for (code, label) in st.keys.iter().zip(&labels) {
                    keys.actions.push(action(
                        "keys.remove",
                        code,
                        self.f("settings.keys.remove", &[("key", label.clone())]),
                    ));
                }
                keys.actions
                    .push(action("keys.reset", "", self.s("settings.keys.reset")));
                panels.push(keys);
                let mut mouse = panel("mouse");
                mouse.lines.push(self.s(if st.mouse_presses {
                    "settings.mouse.on"
                } else {
                    "settings.mouse.off"
                }));
                mouse.actions.push(action(
                    "mouse.toggle",
                    "",
                    self.s(if st.mouse_presses {
                        "settings.mouse.turn_off"
                    } else {
                        "settings.mouse.turn_on"
                    }),
                ));
                panels.push(mouse);
                let mut name = panel("name");
                name.field = Some(Field {
                    id: "name-in".into(),
                    action: "name.set".into(),
                    label: self.s("settings.name"),
                    value: st.name.clone(),
                    max: NAME_MAX,
                    note: self.s("settings.name.note"),
                    multiline: false,
                    readonly: false,
                });
                panels.push(name);
                let mut back = panel("back");
                back.actions
                    .push(action("screen.show", "title", self.s("menu.back")));
                panels.push(back);
            }
            Screen::Calibrate => {
                let mut p = panel("calibrate");
                p.heading = self.s("calibrate.heading");
                if let Some(cal) = &self.calibration {
                    match cal.result {
                        None => {
                            p.lines
                                .push(self.f("calibrate.intro", &[("n", CALIBRATION_PRESSES.to_string())]));
                            p.lines.push(self.f(
                                "calibrate.count",
                                &[
                                    ("n", cal.offsets.len().to_string()),
                                    ("total", CALIBRATION_PRESSES.to_string()),
                                ],
                            ));
                        }
                        Some(ms) => {
                            p.lines.push(self.f(
                                "calibrate.result",
                                &[("ms", ms.to_string()), ("offset", signed_ms(st.audio_offset_ms))],
                            ));
                            p.actions
                                .push(action("calibrate.again", "", self.s("calibrate.again")));
                        }
                    }
                }
                p.actions
                    .push(action("screen.show", "settings", self.s("calibrate.done")));
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
                    if self.together.in_round {
                        draw_standings(&mut list, &self.standings(), s.song_now, &self.strings);
                    }
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
        self.session = Some(Session {
            play,
            start_us: None,
            song_now: t,
        });
        self.screen = Screen::Play;
        self.commands.push(Command::Ui);
        Ok(())
    }

    /// How many targets of the play under way have a judgement, for the
    /// page's `data-judged` (the browser check reads it).
    /// How many players' scores this browser holds for the lobby's round,
    /// for the page's `data-scoring` (the online check reads it).
    pub fn scoring(&self) -> u32 {
        self.together
            .lobby
            .as_ref()
            .map(|l| l.board.len() as u32)
            .unwrap_or(0)
    }

    pub fn judged(&self) -> u32 {
        self.session
            .as_ref()
            .map(|s| s.play.counts.iter().sum())
            .unwrap_or(0)
    }

    pub(crate) fn keep(&mut self) {
        self.commands.push(Command::Store {
            text: save::store(&self.kept),
        });
    }

    /// Takes settings and bests from the browser's save. A save this build
    /// cannot use starts a new one; the old text is kept beside it and the
    /// message says why.
    pub fn restore(&mut self, text: &str) {
        match save::load(text) {
            Loaded::Fresh => self.kept = save::new_game(),
            Loaded::Refused { why, kept } => {
                self.kept = save::new_game();
                self.message = self.s(why);
                self.commands.push(Command::KeepRefused { text: kept });
                self.commands.push(Command::Ui);
            }
            Loaded::Ready(mut k) => {
                k.settings.audio_offset_ms = k
                    .settings
                    .audio_offset_ms
                    .clamp(AUDIO_OFFSET_MS.0, AUDIO_OFFSET_MS.1);
                k.settings.visual_offset_ms = k
                    .settings
                    .visual_offset_ms
                    .clamp(VISUAL_OFFSET_MS.0, VISUAL_OFFSET_MS.1);
                k.settings.volume_pct = k.settings.volume_pct.clamp(VOLUME_PCT.0, VOLUME_PCT.1);
                k.settings.name = clean_name(&k.settings.name);
                self.kept = k;
            }
        }
    }
}

/// A name with control characters taken out, trimmed, at most `NAME_MAX`
/// characters.
pub fn clean_name(raw: &str) -> String {
    let kept: String = raw.chars().filter(|c| !c.is_control()).collect();
    kept.trim()
        .chars()
        .take(NAME_MAX)
        .collect::<String>()
        .trim_end()
        .to_string()
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
            let holding = p.judged[k].map(|j| j != Judgement::Miss).unwrap_or(false)
                && target.hold.is_some()
                && t <= target.end();
            shown && (open || holding)
        })
        .collect();
    for &k in visible.iter().rev() {
        let target = &p.chart.targets[k];
        let fade_in = ((t - (target.t - d.approach_us)) * UNIT as i64 / (d.approach_us / 3).max(1))
            .clamp(0, UNIT as i64) as i32;
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
            list.text(
                target.x,
                target.y,
                r,
                Color::Paper,
                Align::Center,
                &target.combo.to_string(),
            );
            // the approach ring meets the edge at the target's time
            let left = (target.t - t).max(0);
            let ring = r + (2 * r as i64 * left / d.approach_us) as i32;
            list.circle(target.x, target.y, ring, None, Some(Color::Ink), 4);
        } else {
            // a hold under way: the point to follow, and how far it may stray
            let (fx, fy) = target.point_at(t);
            list.circle(
                fx,
                fy,
                r * crate::tuning::HOLD_FOLLOW_PCT / 100,
                None,
                Some(Color::Muted),
                2,
            );
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
                list.shape(
                    Shape::Star,
                    f.x,
                    f.y - rise,
                    r * UNIT * 7 / 10,
                    Color::Perfect,
                    Color::Ink,
                    2,
                );
            }
            Mark::HoldBroken => {
                list.shape(
                    Shape::Cross,
                    f.x,
                    f.y - rise,
                    r * UNIT * 7 / 10,
                    Color::Miss,
                    Color::Ink,
                    2,
                );
            }
        }
    }
    list.alpha(UNIT);
    // score, accuracy and combo
    list.text(
        FIELD_W - 40,
        50,
        44,
        Color::Ink,
        Align::Right,
        &fill(strings, "play.score", &[("score", grouped(p.score))]),
    );
    list.text(
        FIELD_W - 40,
        96,
        28,
        Color::Muted,
        Align::Right,
        &fill(strings, "play.accuracy", &[("accuracy", percent(p.accuracy()))]),
    );
    if p.combo > 0 {
        list.text(
            40,
            FIELD_H - 50,
            44,
            Color::Ink,
            Align::Left,
            &fill(strings, "play.combo", &[("n", grouped(p.combo as i64))]),
        );
    }
    // the cursor
    list.circle(pointer.0, pointer.1, 12, None, Some(Color::Ink), 3);
    list.circle(pointer.0, pointer.1, 4, Some(Color::Ink), None, 0);
}

/// The lobby's scores down the left of the playfield, and the count-in.
fn draw_standings(list: &mut DrawList, rows: &[(String, crate::online::Score)], now: i64, strings: &Strings) {
    let x = 24;
    for (k, (name, sc)) in rows.iter().take(crate::tuning::LOBBY_SEATS).enumerate() {
        let y = 150 + k as i32 * 74;
        let short: String = name.chars().take(12).collect();
        list.text(x, y, 26, Color::Ink, Align::Left, &format!("{}. {short}", k + 1));
        list.text(
            x,
            y + 30,
            22,
            Color::Muted,
            Align::Left,
            &fill(
                strings,
                "play.standing",
                &[("score", grouped(sc.score)), ("n", grouped(sc.combo as i64))],
            ),
        );
    }
    if now < 0 {
        let left = (-now + 999_999) / 1_000_000;
        list.text(
            FIELD_W / 2,
            FIELD_H / 2,
            160,
            Color::Accent,
            Align::Center,
            &left.to_string(),
        );
    }
}

fn draw_calibration(list: &mut DrawList, now: i64, done: bool) {
    list.clear(Color::Paper);
    list.rect(0, 0, FIELD_W, FIELD_H, Color::Field);
    let (cx, cy) = (FIELD_W / 2, FIELD_H / 3);
    let into = now.rem_euclid(CALIBRATION_BEAT_US);
    let pulse = if now >= 0 && !done {
        (120 - into * 120 / CALIBRATION_BEAT_US) as i32
    } else {
        0
    };
    list.circle(cx, cy, 60 + pulse / 2, Some(Color::Accent), Some(Color::Ink), 5);
}

/// The shim's answer to an action: `{"ok":true}` or `{"error":"..."}`.
pub fn result_json(r: Result<(), String>) -> String {
    match r {
        Ok(()) => serde_json::json!({ "ok": true }).to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
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
            first_sound_us: 0,
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
        let ch = chart(
            0,
            vec![
                tap(2000, 400, 400, 1),
                tap(3000, 600, 400, 2),
                tap(4000, 800, 400, 3),
            ],
        );
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
        g.input(
            0,
            0,
            600,
            400,
            FIELD_W,
            FIELD_H,
            10_500_000,
            100_000_000,
            101_500_000,
        );
        g.input(
            1,
            0,
            600,
            400,
            FIELD_W,
            FIELD_H,
            10_500_000,
            100_000_000,
            101_500_000,
        );
        let p = &g.session.as_ref().unwrap().play;
        assert_eq!(p.judged[1], Some(Judgement::Perfect));
        assert_eq!(p.feed.last().unwrap().offset_us, 0);
        // with a +20 ms audio offset, the same press counts 20 ms earlier
        let mut g = playing();
        g.kept.settings.audio_offset_ms = 20;
        g.input(
            1,
            0,
            600,
            400,
            FIELD_W,
            FIELD_H,
            10_500_000,
            100_000_000,
            101_500_000,
        );
        assert_eq!(
            g.session.as_ref().unwrap().play.feed.last().unwrap().offset_us,
            -20_000
        );
    }

    #[test]
    fn a_browser_that_trims_the_song_s_start_is_moved_back_onto_the_chart() {
        // the build heard the first sound at 23 ms; this browser at 12 ms,
        // so everything in its copy comes 11 ms early and song time moves
        // 11 ms on
        let mut g = playing();
        g.index.songs[0].first_sound_us = 23_000;
        // quiet hiss, then the first hit at sample 529
        let mut head = vec![300i16; 44_100];
        for s in head.iter_mut().skip(529) {
            *s = 5000;
        }
        g.audio_head("audio/s.mp3?h=a1", &head, 44_100);
        assert_eq!(g.audio_shift_us, 23_000 - 529 * 1_000_000 / 44_100);
        // a press heard 11 ms before the target's time is on time
        g.input(
            0,
            MOUSE,
            600,
            400,
            FIELD_W,
            FIELD_H,
            10_500_000,
            100_000_000,
            101_488_995,
        );
        g.input(
            1,
            MOUSE,
            600,
            400,
            FIELD_W,
            FIELD_H,
            10_500_000,
            100_000_000,
            101_488_995,
        );
        assert_eq!(g.session.as_ref().unwrap().play.feed.last().unwrap().offset_us, 0);
        // another song's head, or one too far off, changes nothing
        g.audio_head("audio/other.mp3", &head, 44_100);
        assert_eq!(g.audio_shift_us, 0);
        let late = vec![5000i16; 1];
        g.index.songs[0].first_sound_us = 900_000;
        g.audio_head("audio/s.mp3?h=a1", &late, 44_100);
        assert_eq!(g.audio_shift_us, 0);
    }

    #[test]
    fn the_title_credits_each_maker_once_and_not_our_own() {
        let mut g = game();
        let mut more = g.index.songs[0].clone();
        more.by = "Leefy5".into();
        let mut ours = more.clone();
        ours.by = OUR_OWN.into();
        g.index.songs[0].by = "Emracool".into();
        g.index.songs.push(more.clone());
        g.index.songs.push(more);
        g.index.songs.push(ours);
        let lines = &g.ui().panels[0].lines;
        assert_eq!(
            lines[1],
            "4 songs, five difficulties each, by Emracool and Leefy5, and a few of our own."
        );
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
        assert!(ui.panels[0]
            .lines
            .contains(&"Perfect 0, Great 0, Good 0, Miss 3".to_string()));
        assert_eq!(g.kept.bests.get("s:easy"), None, "a score of 0 is no best");
        assert!(!ui.panels[0]
            .lines
            .contains(&"A new best for this chart.".to_string()));
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
        let clicks = c
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c.get("clicks").is_some())
            .unwrap()
            .clone();
        assert_eq!(clicks["clicks"]["at_us"], 6_000_000);
        for k in 0..CALIBRATION_PRESSES as i64 {
            // late by 20 ms, give or take 2, and one stray press 200 ms late
            // that the median ignores
            let wobble = if k == 15 {
                180
            } else {
                [-2, 2, -1, 1][k as usize % 4]
            };
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
        let c = commands(&mut g);
        let stored = c
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find_map(|c| c.get("store"))
            .unwrap()
            .clone();
        assert!(stored["text"]
            .as_str()
            .unwrap()
            .contains("\"visual_offset_ms\":-5"));
    }

    fn keys_of(g: &Game) -> Vec<String> {
        g.kept.settings.keys.clone()
    }

    #[test]
    fn w_a_s_d_can_replace_z_and_x_and_each_presses() {
        let mut g = game();
        g.act("screen.show", "settings").unwrap();
        for code in ["KeyW", "KeyA", "KeyS", "KeyD"] {
            g.act("keys.listen", "").unwrap();
            assert!(g.key(1, code, 0, 0, 0));
        }
        g.act("keys.remove", "KeyZ").unwrap();
        g.act("keys.remove", "KeyX").unwrap();
        assert_eq!(keys_of(&g), ["KeyW", "KeyA", "KeyS", "KeyD"]);
        assert_eq!(
            g.ui().panels.iter().find(|p| p.id == "keys").unwrap().lines[0],
            "Press keys: W, A, S, D"
        );
        // into a play: S presses the second target, Z no longer does
        let saved = g.kept.clone();
        let mut g = playing();
        g.kept = saved;
        g.input(
            0,
            MOUSE,
            600,
            400,
            FIELD_W,
            FIELD_H,
            10_500_000,
            100_000_000,
            101_500_000,
        );
        assert!(
            !g.key(1, "KeyZ", 10_500_000, 100_000_000, 101_500_000),
            "Z is not the game's now"
        );
        assert_eq!(g.session.as_ref().unwrap().play.judged[1], None);
        assert!(g.key(1, "KeyS", 10_500_000, 100_000_000, 101_500_000));
        assert_eq!(
            g.session.as_ref().unwrap().play.judged[1],
            Some(Judgement::Perfect)
        );
    }

    #[test]
    fn a_key_already_bound_is_refused_and_escape_cancels() {
        let mut g = game();
        g.act("keys.listen", "").unwrap();
        g.key(1, "KeyZ", 0, 0, 0);
        assert_eq!(keys_of(&g), ["KeyZ", "KeyX"]);
        assert_eq!(g.ui().message, "Z is a press key already.");
        g.act("keys.listen", "").unwrap();
        g.key(1, ESCAPE, 0, 0, 0);
        assert!(!g.listening);
        assert_eq!(keys_of(&g), ["KeyZ", "KeyX"]);
        assert_eq!(g.screen, Screen::Title, "the cancelling Escape went no further");
    }

    #[test]
    fn there_are_at_most_eight_press_keys() {
        let mut g = game();
        for k in 0..10 {
            g.act("keys.listen", "").unwrap();
            g.key(1, &format!("Digit{k}"), 0, 0, 0);
        }
        assert_eq!(keys_of(&g).len(), MAX_KEYS);
        assert_eq!(
            g.ui().message,
            "You have 8 press keys. Remove one to add another."
        );
    }

    #[test]
    fn with_the_mouse_set_to_aim_only_a_click_moves_and_does_not_press() {
        let mut g = playing();
        g.act("mouse.toggle", "").unwrap();
        g.input(
            1,
            MOUSE,
            600,
            400,
            FIELD_W,
            FIELD_H,
            10_500_000,
            100_000_000,
            101_500_000,
        );
        assert_eq!(g.pointer, (600, 400));
        assert_eq!(g.session.as_ref().unwrap().play.judged[1], None);
        g.key(1, "KeyX", 10_500_000, 100_000_000, 101_500_000);
        assert_eq!(
            g.session.as_ref().unwrap().play.judged[1],
            Some(Judgement::Perfect)
        );
    }

    #[test]
    fn key_codes_read_as_the_key() {
        assert_eq!(key_label("KeyW"), "W");
        assert_eq!(key_label("Digit4"), "4");
        assert_eq!(key_label("ArrowLeft"), "Left");
        assert_eq!(key_label("ShiftLeft"), "Left Shift");
        assert_eq!(key_label("Numpad5"), "Numpad 5");
        assert_eq!(key_label("Space"), "Space");
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
        g.restore(r#"{"version": 1, "settings": {"audio_offset_ms": 900, "visual_offset_ms": 0, "volume_pct": 50, "name": "Cy"}, "bests": {"s:easy": {"score": 10, "accuracy": 9000}}}"#);
        assert_eq!(g.kept.settings.audio_offset_ms, 300);
        assert_eq!(g.kept.settings.name, "Cy");
        assert_eq!(g.kept.bests.len(), 1);
        commands(&mut g);
        g.restore("not json");
        assert_eq!(g.kept, save::new_game());
        assert_eq!(g.ui().message, "Your saved scores and settings could not be read. They are kept in this browser; a new save has started.");
        assert_eq!(commands(&mut g)[0]["keep_refused"]["text"], "not json");
    }

    #[test]
    fn unknown_actions_and_screens_are_refused_by_name() {
        let mut g = game();
        assert_eq!(
            g.act("screen.jump", ""),
            Err("Unknown action \"screen.jump\".".into())
        );
        assert_eq!(
            g.act("screen.show", "nowhere"),
            Err("There is no screen \"nowhere\".".into())
        );
        assert_eq!(g.act("song.pick", "7"), Err("There is no song \"7\".".into()));
    }

    #[test]
    fn a_tile_spec_loads_its_chart_and_holds_the_scene() {
        let mut g = game();
        g.act("tile.spec", "s:hard:2500").unwrap();
        assert_eq!(
            commands(&mut g)[0]["load_chart"]["url"],
            "data/charts/s-hard.json?h=h2"
        );
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
