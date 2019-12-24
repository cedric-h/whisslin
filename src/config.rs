use crate::{na, Vec2};
use std::num::ParseFloatError;
use std::{fmt, iter};

pub type KeyFrame = (f32, Vec2, na::Unit<Vec2>, f32);

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Error {
    ParseError(ParseError),
    NoFile,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ParseError(e) => write!(f, "{}", e),
            Error::NoFile => write!(f, "Couldn't find the `keyframes` file next to Cargo.toml!"),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone)]
pub struct ParseError {
    position: Option<ParsePosition>,
    line: usize,
    element_number: usize,
    source: ErrorKind,
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Error parsing `./keyframes` at line {} {} (around {}): {}",
            self.line + 1,
            self.position
                .as_ref()
                .expect("Couldn't format error; no position supplied!?"),
            match self.element_number {
                std::usize::MAX => "the last element".to_string(),
                x => format!("element #{}", x + 1),
            },
            self.source
        )
    }
}
impl ParseError {
    fn from_source(source: ErrorKind) -> Self {
        Self {
            position: None,
            line: 0,
            element_number: 0,
            source,
        }
    }

    fn with_line(mut self, line: usize) -> Self {
        self.line = line;
        self
    }

    fn with_element_number(mut self, element_number: usize) -> Self {
        self.element_number = element_number;
        self
    }

    fn with_position(mut self, position: ParsePosition) -> Self {
        self.position = Some(position);
        self
    }
}

#[derive(Debug, Clone)]
pub enum ErrorKind {
    NoNumber,
    ParseFloatError(ParseFloatError),
}
impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            ErrorKind::NoNumber => write!(f, "No such number provided!"),
            ErrorKind::ParseFloatError(e) => write!(f, "Couldn't parse number: {}", e),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ParsePosition {
    FrameTime,
    Position(usize),
    Rotation,
    BottomPadding,
}
impl fmt::Display for ParsePosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const NUM_NAMES: &'static [&'static str] = &["first", "second", "third"];
        match *self {
            ParsePosition::FrameTime => {
                write!(f, "at the first element in the line, the frame time")
            }
            ParsePosition::Position(a) => write!(
                f,
                "at the {} element in the line, the {} position component",
                NUM_NAMES[1 + a],
                NUM_NAMES[a]
            ),
            ParsePosition::Rotation => write!(f, "at the fourth element in the line, the rotation"),
            ParsePosition::BottomPadding => {
                write!(f, "at the last element in the line, the bottom padding")
            }
        }
    }
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
                .watch("./../keyframes", RecursiveMode::Recursive)
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
            println!("Change detected, reloading keyframes file!");
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
        let input = {
            use std::io::Read;

            let mut contents = String::new();

            let mut file = std::fs::File::open("../keyframes").map_err(|_| Error::NoFile)?;
            file.read_to_string(&mut contents)
                .map_err(|_| Error::NoFile)?;

            contents
        };

        input
            .chars()
            .filter(|c| !(c.is_whitespace() || ['(', ')', '\t'].contains(c)))
            .collect::<String>()
            .split(',')
            .map(|num| num.parse::<f32>())
            .enumerate()
            .collect::<Vec<_>>()
            .chunks(POSITIONS.len())
            .enumerate()
            .map(|(line, chunk)| parse_chunk(chunk).map_err(|e| e.with_line(line)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::ParseError(e))
    }
}

const POSITIONS: &'static [ParsePosition] = &[
    ParsePosition::FrameTime,
    ParsePosition::Position(1),
    ParsePosition::Position(2),
    ParsePosition::Rotation,
    ParsePosition::BottomPadding,
];

fn parse_chunk(chunks: &[(usize, Result<f32, ParseFloatError>)]) -> Result<KeyFrame, ParseError> {
    let mut c = chunks
        .iter()
        .map(|(i, x)| (*i, x.clone().map_err(|s| ErrorKind::ParseFloatError(s))))
        .chain(iter::repeat((std::usize::MAX, Err(ErrorKind::NoNumber))))
        .zip(POSITIONS.iter())
        .map(|((elem_num, x), pos)| {
            x.map_err(|k| {
                ParseError::from_source(k)
                    .with_position(pos.clone())
                    .with_element_number(elem_num)
            })
        });

    // there's no way these unwraps could fail because of the `.chain(repeat)`
    // and because POSITIONS.len() == chunk size
    Ok((
        c.next().unwrap()?,
        Vec2::new(c.next().unwrap()?, c.next().unwrap()?),
        na::Unit::new_normalize(
            na::UnitComplex::from_angle(c.next().unwrap()?.to_radians())
                .transform_vector(&Vec2::x()),
        ),
        c.next().unwrap()?,
    ))
}
