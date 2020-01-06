use crate::config::Config;
use crate::items::Inventory;
use crate::World;
use crate::{na, Iso2, PhysHandle, Vec2};
use hecs::Entity;
use nalgebra::base::Unit;
use nalgebra::geometry::UnitComplex;
use quicksilver::input::MouseButton;
use quicksilver::lifecycle::Window;

/// Instead of processing rotations as `UnitComplex`es,
/// this function treats them as `Vec2`s, for ease of lerping
/// among a host of other factors.
#[derive(Debug, serde::Deserialize)]
pub struct KeyFrame {
    pub time: f32,
    pub pos: Vec2,
    pub rot: na::Unit<Vec2>,
    pub bottom_padding: f32,
}
impl KeyFrame {
    fn into_iso2(self) -> Iso2 {
        Iso2::from_parts(
            na::Translation2::from(self.pos),
            UnitComplex::rotation_between_axis(&Unit::new_unchecked(-Vec2::y()), &self.rot),
        )
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(try_from = "Vec<toml::value::Table>")]
pub struct KeyFrames(Vec<KeyFrame>);
impl std::ops::Deref for KeyFrames {
    type Target = Vec<KeyFrame>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for KeyFrames {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug)]
pub enum KeyFrameParsingError {
    NoField(&'static str),
    TomlError(toml::de::Error),
}
impl std::fmt::Display for KeyFrameParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Couldn't parse keyframes configuration table:")?;
        match self {
            KeyFrameParsingError::TomlError(e) => write!(f, "Invalid TOML provided: {}", e),
            KeyFrameParsingError::NoField(name) => {
                write!(f, "No field named {} could be found!", name)
            }
        }
    }
}
impl From<toml::de::Error> for KeyFrameParsingError {
    fn from(toml_e: toml::de::Error) -> Self {
        KeyFrameParsingError::TomlError(toml_e)
    }
}
impl std::error::Error for KeyFrameParsingError {}

/// When converting from a list of Keyframes like you'd see
/// in the config file has some key differences from the
/// Keyframes as they're stored in memory. In the config files,
/// the fields from the keyframes before it are inherited by
/// the KeyFrames that come after it, if those fields are
/// missing on the child keyframes.
impl std::convert::TryFrom<Vec<toml::value::Table>> for KeyFrames {
    type Error = KeyFrameParsingError;

    fn try_from(table: Vec<toml::value::Table>) -> Result<Self, Self::Error> {
        let mut before_frame = table[0].clone();

        Ok(KeyFrames(
            table
                .into_iter()
                .map(|mut keyframe| {
                    use KeyFrameParsingError::NoField;

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
                .collect::<Result<Vec<KeyFrame>, KeyFrameParsingError>>()?,
        ))
    }
}

#[derive(PartialEq, Clone, Copy)]
enum WielderState {
    /// That awkward phase between Shooting and
    /// the beginning of Reloading, when a new
    /// weapon is being whipped out of thin air.
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
    state: WielderState,
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
            }
            Shooting => Summoning { timer: 0 },
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

    // animations
    pub equip_time: u16,
    pub readying_time: u16,
    pub animations: String,

    // projectile
    pub force_magnitude: f32,
    /// Range [0, 1] unless you want your Weapon to get exponentially faster each frame.
    pub force_decay: f32,

    // side effects
    pub player_knock_back_force: f32,
    pub player_knock_back_decay: f32,
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
            animations: "spear".to_string(),

            // projectile
            force_magnitude: 1.0,
            force_decay: 1.0,

            // side effects
            player_knock_back_force: 0.5,
            player_knock_back_decay: 0.75,
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
        keyframes: &KeyFrames,
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
            WielderState::Reloading { timer } => Some(Self::reloading_animation_frame(
                (timer as f32) / (self.equip_time as f32),
                keyframes,
                &last,
            )),
            WielderState::Loaded => Some(last),
            WielderState::Readying { timer } => {
                last.bottom_padding *= 1.0 - (timer as f32) / (self.readying_time as f32);
                Some(last)
            }
            WielderState::Readied | WielderState::Shooting => {
                last.bottom_padding = 0.0;
                Some(last)
            }
        }
    }

    fn reloading_animation_frame(
        mut prog: f32,
        keyframes: &KeyFrames,
        last: &KeyFrame,
    ) -> KeyFrame {
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

pub fn aiming(world: &mut World, window: &mut Window, cfg: &Config) {
    use crate::graphics;

    type WieldQuery<'a> = (
        &'a PhysHandle,
        &'a mut Inventory,
        &'a mut Wielder,
        &'a graphics::Appearance,
    );

    // manually splitting the borrow to appease rustc
    let ecs = &world.ecs;
    let l8r = &mut world.l8r;
    let phys = &mut world.phys;

    // updates the weapon's position relative to the wielder,
    // if clicking, queues adding velocity to the weapon and unequips it.
    // if the weapon that's been equipped doesn't have an iso, queue adding one
    let mut try_wield = |(
        wielder_ent,
        (&PhysHandle(wielder_h), inv, wielder, wielder_appearance),
    ): (Entity, WieldQuery)| {
        let wielder_iso = phys.collision_object(wielder_h)?.position();

        let wep_ent = inv.equipped_ent()?;
        let mut weapon = ecs.get_mut::<Weapon>(wep_ent).ok()?;

        // physics temporaries
        let mouse = window.mouse();
        let delta = Unit::new_normalize(
            mouse.pos().into_vector() - (wielder_iso.translation.vector + weapon.offset),
        );

        let keyframes = &cfg
            .weapons
            .get(&weapon.animations)
            .unwrap_or_else(|| {
                panic!(
                    "Can't find keyframes to animate; No weapon config could be found for {}!",
                    weapon.animations
                )
            })
            .equip_keyframes;
        wielder.advance_state(mouse[MouseButton::Left].is_down(), &weapon);
        let frame = weapon.animation_frame(delta, wielder.state, keyframes)?;

        // updating the weapon's appearance
        {
            let mut wep_appearance = ecs.get_mut::<graphics::Appearance>(wep_ent).ok()?;
            wep_appearance.alignment = graphics::Alignment::Bottom(frame.bottom_padding);
            wep_appearance.flip_x = wielder_appearance.flip_x;
        }

        // handle positioning
        let mut frame_iso = frame.into_iso2();
        if wielder_appearance.flip_x {
            frame_iso.translation.vector.x *= -1.0;
        }
        frame_iso.translation.vector += wielder_iso.translation.vector;

        // get and modify if possible or just insert the weapon's current position
        let PhysHandle(wep_h) = *ecs
            .get::<PhysHandle>(wep_ent)
            .map_err(|_| {
                l8r.l8r(move |world| {
                    world.add_hitbox(
                        wep_ent,
                        frame_iso,
                        ncollide2d::shape::Cuboid::new(Vec2::new(0.1, 1.0)),
                        crate::CollisionGroups::new()
                            .with_membership(&[crate::collide::WEAPON])
                            .with_whitelist(&[]),
                    );
                })
            })
            .ok()?;
        let wep_obj = phys.get_mut(wep_h)?;
        wep_obj.set_position(frame_iso);

        // fire the spear if the wielder state indicates to do so!
        if wielder.shooting() {
            // cut off ties between weapon/player
            l8r.insert_one(wielder_ent, crate::items::InventoryConsumeEquipped);
            l8r.remove_one::<super::Chase>(wep_ent);

            // side effect! (knockback)
            l8r.insert_one(
                wielder_ent,
                super::Force::new(
                    delta.into_inner() * -weapon.player_knock_back_force,
                    weapon.player_knock_back_decay,
                ),
            );

            // the spear needs to go forward and run into things now.
            //
            // damage isn't configured here because the spear was Hurtful the entire time,
            // it's only now even able to collide with things.
            wep_obj.set_collision_groups(
                crate::CollisionGroups::new()
                    .with_membership(&[crate::collide::WEAPON])
                    .with_whitelist(&[crate::collide::WORLD, crate::collide::ENEMY]),
            );
            l8r.insert_one(
                wep_ent,
                super::Force::new(
                    delta.into_inner() * weapon.force_magnitude,
                    weapon.force_decay,
                ),
            );

            l8r.insert_one(
                wep_ent,
                crate::graphics::fade::Fade {
                    duration: 100,
                    fade_after: 25,
                },
            );

            let mut wep_appearance = ecs.get_mut::<graphics::Appearance>(wep_ent).ok()?;
            wep_appearance.alignment = graphics::Alignment::Center;
            wep_appearance.z_offset = -1.5;
        }

        Some(())
    };

    ecs.query::<WieldQuery>().into_iter().for_each(|args| {
        try_wield(args);
    });
}
