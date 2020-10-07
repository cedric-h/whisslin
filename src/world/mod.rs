use glsp::{eprn, prn};
use l8r::L8r;
use macroquad::*;

use crate::{
    combat, draw,
    phys::{self, collision, CollisionGroups, CollisionWorld, Cuboid, PhysHandle},
};

pub mod player;
pub use player::Player;
pub mod map;
pub use map::Map;
pub mod prefab;
pub mod script;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub draw_debug: bool,
    pub tile: map::Config,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    pub tile_expanded: bool,
    pub draw: draw::Config,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    pub draw_expanded: bool,
    pub player: player::Config,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    pub player_expanded: bool,
    pub prefab: prefab::Config,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    pub prefab_expanded: bool,
}
#[cfg(feature = "confui")]
pub fn dev_ui(ui_plugin: &mut emigui_miniquad::UiPlugin, world: &mut Game) {
    ui_plugin.macroquad(|ui| {
        egui::menu::bar(ui, |ui| {
            egui::menu::menu(ui, "Config", |ui| {
                #[cfg(not(target = "wasm32-unknown-unknown"))]
                if ui.button("Save To File").clicked {
                    std::fs::write(
                        "config.ron",
                        ron::ser::to_string_pretty(&world.config, Default::default()).unwrap(),
                    )
                    .unwrap()
                }
                ui.checkbox("draw debug geometry", &mut world.config.draw_debug);
            });
            egui::menu::menu(ui, "Widgets", |ui| {
                ui.checkbox("Tiling", &mut world.config.tile_expanded);
                ui.checkbox("Draw", &mut world.config.draw_expanded);
                ui.checkbox("Player", &mut world.config.player_expanded);
                ui.checkbox("Prefabs", &mut world.config.prefab_expanded);
            });
        });

        if world.config.tile_expanded {
            egui::Window::new("Tiling")
                .default_pos(egui::pos2(0.0, 50.0))
                .show(ui.ctx(), |ui| {
                    map::dev_ui(world, ui);
                });
        }

        if world.config.draw_expanded {
            egui::Window::new("Draw")
                .default_pos(egui::pos2(0.0, 100.0))
                .show(ui.ctx(), |ui| world.config.draw.dev_ui(ui));
        }

        if world.config.player_expanded {
            egui::Window::new("Player")
                .default_pos(egui::pos2(0.0, 150.0))
                .show(ui.ctx(), |ui| world.config.player.dev_ui(ui));
        }

        if world.config.prefab_expanded {
            egui::Window::new("Prefabs")
                .default_pos(egui::pos2(0.0, 200.0))
                .show(ui.ctx(), |ui| {
                    ui.collapsing("Instances", |ui| {
                        prefab::instances::dev_ui(ui, world);
                    });

                    ui.collapsing("Prefabs", |ui| {
                        prefab::dev_ui(ui, &mut world.config);
                    });
                });
        }
    });
}

#[derive(Default)]
struct IgnoreInputs {
    keyboard: bool,
    mouse: bool,
}

pub struct World {
    pub ui: emigui_miniquad::UiPlugin,
    pub glsp_runtime: glsp::Runtime,
    #[cfg(feature = "confui")]
    pub file_events: std::sync::mpsc::Receiver<notify::DebouncedEvent>,
}
impl World {
    pub async fn new() -> Self {
        let glsp_runtime = glsp::Runtime::new();
        let config = ron::de::from_reader(&*load_file("config.ron").await.unwrap()).unwrap();
        let images = draw::Images::load(&config).await;
        glsp_runtime.run(move || {
            glsp::add_lib(script::Intake::new());
            match glsp::load("script/entry.glsp").and_then(|c| script::Cache::new(&c)) {
                Ok(script_cache) => glsp::add_lib(script_cache),
                Err(e) => eprn!("couldn't load glsp: {}", e),
            }
            glsp::add_lib(Game::new(images, config));
            Ok(())
        });
        Self {
            ui: emigui_miniquad::UiPlugin::new(),
            glsp_runtime,
            #[cfg(feature = "confui")]
            file_events: {
                use notify::{watcher, RecursiveMode, Watcher};
                use std::{sync::mpsc::channel, time::Duration};

                let (tx, rx) = channel();
                let mut wat = watcher(tx, Duration::from_secs(1)).expect("couldn't make watcher");
                wat.watch(
                    concat!(env!("CARGO_MANIFEST_DIR", "./scripts")),
                    RecursiveMode::Recursive,
                )
                .expect("couldn't watch /scripts");
                Box::leak(Box::new(wat));

                rx
            },
        }
    }

    pub fn update(&mut self) {
        use glsp::Lib;

        let ignore_inputs = IgnoreInputs {
            keyboard: self.ui.egui_ctx.wants_keyboard_input(),
            mouse: self.ui.egui_ctx.wants_mouse_input(),
        };

        self.glsp_runtime.run(move || {
            Game::borrow_mut().update(ignore_inputs);
            script::Cache::borrow_mut().update();
            Game::borrow_mut().apply_l8r();
            script::Cache::borrow_mut().cleanup();
            Game::borrow_mut().cleanup();

            Ok(())
        });

        #[cfg(feature = "confui")]
        while let Ok(event) = self.file_events.try_recv() {
            use notify::DebouncedEvent::{Create, Write};
            if matches!(event, Create(_) | Write(_)) {
                self.glsp_runtime.run(|| {
                    prn!("reloading glsp!");

                    if let Err(e) = glsp::load("script/entry.glsp")
                        .and_then(|c| glsp::lib_mut::<script::Cache>().reload(&c))
                    {
                        eprn!("couldn't load glsp: {}", e);
                    }

                    Ok(())
                });
            }
        }
    }

    pub fn draw(&mut self) {
        let Self {
            glsp_runtime, ui, ..
        } = self;
        glsp_runtime.run(|| {
            let mut game = glsp::lib_mut::<Game>();
            game.draw();

            #[cfg(feature = "confui")]
            dev_ui(ui, &mut game);

            Ok(())
        });
    }
}

/// Marks Entities to be deleted at the end of the frame.
pub struct Dead {
    marks: fxhash::FxHashSet<hecs::Entity>,
}
impl Dead {
    pub fn new() -> Self {
        Dead {
            marks: {
                let mut m = fxhash::FxHashSet::default();
                m.reserve(1000);
                m
            },
        }
    }

    pub fn mark(&mut self, e: hecs::Entity) {
        self.marks.insert(e);
    }

    pub fn is_marked(&mut self, e: hecs::Entity) -> bool {
        self.marks.contains(&e)
    }

    pub fn marks(&self) -> impl Iterator<Item = hecs::Entity> + '_ {
        self.marks.iter().copied()
    }
}

glsp::lib! {
    pub struct Game {
        pub ecs: hecs::World,
        pub l8r: L8r<Game>,
        pub dead: Dead,
        pub map: Map,
        pub phys: CollisionWorld,
        pub config: Config,
        pub player: Player,
        pub images: draw::Images,
        pub draw_state: draw::DrawState,
        pub instance_tracker: prefab::InstanceTracker,
    }
}
impl l8r::ContainsHecsWorld for Game {
    fn ecs(&self) -> &hecs::World {
        &self.ecs
    }

    fn ecs_mut(&mut self) -> &mut hecs::World {
        &mut self.ecs
    }
}
impl Game {
    pub fn new(images: draw::Images, config: Config) -> Self {
        let mut ecs = hecs::World::new();
        let mut phys = CollisionWorld::new(0.02);

        let mut world = Self {
            player: Player::new(&mut ecs, &mut phys, &config),
            map: Map::new(&config),
            l8r: L8r::new(),
            dead: Dead::new(),
            images,
            draw_state: Default::default(),
            instance_tracker: Default::default(),
            config,
            phys,
            ecs,
        };

        prefab::spawn_all_instances(&mut world);

        world
    }

    pub fn remove_physical(&mut self, entity: hecs::Entity, h: PhysHandle) {
        phys::phys_remove(&mut self.ecs, &mut self.phys, entity, h)
    }

    pub fn make_physical(
        &mut self,
        entity: hecs::Entity,
        iso: na::Isometry2<f32>,
        cuboid: Cuboid<f32>,
        groups: CollisionGroups,
    ) -> PhysHandle {
        phys::phys_insert(&mut self.ecs, &mut self.phys, entity, iso, cuboid, groups)
    }

    fn update(&mut self, ignore_inputs: IgnoreInputs) {
        #[cfg(feature = "confui")]
        {
            prefab::instances::keep_fresh(self);
            prefab::clear_removed_prefabs(self);
        }

        if !self.player.state.is_throwing() && !ignore_inputs.keyboard {
            player::movement(self);
        }

        phys::velocity(self);
        phys::chase(self);
        collision::collision(self);

        if !ignore_inputs.mouse {
            player::aiming(self);
        }

        combat::hurtful_damage(self);
        combat::health::remove_out_of_health(self);

        draw::animate(self);
        draw::clear_ghosts(self);

        #[cfg(feature = "confui")]
        if self.config.tile.dirty {
            self.map = Map::new(&self.config);
        }
    }

    fn apply_l8r(&mut self) {
        let mut l8r = L8r::new();
        std::mem::swap(&mut self.l8r, &mut l8r);
        L8r::now(l8r.drain(..), self);
        self.l8r = l8r;
    }

    fn cleanup(&mut self) {
        draw::insert_ghosts(self);
        phys::collision::clear_dead_collision_objects(self);
        prefab::instances::clear_dead(self);

        for entity in self.dead.marks.drain() {
            if let Err(err) = self.ecs.despawn(entity) {
                error!("couldn't remove entity {:?}: {}", entity, err);
            }
        }
    }

    fn draw(&mut self) {
        crate::draw::draw(self);
    }
}
