//! Game states & the tiny app state-machine that drives them.
//!
//! `Loading` → `Menu` → `Driving` ⇄ `OnFoot`. The `Driving` state lives behind
//! the menu; pressing M (paused / wrecked) abandons the run and returns to the
//! menu, which reloads the JSON save so the service record is always fresh.
//! Holding F at a roadside ruin site swaps into the on-foot salvage episode;
//! when it resolves the rig is handed back with the outcome applied.

pub mod driving;
pub mod loading;
pub mod menu;
pub mod onfoot;

use driving::DrivingState;
use loading::LoadingState;
use menu::MenuState;
use onfoot::OnFoot;

use crate::systems::assets::AssetBundle;
use macroquad::prelude::*;

/// Midday wasteland glare for the ruin-site diorama.
const ONFOOT_AMBIENT: f32 = 0.92;

pub enum GameState {
    Loading(LoadingState),
    Menu(MenuState),
    Driving(DrivingState),
    /// The highway run is paused mid-scene while the scavenger works on foot.
    OnFoot { driving: DrivingState, onfoot: OnFoot },
}

pub struct App {
    state: GameState,
    /// The streamed pack, owned by the app so every roll-out gets its handles.
    bundle: AssetBundle,
}

impl App {
    pub fn new() -> App {
        App {
            state: GameState::Loading(LoadingState::new()),
            bundle: AssetBundle::empty(),
        }
    }

    /// One frame: update + draw the active state; handle state transitions.
    pub fn frame(&mut self) {
        let dt = get_frame_time();
        match &mut self.state {
            GameState::Loading(loading) => {
                let done = loading.update(dt);
                loading.draw();
                if done {
                    self.bundle = loading.take_bundle();
                    self.state = GameState::Menu(MenuState::new());
                }
            }
            GameState::Menu(menu) => {
                let start = menu.update(dt);
                menu.draw();
                if start {
                    self.state = GameState::Driving(DrivingState::new(self.bundle.clone()));
                }
            }
            GameState::Driving(driving) => {
                driving.update(dt);
                driving.draw();
                if let Some(seed) = driving.take_bail_request() {
                    // hand the rig over to the on-foot episode, mid-scene
                    let old = std::mem::replace(&mut self.state, GameState::Menu(MenuState::new()));
                    if let GameState::Driving(driving) = old {
                        self.state = GameState::OnFoot {
                            driving,
                            onfoot: OnFoot::new(seed),
                        };
                    }
                } else if driving.take_menu_request() {
                    self.state = GameState::Menu(MenuState::new());
                }
            }
            GameState::OnFoot { driving, onfoot } => {
                onfoot.update(dt, &self.bundle);
                onfoot.draw(ONFOOT_AMBIENT, &self.bundle);
                if let Some(outcome) = onfoot.take_outcome() {
                    driving.apply_onfoot_outcome(outcome);
                    let old = std::mem::replace(&mut self.state, GameState::Menu(MenuState::new()));
                    if let GameState::OnFoot { driving, .. } = old {
                        self.state = GameState::Driving(driving);
                    }
                }
            }
        }
    }
}
