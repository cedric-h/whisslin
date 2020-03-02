use crate::collide;
use crate::graphics::{Appearance, AppearanceKind};
use crate::phys::collision;
use crate::{Iso2, Vec2};
use ncollide2d::pipeline::CollisionGroups;
use ncollide2d::shape::Cuboid;

pub struct Prop;

// TODO: Change to build_fence,
pub fn spawn_fence(world: &mut crate::World, position: Iso2) {
    let fence = world.ecs.spawn((
        Appearance {
            kind: AppearanceKind::image("smol_fence"),
            z_offset: -0.01,
            ..Default::default()
        },
        collision::CollisionStatic,
        Prop,
    ));
    world.add_hitbox(
        fence,
        position,
        Cuboid::new(Vec2::new(1.0, 0.2) / 2.0),
        CollisionGroups::new()
            .with_membership(&[collide::WORLD])
            .with_whitelist(&[collide::PLAYER, collide::ENEMY]),
    );
}

pub fn despawn_props(world: &mut crate::World) {
    let to_unload = world
        .ecs
        .query::<&Prop>()
        .iter()
        .map(|(id, _)| id)
        .collect::<Vec<hecs::Entity>>();
    let to_unload_physic_handles = world
        .ecs
        .query::<(&Prop, &crate::PhysHandle)>()
        .iter()
        .map(|(_, (_, handle))| *handle)
        .collect::<Vec<_>>();

    for ent in to_unload.into_iter() {
        world.ecs.despawn(ent).unwrap_or_else(|err| {
            println!(
                "unload_map: Couldn't delete entity[{:?}] marked with Tile: {}",
                ent, err
            )
        });
    }

    world.phys.remove(&to_unload_physic_handles);
}
