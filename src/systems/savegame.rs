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

pub const SAVE_VERSION: u32 = 1;

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
        }
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
fn parse(json: &str) -> Option<SaveData> {
    match serde_json::from_str::<SaveData>(json) {
        Ok(mut data) => {
            // clamp anything the loader could not trust
            data.armor = data.armor.min(3);
            data.turret = data.turret.min(3);
            data.engine = data.engine.min(3);
            data.magnet = data.magnet.min(3);
            if !data.best_m.is_finite() || data.best_m < 0.0 {
                data.best_m = 0.0;
            }
            Some(data)
        }
        Err(_) => None,
    }
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
        };
        let json = serde_json::to_string_pretty(&data).expect("serialize");
        let back = parse(&json).expect("parse roundtrip");
        assert_eq!(data, back);
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
        let json = "{\"version\":1,\"best_m\":-50.0,\"scrap\":10,\"armor\":9,\"turret\":250,\"engine\":4,\"magnet\":255,\"total_runs\":2,\"total_kills\":7}";
        let data = parse(json).expect("parse clamps");
        assert_eq!(data.best_m, 0.0);
        assert_eq!(data.armor, 3);
        assert_eq!(data.turret, 3);
        assert_eq!(data.engine, 3);
        assert_eq!(data.magnet, 3);
    }

    #[test]
    fn json_field_names_are_stable() {
        let data = SaveData::default();
        let json = serde_json::to_string(&data).expect("serialize");
        for key in [
            "version", "best_m", "scrap", "armor", "turret", "engine", "magnet", "total_runs", "total_kills",
        ] {
            assert!(json.contains(key), "missing key {}", key);
        }
    }
}
