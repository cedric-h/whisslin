use crate::{collide, graphics};

pub struct Tile;

pub fn build_map_entities(world: &mut crate::World, map_name: &str) {
    let conf_test = world.config.tilemaps[map_name].layout.clone();

    conf_test
        .split_whitespace()
        .enumerate()
        .for_each(|(y, row)| {
            row.chars()
                .collect::<Vec<_>>()
                .chunks(2)
                .map(|x| x.iter().collect::<String>())
                .enumerate()
                .for_each(|(x, tile)| {
                    let tile_details = world.config.tiles.get(&tile).cloned().unwrap_or_default();

                    let tile_ent = world.ecs.spawn((graphics::Appearance {
                        kind: graphics::AppearanceKind::image(tile_details.image.clone()),
                        alignment: graphics::Alignment::Center,
                        z_offset: -1000.0,
                        ..Default::default()
                    }, Tile{}));

                    let pos = crate::Iso2::translation(0.5 + (x as f32), 0.5 + (y as f32));

                    if tile_details.farmable {
                        world.ecs
                            .insert_one(tile_ent, crate::farm::Farmable)
                            .unwrap_or_else(|e| {
                                panic!(
                                    "Can't insert Iso2 when building Tile: {}, tile properties: {:?}",
                                    e, tile_details
                                )
                            });
                    }

                    // these two flags require a hitbox for the ent
                    if tile_details.farmable || tile_details.collidable {
                        let groups = crate::CollisionGroups::new()
                            .with_membership(&[collide::WORLD])
                            .with_whitelist(&[]);
                        world.add_hitbox(
                            tile_ent,
                            pos,
                            ncollide2d::shape::Cuboid::new(crate::Vec2::repeat(0.5)),
                            if tile_details.collidable {
                                groups.with_whitelist(&[collide::PLAYER, collide::ENEMY])
                            } else if tile_details.farmable {
                                groups
                                    .with_membership(&[collide::WORLD, collide::FARMABLE])
                                    .with_whitelist(&[collide::PLANTING_CURSOR])
                            } else {
                                unreachable!()
                            },
                        );
                    } else {
                        world.ecs.insert_one(tile_ent, pos).unwrap_or_else(|e| {
                            panic!(
                                "Can't insert Iso2 when building Tile: {}, tile properties: {:?}",
                                e, tile_details
                            )
                        });
                    }
                })
        })
}

pub fn unload_map_entities(world: &mut crate::World) {
    let to_unload = world
        .ecs
        .query::<&Tile>()
        .iter()
        .map(|(id, _)| id)
        .collect::<Vec<hecs::Entity>>();
    let to_unload_physic_handles = world
        .ecs
        .query::<(&Tile, &crate::PhysHandle)>()
        .iter()
        .map(|(_, (_, &crate::PhysHandle(handle)))| handle)
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
