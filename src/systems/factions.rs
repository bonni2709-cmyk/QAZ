//! Faction identity — who rules the stretch of highway you're burning through.
//!
//! Two syndicates contest the dead lands. Their territory follows the biome
//! cycle: the open badlands belong to the DUST DEVILS (cheap scrap-horde,
//! fast and many), the ruined city is ASH CARTEL ground (armored remnant of
//! the old military, patient and vindictive). Encounters get meaner and
//! heavier where the Cartel digs in. No diplomacy tree, no reputation —
//! just the grudges the rig carries.

use macroquad::prelude::Color;

use crate::world::road::Biome;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Faction {
    /// Scrap-horde of the open badlands.
    DustDevils,
    /// Military remnant holding the ruins.
    AshCartel,
}

impl Faction {
    pub fn from_biome(biome: Biome) -> Faction {
        match biome {
            Biome::RuinedCity => Faction::AshCartel,
            Biome::Wasteland | Biome::ToxicSwamp => Faction::DustDevils,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Faction::DustDevils => "DUST DEVILS",
            Faction::AshCartel => "ASH CARTEL",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Faction::DustDevils => "scrap horde · fast & swarming",
            Faction::AshCartel => "military remnant · armored & patient",
        }
    }

    /// Accent used for hostile trim / territory tags.
    pub fn accent(self) -> Color {
        match self {
            Faction::DustDevils => Color::new(0.95, 0.42, 0.12, 1.0),
            Faction::AshCartel => Color::new(0.62, 0.66, 0.34, 1.0),
        }
    }

    /// Spawn weights (biker, buggy, armored) *before* distance scaling.
    /// Dust Devils swarm; the Cartel walks armor up the road.
    pub fn spawn_weights(self) -> (f32, f32, f32) {
        match self {
            Faction::DustDevils => (0.58, 0.32, 0.10),
            Faction::AshCartel => (0.22, 0.33, 0.45),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruined_city_belongs_to_the_cartel() {
        assert_eq!(Faction::from_biome(Biome::RuinedCity), Faction::AshCartel);
        assert_eq!(Faction::from_biome(Biome::Wasteland), Faction::DustDevils);
        assert_eq!(Faction::from_biome(Biome::ToxicSwamp), Faction::DustDevils);
    }

    #[test]
    fn weights_are_valid_distributions() {
        for f in [Faction::DustDevils, Faction::AshCartel] {
            let (b, g, a) = f.spawn_weights();
            assert!((b + g + a - 1.0).abs() < 1e-6, "weights must sum to 1 for {f:?}");
            assert!(a > 0.0 && b > 0.0 && g > 0.0);
        }
    }

    #[test]
    fn cartel_field_more_armor_than_the_horde() {
        let (_, _, a_horde) = Faction::DustDevils.spawn_weights();
        let (_, _, a_cartel) = Faction::AshCartel.spawn_weights();
        assert!(a_cartel > a_horde);
    }
}
