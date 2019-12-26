use crate::config::TileProperty;
use crate::farm::Farmable;
use crate::phys::collision;
use crate::{graphics, Iso2, Vec2};
use hecs::{EntityBuilder, World};
use ncollide2d::shape::Cuboid;
use std::collections::HashMap;

const TILE_SIZE: usize = 16;
const HALF_TILE: usize = TILE_SIZE / 2;

pub fn new_tilemap(tilemap: &str, tile_prop: &HashMap<String, TileProperty>, world: &mut World) {
    let mut tile_builder = EntityBuilder::new();
    let default = TileProperty::default();

    tilemap.split_whitespace().enumerate().for_each(|(y, row)| {
        row.split(',').enumerate().for_each(|(x, tile)| {
            let tile_details = tile_prop.get(tile).unwrap_or(&default);

            tile_builder
                .add(graphics::Appearance {
                    kind: graphics::AppearanceKind::image(tile_details.image.clone()),
                    alignment: graphics::Alignment::Center,
                    z_offset: -1000.0,
                    ..Default::default()
                })
                .add(Iso2::translation(
                    (HALF_TILE + TILE_SIZE * x) as f32,
                    (HALF_TILE + TILE_SIZE * y) as f32,
                ));

            if tile_details.farmable {
                tile_builder.add(Farmable {});
            }
            if tile_details.collidable {
                tile_builder
                    .add(collision::CollisionStatic)
                    .add(Cuboid::new(Vec2::repeat(HALF_TILE as f32)));
            }

            world.spawn(tile_builder.build());
        })
    })
}
