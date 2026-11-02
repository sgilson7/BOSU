//! The lobby, core side (PLAN.md decisions 15 and 16; kits/online).
//!
//! The transport (`web/wire/rtc.js`) connects browsers in a star: the host
//! links to each joiner, and joiners do not link to each other. It hands this
//! module three things: a peer arrived, a peer left, a message from a peer.
//! This module answers by leaving text in an outbox. It never sees a
//! connection, so `cargo test` plays whole lobbies between simulated
//! browsers.
//!
//! No rollback (R1.4, a change to D23): each player plays their own copy of
//! the same chart, and only the lobby's state and the running scores cross
//! the wire. The host is the lobby's authority: it admits players, picks the
//! song and difficulty, says Start, and relays each player's scores to the
//! others, stamping who sent them, so nobody can post a score under
//! someone else's name.
//!
//! Every status is a key for a string in `data/`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::tuning::LOBBY_SEATS;

/// Bumped whenever a message changes shape. It is in the room name, so a
/// stale tab and a fresh one still meet and the hello tells the stale one
/// to reload.
pub const PROTOCOL: u32 = 1;

/// Status keys. Each has a string in `data/` under the same key.
pub const CONNECTING: &str = "online.connecting";
pub const WAITING: &str = "online.waiting";
pub const IN_LOBBY: &str = "online.in_lobby";
pub const RELOAD: &str = "online.reload";
pub const GARBLED: &str = "online.garbled";
pub const FULL: &str = "online.full";
pub const HOST_LEFT: &str = "online.host_left";
pub const LEFT_LOBBY: &str = "online.left";

/// The reasons a `Bye` may carry. Anything else reads as `LEFT_LOBBY`.
const REASONS: &[&str] = &[RELOAD, FULL];

/// Letters for a room code: no 0/O, 1/I/L, 2/Z, 5/S or 8/B to misread.
pub const CODE_LETTERS: &[u8] = b"ACDEFGHJKMNPQRTUVWXY34679";
pub const CODE_LEN: usize = 6;

/// The transport's id for another browser.
pub type Peer = u32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub player: String,
    pub name: String,
    pub ready: bool,
    pub host: bool,
}

/// One player's running or final score in a round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score {
    pub round: u32,
    /// Filled in by the host when it relays; ignored when a joiner sends it.
    #[serde(default)]
    pub player: String,
    pub score: i64,
    pub combo: u32,
    pub accuracy: i64,
    pub done: bool,
}

/// Everything that crosses the wire. JSON text, tagged by `t`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Msg {
    /// Each side's first word. `build` names the charts: two browsers with
    /// different charts must not play together.
    Hello {
        protocol: u32,
        build: String,
        player: String,
        name: String,
    },
    Bye {
        why: String,
    },
    /// The host's view of the lobby, sent whenever it changes.
    Lobby {
        members: Vec<Member>,
        song: usize,
        difficulty: usize,
    },
    Ready {
        ready: bool,
    },
    Start {
        round: u32,
        song: usize,
        difficulty: usize,
    },
    Score(Score),
    /// Ledger records (round 5): which ids a browser holds, which it wants,
    /// and the records themselves, as text the ledger reads.
    Have {
        ids: Vec<String>,
    },
    Want {
        ids: Vec<String>,
    },
    Records {
        records: Vec<String>,
    },
}

pub fn encode(m: &Msg) -> String {
    serde_json::to_string(m).unwrap_or_default()
}

pub fn decode(text: &str) -> Option<Msg> {
    serde_json::from_str(text).ok()
}

/// A room code from 64 bits the page drew.
pub fn room_code(seed: u64) -> String {
    let mut v = seed;
    (0..CODE_LEN)
        .map(|_| {
            let c = CODE_LETTERS[(v % CODE_LETTERS.len() as u64) as usize] as char;
            v /= CODE_LETTERS.len() as u64;
            c
        })
        .collect()
}

/// A typed or linked code, upper-cased and checked; `None` if it cannot be one.
pub fn clean_code(text: &str) -> Option<String> {
    let code: String = text.trim().to_ascii_uppercase();
    let ok = code.len() == CODE_LEN && code.bytes().all(|b| CODE_LETTERS.contains(&b));
    ok.then_some(code)
}

/// The room code in an invite link's hash, `#room=CODE`.
pub fn code_from_hash(hash: &str) -> Option<String> {
    clean_code(hash.strip_prefix("#room=")?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Host,
    Joiner,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct PeerState {
    hello: Option<(String, String)>,
    ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lobby {
    pub role: Role,
    pub room: String,
    build: String,
    pub me: Member,
    peers: BTreeMap<Peer, PeerState>,
    /// The members as the host last described them (a joiner's view), or as
    /// the host keeps them.
    pub members: Vec<Member>,
    pub song: usize,
    pub difficulty: usize,
    pub round: u32,
    pub status: &'static str,
    /// The round to start, once Start arrives or the host presses it.
    pub start: Option<(u32, usize, usize)>,
    /// Each player's latest score in the current round.
    pub board: BTreeMap<String, Score>,
    /// Ledger messages for the game to handle, with the peer they came from.
    pub ledger_in: Vec<(Peer, Msg)>,
    /// Peers whose hello was accepted since the game last looked.
    pub admitted_new: Vec<Peer>,
    pub over: bool,
    outbox: Vec<(Peer, String)>,
}

impl Lobby {
    pub fn new(role: Role, room: &str, build: &str, player: &str, name: &str) -> Lobby {
        let me = Member {
            player: player.into(),
            name: name.into(),
            ready: false,
            host: role == Role::Host,
        };
        let mut l = Lobby {
            role,
            room: room.into(),
            build: build.into(),
            me: me.clone(),
            peers: BTreeMap::new(),
            members: vec![me],
            song: 0,
            difficulty: 0,
            round: 0,
            status: if role == Role::Host { WAITING } else { CONNECTING },
            start: None,
            board: BTreeMap::new(),
            ledger_in: Vec::new(),
            admitted_new: Vec::new(),
            over: false,
            outbox: Vec::new(),
        };
        if role == Role::Joiner {
            l.members.clear();
        }
        l
    }

    fn send(&mut self, to: Peer, m: &Msg) {
        self.outbox.push((to, encode(m)));
    }

    fn hello(&self) -> Msg {
        Msg::Hello {
            protocol: PROTOCOL,
            build: self.build.clone(),
            player: self.me.player.clone(),
            name: self.me.name.clone(),
        }
    }

    /// Peers that have said hello.
    pub fn admitted_peers(&self) -> Vec<Peer> {
        self.admitted()
    }

    fn admitted(&self) -> Vec<Peer> {
        self.peers
            .iter()
            .filter(|(_, s)| s.hello.is_some())
            .map(|(p, _)| *p)
            .collect()
    }

    fn send_all(&mut self, m: &Msg, except: Option<Peer>) {
        for p in self.admitted() {
            if Some(p) != except {
                self.send(p, m);
            }
        }
    }

    fn rebuild_members(&mut self) {
        let mut members = vec![self.me.clone()];
        for s in self.peers.values() {
            if let Some((player, name)) = &s.hello {
                members.push(Member {
                    player: player.clone(),
                    name: name.clone(),
                    ready: s.ready,
                    host: false,
                });
            }
        }
        self.members = members;
    }

    fn broadcast_lobby(&mut self) {
        self.rebuild_members();
        let m = Msg::Lobby {
            members: self.members.clone(),
            song: self.song,
            difficulty: self.difficulty,
        };
        self.send_all(&m, None);
    }

    /// The transport reports a new peer.
    pub fn connected(&mut self, peer: Peer) {
        self.peers.entry(peer).or_default();
        let h = self.hello();
        self.send(peer, &h);
    }

    /// The transport reports a peer gone.
    pub fn left(&mut self, peer: Peer) {
        let Some(state) = self.peers.remove(&peer) else {
            return;
        };
        match self.role {
            Role::Host => {
                if state.hello.is_some() {
                    self.broadcast_lobby();
                }
            }
            Role::Joiner => {
                self.status = HOST_LEFT;
                self.over = true;
            }
        }
    }

    /// A message from a peer.
    pub fn message(&mut self, peer: Peer, text: &str) {
        if !self.peers.contains_key(&peer) {
            return; // a browser that never connected
        }
        let Some(m) = decode(text) else {
            self.status = GARBLED;
            return;
        };
        let said_hello = self.peers.get(&peer).map(|s| s.hello.is_some()).unwrap_or(false);
        match m {
            Msg::Hello {
                protocol,
                build,
                player,
                name,
            } => {
                if protocol != PROTOCOL || build != self.build {
                    self.send(peer, &Msg::Bye { why: RELOAD.into() });
                    self.status = RELOAD;
                    if self.role == Role::Joiner {
                        self.over = true;
                    }
                    return;
                }
                if said_hello {
                    return; // a repeat
                }
                if self.role == Role::Host && self.admitted().len() + 1 >= LOBBY_SEATS {
                    self.send(peer, &Msg::Bye { why: FULL.into() });
                    return;
                }
                let name = crate::game::clean_name(&name);
                if let Some(s) = self.peers.get_mut(&peer) {
                    s.hello = Some((player, name));
                }
                self.admitted_new.push(peer);
                match self.role {
                    Role::Host => {
                        self.status = IN_LOBBY;
                        self.broadcast_lobby();
                    }
                    Role::Joiner => self.status = IN_LOBBY,
                }
            }
            Msg::Bye { why } => {
                let why = REASONS.iter().find(|r| **r == why).copied().unwrap_or(LEFT_LOBBY);
                if self.role == Role::Joiner {
                    self.status = why;
                    self.over = true;
                } else {
                    self.peers.remove(&peer);
                    self.broadcast_lobby();
                }
            }
            _ if !said_hello => self.status = GARBLED,
            Msg::Lobby {
                members,
                song,
                difficulty,
            } if self.role == Role::Joiner => {
                self.me.ready = members
                    .iter()
                    .find(|m| m.player == self.me.player)
                    .map(|m| m.ready)
                    .unwrap_or(false);
                self.members = members;
                self.song = song;
                self.difficulty = difficulty;
            }
            Msg::Ready { ready } if self.role == Role::Host => {
                if let Some(s) = self.peers.get_mut(&peer) {
                    s.ready = ready;
                }
                self.broadcast_lobby();
            }
            Msg::Start {
                round,
                song,
                difficulty,
            } if self.role == Role::Joiner => {
                self.round = round;
                self.song = song;
                self.difficulty = difficulty;
                self.board.clear();
                self.start = Some((round, song, difficulty));
            }
            Msg::Score(mut s) => {
                if s.round != self.round {
                    return;
                }
                if self.role == Role::Host {
                    // the sender is who said hello on this link, whatever it claims
                    s.player = self.peers[&peer]
                        .hello
                        .as_ref()
                        .map(|h| h.0.clone())
                        .unwrap_or_default();
                    let relay = Msg::Score(s.clone());
                    self.send_all(&relay, Some(peer));
                }
                self.board.insert(s.player.clone(), s);
            }
            m @ (Msg::Have { .. } | Msg::Want { .. } | Msg::Records { .. }) => self.ledger_in.push((peer, m)),
            _ => self.status = GARBLED,
        }
    }

    /// The host picks the song and difficulty; everyone is no longer ready.
    pub fn pick(&mut self, song: usize, difficulty: usize) {
        if self.role != Role::Host {
            return;
        }
        self.song = song;
        self.difficulty = difficulty;
        for s in self.peers.values_mut() {
            s.ready = false;
        }
        self.broadcast_lobby();
    }

    /// A joiner says it is ready, or not.
    pub fn set_ready(&mut self, ready: bool) {
        if self.role != Role::Joiner {
            return;
        }
        self.me.ready = ready;
        for p in self.admitted() {
            self.send(p, &Msg::Ready { ready });
        }
    }

    /// Whether the host may start: every joiner is ready.
    pub fn can_start(&self) -> bool {
        self.role == Role::Host && self.peers.values().filter(|s| s.hello.is_some()).all(|s| s.ready)
    }

    /// The host starts a round.
    pub fn begin(&mut self) -> bool {
        if !self.can_start() {
            return false;
        }
        self.round += 1;
        self.board.clear();
        for s in self.peers.values_mut() {
            s.ready = false;
        }
        let m = Msg::Start {
            round: self.round,
            song: self.song,
            difficulty: self.difficulty,
        };
        self.send_all(&m, None);
        self.start = Some((self.round, self.song, self.difficulty));
        self.rebuild_members();
        true
    }

    /// This player's score, sent to the others (through the host).
    pub fn post(&mut self, score: i64, combo: u32, accuracy: i64, done: bool) {
        let s = Score {
            round: self.round,
            player: self.me.player.clone(),
            score,
            combo,
            accuracy,
            done,
        };
        self.board.insert(s.player.clone(), s.clone());
        self.send_all(&Msg::Score(s), None);
    }

    /// Sends a ledger message to one peer, or to all.
    pub fn ledger_send(&mut self, to: Option<Peer>, m: &Msg) {
        match to {
            Some(p) => self.send(p, m),
            None => self.send_all(m, None),
        }
    }

    pub fn leave(&mut self) {
        for p in self.admitted() {
            self.send(
                p,
                &Msg::Bye {
                    why: LEFT_LOBBY.into(),
                },
            );
        }
        self.over = true;
    }

    pub fn take_outbox(&mut self) -> Vec<(Peer, String)> {
        std::mem::take(&mut self.outbox)
    }

    /// A member's name by player id.
    pub fn name_of(&self, player: &str) -> String {
        self.members
            .iter()
            .find(|m| m.player == player)
            .map(|m| m.name.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A host and joiners joined by perfect wires: each pump delivers every
    /// outbox. Peer ids: the host sees joiner k as peer k + 1; each joiner
    /// sees the host as peer 0.
    pub struct Net {
        pub host: Lobby,
        pub joiners: Vec<Lobby>,
    }

    impl Net {
        pub fn new(n: usize, build: &str) -> Net {
            let host = Lobby::new(Role::Host, "ROOM", "b1", "h", "Host");
            let joiners = (0..n)
                .map(|k| Lobby::new(Role::Joiner, "ROOM", build, &format!("j{k}"), &format!("J{k}")))
                .collect();
            let mut wire = Net { host, joiners };
            for k in 0..n {
                wire.host.connected(k as Peer + 1);
                wire.joiners[k].connected(0);
            }
            wire.pump();
            wire
        }

        /// Every joiner presses Ready, and the host starts.
        pub fn start(&mut self) {
            for j in &mut self.joiners {
                j.set_ready(true);
            }
            self.pump();
            assert!(self.host.begin());
            self.pump();
        }

        pub fn pump(&mut self) {
            for _ in 0..8 {
                let from_host = self.host.take_outbox();
                for (to, text) in from_host {
                    if let Some(j) = self.joiners.get_mut(to as usize - 1) {
                        j.message(0, &text);
                    }
                }
                for k in 0..self.joiners.len() {
                    for (_, text) in self.joiners[k].take_outbox() {
                        self.host.message(k as Peer + 1, &text);
                    }
                }
            }
        }
    }

    #[test]
    fn every_message_round_trips() {
        let all = vec![
            Msg::Hello {
                protocol: 1,
                build: "b".into(),
                player: "p".into(),
                name: "Ana".into(),
            },
            Msg::Bye { why: FULL.into() },
            Msg::Lobby {
                members: vec![Member {
                    player: "p".into(),
                    name: "A".into(),
                    ready: true,
                    host: true,
                }],
                song: 2,
                difficulty: 3,
            },
            Msg::Ready { ready: true },
            Msg::Start {
                round: 4,
                song: 1,
                difficulty: 0,
            },
            Msg::Score(Score {
                round: 1,
                player: "p".into(),
                score: 9,
                combo: 2,
                accuracy: 9000,
                done: false,
            }),
            Msg::Have {
                ids: vec!["a".into()],
            },
            Msg::Want { ids: vec![] },
            Msg::Records {
                records: vec!["{}".into()],
            },
        ];
        for m in all {
            assert_eq!(decode(&encode(&m)), Some(m.clone()));
        }
    }

    #[test]
    fn room_codes_use_only_unmistakable_letters_and_read_back() {
        let code = room_code(0x1234_5678_9abc);
        assert_eq!(code.len(), CODE_LEN);
        assert_eq!(clean_code(&code.to_lowercase()), Some(code.clone()));
        assert_eq!(code_from_hash(&format!("#room={code}")), Some(code));
        assert_eq!(clean_code("ABCDE1"), None, "B and 1 are not code letters");
        assert_eq!(clean_code("ACD"), None);
        assert_eq!(code_from_hash("#roam=ACDEFG"), None);
        assert_ne!(room_code(1), room_code(2));
    }

    #[test]
    fn a_lobby_of_eight_agrees_on_the_chart() {
        let mut wire = Net::new(7, "b1");
        assert_eq!(wire.host.members.len(), 8);
        wire.host.pick(3, 4);
        wire.pump();
        for j in &mut wire.joiners {
            assert_eq!((j.song, j.difficulty, j.members.len()), (3, 4, 8));
            j.set_ready(true);
        }
        assert!(!wire.host.can_start(), "nobody's Ready has arrived yet");
        wire.pump();
        assert!(wire.host.begin());
        wire.pump();
        for j in &wire.joiners {
            assert_eq!(j.start, Some((1, 3, 4)));
        }
        assert_eq!(wire.host.start, Some((1, 3, 4)));
    }

    #[test]
    fn the_host_waits_for_every_ready() {
        let mut wire = Net::new(2, "b1");
        wire.joiners[0].set_ready(true);
        wire.pump();
        assert!(!wire.host.can_start());
        assert!(!wire.host.begin());
        wire.joiners[1].set_ready(true);
        wire.pump();
        assert!(wire.host.can_start());
        // a new pick clears every Ready
        wire.host.pick(1, 1);
        wire.pump();
        assert!(!wire.host.can_start());
    }

    #[test]
    fn a_ninth_is_told_the_lobby_is_full() {
        let wire = Net::new(8, "b1");
        assert_eq!(wire.host.members.len(), 8);
        assert_eq!(wire.joiners[7].status, FULL);
        assert!(wire.joiners[7].over);
    }

    #[test]
    fn another_build_or_protocol_is_turned_away_with_a_reason() {
        let wire = Net::new(1, "b2");
        assert_eq!(wire.host.status, RELOAD);
        assert_eq!(wire.joiners[0].status, RELOAD);
        assert_eq!(wire.host.members.len(), 1);
        let mut l = Lobby::new(Role::Host, "R", "b1", "h", "H");
        l.connected(1);
        l.message(
            1,
            r#"{"t":"hello","protocol":99,"build":"b1","player":"x","name":"X"}"#,
        );
        assert_eq!(l.status, RELOAD);
    }

    #[test]
    fn garbage_and_another_build_change_only_the_status() {
        let mut wire = Net::new(1, "b1");
        wire.host.pick(2, 2);
        wire.pump();
        let before = (
            wire.host.members.clone(),
            wire.host.song,
            wire.host.round,
            wire.host.board.clone(),
        );
        for junk in [
            "not json",
            "{}",
            r#"{"t":"nope"}"#,
            r#"{"t":"start","round":1,"song":0,"difficulty":0}"#,
            r#"{"t":"lobby","members":[],"song":0,"difficulty":0}"#,
            r#"{"t":"score","round":7,"score":1,"combo":1,"accuracy":1,"done":false}"#,
        ] {
            wire.host.message(1, junk);
        }
        assert_eq!(wire.host.status, GARBLED);
        assert_eq!(
            before,
            (
                wire.host.members.clone(),
                wire.host.song,
                wire.host.round,
                wire.host.board.clone()
            )
        );
        // a stranger is ignored outright
        let mut l = wire.host.clone();
        l.message(42, "not json");
        assert_eq!(l, wire.host);
    }

    #[test]
    fn a_peer_that_has_not_said_hello_changes_nothing() {
        let mut j = Lobby::new(Role::Joiner, "R", "b1", "j", "J");
        j.connected(0);
        j.message(0, r#"{"t":"start","round":1,"song":2,"difficulty":3}"#);
        assert_eq!(j.start, None);
        assert_eq!(j.status, GARBLED);
        let mut h = Lobby::new(Role::Host, "R", "b1", "h", "H");
        h.connected(5);
        h.message(5, r#"{"t":"have","ids":["x"]}"#);
        assert!(h.ledger_in.is_empty());
        assert_eq!(h.status, GARBLED);
    }

    #[test]
    fn a_joiner_cannot_post_a_score_as_someone_else() {
        let mut wire = Net::new(2, "b1");
        wire.start();
        wire.host.message(
            1,
            r#"{"t":"score","round":1,"player":"j1","score":999999,"combo":1,"accuracy":1,"done":false}"#,
        );
        wire.pump();
        assert_eq!(
            wire.host.board["j0"].score, 999_999,
            "stamped with the sender's own id"
        );
        assert!(!wire.host.board.contains_key("j1"));
        assert_eq!(wire.joiners[1].board["j0"].score, 999_999);
    }

    #[test]
    fn scores_reach_every_player_through_the_host() {
        let mut wire = Net::new(2, "b1");
        wire.start();
        wire.joiners[0].post(1200, 12, 9800, false);
        wire.joiners[1].post(800, 3, 9100, false);
        wire.host.post(1500, 15, 9900, true);
        wire.pump();
        for l in [&wire.host, &wire.joiners[0], &wire.joiners[1]] {
            assert_eq!(l.board.len(), 3);
            assert_eq!(l.board["j0"].score, 1200);
            assert_eq!(l.board["j1"].score, 800);
            assert!(l.board["h"].done);
        }
    }

    #[test]
    fn the_host_leaving_ends_the_lobby_for_joiners_and_a_joiner_leaving_frees_a_seat() {
        let mut wire = Net::new(2, "b1");
        wire.host.left(1);
        wire.pump();
        assert_eq!(wire.host.members.len(), 2);
        assert_eq!(wire.joiners[1].members.len(), 2);
        wire.joiners[1].left(0);
        assert_eq!(wire.joiners[1].status, HOST_LEFT);
        assert!(wire.joiners[1].over);
    }
}
