use hecs::World;
use nalgebra as na;
use ncollide2d::shape::Cuboid;
use quicksilver::{
    geom::Vector,
    lifecycle::{run, Settings, State, Window},
    Result,
};

type Vec2 = na::Vector2<f32>;
type Iso2 = na::Isometry2<f32>;

mod config;
mod items;
use config::Config;
mod phys;
use phys::{aiming, collision, movement};
mod graphics;
use graphics::images::{fetch_images, ImageMap};

struct Game {
    world: World,
    images: ImageMap,
    config: Config,
}

impl State for Game {
    fn new() -> Result<Game> {
        let config = Config::load().unwrap_or_else(|e| panic!("{}", e));
        let images = fetch_images();

        let mut world = World::new();

        let spears: Vec<hecs::Entity> = (0..1000)
            .map(|_| {
                world.spawn((
                    graphics::Appearance {
                        kind: graphics::AppearanceKind::image("spear"),
                        z_offset: 90.0,
                        ..Default::default()
                    },
                    aiming::Weapon::new()
                        .with_bottom_padding(30.0)
                        .with_offset(Vec2::y() * -30.0)
                        .with_equip_time(120)
                        .with_speed(15.3),
                ))
            })
            .collect();

        world.spawn((
            graphics::Appearance {
                kind: graphics::AppearanceKind::image("ferris"),
                ..Default::default()
            },
            Cuboid::new(Vec2::new(58.0, 8.0)),
            Iso2::translation(300.0, 300.0),
            movement::PlayerControlled,
            aiming::PlayerControlled,
            items::Inventory::new_with(&spears[1..1000], &world)
                .unwrap()
                .with_equip(spears[0], &world),
        ));
        for i in 0..4 {
            world.spawn((
                graphics::Appearance {
                    kind: graphics::AppearanceKind::image("fence"),
                    ..Default::default()
                },
                collision::CollisionStatic,
                Cuboid::new(Vec2::new(64.0, 8.0)),
                Iso2::translation(500.0 + (50.0 * i as f32), 300.0 + (50.0 * i as f32)),
            ));
        }

        Ok(Game {
            world,
            images,
            config,
        })
    }

    fn draw(&mut self, window: &mut Window) -> Result<()> {
        graphics::render(window, &self.world, &mut self.images)?;

        Ok(())
    }

    fn update(&mut self, window: &mut Window) -> Result<()> {
        #[cfg(feature = "hot-keyframes")]
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
        Vector::new(800, 600),
        Settings {
            //multisampling: Some(16),
            scale: quicksilver::graphics::ImageScaleStrategy::Blur,
            ..Settings::default()
        },
    );
}
