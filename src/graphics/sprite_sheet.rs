use super::Appearance;
use crate::config::Config;
use crate::World;
use crate::{na, Vec2};
use std::time::Duration;

#[derive(Debug, serde::Deserialize)]
pub struct SerdeEntry {
    pub rows: usize,
    pub cols: usize,
    pub frame_size: Vec2,
    pub frame_millis: Option<Vec<u64>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(from = "SerdeEntry")]
pub struct Entry {
    pub rows: usize,
    pub cols: usize,
    pub frame_size: Vec2,
    pub frame_durations: Option<Vec<Duration>>,
}
impl From<SerdeEntry> for Entry {
    fn from(other: SerdeEntry) -> Self {
        let SerdeEntry {
            rows,
            cols,
            frame_size,
            frame_millis,
        } = other;
        Self {
            rows,
            cols,
            frame_size,
            frame_durations: frame_millis
                .map(|times| times.into_iter().map(Duration::from_millis).collect()),
        }
    }
}

/// Keeps track of when to change an image's sprite sheet index.
#[derive(Default)]
pub struct Animation {
    timer: Option<Duration>,
    frame: usize,
}
impl Animation {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Records where in a sprite sheet is currently being rendered.
pub struct Index(pub Vec2);
impl Index {
    pub fn new() -> Self {
        Index(na::zero())
    }
}

pub fn animate(world: &mut World, cfg: &Config, elapsed: Duration) -> Result<(), Error> {
    for (_, (anim, index, appearance)) in &mut world
        .ecs
        .query::<(&mut Animation, &mut Index, &Appearance)>()
    {
        let appearance_name = appearance.kind.name();
        let entry = cfg
            .sprite_sheets
            .get(appearance_name)
            .ok_or_else(|| Error::NoEntry(appearance_name.into()))?;
        let frame_durations = entry
            .frame_durations
            .as_ref()
            .ok_or_else(|| Error::NoFrameMillisField(appearance_name.into()))?;

        if let Some(timer) = anim.timer {
            anim.timer = timer.checked_sub(elapsed);
        }

        if anim.timer.is_none() {
            anim.frame = if (anim.frame + 1) >= frame_durations.len() {
                0
            } else {
                anim.frame + 1
            };
            anim.timer = Some(frame_durations[anim.frame]);

            // update index
            index.0.x = anim.frame as f32;
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum Error {
    NoEntry(String),
    NoFrameMillisField(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;

        write!(
            f,
            "A sprite_sheet::Animation component was found on an entity with the Appearance "
        )?;

        match &self {
            NoEntry(name) => write!(f, "{}, but no sprite sheet configuration entry was found for that Appearance in config.toml", name),
            NoFrameMillisField(name) => write!(f, "{} but no frame_millis field was found in the configuration for that Appearance in config.toml.", name)
        }
    }
}
impl std::error::Error for Error {}
