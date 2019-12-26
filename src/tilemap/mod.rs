use crate::farm::Farmable;
use crate::phys::collision;
use crate::{graphics, Iso2, Vec2};
use hecs::{EntityBuilder, World};
use ncollide2d::shape::Cuboid;
use std::collections::HashMap;

#[derive(Debug, serde::Deserialize)]
pub struct TileProperties {
    pub image: String,
    #[serde(default)]
    pub flags: Vec<String>
}

impl Default for TileProperties {
    fn default() -> Self {
        Self {
            image: String::from("unknown"),
            flags: vec![]
        }
    }
}
pub fn build_tile_entities(map: &str, tile_properties: &HashMap<String, TileProperties>, world: &mut World) {
    let mut tile_entity = EntityBuilder::new();
    let default = TileProperties::default();

    map.split_whitespace().enumerate().for_each(|(y, row)| {
        row
            .chars()
            .collect::<Vec<_>>()
            .chunks(2)
            .map(|x| x.iter().collect::<String>())
            .enumerate()
            .for_each(|(x, tile_symbol)| {
                let TileProperties { flags, image } = tile_properties
                    .get(&tile_symbol)
                    .unwrap_or(&default);

                tile_entity
                    .add(graphics::Appearance {
                        kind: graphics::AppearanceKind::image(image),
                        alignment: graphics::Alignment::Center,
                        z_offset: -1000.0,
                        ..Default::default()
                    })
                    .add(Iso2::translation(0.5 + (x as f32), 0.5 + (y as f32)));

                for flag in flags.iter() {
                    match flag.as_ref() {
                        "farmable" => {
                            tile_entity.add(Farmable);
                        },
                        "collidable" => {
                            tile_entity
                                .add(collision::CollisionStatic)
                                .add(Cuboid::new(Vec2::repeat(0.5)));
                        },
                        other => println!("Unknown flag: {}", other),
                    };
                }

                world.spawn(tile_entity.build());
            })
        })
}
