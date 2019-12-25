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

#[derive(PartialEq, Clone, Copy)]
enum WielderState {
    /// That awkward phase between Shooting
    /// and the beginning of the Reloading.
    Summoning { timer: u16 },

    /// Watch an animation and think about how you just
    /// wasted that last spear.
    Reloading { timer: u16 },

    /// Start holding down the mouse button to begin readying
    Loaded,

    /// If you keep holding down the mouse button you'll be able to shoot,
    /// if you let go you'll go back to Loaded.
    Readying { timer: u16 },
    
    /// Let go to fire!
    /// TODO: A way to leave this stage.
    Readied,

    /// Lasts exactly one frame.
    /// During this frame, the projectile is launched.
    Shooting,
}

pub struct Wielder {
    state: WielderState
}
impl Wielder {
    pub fn new() -> Self {
        Self {
            state: WielderState::Loaded,
        }
    }

    /// The length of the Summoning State,
    /// i.e. how long it takes for another weapon
    /// to pop out of thin air and into the player's hand
    const SUMMONING_TIME: u16 = 25;
    
    /// Moves timers forward
    fn advance_state(&mut self, mouse_down: bool, weapon: &Weapon) {
        use WielderState::*;

        self.state = match self.state {
            Summoning { mut timer } => {
                timer += 1;
                if timer >= Self::SUMMONING_TIME {
                    Reloading { timer: 0 }
                } else {
                    Summoning { timer }
                }
            }
            Reloading { mut timer } => {
                timer += 1;
                if timer >= weapon.equip_time {
                    Loaded
                } else {
                    Reloading { timer }
                }
            }
            Loaded => {
                if mouse_down {
                    Readying { timer: 0 }
                } else {
                    Loaded
                }
            }
            Readying { mut timer } => {
                timer += 1;
                if !mouse_down { 
                    Loaded
                } else if timer >= weapon.readying_time {
                    Readied
                } else {
                    Readying { timer }
                }
            }
            Readied => {
                if !mouse_down {
                    Shooting
                } else {
                    Readied
                }
            },
            Shooting => {
                Summoning { timer: 0 }
            }
        };
    }

    fn shooting(&self) -> bool {
        self.state == WielderState::Shooting
    }
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
}
impl Default for Weapon {
    fn default() -> Self {
        Self {
            // positioning
            offset: na::zero(),
            bottom_padding: 0.0,

            // timing
            equip_time: 60,
            readying_time: 60,

            // projectile
            speed: 1.0,
        }
    }
}
impl Weapon {
    /// # Input
    /// Takes a unit vector representing the delta
    /// between the player's world position and the mouse.
    /// (These are used to generate the implied last frame, i.e. 
    /// where the spear points at the mouse)
    /// Also takes the keyframes from the game's configuration files.
    ///
    /// # Output
    /// This function returns a KeyFrame representing how
    /// the weapon should be oriented at this point in time.
    ///
    /// However, if the weapon shouldn't be given a position at all
    /// (so that it remains unrendered) None is returned.
    fn animation_frame(
        &mut self,
        mouse_delta: Unit<Vec2>,
        state: WielderState,
        keyframes: &Vec<KeyFrame>,
    ) -> Option<KeyFrame> {

        // the implied last frame of the reloading animtion,
        // pointing towards the mouse.
        let mut last = KeyFrame {
            time: 1.0,
            pos: self.offset,
            rot: mouse_delta,
            bottom_padding: self.bottom_padding,
        };

        // read timers
        match state {
            WielderState::Summoning { .. } => None,
            WielderState::Reloading { timer } => {
                Some(Self::reloading_animation_frame(
                    (timer as f32) / (self.equip_time as f32),
                    keyframes,
                    &last
                ))
            }
            WielderState::Loaded => Some(last),
            WielderState::Readying { timer } => {
                last.bottom_padding *= 1.0 - (timer as f32) / (self.readying_time as f32);
                Some(last)
            }
            WielderState::Readied | WielderState::Shooting => {
                last.bottom_padding = 0.0;
                Some(last)
            },
        }
    }

    fn reloading_animation_frame(mut prog: f32, keyframes: &Vec<KeyFrame>, last: &KeyFrame) -> KeyFrame {
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
            .unwrap_or(last);

        // scale prog according to how close to rf.time it is from lf.time
        // i.e. 1 would mean it's literally rf.time, 0 is literally lf.time
        prog = (prog - lf.time) / (rf.time - lf.time);

        KeyFrame { 
            time: prog,
            pos: lf.pos.lerp(&rf.pos, prog),
            rot: lf.rot.slerp(&rf.rot, prog),
            bottom_padding: lf.bottom_padding + (rf.bottom_padding - lf.bottom_padding) * prog,
        }
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

            // physics temporaries
            let mouse = window.mouse();
            let delta = Unit::new_normalize(
                (wielder_iso.translation.vector + weapon.offset) - mouse.pos().into_vector(),
            );
            wielder.advance_state(mouse[MouseButton::Left].is_down(), &weapon);
            let mut frame = weapon.animation_frame(delta, wielder.state, &cfg.keyframes)?;

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
            if wielder.shooting() {
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
