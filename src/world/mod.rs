use l8r::L8r;
use macroquad::*;

use crate::{
    combat, draw,
    phys::{self, collide, collision, CollisionGroups, CollisionWorld, Cuboid, PhysHandle},
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
    pub fn dev_ui(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("Player")
            .default_size(egui::vec2(100.0, 600.0))
            .default_pos(egui::pos2(320.0, 400.0))
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

        let mut world = Self {
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
        };

        /*
        for i in 0..4 {
            let fence = world
                .ecs
                .spawn((collision::CollisionStatic, world.images.looks("ruin.png")));
            world.add_hitbox(
                fence,
                na::Isometry2::translation(8.0 + 2.0 * i as f32, 5.25),
                Cuboid::new(na::Vector2::new(1.0, 0.2) / 2.0),
                CollisionGroups::new()
                    .with_membership(&[collide::WORLD])
                    .with_whitelist(&[collide::PLAYER, collide::ENEMY]),
            );
        }*/

        const ENEMY_COUNT: usize = 5;
        for i in 0..ENEMY_COUNT {
            let angle = (std::f32::consts::PI * 2.0 / (ENEMY_COUNT as f32)) * (i as f32);
            let loc = na::UnitComplex::from_angle(angle) * na::Vector2::repeat(15.0);
            let base_group = CollisionGroups::new().with_membership(&[collide::ENEMY]);
            let knock_back_not_collide = [collide::ENEMY, collide::PLAYER];

            let slime_art = world.config.draw.art("slime frames/spritesheet.png");
            let bread = world.ecs.spawn((
                draw::Looks::art(slime_art),
                draw::AnimationFrame(
                    i * world
                        .config
                        .draw
                        .get(slime_art)
                        .spritesheet
                        .unwrap()
                        .frame_rate,
                ),
                combat::Health::new(10),
                phys::collision::RigidGroups(base_group.with_blacklist(&knock_back_not_collide)),
                phys::LurchChase::new(world.player.entity, 0.35, 0.89),
                phys::KnockBack {
                    groups: base_group.with_whitelist(&knock_back_not_collide),
                    force_decay: 0.8,
                    force_magnitude: 0.7,
                    use_force_direction: false,
                    minimum_speed: None,
                },
            ));
            world.add_hitbox(
                bread,
                na::Isometry2::new(loc, 0.0),
                Cuboid::new(na::Vector2::new(1.1, 0.35) / 2.0),
                base_group,
            );
        }

        world
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
        player::movement(self);
        phys::velocity(self);
        phys::chase(self);
        collision::collision(self);

        player::aiming(self);
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

        let Self { ui, config, .. } = self;
        ui.macroquad(|ui| config.dev_ui(ui));
    }
}
