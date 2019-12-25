use crate::phys::aiming::KeyFrame;
use crate::{na, Vec2};
use serde::Deserialize;
use std::fmt;
use toml::value::Table;

#[derive(Deserialize)]
pub struct PlayerConfig {
    pub speed: f32,
    pub image: String,
    pub size: Vec2,
    pub pos: Vec2,
}
impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            speed: 4.0,
            image: String::from("player"),
            size: Vec2::new(58.0, 8.0),
            pos: Vec2::new(300.0, 300.0)
        }
    }
}

#[derive(Deserialize)]
pub struct SerdeConfig {
    keyframes: Vec<Table>,
    player: PlayerConfig,
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
            watcher
        }
    }
}

#[derive(Default)]
pub struct Config {
    pub keyframes: Vec<KeyFrame>,
    pub player: PlayerConfig,

    // internal hot reloading stuff
    #[cfg(feature = "hot-config")]
    reloading_handlers: Option<ReloadingHandlers>,
}
impl Config {
    pub fn new() -> Result<Self, Error> {

        let mut s = Self::default();
        s.load()?;

        #[cfg(feature = "hot-config")]
        {
            s.reloading_handlers = Some(ReloadingHandlers::new());
        }

        Ok(s)
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
            match self.load() {
                Err(e) => println!("Couldn't load new keyframe file: {}", e),
                Ok(_) => println!("Reload successful!")
            }
        }
    }

    fn load(&mut self) -> Result<(), Error> {
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

        let serde_config: SerdeConfig = toml::from_str(input)?;

        self.keyframes = keyframes_from_tables(serde_config.keyframes)?;
        self.player = serde_config.player;

        Ok(())
    }
}

fn keyframes_from_tables(table: Vec<Table>) -> Result<Vec<KeyFrame>, Error> {
    let mut before_frame = table[0].clone();

    table
        .into_iter()
        .map(|mut keyframe| {
            use Error::NoField;

            for (key, val) in before_frame.iter() {
                if !keyframe.contains_key(key) {
                    keyframe.insert(key.clone(), val.clone());
                }
            }

            before_frame = keyframe.clone();

            Ok(KeyFrame {
                time: keyframe.remove("time").ok_or(NoField("time"))?.try_into()?,
                pos: keyframe.remove("pos").ok_or(NoField("pos"))?.try_into()?,
                rot: na::Unit::new_normalize(
                    na::UnitComplex::from_angle(
                        keyframe
                        .remove("rot")
                        .ok_or(NoField("rot"))?
                        .try_into::<f32>()?
                        .to_radians(),
                        )
                    .transform_vector(&Vec2::x()),
                    ),
                    bottom_padding: keyframe
                        .remove("bottom_padding")
                        .ok_or(NoField("bottom_padding"))?
                        .try_into()?,
            })
        })
        .collect()
}

#[derive(Debug)]
pub enum Error {
    #[allow(dead_code)]
    NoFile,
    NoField(&'static str),
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
            Error::NoField(name) => {
                write!(f, "No field named `{}` could be found in the TOML.", name)
            }
        }
    }
}
impl From<toml::de::Error> for Error {
    fn from(toml_e: toml::de::Error) -> Self {
        Error::TomlError(toml_e)
    }
}
impl std::error::Error for Error {}
