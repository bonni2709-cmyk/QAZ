//! JSON save game (Module 6) — real persistence, plain files, no ceremony.
//!
//! Stored as `railgun_pursuit_save.json`. Search order for both read and
//! write (first winner sticks):
//!   1. `$RAILGUN_SAVE_PATH` — explicit override (tests, portable setups)
//!   2. beside the executable  (desktop installs)
//!   3. the current working directory
//!   4. the OS temp directory  (last resort, e.g. read-only install dirs)
//!
//! Failure is *soft*: a missing or corrupt file yields `SaveData::default()`
//! and a stderr note — the run is never blocked, and a corrupt file is left
//! untouched for manual recovery. On Android the app sandbox may make every
//! candidate unwritable; the game then silently plays unsaved (README).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SAVE_VERSION: u32 = 2;

/// Hard ceiling on the New Game+ ladder (difficulty +15 %/level, scrap +20 %/level).
pub const NG_PLUS_MAX: u32 = 9;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct SaveData {
    pub version: u32,
    pub best_m: f32,
    pub scrap: u32,
    pub armor: u8,
    pub turret: u8,
    pub engine: u8,
    pub magnet: u8,
    pub total_runs: u32,
    pub total_kills: u64,
    /// New Game+ depth: how many 800 m+ runs have been lost to the wasteland.
    pub ng_plus: u32,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            best_m: 0.0,
            scrap: 0,
            armor: 0,
            turret: 0,
            engine: 0,
            magnet: 0,
            total_runs: 0,
            total_kills: 0,
            ng_plus: 0,
        }
    }
}

/// v1 (pre-Module-8) schema — kept so existing saves migrate instead of
/// being discarded as "corrupt".
#[derive(Deserialize)]
struct SaveDataV1 {
    #[allow(dead_code)]
    version: u32,
    best_m: f32,
    scrap: u32,
    armor: u8,
    turret: u8,
    engine: u8,
    magnet: u8,
    total_runs: u32,
    total_kills: u64,
}

fn clamp_data(data: &mut SaveData) {
    data.armor = data.armor.min(3);
    data.turret = data.turret.min(3);
    data.engine = data.engine.min(3);
    data.magnet = data.magnet.min(3);
    data.ng_plus = data.ng_plus.min(NG_PLUS_MAX);
    if !data.best_m.is_finite() || data.best_m < 0.0 {
        data.best_m = 0.0;
    }
}

fn candidate_paths() -> [PathBuf; 3] {
    let mut exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("railgun_pursuit_save.json")))
        .unwrap_or_else(|| PathBuf::from("railgun_pursuit_save.json"));
    let cwd = PathBuf::from("railgun_pursuit_save.json");
    let temp = std::env::temp_dir().join("railgun_pursuit_save.json");
    if exe_dir == cwd {
        exe_dir = cwd.clone();
    }
    [exe_dir, cwd, temp]
}

/// Parse without panicking. Corrupt or foreign payloads yield `None`.
/// v1 saves are migrated forward (NG+ ladder starts at zero).
fn parse(json: &str) -> Option<SaveData> {
    if let Ok(mut data) = serde_json::from_str::<SaveData>(json) {
        clamp_data(&mut data);
        return Some(data);
    }
    // v1 → v2: every pre-Module-8 save keeps its progress; the NG+ ladder
    // simply had no track back then.
    if let Ok(v1) = serde_json::from_str::<SaveDataV1>(json) {
        let mut data = SaveData {
            version: SAVE_VERSION,
            best_m: v1.best_m,
            scrap: v1.scrap,
            armor: v1.armor,
            turret: v1.turret,
            engine: v1.engine,
            magnet: v1.magnet,
            total_runs: v1.total_runs,
            total_kills: v1.total_kills,
            ng_plus: 0,
        };
        clamp_data(&mut data);
        return Some(data);
    }
    None
}

/// Load the save file. Missing/corrupt ⇒ `SaveData::default()` + a stderr note.
pub fn load() -> SaveData {
    if let Ok(override_path) = std::env::var("RAILGUN_SAVE_PATH") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return load_from(&PathBuf::from(trimmed));
        }
    }
    for path in candidate_paths() {
        if path.exists() {
            return load_from(&path);
        }
    }
    SaveData::default()
}

fn load_from(path: &PathBuf) -> SaveData {
    match std::fs::read_to_string(path) {
        Ok(json) => match parse(&json) {
            Some(data) => data,
            None => {
                eprintln!(
                    "savegame: {} is corrupt or foreign; starting fresh (file left untouched)",
                    path.display()
                );
                SaveData::default()
            }
        },
        Err(e) => {
            eprintln!("savegame: cannot read {}: {}; starting fresh", path.display(), e);
            SaveData::default()
        }
    }
}

/// Persist (best-effort). Returns true when a candidate path accepted the write.
pub fn store(data: &SaveData) -> bool {
    let json = match serde_json::to_string_pretty(data) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("savegame: cannot serialize state: {}", e);
            return false;
        }
    };
    if let Ok(override_path) = std::env::var("RAILGUN_SAVE_PATH") {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            return std::fs::write(PathBuf::from(trimmed), json.as_bytes()).is_ok();
        }
    }
    for path in candidate_paths() {
        if std::fs::write(&path, json.as_bytes()).is_ok() {
            return true;
        }
    }
    eprintln!("savegame: no writable location found — progress kept in memory only");
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_fresh_profile() {
        let d = SaveData::default();
        assert_eq!(d.version, SAVE_VERSION);
        assert_eq!(d.best_m, 0.0);
        assert_eq!(d.scrap, 0);
        assert_eq!(total_upgrade_ranks(&d), 0);
        assert_eq!(d.total_runs, 0);
        assert_eq!(d.total_kills, 0);
        assert_eq!(d.ng_plus, 0);
    }

    fn total_upgrade_ranks(d: &SaveData) -> u8 {
        d.armor + d.turret + d.engine + d.magnet
    }

    #[test]
    fn roundtrip_preserves_every_field() {
        let data = SaveData {
            version: SAVE_VERSION,
            best_m: 4321.5,
            scrap: 777,
            armor: 3,
            turret: 2,
            engine: 1,
            magnet: 0,
            total_runs: 9,
            total_kills: 1234,
            ng_plus: 4,
        };
        let json = serde_json::to_string_pretty(&data).expect("serialize");
        let back = parse(&json).expect("parse roundtrip");
        assert_eq!(data, back);
    }

    #[test]
    fn v1_save_migrates_forward_with_ng_plus_zeroed() {
        let json = "{\"version\":1,\"best_m\":1842.25,\"scrap\":321,\"armor\":2,\"turret\":1,\"engine\":0,\"magnet\":3,\"total_runs\":11,\"total_kills\":987}";
        let data = parse(json).expect("v1 save must migrate, not vanish");
        assert_eq!(data.version, SAVE_VERSION);
        assert_eq!(data.ng_plus, 0);
        assert_eq!(data.best_m, 1842.25);
        assert_eq!(data.scrap, 321);
        assert_eq!(data.armor, 2);
        assert_eq!(data.magnet, 3);
        assert_eq!(data.total_runs, 11);
        assert_eq!(data.total_kills, 987);
    }

    #[test]
    fn corrupt_payload_yields_none() {
        assert!(parse("{ not json").is_none());
        assert!(parse("").is_none());
        assert!(parse("[1,2,3]").is_none());
        // foreign but valid JSON must not impersonate a save file
        assert!(parse("{\"hello\":\"world\"}").is_none());
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let json = "{\"version\":2,\"best_m\":-50.0,\"scrap\":10,\"armor\":9,\"turret\":250,\"engine\":4,\"magnet\":255,\"total_runs\":2,\"total_kills\":7,\"ng_plus\":250}";
        let data = parse(json).expect("parse clamps");
        assert_eq!(data.best_m, 0.0);
        assert_eq!(data.armor, 3);
        assert_eq!(data.turret, 3);
        assert_eq!(data.engine, 3);
        assert_eq!(data.magnet, 3);
        assert_eq!(data.ng_plus, NG_PLUS_MAX);
    }

    #[test]
    fn v1_out_of_range_values_are_clamped_through_migration() {
        let json = "{\"version\":1,\"best_m\":-50.0,\"scrap\":10,\"armor\":9,\"turret\":4,\"engine\":4,\"magnet\":255,\"total_runs\":2,\"total_kills\":7}";
        let data = parse(json).expect("v1 clamps through migration");
        assert_eq!(data.best_m, 0.0);
        assert_eq!(data.armor, 3);
        assert_eq!(data.turret, 3);
        assert_eq!(data.magnet, 3);
        assert_eq!(data.ng_plus, 0);
    }

    #[test]
    fn json_field_names_are_stable() {
        let data = SaveData::default();
        let json = serde_json::to_string(&data).expect("serialize");
        for key in [
            "version", "best_m", "scrap", "armor", "turret", "engine", "magnet", "total_runs", "total_kills", "ng_plus",
        ] {
            assert!(json.contains(key), "missing key {}", key);
        }
    }
}
