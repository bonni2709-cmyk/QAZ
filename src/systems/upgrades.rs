//! Upgrade state: salvage (scrap) economy + the wheeled-workbench tracks.
//!
//! Four tracks, three ranks each. Entities never read this type directly —
//! the driving state pulls effect multipliers out of it every frame and hands
//! them to the truck / turret / magnet, which keeps the dependency arrow
//! one-way (UI & driving → entities).
//!
//! Upgrades and scrap persist across sessions via the JSON save system
//! (Module 6, `systems::savegame`) — see `snapshot()` / `restore()`.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UpgradeTrack {
    Armor,
    Turret,
    Engine,
    Magnet,
}

/// Display order of the workbench panel rows.
pub const TRACKS: [UpgradeTrack; 4] = [
    UpgradeTrack::Armor,
    UpgradeTrack::Turret,
    UpgradeTrack::Engine,
    UpgradeTrack::Magnet,
];

const MAX_LEVEL: u8 = 3;

impl UpgradeTrack {
    pub fn name(self) -> &'static str {
        match self {
            UpgradeTrack::Armor => "COMPOSITE PLATING",
            UpgradeTrack::Turret => "AUTOLOADER & COOLING",
            UpgradeTrack::Engine => "BIG-BLOCK ENGINE",
            UpgradeTrack::Magnet => "SALVAGE MAGNET",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            UpgradeTrack::Armor => "+25 max HULL · 6% flat resist / rank",
            UpgradeTrack::Turret => "+9% fire rate · +18% cooling / rank",
            UpgradeTrack::Engine => "+10% top speed · +12% acceleration / rank",
            UpgradeTrack::Magnet => "+2.4 m beam radius / rank",
        }
    }

    fn cost(self, target_rank: u8) -> u32 {
        let idx = (target_rank.saturating_sub(1)).min(2) as usize;
        match self {
            UpgradeTrack::Armor => [45, 75, 120][idx],
            UpgradeTrack::Turret => [45, 75, 120][idx],
            UpgradeTrack::Engine => [40, 70, 115][idx],
            UpgradeTrack::Magnet => [35, 60, 100][idx],
        }
    }
}

#[derive(Clone)]
pub struct UpgradeState {
    armor: u8,
    turret: u8,
    engine: u8,
    magnet: u8,
    scrap: u32,
}

impl UpgradeState {
    pub fn new() -> UpgradeState {
        UpgradeState {
            armor: 0,
            turret: 0,
            engine: 0,
            magnet: 0,
            scrap: 0,
        }
    }

    pub fn scrap(&self) -> u32 {
        self.scrap
    }

    pub fn add_scrap(&mut self, amount: u32) {
        self.scrap += amount;
    }

    pub fn level(&self, track: UpgradeTrack) -> u8 {
        match track {
            UpgradeTrack::Armor => self.armor,
            UpgradeTrack::Turret => self.turret,
            UpgradeTrack::Engine => self.engine,
            UpgradeTrack::Magnet => self.magnet,
        }
    }

    /// Cost of the next rank, or None when the track is maxed.
    pub fn cost(&self, track: UpgradeTrack) -> Option<u32> {
        let level = self.level(track);
        if level >= MAX_LEVEL {
            None
        } else {
            Some(track.cost(level + 1))
        }
    }

    pub fn can_buy(&self, track: UpgradeTrack) -> bool {
        self.cost(track).map(|c| self.scrap >= c).unwrap_or(false)
    }

    /// Spend scrap on the next rank of `track`. Returns true on success.
    pub fn buy(&mut self, track: UpgradeTrack) -> bool {
        let Some(cost) = self.cost(track) else {
            return false;
        };
        if self.scrap < cost {
            return false;
        }
        self.scrap -= cost;
        match track {
            UpgradeTrack::Armor => self.armor += 1,
            UpgradeTrack::Turret => self.turret += 1,
            UpgradeTrack::Engine => self.engine += 1,
            UpgradeTrack::Magnet => self.magnet += 1,
        }
        true
    }

    // ---- effect providers (read every frame by the driving state) ----

    pub fn damage_reduction(&self) -> f32 {
        self.armor as f32 * 0.06
    }

    pub fn hull_bonus(&self) -> f32 {
        self.armor as f32 * 25.0
    }

    pub fn fire_rate_mult(&self) -> f32 {
        1.0 + self.turret as f32 * 0.09
    }

    pub fn cooling_mult(&self) -> f32 {
        1.0 + self.turret as f32 * 0.18
    }

    pub fn heat_leak_mult(&self) -> f32 {
        (1.0 - self.turret as f32 * 0.06).max(0.5)
    }

    pub fn top_speed_mult(&self) -> f32 {
        1.0 + self.engine as f32 * 0.10
    }

    pub fn acceleration_mult(&self) -> f32 {
        1.0 + self.engine as f32 * 0.12
    }

    pub fn magnet_radius(&self) -> f32 {
        6.0 + self.magnet as f32 * 2.4
    }

    /// Snapshot for the JSON save: (scrap bank, [armor, turret, engine, magnet] ranks).
    pub fn snapshot(&self) -> (u32, [u8; 4]) {
        (self.scrap, [self.armor, self.turret, self.engine, self.magnet])
    }

    /// Rebuild from a save file. Ranks are clamped to MAX_LEVEL.
    pub fn restore(scrap: u32, ranks: [u8; 4]) -> UpgradeState {
        UpgradeState {
            armor: ranks[0].min(MAX_LEVEL),
            turret: ranks[1].min(MAX_LEVEL),
            engine: ranks[2].min(MAX_LEVEL),
            magnet: ranks[3].min(MAX_LEVEL),
            scrap,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_restore_roundtrip() {
        let mut state = UpgradeState::new();
        state.add_scrap(340);
        assert!(state.buy(UpgradeTrack::Armor));
        assert!(state.buy(UpgradeTrack::Magnet));
        assert!(state.buy(UpgradeTrack::Magnet));
        let (scrap, ranks) = state.snapshot();
        let restored = UpgradeState::restore(scrap, ranks);
        assert_eq!(restored.scrap(), scrap);
        assert_eq!(restored.level(UpgradeTrack::Armor), 1);
        assert_eq!(restored.level(UpgradeTrack::Magnet), 2);
        assert_eq!(restored.level(UpgradeTrack::Turret), 0);
    }

    #[test]
    fn restore_clamps_foreign_ranks() {
        let restored = UpgradeState::restore(0, [250, 9, 7, 4]);
        assert_eq!(restored.level(UpgradeTrack::Armor), MAX_LEVEL);
        assert_eq!(restored.level(UpgradeTrack::Turret), MAX_LEVEL);
        assert_eq!(restored.level(UpgradeTrack::Engine), MAX_LEVEL);
        assert_eq!(restored.level(UpgradeTrack::Magnet), MAX_LEVEL);
    }
}
