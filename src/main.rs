use hecs::World;
use nalgebra as na;
use ncollide2d::shape::Cuboid;
use quicksilver::{
    geom::Vector,
    lifecycle::{run, Settings, State, Window},
    Result,
};
use std::time::Instant;

type Vec2 = na::Vector2<f32>;
type Iso2 = na::Isometry2<f32>;

const DIMENSIONS: Vector = Vector { x: 480.0, y: 270.0 };
const TILE_SIZE: f32 = 16.0;
const SCALE: f32 = 3.0;

mod config;
use config::ConfigHandler;
mod farm;
mod items;
mod phys;
mod tilemap;
use phys::{aiming, collision, movement};
mod graphics;
use graphics::images::{fetch_images, ImageMap};

struct Game {
    last_render: Instant,
    world: World,
    images: ImageMap,
    config: ConfigHandler,
    sprite_sheet_animation_failed: bool,
}

impl State for Game {
    fn new() -> Result<Game> {
        let mut config = ConfigHandler::new().unwrap_or_else(|e| panic!("{}", e));
        let images = fetch_images();

        let mut world = World::new();

        let last_keyframe = config.keyframes.pop().unwrap();
        let spears: Vec<hecs::Entity> = (0..1000)
            .map(|_| {
                world.spawn((
                    graphics::Appearance {
                        kind: graphics::AppearanceKind::image("trench_shovel"),
                        z_offset: 90.0,
                        ..Default::default()
                    },
                    aiming::Weapon {
                        bottom_padding: last_keyframe.bottom_padding,
                        offset: last_keyframe.pos,
                        equip_time: 50,
                        speed: 3.0,
                        ..Default::default()
                    },
                ))
            })
            .collect();

        world.spawn((
            graphics::Appearance {
                kind: graphics::AppearanceKind::image(&config.player.image),
                ..Default::default()
            },
            Cuboid::new(config.player.size / 2.0),
            Iso2::translation(config.player.pos.x, config.player.pos.y),
            movement::PlayerControlled {
                speed: config.player.speed,
            },
            aiming::Wielder::new(),
            items::Inventory::new_with(&spears[1..1000], &world)
                .unwrap()
                .with_equip(spears[0], &world),
            graphics::sprite_sheet::Animation::new(),
            graphics::sprite_sheet::Index::new(),
        ));

        for i in 0..4 {
            world.spawn((
                graphics::Appearance {
                    kind: graphics::AppearanceKind::image("smol_fence"),
                    ..Default::default()
                },
                collision::CollisionStatic,
                Cuboid::new(Vec2::new(1.0, 0.2) / 2.0),
                Iso2::translation(8.0 + i as f32, 5.0),
            ));
        }

        // Tilemap stuffs
        tilemap::new_tilemap(&config.tilemap, &config.tiles, &mut world);

        Ok(Game {
            world,
            images,
            config,
            last_render: Instant::now(),
            sprite_sheet_animation_failed: false,
        })
    }

    fn draw(&mut self, window: &mut Window) -> Result<()> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_render);

        if !self.sprite_sheet_animation_failed {
            graphics::sprite_sheet::animate(&mut self.world, &self.config, elapsed).unwrap_or_else(
                |e| {
                    println!("Disabling sprite sheet animation: {}", e);
                    self.sprite_sheet_animation_failed = true;
                },
            );
        }
        graphics::render(window, &self.world, &mut self.images, &self.config)?;

        self.last_render = now;
        Ok(())
    }

    fn update(&mut self, window: &mut Window) -> Result<()> {
        #[cfg(feature = "hot-config")]
        self.config.reload();

        movement::movement(&mut self.world, window);
        phys::velocity(&mut self.world);
        collision::collision(&mut self.world);
        aiming::aiming(&mut self.world, window, &self.config);

        Ok(())
    }
}

fn main() {
    run::<Game>(
        "Game",
        DIMENSIONS * SCALE,
        Settings {
            resize: quicksilver::graphics::ResizeStrategy::IntegerScale {
                width: 480,
                height: 270,
            },
            ..Settings::default()
        },
    );
}
