// #![feature(array_value_iter)]

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
type CollisionWorld = ncollide2d::world::CollisionWorld<f32, hecs::Entity>;
pub use ncollide2d::pipeline::CollisionGroups;

#[derive(Clone, Copy)]
pub struct PhysHandle(pub ncollide2d::pipeline::CollisionObjectSlabHandle);

const DIMENSIONS: Vector = Vector { x: 480.0, y: 270.0 };
const TILE_SIZE: f32 = 16.0;
const SCALE: f32 = 4.0;

/// Collision Group Constants
pub mod collide {
    pub const PLAYER: usize = 1;
    pub const WEAPON: usize = 2;
    pub const ENEMY: usize = 3;
    pub const PARTICLE: usize = 4;

    /// Fences, Terrain, etc.
    pub const WORLD: usize = 5;
    pub const FARMABLE: usize = 6;

    // yeah
    pub const GUI: usize = 10;
    pub const PLANTING_CURSOR: usize = 11;
}

mod combat;
mod config;
mod farm;
mod gui;
mod items;
mod phys;
use config::ConfigHandler;
mod tilemap;
use phys::{aiming, collision, movement};
mod graphics;
use graphics::images::{fetch_images, ImageMap};

pub struct L8r(Vec<Box<dyn FnOnce(&mut World)>>);
impl L8r {
    pub fn new() -> Self {
        L8r(Vec::new())
    }

    pub fn schedule(&mut self, then: Box<dyn FnOnce(&mut World)>) {
        self.0.push(then);
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

    pub fn spawn<C: 'static + Send + Sync + hecs::DynamicBundle>(&mut self, components_bundle: C) {
        self.l8r(move |world| drop(world.ecs.spawn(components_bundle)))
    }

    pub fn despawn(&mut self, entity: hecs::Entity) {
        self.l8r(move |world| drop(world.ecs.despawn(entity)))
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
    pub phys: CollisionWorld,
}
impl World {
    fn new() -> Self {
        Self {
            ecs: hecs::World::new(),
            l8r: L8r::new(),
            phys: CollisionWorld::new(0.02),
        }
    }

    #[inline]
    fn add_hitbox(
        &mut self,
        entity: hecs::Entity,
        iso: Iso2,
        cuboid: Cuboid<f32>,
        groups: CollisionGroups,
    ) -> PhysHandle {
        let (h, _) = self.phys.add(
            iso,
            ncollide2d::shape::ShapeHandle::new(cuboid),
            groups,
            ncollide2d::pipeline::GeometricQueryType::Contacts(0.0, 0.0),
            entity,
        );
        let hnd = PhysHandle(h);
        self.ecs
            .insert_one(entity, phys::collision::Contacts::new())
            .unwrap_or_else(|e| {
                panic!(
                    "Couldn't insert Contacts for Entity[{:?}] when adding hitbox: {}",
                    entity, e
                )
            });
        self.ecs.insert_one(entity, hnd).unwrap_or_else(|e| {
            panic!(
                "Couldn't insert PhysHandle[{:?}] for Entity[{:?}] to add hitbox: {}",
                h, entity, e
            )
        });

        hnd
    }
}

struct Game {
    last_render: Instant,
    world: World,
    images: ImageMap,
    font: Asset<Font>,
    config: ConfigHandler,
    particle_manager: graphics::particle::Manager,
    gui: gui::GuiState,
    sprite_sheet_animation_failed: bool,
}

impl State for Game {
    fn new() -> Result<Game> {
        let config = ConfigHandler::new().unwrap_or_else(|e| panic!("{}", e));
        let images = fetch_images();

        let mut world = World::new();

        let player = config.spawn(&mut world);
        let player_loc = (|| {
            let PhysHandle(h) = *world.ecs.get::<PhysHandle>(player).ok()?;
            Some(
                world
                    .phys
                    .collision_object(h)?
                    .position()
                    .translation
                    .vector,
            )
        })()
        .unwrap();

        for i in 0..4 {
            let fence = world.ecs.spawn((
                graphics::Appearance {
                    kind: graphics::AppearanceKind::image("smol_fence"),
                    z_offset: -0.01,
                    ..Default::default()
                },
                collision::CollisionStatic,
            ));
            world.add_hitbox(
                fence,
                Iso2::translation(8.0 + 2.0 * i as f32, 5.25),
                Cuboid::new(Vec2::new(1.0, 0.2) / 2.0),
                CollisionGroups::new()
                    .with_membership(&[collide::WORLD])
                    .with_whitelist(&[collide::PLAYER, collide::ENEMY]),
            );
        }

        const ENEMY_COUNT: usize = 4;
        for i in 0..ENEMY_COUNT {
            let angle = (std::f32::consts::PI * 2.0 / (ENEMY_COUNT as f32)) * (i as f32);
            let loc = player_loc + na::UnitComplex::from_angle(angle) * Vec2::repeat(5.0);
            let base_group = CollisionGroups::new().with_membership(&[collide::ENEMY]);
            let knock_back_not_collide = [collide::ENEMY, collide::PLAYER];

            let bread = world.ecs.spawn((
                graphics::Appearance {
                    kind: graphics::AppearanceKind::image("sandwich"),
                    alignment: graphics::Alignment::Center,
                    ..Default::default()
                },
                combat::health::Health::new(10),
                combat::DamageReceivedParticleEmitters(vec![
                    config.particles["blood_splash"].clone()
                ]),
                DeathParticleEmitters(vec![config.particles["arterial_spray"].clone()]),
                phys::collision::RigidGroups(base_group.with_blacklist(&knock_back_not_collide)),
                phys::Charge::new(0.05),
                phys::LookChase::new(player, 0.025),
                phys::KnockBack {
                    groups: base_group.with_whitelist(&knock_back_not_collide),
                    force_decay: 0.75,
                    force_magnitude: 0.2,
                    use_force_direction: false,
                    minimum_speed: None,
                },
            ));
            world.add_hitbox(
                bread,
                Iso2::new(loc, angle),
                Cuboid::new(Vec2::new(1.0, 1.0) / 2.0),
                base_group,
            );
        }

        // Tilemap stuffs
        tilemap::build_map_entities(&mut world, &config);
        farm::build_planting_cursor_entity(&mut world, &config);

        Ok(Game {
            world,
            images,
            font: Asset::new(Font::load("min.ttf")),
            config,
            particle_manager: Default::default(),
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
        self.config.reload(&mut self.world);

        graphics::fade::fade(&mut self.world);

        movement::movement(&mut self.world, window);
        phys::velocity(&mut self.world);
        phys::chase(&mut self.world);
        collision::collision(&mut self.world);

        let mouse = window.mouse();
        let draggable_under_mouse = self.gui.draggable_under(mouse.pos(), &self.world);
        if draggable_under_mouse.is_some() || self.gui.is_dragging() {
            self.gui
                .update_draggable_under_mouse(&mut self.world, draggable_under_mouse, &mouse);
        } else {
            aiming::aiming(&mut self.world, window, &self.config);
            farm::planting(&mut self.world, window);
        }

        combat::hurtful_damage(&mut self.world);
        combat::health::remove_out_of_health(&mut self.world);

        let scheduled_world_edits = self.world.l8r.drain();
        L8r::now(scheduled_world_edits, &mut self.world);

        self.particle_manager.emit_particles(&mut self.world);

        let scheduled_world_edits = self.world.l8r.drain();
        L8r::now(scheduled_world_edits, &mut self.world);

        death_particles(&mut self.world);
        phys::collision::clear_dead_collision_objects(&mut self.world);
        clear_dead(&mut self.world);

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
            //multisampling: Some(16),
            resize: quicksilver::graphics::ResizeStrategy::IntegerScale {
                width: 480,
                height: 270,
            },
            //fullscreen: true,
            ..Settings::default()
        },
    );
}

pub struct Dead;
pub struct DeathParticleEmitters(Vec<graphics::particle::Emitter>);

pub fn death_particles(world: &mut World) {
    let ecs = &world.ecs;
    let phys = &world.phys;
    let l8r = &mut world.l8r;

    for (_, (_, &PhysHandle(h), particles)) in
        &mut ecs.query::<(&Dead, &_, &DeathParticleEmitters)>()
    {
        (|| {
            let mut iso = Iso2::identity();
            iso.translation = phys.collision_object(h)?.position().translation;

            for emitter in particles.0.iter().cloned() {
                l8r.l8r(move |world| {
                    emitter.spawn_instance(world, iso);
                });
            }

            Some(())
        })();
    }
}

pub fn clear_dead(world: &mut World) {
    let to_kill = world
        .ecs
        .query::<&Dead>()
        .iter()
        .map(|(ent, _)| ent)
        .collect::<Vec<hecs::Entity>>();

    to_kill.into_iter().for_each(|ent| {
        world
            .ecs
            .despawn(ent)
            .unwrap_or_else(|e| panic!("Couldn't kill Dead[{:?}]: {}", ent, e))
    });
}
