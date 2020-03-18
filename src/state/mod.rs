mod combat;
mod farming;

use crate::graphics::images::fetch_images;
use crate::{Game, World};
use combat::*;
use farming::*;
use quicksilver::{
    graphics::Font,
    lifecycle::{Asset, State, Window},
    Result,
};
use std::time::Instant;

pub enum GameState {
    FARMING,
    COMBAT,
}

impl State for Game {
    fn new() -> Result<Game> {
        let images = fetch_images();

        Ok(Game {
            world: World::new(),
            images,
            font: Asset::new(Font::load("min.ttf")),
            particle_manager: Default::default(),
            gui: crate::gui::GuiState::new(),
            last_render: Instant::now(),
            sprite_sheet_animation_failed: false,
            state: GameState::FARMING,
            entered: true,
        })
    }

    fn draw(&mut self, window: &mut Window) -> Result<()> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_render);

        if !self.sprite_sheet_animation_failed {
            crate::graphics::sprite_sheet::animate(&mut self.world, elapsed).unwrap_or_else(|e| {
                println!("Disabling sprite sheet animation: {}", e);
                self.sprite_sheet_animation_failed = true;
            });
        }
        crate::graphics::render(window, &self.world, &mut self.images, &mut self.font)?;

        self.last_render = now;
        Ok(())
    }

    fn update(&mut self, window: &mut Window) -> Result<()> {
        if self.entered {
            match self.state {
                GameState::FARMING => farming_enter(self, window),
                GameState::COMBAT => combat_enter(self, window),
            }
            self.entered = false;
        }

        let transition = match self.state {
            GameState::FARMING => farming_update(self, window),
            GameState::COMBAT => combat_update(self, window),
        };

        match transition {
            None => (),
            Some(state) => {
                match self.state {
                    GameState::FARMING => farming_exit(self, window),
                    GameState::COMBAT => combat_exit(self, window),
                }
                self.state = state;
                self.entered = true;
            }
        }

        Ok(())
    }
}
