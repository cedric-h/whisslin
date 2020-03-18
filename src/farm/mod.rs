use crate::graphics::particle;
use crate::{collide, CollisionGroups, Iso2, PhysHandle, Vec2};

pub struct Farmable;

#[derive(Debug, Clone, serde::Deserialize)]
/// Replaces this entity with the given Item (as described in the config file)
/// after the supplied duration.
pub struct Growth {
    /// Indicates which item in the config file should replace this entity
    /// when this one completes its growth.
    after: String,

    /// How long this growth stage lasts, in frames.
    duration: usize,
}

/// Entities with this component will be dragged toward the center of the tile the player's mouse
/// is over, if that tile can be planted on and the player is holding a seed.
pub struct PlantingCursor;

pub fn build_planting_cursor_entity(world: &mut crate::World) {
    let mut emitter = world.config.particles["planting_cursor"].clone();
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

pub fn growing(world: &mut crate::World) -> Option<()> {
    let ecs = &world.ecs;
    let l8r = &mut world.l8r;

    for (growing_ent, growth) in &mut ecs.query::<&mut Growth>() {
        growth.duration -= 1;
        if growth.duration == 0 {
            let after = growth.after.clone();
            l8r.l8r(move |world: &mut crate::World| {
                let config = std::rc::Rc::clone(&world.config);

                let old_h = world
                    .ecs
                    .get::<PhysHandle>(growing_ent)
                    .ok()
                    .as_deref()
                    .map(|x| x.clone());
                world
                    .l8r
                    .insert_one(growing_ent, crate::graphics::particle::death::Dead);

                let next_stage_ent = config
                    .items
                    .get(&after)
                    .unwrap_or_else(|| {
                        panic!(
                            "Growth[{:?}] referenced invalid {:?} item!",
                            growing_ent, after
                        )
                    })
                    .spawn(world);

                if let Some(old_obj) = old_h.and_then(|h| world.phys.collision_object(h)) {
                    let old_pos = old_obj.position().clone();
                    let old_shape = old_obj
                        .shape()
                        .as_shape::<ncollide2d::shape::Cuboid<f32>>()
                        .clone()
                        .unwrap_or_else(|| {
                            panic!(
                                "PhysHandle[{:?}] had PhysHandle component but no Cuboid shape!",
                                growing_ent
                            )
                        })
                        .clone();
                    let old_groups = old_obj.collision_groups().clone();
                    drop(old_obj);

                    world.add_hitbox(next_stage_ent, old_pos, old_shape, old_groups);
                }
            })
        }
    }

    Some(())
}

pub fn planting(
    world: &mut crate::World,
    window: &mut quicksilver::lifecycle::Window,
) -> Option<()> {
    let mut planting_query = world
        .ecs
        .query::<(&PlantingCursor, &PhysHandle, &mut particle::Emitter)>();
    let (_, (_, cursor_h, emitter)) = planting_query.iter().next()?;

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
    let cursor_obj = world.phys.get_mut(*cursor_h)?;

    cursor_obj.set_position(farm_tile_iso);

    Some(())
}
