use crate::config::TileProperty;
use crate::farm::Farmable;
use crate::phys::collision;
use crate::World;
use crate::{graphics, Iso2, Vec2};
use hecs::EntityBuilder;
use ncollide2d::shape::Cuboid;
use fxhash::FxHashMap;

pub fn new_tilemap(tilemap: &str, tile_prop: &FxHashMap<String, TileProperty>, world: &mut World) {
    let mut tile_builder = EntityBuilder::new();
    let default = TileProperty::default();

    tilemap.split_whitespace().enumerate().for_each(|(y, row)| {
        row.chars()
            .collect::<Vec<_>>()
            .chunks(2)
            .map(|x| x.iter().collect::<String>())
            .enumerate()
            .for_each(|(x, tile)| {
                let tile_details = tile_prop.get(&tile).unwrap_or(&default);

                tile_builder
                    .add(graphics::Appearance {
                        kind: graphics::AppearanceKind::image(tile_details.image.clone()),
                        alignment: graphics::Alignment::Center,
                        z_offset: -1000.0,
                        ..Default::default()
                    })
                    .add(Iso2::translation(0.5 + (x as f32), 0.5 + (y as f32)));

                if tile_details.farmable {
                    tile_builder.add(Farmable {});
                }
                if tile_details.collidable {
                    tile_builder
                        .add(collision::CollisionStatic)
                        .add(Cuboid::new(Vec2::repeat(0.5)));
                }

                world.ecs.spawn(tile_builder.build());
            })
    })
}
