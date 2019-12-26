use crate::phys::aiming::KeyFrames;
use crate::Vec2;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Deserialize)]
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
pub struct PlayerConfig {
    pub speed: f32,
    pub image: String,
    pub size: Vec2,
    pub pos: Vec2,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub keyframes: KeyFrames,
    pub player: PlayerConfig,
    pub tiles: HashMap<String, TileProperty>,
    pub tilemap: String,
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
            reloading_handlers: Some(ReloadingHandlers::new()),
        })
    }

    #[cfg(feature = "hot-config")]
    /// Reloads config file if notify indicates to do so.
    pub fn reload(&mut self) {
        use notify::{Event, EventKind::Create};
        while let Ok(Ok(Event {
            kind: Create(_), ..
        })) = self.reloading_handlers.as_ref().unwrap().notify.try_recv()
        {
            println!("Change detected, reloading config.toml file!");
            match Config::load() {
                Err(e) => println!("Couldn't load new keyframe file: {}", e),
                Ok(config) => {
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
