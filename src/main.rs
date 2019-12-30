use nalgebra as na;
use ncollide2d::shape::Cuboid;
use quicksilver::{
    geom::Vector,
    graphics::Font,
    lifecycle::{run, Asset, Settings, State, Window},
    Result,
};
use std::time::Instant;

type Vec2 = na::Vector2<f32>;
type Iso2 = na::Isometry2<f32>;

const DIMENSIONS: Vector = Vector { x: 480.0, y: 270.0 };
const TILE_SIZE: f32 = 16.0;
const SCALE: f32 = 4.0;

mod config;
use config::ConfigHandler;
mod farm;
mod gui;
mod items;
mod phys;
mod tilemap;
use phys::{aiming, collision, movement};
mod graphics;
use graphics::images::{fetch_images, ImageMap};

pub struct L8r(Vec<Box<dyn FnOnce(&mut World)>>);
impl L8r {
    pub fn new() -> Self {
        L8r(Vec::new())
    }

    pub fn l8r<F: 'static + Send + Sync + FnOnce(&mut World)>(&mut self, then: F) {
        self.0.push(Box::new(then));
    }

    pub fn insert_one<C: hecs::Component>(&mut self, ent: hecs::Entity, component: C) {
        self.l8r(move |world| world.ecs.insert_one(ent, component).unwrap())
    }

    pub fn remove_one<C: hecs::Component>(&mut self, ent: hecs::Entity) {
        self.l8r(move |world| drop(world.ecs.remove_one::<C>(ent)))
    }

    pub fn insert<C: 'static + Send + Sync + hecs::DynamicBundle>(
        &mut self,
        ent: hecs::Entity,
        components_bundle: C,
    ) {
        self.l8r(move |world| world.ecs.insert(ent, components_bundle).unwrap())
    }

    pub fn drain(&mut self) -> Vec<Box<dyn FnOnce(&mut World)>> {
        self.0.drain(..).collect::<Vec<_>>()
    }

    pub fn now(l8rs: Vec<Box<dyn FnOnce(&mut World)>>, world: &mut World) {
        for l8r in l8rs.into_iter() {
            l8r(world);
        }
    }
}

pub struct World {
    pub ecs: hecs::World,
    pub l8r: L8r,
}
impl World {
    fn new() -> Self {
        Self {
            ecs: hecs::World::new(),
            l8r: L8r::new(),
        }
    }
}

struct Game {
    last_render: Instant,
    world: World,
    images: ImageMap,
    font: Asset<Font>,
    config: ConfigHandler,
    gui: gui::GuiState,
    sprite_sheet_animation_failed: bool,
}

impl State for Game {
    fn new() -> Result<Game> {
        let mut config = ConfigHandler::new().unwrap_or_else(|e| panic!("{}", e));
        let images = fetch_images();

        let mut world = World::new();

        let last_keyframe = config.keyframes.pop().unwrap();

        let player = world.ecs.spawn((
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
            items::Inventory::new(),
            items::InventoryEquip("spear"),
            graphics::sprite_sheet::Animation::new(),
            graphics::sprite_sheet::Index::new(),
        ));

        for _ in 0..99 {
            world.ecs.spawn((
                graphics::Appearance {
                    kind: graphics::AppearanceKind::image("spear"),
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
                items::InventoryInsert(player),
            ));
        }

        for i in 0..4 {
            world.ecs.spawn((
                graphics::Appearance {
                    kind: graphics::AppearanceKind::image("smol_fence"),
                    ..Default::default()
                },
                collision::CollisionStatic,
                Cuboid::new(Vec2::new(1.0, 0.2) / 2.0),
                Iso2::translation(8.0 + i as f32, 5.25),
            ));
        }

        // Tilemap stuffs
        tilemap::new_tilemap(&config.tilemap, &config.tiles, &mut world);

        // attach the inventory GUI window to the player
        let window = gui::build_inventory_gui_entities(&mut world);
        world.ecs.insert_one(player, window).unwrap();

        Ok(Game {
            world,
            images,
            font: Asset::new(Font::load("min.ttf")),
            config,
            gui: gui::GuiState::new(),
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
        graphics::render(
            window,
            &self.world,
            &mut self.images,
            &mut self.font,
            &self.config,
        )?;

        self.last_render = now;
        Ok(())
    }

    fn update(&mut self, window: &mut Window) -> Result<()> {
        #[cfg(feature = "hot-config")]
        self.config.reload();

        movement::movement(&mut self.world, window);
        phys::velocity(&mut self.world);
        collision::collision(&mut self.world);

        let mouse = window.mouse();
        let draggable_under_mouse = self.gui.draggable_under(mouse.pos(), &self.world);
        if draggable_under_mouse.is_some() || self.gui.is_dragging() {
            self.gui
                .update_draggable_under_mouse(&mut self.world, draggable_under_mouse, &mouse);
        } else {
            aiming::aiming(&mut self.world, window, &self.config);
        }

        let scheduled_world_edits = self.world.l8r.drain();
        L8r::now(scheduled_world_edits, &mut self.world);

        gui::inventory_events(&mut self.world, &mut self.images);
        items::inventory_inserts(&mut self.world);

        Ok(())
    }
}

fn main() {
    run::<Game>(
        "Game",
        dbg!(DIMENSIONS * SCALE),
        Settings {
            resize: quicksilver::graphics::ResizeStrategy::IntegerScale {
                width: 480,
                height: 270,
            },
            ..Settings::default()
        },
    );
}
