//! Game states & the tiny app state-machine that drives them.
//!
//! `Loading` → `Menu` → `Driving`. The `Driving` state lives behind the
//! menu; pressing M (paused / wrecked) abandons the run and returns to the
//! menu, which reloads the JSON save so the service record is always fresh.

pub mod driving;
pub mod loading;
pub mod menu;

use driving::DrivingState;
use loading::LoadingState;
use menu::MenuState;

use crate::systems::assets::AssetBundle;
use macroquad::prelude::*;

pub enum GameState {
    Loading(LoadingState),
    Menu(MenuState),
    Driving(DrivingState),
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
                if driving.take_menu_request() {
                    self.state = GameState::Menu(MenuState::new());
                }
            }
        }
    }
}
