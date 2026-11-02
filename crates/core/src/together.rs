//! Playing together: the game's side of the lobby (round 4) and the ledger
//! (round 5). `online.rs` keeps the lobby's protocol and `ledger.rs` the
//! records; this module joins them to the screens, the play and the save.
//!
//! The page passes random bytes once at load (`page.random`); they become
//! the player's signing key the first time, and the seed of every room code.
//! Nothing connects until Host or Join is pressed (BO-7): only then does
//! the core ask the page to open the transport.

use std::collections::BTreeMap;

use crate::chart::Chart;
use crate::game::{action, panel, Command, Field, Game, Panel, Screen};
use crate::ledger::{self, Checked, Key, Record};
use crate::online::{self, Lobby, Msg, Peer, Role};
use crate::page::{grouped, percent};
use crate::tuning::DIFFICULTIES;

/// Seconds counted in before a lobby's round starts (PLAN.md decision 16).
pub const COUNT_IN_US: i64 = 3_000_000;
/// How often each player's running score goes to the others, in song time.
pub const POST_EVERY_US: i64 = 500_000;
/// Rows of a chart's leaderboard shown at once.
pub const BOARD_ROWS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Together {
    pub lobby: Option<Lobby>,
    /// Names the charts this build plays: two browsers must agree on it.
    pub build: String,
    /// The page's address, for the invite link.
    pub address: String,
    /// A room code from an invite link, waiting for Join to be pressed.
    pub invited: Option<String>,
    /// Pasted codes: the host's invitation and the joiner's reply.
    pub invitation: String,
    pub reply: String,
    /// Random bytes from the page, and how many room codes they have made.
    entropy: Vec<u8>,
    codes_made: u32,
    /// Records waiting for their chart to arrive before they can be checked.
    pending: Vec<String>,
    /// Charts fetched to check records, by board.
    charts: BTreeMap<String, Chart>,
    /// The next song time to post the running score at.
    pub next_post_us: i64,
    /// Whether the round under way came from the lobby.
    pub in_round: bool,
}

/// The actions this module answers.
pub fn handles(name: &str) -> bool {
    ["online.", "lobby.", "invite.", "check.", "page."]
        .iter()
        .any(|p| name.starts_with(p))
}

impl Together {
    pub fn new(build: &str) -> Together {
        Together {
            build: build.into(),
            ..Default::default()
        }
    }
}

impl Game {
    fn signing_key(&self) -> Option<Key> {
        Key::from_hex(&self.kept.secret)
    }

    pub(crate) fn player_id(&self) -> String {
        self.signing_key().map(|k| k.player()).unwrap_or_default()
    }

    /// The name others see: the chosen one, or "Player" and the start of
    /// the player's id.
    pub(crate) fn shown_name(&self) -> String {
        if self.kept.settings.name.is_empty() {
            let id = self.player_id();
            self.f(
                "online.anonymous",
                &[("id", id.get(..4).unwrap_or("").to_uppercase())],
            )
        } else {
            self.kept.settings.name.clone()
        }
    }

    fn next_seed(&mut self) -> u64 {
        self.together.codes_made += 1;
        let mut bytes = self.together.entropy.clone();
        bytes.extend(self.together.codes_made.to_le_bytes());
        let h = ledger::sha256_hex(&bytes);
        u64::from_str_radix(h.get(..16).unwrap_or("0"), 16).unwrap_or(0)
    }

    fn open_lobby(&mut self, role: Role, room: &str, mode: &str) {
        self.leave_lobby();
        let lobby = Lobby::new(
            role,
            room,
            &self.together.build,
            &self.player_id(),
            &self.shown_name(),
        );
        self.together.lobby = Some(lobby);
        self.together.invitation.clear();
        self.together.reply.clear();
        self.commands.push(Command::WireOpen {
            host: role == Role::Host,
            mode: mode.into(),
            room: room.into(),
        });
        self.show(Screen::Lobby);
    }

    fn leave_lobby(&mut self) {
        if let Some(l) = self.together.lobby.as_mut() {
            l.leave();
        }
        self.flush_wire();
        if self.together.lobby.take().is_some() {
            self.commands.push(Command::WireClose);
        }
        self.together.in_round = false;
    }

    pub(crate) fn together_act(&mut self, name: &str, value: &str) -> Result<(), String> {
        match name {
            "page.random" => {
                // comma-separated byte values from crypto.getRandomValues
                let bytes: Vec<u8> = value.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                if bytes.len() < 32 {
                    return Err("The page sent too few random bytes.".into());
                }
                if self.kept.secret.is_empty() {
                    self.kept.secret = ledger::hex(&bytes[..32]);
                    self.keep();
                }
                self.together.entropy = bytes;
            }
            "page.address" => self.together.address = value.to_string(),
            "page.hash" => {
                if let Some(code) = online::code_from_hash(value) {
                    self.together.invited = Some(code);
                    self.show(Screen::Join);
                }
            }
            "online.host" => {
                let code = online::room_code(self.next_seed());
                self.open_lobby(Role::Host, &code, "room");
            }
            "online.join" => {
                let code = online::clean_code(value).ok_or_else(|| self.s("online.bad_room"))?;
                self.open_lobby(Role::Joiner, &code, "room");
            }
            "online.host_code" => {
                self.open_lobby(Role::Host, "", "code");
                self.commands.push(Command::WireInvitation);
            }
            "online.invite" => self.commands.push(Command::WireInvitation),
            "online.invitation" => {
                self.together.invitation = value.to_string();
                self.commands.push(Command::Ui);
            }
            "online.accept" => {
                self.commands.push(Command::WireAccept {
                    text: value.trim().to_string(),
                });
            }
            "online.accepted" => {}
            "online.join_code" => {
                self.open_lobby(Role::Joiner, "", "code");
                self.commands.push(Command::WireReply {
                    text: value.trim().to_string(),
                });
            }
            "online.reply" => {
                self.together.reply = value.to_string();
                self.commands.push(Command::Ui);
            }
            "online.leave" => {
                self.leave_lobby();
                self.show(Screen::Title);
            }
            "lobby.ready" => {
                if let Some(l) = self.together.lobby.as_mut() {
                    let now = !l.me.ready;
                    l.set_ready(now);
                }
                self.after_wire();
            }
            "lobby.start" => {
                if let Some(l) = self.together.lobby.as_mut() {
                    l.begin();
                }
                self.after_wire();
            }
            "lobby.back" => {
                self.stop_round();
                self.show(if self.together.lobby.is_some() {
                    Screen::Lobby
                } else {
                    Screen::Title
                });
            }
            "invite.copy" => {
                let link = self.invite_link();
                self.commands.push(Command::Copy { text: link });
                self.message = self.s("online.copied");
                self.commands.push(Command::Ui);
            }
            "check.chart" => {
                let chart = Chart::parse(value).map_err(|e| e.message())?;
                let board = self.board_of_chart(&chart);
                self.together.charts.insert(board, chart);
                self.check_pending();
            }
            other => return Err(format!("Unknown action \"{other}\".")),
        }
        Ok(())
    }

    fn stop_round(&mut self) {
        self.together.in_round = false;
    }

    pub(crate) fn invite_link(&self) -> String {
        let room = self
            .together
            .lobby
            .as_ref()
            .map(|l| l.room.clone())
            .unwrap_or_default();
        format!("{}#room={room}", self.together.address)
    }

    // ---- the wire ---------------------------------------------------------

    pub fn wire_peer(&mut self, peer: Peer) {
        if let Some(l) = self.together.lobby.as_mut() {
            l.connected(peer);
        }
        self.after_wire();
    }

    pub fn wire_left(&mut self, peer: Peer) {
        if let Some(l) = self.together.lobby.as_mut() {
            l.left(peer);
        }
        self.after_wire();
    }

    pub fn wire_message(&mut self, peer: Peer, text: &str) {
        if let Some(l) = self.together.lobby.as_mut() {
            l.message(peer, text);
        }
        self.after_wire();
    }

    pub fn wire_error(&mut self, key: &str) {
        let known = [
            "online.no_relay",
            "online.no_path",
            "online.signal_failed",
            "online.bad_code",
        ];
        let key = known
            .iter()
            .find(|k| **k == key)
            .copied()
            .unwrap_or("online.signal_failed");
        self.message = self.s(key);
        self.commands.push(Command::Ui);
    }

    fn flush_wire(&mut self) {
        if let Some(l) = self.together.lobby.as_mut() {
            for (peer, text) in l.take_outbox() {
                self.commands.push(Command::Send { peer, text });
            }
        }
    }

    /// After anything that may have changed the lobby: greet new players
    /// with this browser's ledger, handle ledger messages, start a round the
    /// host started, notice the lobby ending, and send what is waiting.
    pub(crate) fn after_wire(&mut self) {
        let Some(l) = self.together.lobby.as_mut() else {
            return;
        };
        let new_peers = std::mem::take(&mut l.admitted_new);
        let ledger_in = std::mem::take(&mut l.ledger_in);
        let start = l.start.take();
        let over = l.over;
        let status = l.status;
        for peer in new_peers {
            self.send_have(Some(peer), self.kept.ledger.ids());
        }
        for (peer, m) in ledger_in {
            self.ledger_message(peer, m);
        }
        if let Some((_, song, difficulty)) = start {
            if song < self.index.songs.len() && difficulty < DIFFICULTIES.len() {
                self.song = song;
                self.difficulty = difficulty;
                self.together.in_round = true;
                self.together.next_post_us = 0;
                self.begin();
            }
        }
        self.flush_wire();
        if over {
            self.together.lobby = None;
            self.commands.push(Command::WireClose);
            if self.screen != Screen::Play {
                self.screen = Screen::Title;
            }
            self.message = self.s(status);
        }
        self.commands.push(Command::Ui);
    }

    // ---- the ledger over the wire ---------------------------------------

    fn send_have(&mut self, to: Option<Peer>, ids: Vec<String>) {
        let Some(l) = self.together.lobby.as_mut() else {
            return;
        };
        for chunk in ids.chunks(ledger::IDS_PER_MESSAGE) {
            l.ledger_send(to, &Msg::Have { ids: chunk.to_vec() });
        }
    }

    fn ledger_message(&mut self, peer: Peer, m: Msg) {
        match m {
            Msg::Have { ids } => {
                let mine = self.kept.ledger.ids();
                let want: Vec<String> = ids.into_iter().filter(|id| !mine.contains(id)).collect();
                if let Some(l) = self.together.lobby.as_mut() {
                    for chunk in want.chunks(ledger::IDS_PER_MESSAGE) {
                        l.ledger_send(Some(peer), &Msg::Want { ids: chunk.to_vec() });
                    }
                }
            }
            Msg::Want { ids } => {
                let texts: Vec<String> = ids
                    .iter()
                    .filter_map(|id| self.kept.ledger.get(id))
                    .map(Record::to_text)
                    .collect();
                if let Some(l) = self.together.lobby.as_mut() {
                    for chunk in texts.chunks(ledger::RECORDS_PER_MESSAGE) {
                        l.ledger_send(
                            Some(peer),
                            &Msg::Records {
                                records: chunk.to_vec(),
                            },
                        );
                    }
                }
            }
            Msg::Records { records } => {
                let gained = self.take_records(records);
                // the host passes what it learned on to everyone else
                let host = self
                    .together
                    .lobby
                    .as_ref()
                    .map(|l| l.role == Role::Host)
                    .unwrap_or(false);
                if host && !gained.is_empty() {
                    if let Some(l) = self.together.lobby.as_mut() {
                        for chunk in gained.chunks(ledger::IDS_PER_MESSAGE) {
                            l.ledger_send_except(peer, &Msg::Have { ids: chunk.to_vec() });
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The board key and this build's chart file for a record, if this
    /// build has that chart.
    fn chart_file_for(&self, r: &Record) -> Option<String> {
        let song = self.index.songs.iter().find(|s| s.id == r.body.song)?;
        let entry = song.charts.get(r.body.difficulty)?;
        (entry.hash == r.body.chart).then(|| format!("data/{}?h={}", entry.file, entry.hash))
    }

    fn board_of_chart(&self, c: &Chart) -> String {
        let hash = self
            .index
            .songs
            .iter()
            .find(|s| s.id == c.song)
            .and_then(|s| s.charts.get(c.difficulty))
            .map(|e| e.hash.clone())
            .unwrap_or_default();
        format!("{}:{}:{}", c.song, DIFFICULTIES[c.difficulty.min(4)].key, hash)
    }

    /// Checks and adds records; a record whose chart is not loaded yet waits
    /// for it. Returns the ids that were new.
    pub(crate) fn take_records(&mut self, texts: Vec<String>) -> Vec<String> {
        self.together.pending.extend(texts);
        self.check_pending()
    }

    fn check_pending(&mut self) -> Vec<String> {
        let pending = std::mem::take(&mut self.together.pending);
        let mut gained = Vec::new();
        let mut asked: Vec<String> = Vec::new();
        for text in pending {
            let Ok(r) = serde_json::from_str::<Record>(&text) else {
                continue;
            };
            let chart = match self.chart_file_for(&r) {
                None => None,
                Some(url) => match self.together.charts.get(&r.board()) {
                    Some(c) => Some(c.clone()),
                    None if !r.replay.is_empty() => {
                        if !asked.contains(&url) {
                            asked.push(url.clone());
                            self.commands.push(Command::LoadChart { url, check: true });
                        }
                        self.together.pending.push(text);
                        continue;
                    }
                    None => None,
                },
            };
            let id = r.id();
            if let Ok(checked) = ledger::check(&r, chart.as_ref()) {
                if self.kept.ledger.add(r, checked) {
                    gained.push(id);
                }
            }
        }
        if !gained.is_empty() {
            self.kept.ledger.trim();
            self.keep();
            self.commands.push(Command::Ui);
        }
        gained
    }

    /// After a play: the record goes in this browser's ledger and, in a
    /// lobby, to everyone there.
    pub(crate) fn record_play(&mut self, p: &crate::play::Play) {
        let Some(key) = self.signing_key() else {
            return;
        };
        if p.score == 0 {
            return;
        }
        let song = &self.index.songs[self.song];
        let hash = song.charts[self.difficulty].hash.clone();
        let r = ledger::make(&key, &self.shown_name(), &song.id.clone(), &hash, p);
        let id = r.id();
        if self.kept.ledger.add(r, Checked::Replayed) {
            self.kept.ledger.trim();
            self.keep();
            if self.together.lobby.is_some() {
                self.send_have(None, vec![id]);
            }
        }
    }

    // ---- screens ----------------------------------------------------------

    /// The current chart's leaderboard, best first.
    pub(crate) fn board_panel(&self) -> Panel {
        let mut p = panel("board");
        p.heading = self.s("board.heading");
        let song = &self.index.songs[self.song];
        let hash = &song.charts[self.difficulty].hash;
        let key = format!("{}:{}:{}", song.id, DIFFICULTIES[self.difficulty].key, hash);
        let rows = self.kept.ledger.board(&key);
        if rows.is_empty() {
            p.lines.push(self.s("board.empty"));
            return p;
        }
        let me = self.player_id();
        for (k, e) in rows.iter().take(BOARD_ROWS).enumerate() {
            let b = &e.record.body;
            let mark = if b.player == me {
                self.s("board.you")
            } else {
                self.s(match e.checked {
                    Checked::Replayed => "board.replayed",
                    Checked::Signed => "board.signed",
                    Checked::OtherChart => "board.signed",
                })
            };
            p.lines.push(self.f(
                "board.row",
                &[
                    ("rank", (k + 1).to_string()),
                    ("name", b.name.clone()),
                    ("score", grouped(b.score)),
                    ("accuracy", percent(b.accuracy)),
                    ("mark", mark),
                ],
            ));
        }
        p.lines
            .push(self.f("board.count", &[("n", rows.len().to_string())]));
        p
    }

    pub(crate) fn join_panels(&self, panels: &mut Vec<Panel>) {
        let mut p = panel("join");
        p.heading = self.s("join.heading");
        if let Some(code) = &self.together.invited {
            p.lines.push(self.f("join.invited", &[("code", code.clone())]));
            p.actions.push(action(
                "online.join",
                code,
                self.f("join.button", &[("code", code.clone())]),
            ));
        }
        p.field = Some(Field {
            id: "room-in".into(),
            action: "online.join".into(),
            label: self.s("join.code"),
            value: String::new(),
            max: online::CODE_LEN,
            note: self.s("join.note"),
            multiline: false,
            readonly: false,
        });
        panels.push(p);
        let mut by_code = panel("paste");
        by_code.heading = self.s("paste.heading");
        by_code
            .actions
            .push(action("online.host_code", "", self.s("paste.host_instead")));
        by_code.field = Some(Field {
            id: "offer-in".into(),
            action: "online.join_code".into(),
            label: self.s("paste.invitation"),
            value: String::new(),
            max: 0,
            note: self.s("paste.joiner_note"),
            multiline: true,
            readonly: false,
        });
        panels.push(by_code);
        let mut back = panel("back");
        back.actions
            .push(action("screen.show", "title", self.s("menu.back")));
        panels.push(back);
    }

    pub(crate) fn lobby_panels(&self, panels: &mut Vec<Panel>) {
        let Some(l) = &self.together.lobby else {
            return;
        };
        let host = l.role == Role::Host;
        let mut head = panel("lobby");
        head.heading = if l.room.is_empty() {
            self.s("lobby.heading.code")
        } else {
            self.f("lobby.heading", &[("code", l.room.clone())])
        };
        head.lines.push(self.s(l.status));
        if !l.room.is_empty() {
            head.lines
                .push(self.f("lobby.link", &[("link", self.invite_link())]));
            head.actions.push(action("invite.copy", "", self.s("lobby.copy")));
        }
        panels.push(head);
        let mut who = panel("members");
        who.heading = self.f(
            "lobby.members",
            &[
                ("n", l.members.len().to_string()),
                ("seats", crate::tuning::LOBBY_SEATS.to_string()),
            ],
        );
        for m in &l.members {
            let key = if m.host {
                "lobby.member.host"
            } else if m.ready {
                "lobby.member.ready"
            } else {
                "lobby.member"
            };
            who.lines.push(self.f(key, &[("name", m.name.clone())]));
        }
        panels.push(who);
        let song = &self.index.songs[l.song.min(self.index.songs.len() - 1)];
        let diff = self.s(&format!("difficulty.{}", DIFFICULTIES[l.difficulty.min(4)].key));
        let mut chosen = panel("chosen");
        chosen.lines.push(self.f(
            "lobby.chosen",
            &[("song", song.title.clone()), ("difficulty", diff)],
        ));
        if host {
            let mut start = action("lobby.start", "", self.s("lobby.start"));
            start.enabled = l.can_start();
            chosen.actions.push(start);
            if !l.can_start() {
                chosen.lines.push(self.s("lobby.waiting_ready"));
            }
        } else {
            let mut ready = action(
                "lobby.ready",
                "",
                self.s(if l.me.ready {
                    "lobby.unready"
                } else {
                    "lobby.ready"
                }),
            );
            ready.selected = l.me.ready;
            chosen.actions.push(ready);
        }
        panels.push(chosen);
        if host {
            // the host picks from the same lists as the solo song select
            let mut songs = panel("songs");
            songs.heading = self.s("select.heading");
            for (k, s) in self.index.songs.iter().enumerate() {
                let mut a = action("song.pick", k, s.title.clone());
                a.selected = k == l.song;
                songs.actions.push(a);
            }
            panels.push(songs);
            let mut diffs = panel("difficulty");
            diffs.heading = self.s("select.difficulty");
            for (k, d) in DIFFICULTIES.iter().enumerate() {
                let mut a = action("difficulty.pick", d.key, self.s(&format!("difficulty.{}", d.key)));
                a.selected = k == l.difficulty;
                diffs.actions.push(a);
            }
            panels.push(diffs);
            let mut paste = panel("paste");
            paste.heading = self.s("paste.heading");
            paste.lines.push(self.s("paste.host_note"));
            paste
                .actions
                .push(action("online.invite", "", self.s("paste.make")));
            if !self.together.invitation.is_empty() {
                paste.field = Some(Field {
                    id: "offer-out".into(),
                    action: String::new(),
                    label: self.s("paste.your_invitation"),
                    value: self.together.invitation.clone(),
                    max: 0,
                    note: String::new(),
                    multiline: true,
                    readonly: true,
                });
            }
            panels.push(paste);
            if !self.together.invitation.is_empty() {
                let mut back_in = panel("reply");
                back_in.field = Some(Field {
                    id: "answer-in".into(),
                    action: "online.accept".into(),
                    label: self.s("paste.their_reply"),
                    value: String::new(),
                    max: 0,
                    note: String::new(),
                    multiline: true,
                    readonly: false,
                });
                panels.push(back_in);
            }
        } else if !self.together.reply.is_empty() {
            let mut reply = panel("reply");
            reply.heading = self.s("paste.heading");
            reply.field = Some(Field {
                id: "answer-out".into(),
                action: String::new(),
                label: self.s("paste.your_reply"),
                value: self.together.reply.clone(),
                max: 0,
                note: self.s("paste.reply_note"),
                multiline: true,
                readonly: true,
            });
            panels.push(reply);
        }
        let mut leave = panel("leave");
        leave
            .actions
            .push(action("online.leave", "", self.s("lobby.leave")));
        panels.push(leave);
    }

    /// The lobby's scores for the round, best first: (name, score, done).
    pub(crate) fn standings(&self) -> Vec<(String, online::Score)> {
        let Some(l) = &self.together.lobby else {
            return Vec::new();
        };
        let mut v: Vec<(String, online::Score)> = l
            .board
            .values()
            .map(|s| {
                let name = if s.player == l.me.player {
                    l.me.name.clone()
                } else {
                    l.name_of(&s.player)
                };
                (name, s.clone())
            })
            .collect();
        v.sort_by(|a, b| b.1.score.cmp(&a.1.score).then(a.0.cmp(&b.0)));
        v
    }

    /// In a lobby round: post the running score every half second of song
    /// time, and the final one at the end.
    pub(crate) fn post_score(&mut self, now: i64, done: bool) {
        if !self.together.in_round {
            return;
        }
        let Some(s) = &self.session else {
            return;
        };
        if !done && now < self.together.next_post_us {
            return;
        }
        self.together.next_post_us = now + POST_EVERY_US;
        let (score, combo, accuracy) = (s.play.score, s.play.combo, s.play.accuracy());
        if let Some(l) = self.together.lobby.as_mut() {
            l.post(score, combo, accuracy, done);
        }
        self.flush_wire();
    }
}

impl Lobby {
    /// Sends to every admitted peer but one.
    pub fn ledger_send_except(&mut self, except: Peer, m: &Msg) {
        for p in self.admitted_peers() {
            if p != except {
                self.ledger_send(Some(p), m);
            }
        }
    }
}
