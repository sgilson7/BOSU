//! The save the online browser check gives its host: a version 3 save
//! holding one signed record, "Ana" playing placeholder-1 on Easy with
//! every press perfect. tests/browser/online.py checks that the record
//! reaches the joiner over the wire and shows on its leaderboard.
//!
//! Signing is deterministic, so the fixture is the same each time; this
//! test writes it when it is missing and fails when it is stale.

use std::fs;
use std::path::Path;

use tool_core::catalog;
use tool_core::chart::Chart;
use tool_core::ledger::{self, Checked, Key, Ledger};
use tool_core::play::{perfect_inputs, replay};

#[test]
fn the_browser_ledger_fixture_is_current() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let index =
        catalog::parse_index(&fs::read_to_string(root.join("data/charts/index.json")).unwrap()).unwrap();
    let song = &index.songs[0];
    let entry = &song.charts[0];
    let text = fs::read_to_string(root.join("data").join(&entry.file)).unwrap();
    let chart = Chart::parse(&text).unwrap();
    let key = Key::from_hex(&ledger::hex(&[5; 32])).unwrap();
    let p = replay(chart.clone(), &perfect_inputs(&chart));
    let r = ledger::make(&key, "Ana", &song.id, &entry.hash, &p);
    assert_eq!(ledger::check(&r, Some(&chart)), Ok(Checked::Replayed));
    let mut l = Ledger::default();
    l.add(r, Checked::Replayed);
    let mut save = tool_core::save::new_game();
    save.settings.name = "Ana".into();
    save.secret = ledger::hex(&[5; 32]);
    save.ledger = l;
    let want = tool_core::save::store(&save) + "\n";
    let path = root.join("tests/browser/ledger_save.json");
    match fs::read_to_string(&path) {
        Ok(have) => assert_eq!(
            have, want,
            "tests/browser/ledger_save.json is stale: delete it and run cargo test"
        ),
        Err(_) => fs::write(&path, want).unwrap(),
    }
}
