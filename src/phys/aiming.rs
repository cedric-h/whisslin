use crate::config::Config;
use crate::items::Inventory;
use crate::{na, Iso2, Vec2};
use hecs::{Entity, World};
use nalgebra::base::Unit;
use nalgebra::geometry::UnitComplex;
use quicksilver::input::MouseButton;
use quicksilver::lifecycle::Window;

/// Instead of processing rotations as `UnitComplex`es,
/// this function treats them as `Vec2`s, for ease of lerping
/// among a host of other factors.
pub struct KeyFrame {
    pub time: f32,
    pub pos: Vec2,
    pub rot: na::Unit<Vec2>,
    pub bottom_padding: f32,
}

pub struct Wielder {
    weapon_state: WeaponState
}
impl Wielder {
    pub fn new() -> Self {
        Self {
            weapon_state: WeaponState::Loaded,
        }
    }
}

#[derive(PartialEq, Clone)]
enum WeaponState {
    Reloading { timer: u16 },
    Loaded,
    Readying { timer: u16 },
    Readied,
    Shooting,
}

pub struct Weapon {
    // positioning
    pub offset: Vec2,
    pub bottom_padding: f32,

    // timing
    pub equip_time: u16,
    pub readying_time: u16,

    // projectile
    pub speed: f32,

    // internal
    state: WeaponState
}
impl Weapon {
    pub fn new() -> Self {
        Self {
            // positioning
            offset: na::zero(),
            bottom_padding: 0.0,

            // timing
            equip_time: 60,
            readying_time: 60,

            // projectile
            speed: 1.0,

            //internal
            state: WeaponState::Loaded,
        }
    }

    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    pub fn with_bottom_padding(mut self, padding: f32) -> Self {
        self.bottom_padding = padding;
        self
    }

    pub fn with_equip_time(mut self, time: u16) -> Self {
        self.equip_time = time;
        self
    }
    
    pub fn with_readying_time(mut self, time: u16) -> Self {
        self.readying_time = time;
        self
    }

    /// # Input
    /// Takes a unit vector representing the delta
    /// between the player's world position and the mouse.
    /// Also takes the keyframes from the game's configuration files.
    ///
    /// # Output
    /// This function returns a KeyFrame representing how
    /// the weapon should be oriented at this point in time.
    /// It also returns a boolean indicating whether or not to shoot.
    fn animate(
        &mut self,
        mouse_delta: Unit<Vec2>,
        mouse_down: bool,
        keyframes: &Vec<KeyFrame>,
    ) -> (KeyFrame, bool) {

        // move timers forward
        self.advance_state(mouse_down);

        // the implied last frame, pointing towards the mouse
        // also returned if state is "Readied"
        let mut last = KeyFrame {
            time: 1.0,
            pos: self.offset,
            rot: mouse_delta,
            bottom_padding: self.bottom_padding,
        };

        // read timers
        let mut prog = match self.state {
            WeaponState::Reloading { timer } => (timer as f32) / (self.equip_time as f32),
            WeaponState::Loaded => return (last, false),
            WeaponState::Readying { timer } => {
                last.bottom_padding *= 1.0 - (timer as f32) / (self.readying_time as f32);
                return (last, false);
            }
            WeaponState::Readied | WeaponState::Shooting => {
                last.bottom_padding = 0.0;
                return (last, self.state == WeaponState::Shooting);
            },
        };

        let mut frames = keyframes.iter();

        // find the key frames before and after our current time
        let mut lf = frames.next().unwrap();
        let rf = frames
            .find_map(|rf| {
                if rf.time > prog {
                    // short circuit, we found the first frame with a higher timestamp
                    Some(rf)
                } else {
                    // not high enough, but maybe it's a lower bound?
                    lf = rf;
                    None
                }
            })
            .unwrap_or(&last);

        // scale prog according to how close to rf.time it is from lf.time
        // i.e. 1 would mean it's literally rf.time, 0 is literally lf.time
        prog = (prog - lf.time) / (rf.time - lf.time);

        (
            KeyFrame { 
                time: prog,
                pos: lf.pos.lerp(&rf.pos, prog),
                rot: lf.rot.slerp(&rf.rot, prog),
                bottom_padding: lf.bottom_padding + (rf.bottom_padding - lf.bottom_padding) * prog,
            },
            false,
        )
    }

    /// Moves timers forward, changes state if necessary.
    fn advance_state(&mut self, mouse_down: bool) {
        self.state = match self.state {
            WeaponState::Reloading { mut timer } => {
                timer += 1;
                if timer >= self.equip_time {
                    WeaponState::Loaded
                } else {
                    WeaponState::Reloading { timer }
                }
            }
            WeaponState::Loaded => {
                if mouse_down {
                    WeaponState::Readying { timer: 0 }
                } else {
                    WeaponState::Loaded
                }
            }
            WeaponState::Readying { mut timer } => {
                timer += 1;
                if !mouse_down { 
                    WeaponState::Loaded
                } else if timer >= self.readying_time {
                    WeaponState::Readied
                } else {
                    WeaponState::Readying { timer }
                }
            }
            WeaponState::Readied => {
                if !mouse_down {
                    WeaponState::Shooting
                } else {
                    WeaponState::Readied
                }
            },
            WeaponState::Shooting => {
                WeaponState::Reloading { timer: 0 }
            }
        };
    }
}

enum QueuedAction {
    LaunchWeapon(Entity, Vec2),
    InsertIso(Entity, Iso2),
}

pub fn aiming(world: &mut World, window: &mut Window, cfg: &Config) {
    let needs_velocity: Vec<QueuedAction> = world
        .query::<(&Iso2, &mut Inventory, &mut Wielder)>()
        .into_iter()
        // updates the weapon's position relative to the wielder,
        // if clicking, queues adding velocity to the weapon and unequips it.
        // if the weapon that's been equipped doesn't have an iso, queue adding one
        .filter_map(|(_, (wielder_iso, inv, wielder))| {
            let wep_ent = inv.equipped()?;
            let mut weapon = world.get_mut::<Weapon>(wep_ent).ok()?;
            let mut appearance = world.get_mut::<crate::graphics::Appearance>(wep_ent).ok()?;

            weapon.state = wielder.weapon_state.clone();

            // physics temporaries
            let mouse = window.mouse();
            let delta = Unit::new_normalize(
                (wielder_iso.translation.vector + weapon.offset) - mouse.pos().into_vector(),
            );
            let (mut frame, should_shoot) = weapon.animate(delta, mouse[MouseButton::Left].is_down(), &cfg.keyframes);

            wielder.weapon_state = weapon.state.clone();

            // apply the bottom padding to the Appearance
            appearance.alignment = crate::graphics::Alignment::Bottom(frame.bottom_padding);

            // get the final position by applying the offset to the world pos
            frame.pos += wielder_iso.translation.vector;

            // get the final rotation by converting it to a UnitComplex and adjusting
            let rot = UnitComplex::rotation_between_axis(&Unit::new_unchecked(Vec2::x()), &frame.rot)
                * UnitComplex::new(-std::f32::consts::FRAC_PI_2);
                //* UnitComplex::rotation_between_axis(&Unit::new_unchecked(Vec2::x()), &delta);

            // get the weapon's current position if it has one,
            // otherwise get one inserted onto it ASAP.
            let mut wep_iso = match world.get_mut::<Iso2>(wep_ent) {
                Ok(iso) => iso,
                Err(_) => {
                    let mut new_pos = *wielder_iso;
                    new_pos.translation.vector = frame.pos;
                    new_pos.rotation = rot;
                    return Some(QueuedAction::InsertIso(wep_ent, new_pos));
                }
            };

            // applying translation and rotation provided by weapon_frame
            wep_iso.translation.vector = frame.pos;
            wep_iso.rotation = rot;

            // queue launch if clicking
            if should_shoot {
                inv.consume_equipped();
                Some(QueuedAction::LaunchWeapon(
                    wep_ent,
                    delta.into_inner() * weapon.speed,
                ))
            } else {
                None
            }
        })
        .collect();

    needs_velocity.into_iter().for_each(|action| {
        use QueuedAction::*;
        match action {
            LaunchWeapon(wep_ent, launch_towards) => {
                world
                    .insert_one(wep_ent, super::Velocity(-launch_towards))
                    .expect("Couldn't insert velocity to launch weapon!");
            }
            InsertIso(wep_ent, wep_iso) => {
                world
                    .insert_one(wep_ent, wep_iso)
                    .expect("Couldn't insert iso onto wep_ent!");
            }
        }
    });
}
