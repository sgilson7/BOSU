//! The ledger (T2; PLAN.md decisions 17 to 20): every player's best play
//! per chart, carried from browser to browser through lobbies, so the
//! leaderboard grows with each game played together.
//!
//! A record is one finished play. Its player is an Ed25519 public key, made
//! once per browser from 32 random bytes the page drew and kept in the save;
//! the record is signed with it (BO-Q13: B, T3). Its id is the SHA-256 of
//! the signed text. The replay (every press, and the moves that matter) is
//! outside the signed text but its SHA-256 is inside, so a record keeps its
//! signature when its replay is dropped to save room.
//!
//! A record is kept only if its signature verifies, and, when it carries a
//! replay and this build has its chart, only if replaying the presses gives
//! the score it claims. That stops a typed-in score and a score under
//! someone else's key. It does not stop a bot that plays perfectly.
//!
//! Merging only adds. For each player and chart the better record stays
//! (higher score; equal scores, the smaller id), so merging in any order
//! gives the same ledger.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::chart::{Chart, Target};
use crate::play::{replay, tick_times, Input};
use crate::tuning::DIFFICULTIES;

/// Records kept per chart, best first.
pub const MAX_PER_CHART: usize = 200;
/// Replays kept per chart, best first (BO-Q14: A).
pub const REPLAYS_PER_CHART: usize = 100;
/// All replays together stay under this many bytes of text, the best kept
/// first, so the save fits the browser's storage (R5.2).
pub const REPLAY_BUDGET: usize = 1_500_000;
/// Records in one message, and ids in one message.
pub const RECORDS_PER_MESSAGE: usize = 8;
pub const IDS_PER_MESSAGE: usize = 400;

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|k| u8::from_str_radix(text.get(k..k + 2)?, 16).ok())
        .collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// A player's key, from the 32 bytes kept in the save.
#[derive(Debug, Clone)]
pub struct Key(SigningKey);

impl Key {
    pub fn from_hex(secret: &str) -> Option<Key> {
        let bytes: [u8; 32] = unhex(secret)?.try_into().ok()?;
        Some(Key(SigningKey::from_bytes(&bytes)))
    }

    /// The player id others see: the public key, in hex.
    pub fn player(&self) -> String {
        hex(self.0.verifying_key().as_bytes())
    }
}

/// What a record says, signed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub player: String,
    pub name: String,
    pub song: String,
    pub difficulty: usize,
    /// The chart's hash from the index: records for another version of a
    /// chart never compete with this one's.
    pub chart: String,
    pub score: i64,
    pub accuracy: i64,
    pub max_combo: u32,
    pub counts: [u32; 4],
    pub replay_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub body: Body,
    pub sig: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub replay: String,
}

impl Record {
    fn signed_text(body: &Body) -> String {
        serde_json::to_string(body).unwrap_or_default()
    }

    pub fn id(&self) -> String {
        sha256_hex(format!("{}{}", Self::signed_text(&self.body), self.sig).as_bytes())
    }

    /// The board this record competes on.
    pub fn board(&self) -> String {
        format!(
            "{}:{}:{}",
            self.body.song,
            DIFFICULTIES[self.body.difficulty.min(4)].key,
            self.body.chart
        )
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Makes and signs a record of a finished play.
pub fn make(key: &Key, name: &str, song: &str, chart_hash: &str, p: &crate::play::Play) -> Record {
    let text = encode_replay(&compact(&p.chart, &p.replay));
    let body = Body {
        player: key.player(),
        name: crate::game::clean_name(name),
        song: song.into(),
        difficulty: p.chart.difficulty,
        chart: chart_hash.into(),
        score: p.score,
        accuracy: p.accuracy(),
        max_combo: p.max_combo,
        counts: p.counts,
        replay_hash: sha256_hex(text.as_bytes()),
    };
    let sig = hex(&key.0.sign(Record::signed_text(&body).as_bytes()).to_bytes());
    Record {
        body,
        sig,
        replay: text,
    }
}

/// Why a record was turned away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Unreadable,
    BadSignature,
    BadReplay,
}

/// How far this browser could check a record it kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Checked {
    /// This browser replayed the presses and got the same score.
    Replayed,
    /// The signature holds; the replay was dropped before it reached this
    /// browser, so another browser checked it.
    Signed,
    /// The signature holds; this build lacks the chart, so the replay could
    /// not be run.
    OtherChart,
}

fn verify_signature(r: &Record) -> bool {
    let Some(pk) = unhex(&r.body.player).and_then(|b| <[u8; 32]>::try_from(b).ok()) else {
        return false;
    };
    let Some(sig) = unhex(&r.sig).and_then(|b| <[u8; 64]>::try_from(b).ok()) else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&pk) else {
        return false;
    };
    key.verify(
        Record::signed_text(&r.body).as_bytes(),
        &Signature::from_bytes(&sig),
    )
    .is_ok()
}

/// Checks a record. `chart` is this build's chart for the record's board,
/// if it has one.
pub fn check(r: &Record, chart: Option<&Chart>) -> Result<Checked, Refused> {
    let b = &r.body;
    let sane = b.difficulty < DIFFICULTIES.len()
        && b.name == crate::game::clean_name(&b.name)
        && b.player.len() == 64
        && r.sig.len() == 128
        && b.replay_hash.len() == 64;
    if !sane {
        return Err(Refused::Unreadable);
    }
    if !verify_signature(r) {
        return Err(Refused::BadSignature);
    }
    if r.replay.is_empty() {
        return Ok(Checked::Signed);
    }
    if sha256_hex(r.replay.as_bytes()) != b.replay_hash {
        return Err(Refused::BadReplay);
    }
    let Some(chart) = chart else {
        return Ok(Checked::OtherChart);
    };
    let inputs = decode_replay(&r.replay).ok_or(Refused::BadReplay)?;
    let p = replay(chart.clone(), &inputs);
    let same = p.score == b.score
        && p.accuracy() == b.accuracy
        && p.max_combo == b.max_combo
        && p.counts == b.counts;
    if same {
        Ok(Checked::Replayed)
    } else {
        Err(Refused::BadReplay)
    }
}

// ---- the replay, small ------------------------------------------------------

/// The inputs a judgement can depend on: every press and release, the
/// cursor just before each press, before each release during a hold, and at
/// each hold tick and end. Times are made non-decreasing first, as `Play`
/// takes them. Replaying the result gives the same play as the whole list
/// (`a_compact_replay_plays_the_same_as_the_whole`).
pub fn compact(chart: &Chart, inputs: &[Input]) -> Vec<Input> {
    let d = DIFFICULTIES[chart.difficulty.min(4)];
    let mut now = i64::MIN;
    let timed: Vec<Input> = inputs
        .iter()
        .map(|i| {
            now = now.max(i.at());
            match *i {
                Input::Move { x, y, .. } => Input::Move { t: now, x, y },
                Input::Down { button, .. } => Input::Down { t: now, button },
                Input::Up { button, .. } => Input::Up { t: now, button },
            }
        })
        .collect();
    let in_hold = |t: i64| {
        chart
            .targets
            .iter()
            .any(|tg: &Target| tg.hold.is_some() && t >= tg.t - d.good_us && t <= tg.end() + d.good_us)
    };
    let mut events: Vec<i64> = Vec::new();
    for tg in &chart.targets {
        if tg.hold.is_some() {
            events.extend(tick_times(tg, chart.beat_us, d.good_us));
            events.push(tg.end());
        }
    }
    events.sort();
    let mut keep = vec![false; timed.len()];
    let mut last_move: Option<usize> = None;
    let mut next_event = 0;
    for (k, i) in timed.iter().enumerate() {
        // hold events that pass before this input read the cursor as it is
        while next_event < events.len() && events[next_event] < i.at() {
            if let Some(m) = last_move {
                keep[m] = true;
            }
            next_event += 1;
        }
        match i {
            Input::Move { .. } => last_move = Some(k),
            Input::Down { .. } => {
                keep[k] = true;
                if let Some(m) = last_move {
                    keep[m] = true;
                }
            }
            Input::Up { t, .. } => {
                keep[k] = true;
                if in_hold(*t) {
                    if let Some(m) = last_move {
                        keep[m] = true;
                    }
                }
            }
        }
    }
    if next_event < events.len() {
        if let Some(m) = last_move {
            keep[m] = true;
        }
    }
    timed
        .into_iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(i, _)| i)
        .collect()
}

fn b36(mut v: u64) -> String {
    const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if v == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while v > 0 {
        out.push(D[(v % 36) as usize]);
        v /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Inputs as text: `m<dt>,<x>,<y>`, `d<dt>,<button>` or `u<dt>,<button>`,
/// separated by `;`, each time in base-36 microseconds after the one
/// before (the first after 0, or `-` and its distance before 0).
pub fn encode_replay(inputs: &[Input]) -> String {
    let mut last = 0i64;
    let mut parts = Vec::with_capacity(inputs.len());
    for i in inputs {
        let t = i.at();
        let dt = t - last;
        last = t;
        let dts = if dt < 0 {
            format!("-{}", b36(dt.unsigned_abs()))
        } else {
            b36(dt as u64)
        };
        parts.push(match *i {
            Input::Move { x, y, .. } => format!("m{dts},{x},{y}"),
            Input::Down { button, .. } => format!("d{dts},{button}"),
            Input::Up { button, .. } => format!("u{dts},{button}"),
        });
    }
    parts.join(";")
}

pub fn decode_replay(text: &str) -> Option<Vec<Input>> {
    let mut last = 0i64;
    let mut out = Vec::new();
    if text.is_empty() {
        return Some(out);
    }
    for part in text.split(';') {
        let kind = part.get(..1)?;
        let mut fields = part.get(1..)?.split(',');
        let dts = fields.next()?;
        let dt = match dts.strip_prefix('-') {
            Some(n) => -(i64::from_str_radix(n, 36).ok()?),
            None => i64::from_str_radix(dts, 36).ok()?,
        };
        let t = last.checked_add(dt)?;
        last = t;
        let mut num = || -> Option<i64> { fields.next()?.parse().ok() };
        out.push(match kind {
            "m" => Input::Move {
                t,
                x: num()? as i32,
                y: num()? as i32,
            },
            "d" => Input::Down {
                t,
                button: u8::try_from(num()?).ok()?,
            },
            "u" => Input::Up {
                t,
                button: u8::try_from(num()?).ok()?,
            },
            _ => return None,
        });
    }
    Some(out)
}

// ---- the ledger ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub record: Record,
    pub checked: Checked,
}

/// The best record per player per board, by board then player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub boards: BTreeMap<String, BTreeMap<String, Entry>>,
}

/// Whether record `a` beats record `b` on the same board.
fn beats(a: &Record, b: &Record) -> bool {
    (a.body.score, std::cmp::Reverse(a.id())) > (b.body.score, std::cmp::Reverse(b.id()))
}

impl Ledger {
    pub fn ids(&self) -> Vec<String> {
        self.boards
            .values()
            .flat_map(|b| b.values().map(|e| e.record.id()))
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&Record> {
        self.boards
            .values()
            .flat_map(|b| b.values())
            .map(|e| &e.record)
            .find(|r| r.id() == id)
    }

    /// Adds a record already checked. Returns whether the ledger changed.
    pub fn add(&mut self, r: Record, checked: Checked) -> bool {
        let board = self.boards.entry(r.board()).or_default();
        let player = r.body.player.clone();
        match board.get_mut(&player) {
            Some(e) if e.record.id() == r.id() => {
                // the same play again: keep the replay if this copy has one
                if e.record.replay.is_empty() && !r.replay.is_empty() {
                    e.record.replay = r.replay;
                    e.checked = checked;
                    return true;
                }
                false
            }
            Some(e) if !beats(&r, &e.record) => false,
            _ => {
                board.insert(player, Entry { record: r, checked });
                true
            }
        }
    }

    /// Checks then adds. `chart_for` finds this build's chart for a board.
    pub fn receive(
        &mut self,
        text: &str,
        chart_for: &dyn Fn(&Record) -> Option<Chart>,
    ) -> Result<bool, Refused> {
        let r: Record = serde_json::from_str(text).map_err(|_| Refused::Unreadable)?;
        let chart = chart_for(&r);
        let checked = check(&r, chart.as_ref())?;
        Ok(self.add(r, checked))
    }

    /// One board, best first.
    pub fn board(&self, board: &str) -> Vec<&Entry> {
        let mut v: Vec<&Entry> = self
            .boards
            .get(board)
            .map(|b| b.values().collect())
            .unwrap_or_default();
        v.sort_by(|a, b| {
            b.record
                .body
                .score
                .cmp(&a.record.body.score)
                .then_with(|| a.record.id().cmp(&b.record.id()))
        });
        v
    }

    /// Keeps the best `MAX_PER_CHART` records a board, and replays for the
    /// best `REPLAYS_PER_CHART`, all within `REPLAY_BUDGET`: ranks are taken
    /// in turn across boards, best first, until the budget runs out.
    pub fn trim(&mut self) {
        let names: Vec<String> = self.boards.keys().cloned().collect();
        let mut ranked: Vec<Vec<(String, String)>> = Vec::new();
        for name in &names {
            let order: Vec<(String, String)> = self
                .board(name)
                .iter()
                .map(|e| (name.clone(), e.record.body.player.clone()))
                .collect();
            let board = self.boards.get_mut(name).expect("a board just listed");
            for (_, player) in order.iter().skip(MAX_PER_CHART) {
                board.remove(player);
            }
            ranked.push(order.into_iter().take(MAX_PER_CHART).collect());
        }
        let mut used = 0usize;
        let deepest = ranked.iter().map(Vec::len).max().unwrap_or(0);
        for rank in 0..deepest {
            for list in &ranked {
                let Some((name, player)) = list.get(rank) else {
                    continue;
                };
                let e = self
                    .boards
                    .get_mut(name)
                    .and_then(|b| b.get_mut(player))
                    .expect("ranked entry");
                let size = e.record.replay.len();
                if rank < REPLAYS_PER_CHART && used + size <= REPLAY_BUDGET {
                    used += size;
                } else if size > 0 {
                    e.record.replay.clear();
                    if e.checked == Checked::Replayed {
                        e.checked = Checked::Signed;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::tests::{chart, tap};
    use crate::chart::Hold;
    use crate::play::{perfect_inputs, Play};

    pub fn key(n: u8) -> Key {
        Key::from_hex(&hex(&[n; 32])).expect("key")
    }

    fn test_chart() -> Chart {
        let mut c = chart(
            0,
            (1..=6)
                .map(|k| tap(k * 1000, 400 + 100 * k as i32, 450, k as u32))
                .collect(),
        );
        c.targets[2].hold = Some(Hold {
            end: 3_800_000,
            path: vec![[700, 450], [900, 450]],
        });
        c
    }

    fn played(c: &Chart, inputs: &[Input]) -> Play {
        replay(c.clone(), inputs)
    }

    /// A play with every press `late` microseconds late and some extra moves.
    fn sloppy(c: &Chart, late: i64) -> Vec<Input> {
        let mut out = Vec::new();
        for i in perfect_inputs(c) {
            let t = i.at() + late;
            out.push(Input::Move {
                t: t - 3000,
                x: 1,
                y: 1,
            });
            out.push(match i {
                Input::Move { x, y, .. } => Input::Move { t, x: x + 5, y },
                Input::Down { button, .. } => Input::Down { t, button },
                Input::Up { button, .. } => Input::Up { t, button },
            });
        }
        out
    }

    fn record(n: u8, name: &str, inputs: &[Input]) -> Record {
        let c = test_chart();
        make(&key(n), name, "test", "c1", &played(&c, inputs))
    }

    fn chart_for(_: &Record) -> Option<Chart> {
        Some(test_chart())
    }

    #[test]
    fn sha256_matches_the_fips_180_4_vectors() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn a_compact_replay_plays_the_same_as_the_whole() {
        let c = test_chart();
        for late in [0, 30_000, -70_000, 120_000] {
            let whole = sloppy(&c, late);
            let small = compact(&c, &whole);
            assert!(small.len() < whole.len());
            let (a, b) = (played(&c, &whole), played(&c, &small));
            assert_eq!(
                (a.score, a.counts, a.max_combo, a.ticks_hit),
                (b.score, b.counts, b.max_combo, b.ticks_hit),
                "late {late}"
            );
            assert_eq!(decode_replay(&encode_replay(&small)), Some(small));
        }
    }

    #[test]
    fn a_record_round_trips_and_keeps_its_id() {
        let r = record(1, "Ana", &perfect_inputs(&test_chart()));
        let back: Record = serde_json::from_str(&r.to_text()).unwrap();
        assert_eq!(back.id(), r.id());
        assert_eq!(check(&back, Some(&test_chart())), Ok(Checked::Replayed));
        assert_eq!(r.body.player.len(), 64);
    }

    #[test]
    fn a_record_with_a_bad_signature_is_refused() {
        let mut r = record(1, "Ana", &perfect_inputs(&test_chart()));
        r.body.name = "Bea".into();
        assert_eq!(check(&r, Some(&test_chart())), Err(Refused::BadSignature));
        // Bea's key cannot sign as Ana's
        let mut r2 = record(2, "Bea", &perfect_inputs(&test_chart()));
        r2.body.player = key(1).player();
        assert_eq!(check(&r2, Some(&test_chart())), Err(Refused::BadSignature));
    }

    #[test]
    fn a_record_whose_score_does_not_match_its_replay_is_refused() {
        let c = test_chart();
        // signed honestly by its own key, but the replay shows a worse play
        let good = played(&c, &perfect_inputs(&c));
        let mut r = make(&key(3), "Cy", "test", "c1", &good);
        let worse = encode_replay(&compact(&c, &sloppy(&c, 120_000)));
        r.replay = worse.clone();
        r.body.replay_hash = sha256_hex(worse.as_bytes());
        r.sig = hex(&key(3).0.sign(Record::signed_text(&r.body).as_bytes()).to_bytes());
        assert_eq!(check(&r, Some(&c)), Err(Refused::BadReplay));
        // a replay swapped after signing fails its hash
        let mut r = make(&key(3), "Cy", "test", "c1", &good);
        r.replay = worse;
        assert_eq!(check(&r, Some(&c)), Err(Refused::BadReplay));
        // ... even where this build lacks the chart to replay it on
        assert_eq!(check(&r, None), Err(Refused::BadReplay));
    }

    #[test]
    fn a_record_for_a_chart_this_build_lacks_is_kept_unchecked_and_marked() {
        let r = record(1, "Ana", &perfect_inputs(&test_chart()));
        assert_eq!(check(&r, None), Ok(Checked::OtherChart));
        let mut stripped = r.clone();
        stripped.replay.clear();
        assert_eq!(check(&stripped, Some(&test_chart())), Ok(Checked::Signed));
        assert_eq!(stripped.id(), r.id(), "dropping the replay keeps the id");
    }

    #[test]
    fn a_worse_record_never_replaces_a_better_one() {
        let c = test_chart();
        let best = record(1, "Ana", &perfect_inputs(&c));
        let worse = record(1, "Ana", &sloppy(&c, 120_000));
        assert!(worse.body.score < best.body.score);
        let mut l = Ledger::default();
        l.add(best.clone(), Checked::Replayed);
        assert!(!l.add(worse.clone(), Checked::Replayed));
        assert_eq!(l.board(&best.board())[0].record, best);
        let mut l = Ledger::default();
        l.add(worse, Checked::Replayed);
        assert!(l.add(best.clone(), Checked::Replayed));
        assert_eq!(l.board(&best.board()).len(), 1);
        assert_eq!(l.board(&best.board())[0].record, best);
    }

    #[test]
    fn merging_in_any_order_gives_the_same_ledger() {
        let c = test_chart();
        let texts: Vec<String> = [
            record(1, "Ana", &perfect_inputs(&c)),
            record(1, "Ana", &sloppy(&c, 60_000)),
            record(2, "Ben", &sloppy(&c, 30_000)),
            record(2, "Ben", &sloppy(&c, -70_000)),
            record(3, "Cy", &sloppy(&c, 0)),
        ]
        .iter()
        .map(Record::to_text)
        .collect();
        let merged = |order: &[usize]| {
            let mut l = Ledger::default();
            for &k in order {
                l.receive(&texts[k], &chart_for).unwrap();
            }
            l
        };
        let first = merged(&[0, 1, 2, 3, 4]);
        for order in [[4, 3, 2, 1, 0], [2, 0, 4, 1, 3], [1, 3, 0, 4, 2]] {
            assert_eq!(merged(&order), first);
        }
        assert_eq!(first.boards.values().next().unwrap().len(), 3);
    }

    #[test]
    fn ana_s_score_reaches_cy_through_ben() {
        let c = test_chart();
        let mut ana = Ledger::default();
        let mut ben = Ledger::default();
        let mut cy = Ledger::default();
        ana.add(record(1, "Ana", &perfect_inputs(&c)), Checked::Replayed);
        // Ana and Ben play together: each takes what it lacks
        let send = |from: &Ledger, to: &mut Ledger| {
            for id in from.ids() {
                if !to.ids().contains(&id) {
                    to.receive(&from.get(&id).unwrap().to_text(), &chart_for).unwrap();
                }
            }
        };
        send(&ana, &mut ben);
        send(&ben, &mut ana);
        // later Ben and Cy play; Cy never meets Ana
        send(&ben, &mut cy);
        let names: Vec<String> = cy
            .boards
            .values()
            .flat_map(|b| b.values().map(|e| e.record.body.name.clone()))
            .collect();
        assert_eq!(names, ["Ana"]);
        assert_eq!(cy.ids(), ana.ids());
    }

    #[test]
    fn the_ledger_drops_replays_below_the_top_100_and_records_below_200() {
        let c = test_chart();
        let mut l = Ledger::default();
        for n in 0..210u32 {
            let k = Key::from_hex(&hex(&[
                (n % 251) as u8 + 1,
                (n / 251) as u8 + 7,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
                9,
            ]))
            .unwrap();
            // each a little later than the last, so scores differ
            let p = played(&c, &sloppy(&c, n as i64 * 600));
            l.add(make(&k, "P", "test", "c1", &p), Checked::Replayed);
        }
        l.trim();
        let board = l.board(&l.boards.keys().next().unwrap().clone());
        assert_eq!(board.len(), MAX_PER_CHART);
        let with_replay = board.iter().filter(|e| !e.record.replay.is_empty()).count();
        assert_eq!(with_replay, REPLAYS_PER_CHART);
        assert!(
            board[..REPLAYS_PER_CHART]
                .iter()
                .all(|e| !e.record.replay.is_empty()),
            "the best keep theirs"
        );
        assert!(board[REPLAYS_PER_CHART..]
            .iter()
            .all(|e| e.checked == Checked::Signed));
    }

    #[test]
    fn hex_reads_back_and_refuses_odd_text() {
        assert_eq!(unhex(&hex(&[0, 15, 255])), Some(vec![0, 15, 255]));
        assert_eq!(unhex("abc"), None);
        assert_eq!(unhex("zz"), None);
    }
}
