use l8r::L8r;
use macroquad::*;

use crate::{
    combat::{self, aiming},
    draw,
    phys::{
        self, collide, collision, movement, CollisionGroups, CollisionWorld, Cuboid, PhysHandle,
    },
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

pub struct World {
    pub ecs: hecs::World,
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
        use aiming::Rot;
        let config = Config {
            draw: draw::Config {
                zoom: 8.0,
                tile_size: 1.5,
                art: vec![
                    draw::ArtConfig {
                        file: "golem frames/down_spritesheet.png".to_string(),
                        scale: 0.0078125,
                        spritesheet: Some(draw::Spritesheet {
                            rows: 3,
                            columns: 4,
                            total: 12,
                            frame_rate: 2,
                            hold_at: 3
                        })
                    },
                    draw::ArtConfig {
                        file: "golem frames/side_spritesheet.png".to_string(),
                        scale: 0.0078125,
                        spritesheet: Some(draw::Spritesheet {
                            rows: 3,
                            columns: 4,
                            total: 12,
                            frame_rate: 2,
                            hold_at: 3
                        })
                    },
                    draw::ArtConfig {
                        file: "baseball.png".to_string(),
                        scale: 0.005,
                        spritesheet: None
                    },
                    draw::ArtConfig {
                        file: "grass_1.png".to_string(),
                        scale: 0.005,
                        spritesheet: None
                    },
                    draw::ArtConfig {
                        file: "grass_2.png".to_string(),
                        scale: 0.005,
                        spritesheet: None
                    },
                    draw::ArtConfig {
                        file: "grass_3.png".to_string(),
                        scale: 0.005,
                        spritesheet: None
                    },
                    draw::ArtConfig {
                        file: "slime frames/spritesheet.png".to_string(),
                        scale: 0.0078125,
                        spritesheet: Some(draw::Spritesheet {
                            rows: 4,
                            columns: 5,
                            total: 19,
                            frame_rate: 3,
                            hold_at: 0
                        })
                    }
                ]
            },
            player: player::Config {
                weapon: aiming::Weapon {
                    bottom_offset: 0.325,
                    offset: na::Vector2::new(0.115, -0.385),
                    readying_time: 60,
                    equip_time: 60,
                    hitbox_size: na::Vector2::new(0.24, 2.2),
                    hitbox_groups: aiming::weapon_hitbox_groups(),
                    prelaunch_groups: aiming::weapon_prelaunch_groups(),
                    force_magnitude: 2.125,
                    force_decay: 0.75,
                    boomerang: true,
                    player_knock_back_force: 0.5,
                    player_knock_back_decay: 0.75
                },
                keyframes: vec![
                    aiming::KeyFrame {
                        time: 0.0,
                        pos: na::Vector2::new(-0.2, -0.4),
                        rot: Rot(25.0),
                        bottom_offset: -0.5
                    },
                    aiming::KeyFrame {
                        time: 0.2,
                        pos: na::Vector2::new(0.5, -0.8),
                        rot: Rot(-45.0),
                        bottom_offset: -0.4
                    },
                    aiming::KeyFrame {
                        time: 0.4,
                        pos: na::Vector2::new(0.6, -0.9),
                        rot: Rot(-200.0),
                        bottom_offset: -0.6
                    },
                    aiming::KeyFrame {
                        time: 0.6,
                        pos: na::Vector2::new(0.0, -0.7),
                        rot: Rot(-350.0),
                        bottom_offset: -0.3
                    },
                    aiming::KeyFrame {
                        time: 0.7,
                        pos: na::Vector2::new(0.0, -0.7),
                        rot: Rot(25.0),
                        bottom_offset: 0.2
                    }
                ],
                direction_art: movement::WalkDirectionArtConfig {
                    side: draw::ArtHandle(0),
                    down: draw::ArtHandle(1),
                },
                speed: 0.134,
            },
        };

        let mut world = Self {
            player: Player::new(&mut ecs, &mut phys, &config),
            map: Map::new(&config),
            images: draw::Images::load(&config).await,
            config,
            phys,
            ecs,
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
        movement::movement(self);
        phys::velocity(self);
        phys::chase(self);
        collision::collision(self);

        aiming::aiming(self);
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
}
