use l8r::L8r;
use macroquad::*;

use crate::{
    combat, draw,
    phys::{self, collision, CollisionGroups, CollisionWorld, Cuboid, PhysHandle},
};

pub mod player;
pub use player::Player;
pub mod map;
pub use map::Map;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub draw: draw::Config,
    pub player: player::Config,
}
impl Config {
    #[cfg(feature = "confui")]
    pub fn dev_ui(&mut self, ui: &mut egui::Ui) {
        #[cfg(not(target = "wasm32-unknown-unknown"))]
        egui::Window::new("Save")
            .fixed_size(egui::vec2(150.0, 70.0))
            .default_pos(egui::pos2(0.0, 0.0))
            .show(ui.ctx(), |ui| {
                if ui.button("To File").clicked {
                    std::fs::write("config.json", serde_json::to_vec_pretty(&self).unwrap())
                        .unwrap()
                }
            });

        egui::Window::new("Draw")
            .default_size(egui::vec2(150.0, 600.0))
            .default_pos(egui::pos2(0.0, 72.0))
            .show(ui.ctx(), |ui| self.draw.dev_ui(ui));

        egui::Window::new("Player")
            .default_size(egui::vec2(300.0, 600.0))
            .default_pos(egui::pos2(164.0, 0.0))
            .show(ui.ctx(), |ui| self.player.dev_ui(ui));
    }
}

pub struct World {
    pub ecs: hecs::World,
    pub ui: emigui_miniquad::UiPlugin,
    pub l8r: L8r<World>,
    pub dead: Vec<hecs::Entity>,
    pub map: Map,
    pub phys: CollisionWorld,
    pub config: Config,
    pub player: Player,
    pub images: draw::Images,
    pub draw_state: draw::DrawState,
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
    pub async fn new() -> Self {
        let mut ecs = hecs::World::new();
        let mut phys = CollisionWorld::new(0.02);
        let config = serde_json::from_slice(&load_file("config.json").await.unwrap()).unwrap();

        Self {
            player: Player::new(&mut ecs, &mut phys, &config),
            map: Map::new(&config),
            images: draw::Images::load(&config).await,
            config,
            phys,
            ecs,
            ui: emigui_miniquad::UiPlugin::new(),
            l8r: L8r::new(),
            dead: vec![],
            draw_state: draw::DrawState::default(),
        }
    }

    #[inline]
    pub fn add_hitbox(
        &mut self,
        entity: hecs::Entity,
        iso: na::Isometry2<f32>,
        cuboid: Cuboid<f32>,
        groups: CollisionGroups,
    ) -> PhysHandle {
        phys::phys_insert(&mut self.ecs, &mut self.phys, entity, iso, cuboid, groups)
    }

    pub fn update(&mut self) {
        if !self.player.state.is_throwing() && !self.ui.egui_ctx.wants_keyboard_input() {
            player::movement(self);
        }

        phys::velocity(self);
        phys::chase(self);
        collision::collision(self);

        if !self.ui.egui_ctx.wants_mouse_input() {
            player::aiming(self);
        }

        combat::hurtful_damage(self);
        combat::health::remove_out_of_health(self);

        draw::animate(self);

        let scheduled_world_edits: Vec<_> = self.l8r.drain(..).collect();
        L8r::now(scheduled_world_edits, self);

        phys::collision::clear_dead_collision_objects(self);

        for e in self.dead.drain(..) {
            if let Err(e) = self.ecs.despawn(e) {
                error!("couldn't remove entity {}", e);
            }
        }
    }

    pub fn draw(&mut self) {
        crate::draw::draw(self);

        {
            #[allow(unused_variables)]
            let Self { ui, config, .. } = self;
            ui.macroquad(|ui| {
                #[cfg(feature = "confui")]
                config.dev_ui(ui)
            });
        }
    }
}
