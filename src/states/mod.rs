//! Game states & the tiny app state-machine that drives them.
//!
//! Two states for now: `Loading` → `Driving` (Module 6 turns this into the
//! full menu / garage / pause stack; nothing here pre-decides its shape).

pub mod driving;
pub mod loading;

use driving::DrivingState;
use loading::LoadingState;

use macroquad::prelude::*;

pub enum GameState {
    Loading(LoadingState),
    Driving(DrivingState),
}

pub struct App {
    state: GameState,
}

impl App {
    pub fn new() -> App {
        App {
            state: GameState::Loading(LoadingState::new()),
        }
    }

    /// One frame: update + draw the active state; handle state transitions.
    pub fn frame(&mut self) {
        let dt = get_frame_time();
        let finished_loading = match &mut self.state {
            GameState::Loading(loading) => {
                let done = loading.update(dt);
                loading.draw();
                done
            }
            GameState::Driving(driving) => {
                driving.update(dt);
                driving.draw();
                false
            }
        };
        if finished_loading {
            match &mut self.state {
                GameState::Loading(loading) => {
                    let bundle = loading.take_bundle();
                    self.state = GameState::Driving(DrivingState::new(bundle));
                }
                GameState::Driving(_) => {}
            }
        }
    }
}
