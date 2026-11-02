//! Saves that survive a deploy. Part of the saves kit (`kits/saves/README.md`).
//!
//! The core owns the save: what is in it, its version, and how a save from
//! an older build becomes one this build can use. The page stores the text
//! the core gives it (`web/save/store.js`) and hands it back; it never looks
//! inside. A save this build cannot read is refused and kept, never
//! overwritten, so a bad deploy costs a reload and not a player's progress.
//!
//! Every change to what a save holds bumps `VERSION` and adds one step to
//! `migrate`. `tests/old_saves.rs` loads a save written by every deployed
//! build (`tests/saves/`), and the project writes one more before each
//! deploy. A field an old save holds either reaches `Save` or is listed in
//! `DROPPED` with the reason, because the loader refuses fields it does not
//! know.
//!
//! Countless Days keeps the settings and the best score per chart
//! (BO-Q10). Round 5 adds the player's key and the ledger as version 2.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

use crate::game::{Best, Settings};

/// The shape this build writes. Bump it, and add a step to `migrate`,
/// whenever a field is added, renamed, moved or removed.
pub const VERSION: u32 = 3;

/// Fields an old save holds that a migration removed or changed on purpose,
/// each with the version that did it and why. Every other field comes back
/// from loading with the value the old save held.
pub const DROPPED: &[(&str, u32, &str)] = &[(
    "settings",
    2,
    "gained keys and mouse_presses; every version 1 setting keeps its value (a_version_one_save_keeps_its_settings_and_gets_z_and_x)",
)];

/// Status keys. Each has a string in `data/` under the same key.
pub const UNREADABLE: &str = "save.unreadable";
pub const NEWER: &str = "save.newer";
pub const NO_STEP: &str = "save.no_step";

/// What a save holds. Unknown fields are refused, not dropped; a missing
/// field takes its default, which is why old_saves.rs checks that none is lost.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Save {
    pub version: u32,
    pub settings: Settings,
    /// The best play per chart, keyed "song:difficulty".
    pub bests: BTreeMap<String, Best>,
    /// 32 random bytes in hex, drawn by the page once: the player's signing
    /// key. Empty until the page's first random bytes arrive.
    pub secret: String,
    /// Every player's best per chart that has reached this browser (T2).
    pub ledger: crate::ledger::Ledger,
}

/// What came back from the page's storage or a file the player chose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    /// Nothing was stored: start a new game.
    Fresh,
    /// A save this build can use, migrated to `VERSION`.
    Ready(Save),
    /// A save this build cannot use. `kept` is the text, unchanged, for the
    /// page to keep beside the new game and offer back as a file; `why` is a
    /// status key.
    Refused { why: &'static str, kept: String },
}

pub fn new_game() -> Save {
    Save {
        version: VERSION,
        ..Save::default()
    }
}

/// The text the page stores.
pub fn store(save: &Save) -> String {
    let mut s = save.clone();
    s.version = VERSION;
    serde_json::to_string(&s).expect("a Save always serialises")
}

/// One step from a save of version `from` to `from + 1`. Pure: it reads the
/// old fields and writes the new ones.
fn migrate(from: u32, mut v: Map<String, Value>) -> Option<Map<String, Value>> {
    match from {
        // version 2 added the press keys and whether the mouse presses
        1 => {
            let settings = v.entry("settings").or_insert_with(|| json!({}));
            let fields = settings.as_object_mut()?;
            fields.insert("keys".into(), json!(crate::game::default_keys()));
            fields.insert("mouse_presses".into(), json!(true));
            Some(v)
        }
        // version 3 added the player's key and the ledger (round 5)
        2 => {
            v.insert("secret".into(), json!(""));
            v.insert("ledger".into(), json!({ "boards": {} }));
            Some(v)
        }
        _ => None,
    }
}

pub fn load(text: &str) -> Loaded {
    if text.trim().is_empty() {
        return Loaded::Fresh;
    }
    let refuse = |why| Loaded::Refused {
        why,
        kept: text.to_string(),
    };
    let Ok(Value::Object(mut v)) = serde_json::from_str::<Value>(text) else {
        return refuse(UNREADABLE);
    };
    let Some(mut version) = v.get("version").and_then(Value::as_u64) else {
        return refuse(UNREADABLE);
    };
    if version > VERSION as u64 {
        return refuse(NEWER);
    }
    while version < VERSION as u64 {
        match migrate(version as u32, v) {
            Some(next) => v = next,
            None => return refuse(NO_STEP),
        }
        version += 1;
        v.insert("version".into(), json!(version));
    }
    match serde_json::from_value::<Save>(Value::Object(v)) {
        Ok(save) => Loaded::Ready(save),
        Err(_) => refuse(UNREADABLE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Save {
        let mut bests = BTreeMap::new();
        bests.insert(
            "placeholder-1:easy".to_string(),
            Best {
                score: 4500,
                accuracy: 9712,
            },
        );
        Save {
            version: VERSION,
            settings: Settings {
                audio_offset_ms: 25,
                visual_offset_ms: -5,
                volume_pct: 60,
                name: "Ana".into(),
                keys: vec!["KeyW".into(), "KeyA".into()],
                mouse_presses: false,
            },
            bests,
            secret: "07".repeat(32),
            ledger: crate::ledger::Ledger::default(),
        }
    }

    #[test]
    fn a_save_reads_back_as_it_was_written() {
        assert_eq!(load(&store(&sample())), Loaded::Ready(sample()));
    }

    #[test]
    fn nothing_stored_is_a_new_game() {
        assert_eq!(load(""), Loaded::Fresh);
        assert_eq!(load("  \n"), Loaded::Fresh);
    }

    #[test]
    fn a_save_this_build_cannot_use_is_refused_and_kept_word_for_word() {
        for (text, why) in [
            ("{nope", UNREADABLE),
            ("[1,2]", UNREADABLE),
            (r#"{"bests":{}}"#, UNREADABLE),
            (r#"{"version":99}"#, NEWER),
            (r#"{"version":0}"#, NO_STEP),
            (r#"{"version":3,"cheat":true}"#, UNREADABLE),
        ] {
            assert_eq!(
                load(text),
                Loaded::Refused {
                    why,
                    kept: text.to_string()
                },
                "{text}"
            );
        }
    }

    #[test]
    fn a_version_one_save_keeps_its_settings_and_gets_z_and_x() {
        let old = r#"{"version":1,"settings":{"audio_offset_ms":10,"visual_offset_ms":-5,"volume_pct":70,"name":"Cy"},"bests":{}}"#;
        let Loaded::Ready(save) = load(old) else {
            panic!("refused");
        };
        assert_eq!(save.settings.audio_offset_ms, 10);
        assert_eq!(save.settings.name, "Cy");
        assert_eq!(save.settings.keys, ["KeyZ", "KeyX"]);
        assert!(save.settings.mouse_presses);
    }

    #[test]
    fn a_new_game_stores_at_this_version() {
        assert!(store(&new_game()).contains(&format!("\"version\":{VERSION}")));
    }
}
