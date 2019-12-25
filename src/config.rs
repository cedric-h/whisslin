use crate::phys::aiming::KeyFrame;
use crate::{na, Vec2};
use serde::Deserialize;
use std::num::ParseFloatError;
use std::{fmt, iter};

#[derive(Debug)]
pub enum Error {
    NoFile,
    NoField(&'static str),
    TomlError(toml::de::Error),
    EmptyToml,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoFile => write!(
                f,
                "Couldn't find the `keyframes.toml` file next to Cargo.toml!"
            ),
            Error::TomlError(e) => write!(f, "Invalid TOML provided in `keyframes.toml`: {}", e),
            Error::NoField(name) => {
                write!(f, "No field named `{}` could be found in the TOML.", name)
            }
            Error::EmptyToml => write!(f, "The `keyframes.toml` file was empty..."),
        }
    }
}
impl From<toml::de::Error> for Error {
    fn from(toml_e: toml::de::Error) -> Self {
        Error::TomlError(toml_e)
    }
}
impl std::error::Error for Error {}

#[derive(Deserialize)]
pub struct SerdeConfig {
    keyframes: Vec<toml::value::Table>,
}

pub struct Config {
    pub keyframes: Vec<KeyFrame>,
    #[cfg(feature = "hot-keyframes")]
    notify: crossbeam_channel::Receiver<notify::Result<notify::event::Event>>,
    #[cfg(feature = "hot-keyframes")]
    #[allow(dead_code)]
    watcher: notify::RecommendedWatcher,
}
impl Config {
    pub fn load() -> Result<Self, Error> {
        #[cfg(feature = "hot-keyframes")]
        let (notify, watcher) = {
            use notify::{RecommendedWatcher, RecursiveMode, Watcher};
            use std::time::Duration;
            let (tx, rx) = crossbeam_channel::unbounded();

            let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(1)).unwrap();
            watcher
                .watch("./../keyframes.toml", RecursiveMode::Recursive)
                .unwrap();
            (rx, watcher)
        };

        Ok(Self {
            keyframes: Self::load_keyframes()?,
            #[cfg(feature = "hot-keyframes")]
            notify,
            #[cfg(feature = "hot-keyframes")]
            watcher,
        })
    }

    #[cfg(feature = "hot-keyframes")]
    /// Reloads config file if notify indicates to do so.
    pub fn reload(&mut self) {
        use notify::{Event, EventKind::Create};
        while let Ok(Ok(Event {
            kind: Create(_), ..
        })) = self.notify.try_recv()
        {
            println!("Change detected, reloading keyframes.toml file!");
            match Self::load_keyframes() {
                Ok(kfs) => {
                    self.keyframes = kfs;
                    return;
                }
                Err(e) => println!("Couldn't load new keyframe file: {}", e),
            }
        }
    }

    fn load_keyframes() -> Result<Vec<KeyFrame>, Error> {
        #[cfg(not(feature = "hot-keyframes"))]
        let input = include_str!("../keyframes");

        #[cfg(feature = "hot-keyframes")]
        let tempput = {
            use std::io::Read;

            let mut contents = String::new();

            let mut file = std::fs::File::open("../keyframes.toml").map_err(|_| Error::NoFile)?;
            file.read_to_string(&mut contents)
                .map_err(|_| Error::NoFile)?;

            contents
        };
        #[cfg(feature = "hot-keyframes")]
        let input = &tempput;

        let serde_config: SerdeConfig = toml::from_str(input)?;
        let keyframes = serde_config.keyframes;
        let mut before_frame = keyframes[0].clone();

        keyframes
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
}
