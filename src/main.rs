// #![feature(array_value_iter)]
mod combat;
mod config;
mod core;
mod farm;
mod graphics;
mod gui;
mod items;
mod phys;
mod props;
mod state;
mod tilemap;

use crate::core::*;
use config::ConfigHandler;
use graphics::images::ImageMap;
use l8r::L8r;
use nalgebra as na;
use ncollide2d::pipeline::CollisionGroups;
use ncollide2d::shape::Cuboid;
use phys::{aiming, movement};
use quicksilver::{
    graphics::Font,
    lifecycle::{run, Asset, Settings},
};
use state::GameState;
use std::time::Instant;

pub struct World {
    pub ecs: hecs::World,
    pub l8r: L8r<World>,
    pub phys: CollisionWorld,
    pub config: std::rc::Rc<ConfigHandler>,
}
impl l8r::ContainsHecsWorld for World {
    fn ecs(&self) -> &hecs::World {
        &self.ecs
    }

    fn ecs_mut(&mut self) -> &mut hecs::World {
        &mut self.ecs
    }
}
impl World {
    fn new() -> Self {
        Self {
            ecs: hecs::World::new(),
            l8r: L8r::new(),
            phys: CollisionWorld::new(0.02),
            config: std::rc::Rc::new(ConfigHandler::new().unwrap_or_else(|e| panic!("{}", e))),
        }
    }

    #[inline]
    fn add_hitbox(
        &mut self,
        entity: hecs::Entity,
        iso: Iso2,
        cuboid: Cuboid<f32>,
        groups: CollisionGroups,
    ) -> PhysHandle {
        let (h, _) = self.phys.add(
            iso,
            ncollide2d::shape::ShapeHandle::new(cuboid),
            groups,
            ncollide2d::pipeline::GeometricQueryType::Contacts(0.0, 0.0),
            entity,
        );
        let hnd = h;
        self.ecs
            .insert_one(entity, phys::collision::Contacts::new())
            .unwrap_or_else(|e| {
                panic!(
                    "Couldn't insert Contacts for Entity[{:?}] when adding hitbox: {}",
                    entity, e
                )
            });
        self.ecs.insert_one(entity, hnd).unwrap_or_else(|e| {
            panic!(
                "Couldn't insert PhysHandle[{:?}] for Entity[{:?}] to add hitbox: {}",
                h, entity, e
            )
        });

        hnd
    }
}

pub struct Game {
    last_render: Instant,
    world: World,
    images: ImageMap,
    font: Asset<Font>,
    particle_manager: graphics::particle::Manager,
    gui: gui::GuiState,
    sprite_sheet_animation_failed: bool,
    state: GameState,
    entered: bool,
}

fn main() {
    run::<Game>(
        "Game",
        dbg!(DIMENSIONS * SCALE),
        Settings {
            //multisampling: Some(16),
            resize: quicksilver::graphics::ResizeStrategy::IntegerScale {
                width: 480,
                height: 270,
            },
            //fullscreen: true,
            ..Settings::default()
        },
    );
}
