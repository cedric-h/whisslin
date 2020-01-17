use crate::graphics::particle;
use crate::{collide, CollisionGroups, Iso2, PhysHandle, Vec2};

pub struct Farmable;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GrowthStage {
}

/// Entities with this component will be dragged toward the center of the tile the player's mouse
/// is over, if that tile can be planted on and the player is holding a seed.
pub struct PlantingCursor;

pub fn build_planting_cursor_entity(world: &mut crate::World, config: &crate::config::Config) {
    let mut emitter = config.particles["planting_cursor"].clone();
    emitter.offset_direction_bounds(Vec2::y_axis());
    emitter.status = particle::EmitterStatus::Disabled;

    let planting_cursor = world.ecs.spawn((emitter, PlantingCursor));
    world.add_hitbox(
        planting_cursor,
        Iso2::identity(),
        ncollide2d::shape::Cuboid::new(Vec2::repeat(0.5)),
        CollisionGroups::new()
            .with_membership(&[])
            .with_whitelist(&[]),
    );
}

pub fn planting(
    world: &mut crate::World,
    window: &mut quicksilver::lifecycle::Window,
) -> Option<()> {
    let mut planting_query = world
        .ecs
        .query::<(&PlantingCursor, &_, &mut particle::Emitter)>();
    let (_, (_, &PhysHandle(cursor_h), emitter)) = planting_query.iter().next()?;

    let farmable_under_mouse = world
        .phys
        .interferences_with_point(
            &window.mouse().pos().into_vector().into(),
            &CollisionGroups::new()
                .with_membership(&[collide::PLANTING_CURSOR])
                .with_whitelist(&[collide::FARMABLE]),
        )
        .next();

    emitter.status = if farmable_under_mouse.is_some() {
        particle::EmitterStatus::Active
    } else {
        particle::EmitterStatus::Disabled
    };

    let farm_tile_iso = {
        let (_, farm_tile_obj) = farmable_under_mouse?;
        farm_tile_obj.position().clone()
    };
    let cursor_obj = world.phys.get_mut(cursor_h)?;

    cursor_obj.set_position(farm_tile_iso);

    Some(())
}
