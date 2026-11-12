//! Field codex (Module 8) — what the wasteland has taught you, in writing.
//!
//! A dead-man's handbook recovered from countless rigs. Every entry below is
//! a real system in the game — the codex is not lore bolted to fibreglass;
//! it is the manual you were never given.
//!
//! Entries unlock with career progress (runs / kills / best distance / NG+),
//! so the book fills itself as you play. Read it from the main menu with [C].

use crate::systems::savegame::SaveData;

#[derive(Clone, Copy, PartialEq)]
pub enum Unlock {
    Always,
    Runs(u32),
    Kills(u32),
    BestM(f32),
    NgPlus(u32),
}

pub struct CodexEntry {
    pub title: &'static str,
    pub body: &'static [&'static str],
    pub unlock: Unlock,
}

impl CodexEntry {
    pub fn unlocked(&self, save: &SaveData) -> bool {
        match self.unlock {
            Unlock::Always => true,
            Unlock::Runs(n) => save.total_runs >= n,
            Unlock::Kills(n) => save.total_kills >= n,
            Unlock::BestM(m) => save.best_m >= m,
            Unlock::NgPlus(n) => save.ng_plus >= n,
        }
    }
}

pub const ENTRIES: &[CodexEntry] = &[
    CodexEntry {
        title: "THE HIGHWAY",
        body: &[
            "Nine hundred kilometres of asphalt laid for an evacuation that never came.",
            "Blowout at speed is survivable. Off the shoulder is not — soft sand eats",
            "momentum and turns you into a silhouette.",
        ],
        unlock: Unlock::Always,
    },
    CodexEntry {
        title: "DUST DEVILS",
        body: &[
            "Faction of the open badlands. Rust-orange crews on bikes and hot rods.",
            "Cheap, fast, fearless. They chase in packs and die fast — 68/32/00 style.",
            "Armored iron is a rumor where they run; the turret is your argument.",
        ],
        unlock: Unlock::Always,
    },
    CodexEntry {
        title: "ASH CARTEL",
        body: &[
            "Faction of the Ruined City. Ash-grey commanders in welded armor.",
            "22/33/45 — their doctrine favors the armored car: slow, plated, rocket-armed.",
            "Bring incendiary. Their hulls drink lead and sweat flame.",
        ],
        unlock: Unlock::Always,
    },
    CodexEntry {
        title: "JUGGERNAUT",
        body: &[
            "A raffinate tanker that refused to stop rolling. 900 points of welded tank",
            "steel between you and its crew. Hangs ~24 m ahead; contact is 26 hull.",
            "AP 80%, fire 35%. Dodge the mines it trickles off the tail.",
        ],
        unlock: Unlock::BestM(1_200.0),
    },
    CodexEntry {
        title: "THE CRAWLER",
        body: &[
            "Something under the asphalt learned the shape of your tires. Six segments,",
            "one appetite. It stalks the back of the rig and touches steel for 16.",
            "Incendiary is scripture. AP is a shrug. Keep it behind you.",
        ],
        unlock: Unlock::BestM(1_800.0),
    },
    CodexEntry {
        title: "BOMBER DRONE",
        body: &[
            "A maintenance quad that found new work. One pass at thirty metres, three",
            "dumb bombs, one painted-amber promise of where they will land.",
            "You cannot shoot it. You can only not be there.",
        ],
        unlock: Unlock::Kills(25),
    },
    CodexEntry {
        title: "RUIN SITES",
        body: &[
            "Sea-containers, safes and stripped shelves under a skeletal overpass.",
            "Bail at the marker (hold F / BAIL). NERVE bleeds while you work;",
            "marauder skiffs circle. The giants that hold the big scores do not ask twice.",
        ],
        unlock: Unlock::Runs(3),
    },
    CodexEntry {
        title: "NEW GAME+",
        body: &[
            "An eight-hundred-metre run does not end when the rig does — it escalates.",
            "Every NG+ rank: hostile hulls +15%, your damage taken +15%, scrap +20%.",
            "The wasteland remembers exactly how far you got. It is proud of you. It is hungry.",
        ],
        unlock: Unlock::NgPlus(1),
    },
    CodexEntry {
        title: "THE WORK BENCH",
        body: &[
            "TAB stops the world for the length of a cigarette you never light.",
            "Alloy plates, dual feeds, bored-out injectors, magnet coils.",
            "Steel you bolt on is steel you keep. Spend scrap before the road does.",
        ],
        unlock: Unlock::Always,
    },
];

/// How many entries are open right now (shown as `n / N` in the codex header).
pub fn unlocked_count(save: &SaveData) -> usize {
    ENTRIES.iter().filter(|e| e.unlocked(save)).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save() -> SaveData {
        SaveData::default()
    }

    #[test]
    fn fresh_profile_sees_the_basics_only() {
        let s = save();
        let open = unlocked_count(&s);
        assert!(open >= 4);
        assert!(ENTRIES.iter().filter(|e| e.unlocked(&s)).all(|e| e.title != "THE CRAWLER"));
    }

    #[test]
    fn crawler_entry_opens_at_1800_metres() {
        let mut s = save();
        s.best_m = 1799.0;
        assert!(!ENTRIES.iter().find(|e| e.title == "THE CRAWLER").unwrap().unlocked(&s));
        s.best_m = 1800.0;
        assert!(ENTRIES.iter().find(|e| e.title == "THE CRAWLER").unwrap().unlocked(&s));
    }

    #[test]
    fn ng_plus_entries_track_the_ladder() {
        let mut s = save();
        assert!(!ENTRIES.iter().any(|e| e.title == "NEW GAME+" && e.unlocked(&s)));
        s.ng_plus = 1;
        assert!(ENTRIES.iter().find(|e| e.title == "NEW GAME+").unwrap().unlocked(&s));
    }
}
