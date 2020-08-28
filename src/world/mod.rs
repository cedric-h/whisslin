use l8r::L8r;
use macroquad::*;

use crate::{
    combat::{self, aiming},
    draw,
    phys::{
        self, collide, collision, movement, CollisionGroups, CollisionWorld, Cuboid, PhysHandle,
    },
};

mod player;
pub use player::Player;
pub mod map;
pub use map::Map;

pub struct World {
    pub ecs: hecs::World,
    pub l8r: L8r<World>,
    pub map: Map,
    pub phys: CollisionWorld,
    pub player: Player,
    pub dead: Vec<hecs::Entity>,
    pub camera: draw::CedCam2D,
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
    pub fn new() -> Self {
        let mut ecs = hecs::World::new();
        let mut phys = CollisionWorld::new(0.02);
        let player = Player::new(&mut ecs, &mut phys);

        let mut world = Self {
            l8r: L8r::new(),
            dead: vec![],
            camera: draw::CedCam2D::with_zoom(10.0),
            map: Map::new(),
            player,
            phys,
            ecs,
        };

        for i in 0..4 {
            let fence = world
                .ecs
                .spawn((collision::CollisionStatic, draw::Looks::default()));
            world.add_hitbox(
                fence,
                na::Isometry2::translation(8.0 + 2.0 * i as f32, 5.25),
                Cuboid::new(na::Vector2::new(1.0, 0.2) / 2.0),
                CollisionGroups::new()
                    .with_membership(&[collide::WORLD])
                    .with_whitelist(&[collide::PLAYER, collide::ENEMY]),
            );
        }

        const ENEMY_COUNT: usize = 4;
        for i in 0..ENEMY_COUNT {
            let angle = (std::f32::consts::PI * 2.0 / (ENEMY_COUNT as f32)) * (i as f32);
            let loc = na::UnitComplex::from_angle(angle) * na::Vector2::repeat(5.0);
            let base_group = CollisionGroups::new().with_membership(&[collide::ENEMY]);
            let knock_back_not_collide = [collide::ENEMY, collide::PLAYER];

            let bread = world.ecs.spawn((
                draw::Looks::default(),
                combat::health::Health::new(10),
                phys::collision::RigidGroups(base_group.with_blacklist(&knock_back_not_collide)),
                phys::Chase::determined(world.player.entity, 0.05),
                phys::KnockBack {
                    groups: base_group.with_whitelist(&knock_back_not_collide),
                    force_decay: 0.75,
                    force_magnitude: 0.2,
                    use_force_direction: false,
                    minimum_speed: None,
                },
            ));
            world.add_hitbox(
                bread,
                na::Isometry2::new(loc, angle),
                Cuboid::new(na::Vector2::new(1.0, 0.2) / 2.0),
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
