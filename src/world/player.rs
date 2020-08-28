use crate::{
    combat::{self, aiming},
    draw::Looks,
    phys::{self, collide, CollisionGroups, Cuboid, PhysHandle},
};

pub struct Player {
    pub entity: hecs::Entity,
    pub phys_handle: PhysHandle,
    pub weapon: Option<hecs::Entity>,
    pub wielder: aiming::Wielder,
}
impl Player {
    pub fn new(ecs: &mut hecs::World, phys: &mut phys::CollisionWorld) -> Self {
        let wep_ent = ecs.spawn((
            Looks::size(na::Vector2::new(0.24, 2.2)),
            aiming::Weapon {
                // default positioning
                bottom_offset: 0.325,
                offset: na::Vector2::new(0.115, -0.385),
                // animations
                readying_time: 60,
                equip_time: 60,
                // projectile
                hitbox_size: na::Vector2::new(0.24, 2.2),
                hitbox_groups: {
                    phys::CollisionGroups::new()
                        .with_membership(&[phys::collide::WEAPON])
                        .with_whitelist(&[phys::collide::WORLD, phys::collide::ENEMY])
                },
                prelaunch_groups: {
                    phys::CollisionGroups::new()
                        .with_membership(&[phys::collide::WEAPON])
                        .with_blacklist(&[phys::collide::PLAYER, phys::collide::ENEMY])
                },
                force_magnitude: 2.125,
                force_decay: 0.75,
                boomerang: true,
                // side effects
                player_knock_back_force: 0.5,
                player_knock_back_decay: 0.75,
            },
            combat::Hurtful {
                minimum_speed: 0.05,
                raw_damage: 1.0,
                minimum_damage: 1,
                kind: combat::HurtfulKind::Ram {
                    speed_damage_coefficient: 1.0,
                },
            },
            phys::KnockBack {
                groups: CollisionGroups::new()
                    .with_membership(&[collide::WEAPON])
                    .with_whitelist(&[collide::ENEMY]),
                force_decay: 0.75,
                force_magnitude: 0.75,
                use_force_direction: true,
                // TODO: separate minimum_speed_to_knock_back
                minimum_speed: Some(0.05),
            },
        ));

        let ent = ecs.spawn((Looks::size(na::Vector2::new(0.7, 1.2)),));
        Player {
            entity: ent,
            phys_handle: phys::phys_insert(
                ecs,
                phys,
                ent,
                na::Isometry::identity(),
                Cuboid::new(na::Vector2::new(0.7, 0.3) / 2.0),
                CollisionGroups::new().with_membership(&[phys::collide::PLAYER]),
            ),
            weapon: Some(wep_ent),
            wielder: aiming::Wielder::new(),
        }
    }
}
