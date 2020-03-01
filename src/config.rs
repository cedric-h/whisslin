use crate::phys::aiming::KeyFrames;
use crate::Vec2;
use fxhash::FxHashMap;
use serde::Deserialize;
use std::fmt;

#[cfg(feature = "hot-config")]
pub struct ReloadWithConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct TileProperty {
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub farmable: bool,
    #[serde(default)]
    pub collidable: bool,
}
impl Default for TileProperty {
    fn default() -> Self {
        Self {
            name: "unknown".into(),
            image: "unknown".into(),
            farmable: false,
            collidable: false,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct InventoryEntry {
    pub name: String,
    pub count: Option<usize>,
    #[serde(default)]
    pub flags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlayerConfig {
    pub speed: f32,
    pub image: String,
    pub size: Vec2,
    pub pos: Vec2,
    pub inventory: Vec<InventoryEntry>,
}
impl PlayerConfig {
    pub fn spawn(
        &self,
        world: &mut crate::World,
        items: &FxHashMap<String, ItemConfig>,
    ) -> hecs::Entity {
        use crate::Iso2;
        use crate::{aiming, graphics, items, movement, phys};

        #[cfg(feature = "hot-config")]
        let player = world.ecs.spawn((
            graphics::Appearance {
                kind: graphics::AppearanceKind::image(&self.image),
                ..Default::default()
            },
            movement::PlayerControlled { speed: self.speed },
            aiming::Wielder::new(),
            items::Inventory::new(),
            graphics::sprite_sheet::Animation::new(),
            graphics::sprite_sheet::Index::new(),
            ReloadWithConfig,
        ));
        #[cfg(not(feature = "hot-config"))]
        let player = world.ecs.spawn((
            graphics::Appearance {
                kind: graphics::AppearanceKind::image(&self.image),
                ..Default::default()
            },
            movement::PlayerControlled { speed: self.speed },
            aiming::Wielder::new(),
            items::Inventory::new(),
            graphics::sprite_sheet::Animation::new(),
            graphics::sprite_sheet::Index::new(),
        ));
        world.add_hitbox(
            player,
            Iso2::new(self.pos, 0.0),
            ncollide2d::shape::Cuboid::new(self.size / 2.0),
            crate::CollisionGroups::new().with_membership(&[crate::collide::PLAYER]),
        );

        for InventoryEntry { name, count, flags } in self.inventory.iter() {
            let count = count.unwrap_or(1);

            for flag in flags.iter() {
                // TODO: compile time function table!
                match flag.as_str() {
                    "equipped" | "equip" => {
                        world
                            .l8r
                            .insert_one(player, items::InventoryEquip(Some(name.clone())));
                    }
                    _ => panic!(
                        concat!(
                            "unknown flag {:?} provided in config file!",
                            "flag must be one of: [\"equipped\", \"equip\"]!"
                        ),
                        flag,
                    ),
                }
            }

            for _ in 0..count {
                let ent = items
                    .get(name)
                    .unwrap_or_else(|| {
                        panic!(
                            "Couldn't spawn {} {:?}{} for player's inventory: no items config found for {}!",
                            count,
                            &name,
                            Some(flags)
                                .filter(|f| !f.is_empty())
                                .map(|f| format!(" with flags {:?}", &f))
                                .unwrap_or_default(),
                            &name
                        )
                    })
                    .spawn(world);
                world.l8r.insert_one(ent, items::InventoryInsert(player));
                world
                    .l8r
                    .insert_one(ent, phys::Chase::new(player, self.speed));
            }
        }

        player
    }
}

pub use string_range::StringRange;
pub mod string_range {
    use rand::distributions::uniform::{SampleUniform, Uniform};
    use serde::Deserialize;

    #[derive(Debug, Clone, Deserialize)]
    #[serde(from = "String")]
    #[serde(bound(deserialize = "T: Clone + std::str::FromStr"))]
    pub struct StringRange<T> {
        lo: T,
        hi: T,
        inclusive: bool,
    }
    impl<T: std::str::FromStr + Clone> From<String> for StringRange<T> {
        fn from(input: String) -> Self {
            let find_in = |key| {
                input.find(key).map(|_| {
                    let mut nums = input.split(key).map(|n| {
                        n.parse::<T>().unwrap_or_else(|_| {
                            //panic!("Couldn't parse {} in range \"{}\": {}", n, input, e)
                            panic!(
                                "Couldn't parse {} in range \"{}\": invalid number!",
                                n, input
                            )
                        })
                    });
                    (nums.next().unwrap(), nums.next().unwrap())
                })
            };
            if let Some((a, b)) = find_in("..=") {
                StringRange {
                    lo: a,
                    hi: b,
                    inclusive: true,
                }
            } else if let Some((a, b)) = find_in("..") {
                StringRange {
                    lo: a,
                    hi: b,
                    inclusive: false,
                }
            } else if let Ok(parsed) = input.parse::<T>() {
                StringRange {
                    lo: parsed.clone(),
                    hi: parsed,
                    inclusive: true,
                }
            } else {
                panic!("Invalid range: \"{}\"", input)
            }
        }
    }

    impl<T: SampleUniform> Into<Uniform<T>> for StringRange<T> {
        fn into(self) -> Uniform<T> {
            if self.inclusive {
                (self.lo..=self.hi).into()
            } else {
                (self.lo..self.hi).into()
            }
        }
    }
    pub mod uniform {
        use super::StringRange;
        use rand::distributions::uniform::{SampleUniform, Uniform};
        use serde::{Deserialize, Deserializer};
        pub fn range<'de, D, T>(deserializer: D) -> Result<Uniform<T>, D::Error>
        where
            D: Deserializer<'de>,
            T: SampleUniform,
            StringRange<T>: Deserialize<'de>,
        {
            <StringRange<T>>::deserialize(deserializer).map(|x| x.into())
        }
        /*
        pub fn range_usize<'de, D>(deserializer: D) -> Result<Uniform<usize>, D::Error>
        where
            D: Deserializer<'de>,
        {
            <StringRange<usize>>::deserialize(deserializer).map(|x| x.into())
        }*/
        pub fn range_array_4<'de, D, T>(deserializer: D) -> Result<[Uniform<T>; 4], D::Error>
        where
            D: Deserializer<'de>,
            T: SampleUniform + Clone,
            StringRange<T>: Deserialize<'de>,
        {
            let raw = <[StringRange<T>; 4]>::deserialize(deserializer)?;

            let mut converted = raw.iter().cloned().map(|x| -> Uniform<T> { x.into() });

            Ok([
                converted.next().unwrap(),
                converted.next().unwrap(),
                converted.next().unwrap(),
                converted.next().unwrap(),
            ])
        }
        pub fn range_array_2<'de, D, T>(deserializer: D) -> Result<[Uniform<T>; 2], D::Error>
        where
            D: Deserializer<'de>,
            T: SampleUniform + Clone,
            StringRange<T>: Deserialize<'de>,
        {
            let raw = <[StringRange<T>; 2]>::deserialize(deserializer)?;

            let mut converted = raw.iter().cloned().map(|x| -> Uniform<T> { x.into() });
            Ok([converted.next().unwrap(), converted.next().unwrap()])
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct ItemConfig {
    // appearance
    pub image: String,
    pub weapon: Option<crate::phys::aiming::Weapon>,
    pub hurtful: Option<crate::combat::Hurtful>,
    pub growth: Option<crate::farm::Growth>,
}

impl ItemConfig {
    pub fn spawn(&self, world: &mut crate::World) -> hecs::Entity {
        use crate::{collide, graphics, phys};
        use hecs::EntityBuilder;
        let mut item_builder = EntityBuilder::new();

        item_builder.add(graphics::Appearance {
            kind: graphics::AppearanceKind::image(self.image.clone()),
            z_offset: 0.5,
            ..Default::default()
        });
        item_builder.add(phys::collision::RigidGroups(
            crate::CollisionGroups::new()
                .with_membership(&[collide::WEAPON])
                .with_blacklist(&[collide::PLAYER, collide::ENEMY]),
        ));

        if let Some(hurtful) = &self.hurtful {
            item_builder.add(hurtful.clone());

            item_builder.add(phys::KnockBack {
                groups: crate::CollisionGroups::new()
                    .with_membership(&[collide::WEAPON])
                    .with_whitelist(&[collide::ENEMY]),
                force_decay: 0.75,
                force_magnitude: 0.75,
                use_force_direction: true,
                // TODO: separate minimum_speed_to_knock_back
                minimum_speed: Some(hurtful.minimum_speed),
            });
        }
        if let Some(weapon) = &self.weapon {
            item_builder.add(weapon.clone());
        }
        if let Some(growth) = &self.growth {
            item_builder.add(growth.clone());
        }

        #[cfg(feature = "hot-config")]
        item_builder.add(ReloadWithConfig);

        world.ecs.spawn(item_builder.build())
    }
}

#[derive(Debug, Deserialize)]
pub struct TilemapDetails {
    pub layout: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub tilemaps: FxHashMap<String, TilemapDetails>,
    pub player: PlayerConfig,
    pub animations: FxHashMap<String, KeyFrames>,
    pub particles: FxHashMap<String, crate::graphics::particle::Emitter>,
    pub items: FxHashMap<String, ItemConfig>,
    pub tiles: FxHashMap<String, TileProperty>,
    pub sprite_sheets: FxHashMap<String, crate::graphics::sprite_sheet::Entry>,
}

impl Config {
    fn load() -> Result<Self, Error> {
        #[cfg(not(feature = "hot-config"))]
        let input = include_str!("../config.toml");

        #[cfg(feature = "hot-config")]
        let tempput = {
            use std::io::Read;

            let mut contents = String::new();

            let mut file = std::fs::File::open("../config.toml").map_err(|_| Error::NoFile)?;
            file.read_to_string(&mut contents)
                .map_err(|_| Error::NoFile)?;

            contents
        };
        #[cfg(feature = "hot-config")]
        let input = &tempput;

        toml::from_str(input).map_err(|e| e.into())
    }

    pub fn spawn(&self, world: &mut crate::World) -> hecs::Entity {
        let player = self.player.spawn(world, &self.items);

        // attach the inventory GUI window to the player
        let window = crate::gui::build_inventory_gui_entities(world, player);
        world.ecs.insert_one(player, window).unwrap();

        player
    }
}

#[cfg(feature = "hot-config")]
pub struct ReloadingHandlers {
    notify: crossbeam_channel::Receiver<notify::Result<notify::event::Event>>,

    // gotta hold onto this otherwise it goes out of scope and is dropped
    #[allow(dead_code)]
    watcher: notify::RecommendedWatcher,
}
#[cfg(feature = "hot-config")]
impl ReloadingHandlers {
    fn new() -> Self {
        use notify::{RecommendedWatcher, RecursiveMode, Watcher};
        use std::time::Duration;
        let (tx, rx) = crossbeam_channel::unbounded();

        let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(1)).unwrap();
        watcher
            .watch("./../config.toml", RecursiveMode::Recursive)
            .unwrap();

        Self {
            notify: rx,
            watcher,
        }
    }
}

pub struct ConfigHandler {
    config: Config,
    // internal hot reloading stuff
    #[cfg(feature = "hot-config")]
    reloading_handlers: ReloadingHandlers,
}
impl ConfigHandler {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            config: Config::load()?,
            #[cfg(feature = "hot-config")]
            reloading_handlers: ReloadingHandlers::new(),
        })
    }

    #[cfg(feature = "hot-config")]
    /// Reloads config file if notify indicates to do so.
    pub fn reload(&mut self, world: &mut crate::World) {
        use notify::{Event, EventKind::Create};
        while let Ok(Ok(Event {
            kind: Create(_), ..
        })) = self.reloading_handlers.notify.try_recv()
        {
            println!("Change detected, reloading config.toml file!");
            match Config::load() {
                Err(e) => println!("Couldn't load new keyframe file: {}", e),
                Ok(config) => {
                    let to_reload = world
                        .ecs
                        .query::<&ReloadWithConfig>()
                        .iter()
                        .map(|(id, _)| id)
                        .collect::<Vec<hecs::Entity>>();

                    println!(
                        "Deleting {} entities marked with 'ReloadWithConfig' components.",
                        to_reload.len()
                    );

                    for ent in to_reload.into_iter() {
                        world.ecs.despawn(ent).unwrap_or_else(|e| panic!(
                            "Couldn't delete entity[{:?}] marked with 'ReloadWithConfig' component during reloading: {}",
                            ent,
                            e
                        ));
                    }

                    config.spawn(world);

                    println!(
                        "Respawned {} entities.",
                        world.ecs.query::<&ReloadWithConfig>().iter().len()
                    );

                    println!("Reload successful!");
                    self.config = config;
                }
            }
        }
    }
}
impl std::ops::Deref for ConfigHandler {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}
impl std::ops::DerefMut for ConfigHandler {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}

#[derive(Debug)]
pub enum Error {
    #[allow(dead_code)]
    NoFile,
    TomlError(toml::de::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoFile => write!(
                f,
                "Couldn't find the `config.toml` file next to Cargo.toml!"
            ),
            Error::TomlError(e) => write!(f, "Invalid TOML provided in `config.toml`: {}", e),
        }
    }
}
impl From<toml::de::Error> for Error {
    fn from(toml_e: toml::de::Error) -> Self {
        Error::TomlError(toml_e)
    }
}
impl std::error::Error for Error {}
