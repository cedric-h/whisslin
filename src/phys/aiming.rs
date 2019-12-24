use crate::config::{Config, KeyFrame};
use crate::items::Inventory;
use crate::{na, Iso2, Vec2};
use hecs::{Entity, World};
use nalgebra::base::Unit;
use nalgebra::geometry::UnitComplex;
use quicksilver::input::MouseButton;
use quicksilver::lifecycle::Window;

pub struct PlayerControlled;

pub struct Weapon {
    pub offset: Vec2,
    pub equip_time: u16,
    pub bottom_padding: f32,
    pub speed: f32,
    timer: u16,
}
impl Weapon {
    pub fn new() -> Self {
        Self {
            offset: na::zero(),
            equip_time: 60,
            timer: 0,
            bottom_padding: 0.0,
            speed: 1.0,
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

    /// # Input
    /// Takes a unit vector representing the delta
    /// between the player's world position and the mouse.
    /// Also takes the keyframes from the game's configuration files.
    ///
    /// # Output
    /// This function returns the offset the weapon
    /// should have from the player, according to this
    /// weapon's offset field and any reloading animation
    /// that may be taking place.
    ///
    /// # Representation
    /// Instead of processing rotations as `UnitComplex`es,
    /// this function treats them as `Vec2`s, for ease of lerping
    /// among a host of other factors.
    fn weapon_frame(
        &mut self,
        mouse_delta: Unit<Vec2>,
        keyframes: &Vec<KeyFrame>,
    ) -> (Vec2, Unit<Vec2>, f32) {
        let mut prog = match self.progress_reload() {
            ReloadProgress::Complete => return (self.offset, mouse_delta, self.bottom_padding),
            ReloadProgress::Progress(prog) => prog,
        };

        let mut frames = keyframes.iter();

        // the implied last frame, pointing towards the mouse
        let last = (1.0, self.offset, mouse_delta, self.bottom_padding);

        // find the key frames before and after our current time
        let mut lf = frames.next().unwrap();
        let (r_time, r_pos, r_rot, r_padding) = frames
            .find_map(|rf| {
                if rf.0 > prog {
                    // short circuit, we found the first
                    // frame with a timestamp higher than ours
                    Some(rf)
                } else {
                    // not high enough, but maybe it's a lower bound?
                    lf = rf;
                    None
                }
            })
            .unwrap_or(&last);
        let (l_time, l_pos, l_rot, l_padding) = lf;

        // scale prog according to how close to rft it is from lft
        // i.e. 1 would mean it's literally rft, 0 is literally lft
        prog = (prog - l_time) / (r_time - l_time);

        (
            l_pos.lerp(&r_pos, prog),
            l_rot.slerp(&r_rot, prog),
            l_padding + (r_padding - l_padding) * prog,
        )
    }

    /// Moves timer forward. Returns an enum indicating whether
    /// or not reloading is complete and if not how close it is
    /// to being ready.
    fn progress_reload(&mut self) -> ReloadProgress {
        if self.timer >= self.equip_time {
            ReloadProgress::Complete
        } else {
            self.timer += 1;
            ReloadProgress::Progress((self.timer as f32) / (self.equip_time as f32))
        }
    }

    fn can_launch(&self) -> bool {
        self.timer >= self.equip_time
    }

    /// Resets the timer
    fn launch(&mut self) {
        self.timer = 0;
    }
}

enum ReloadProgress {
    Complete,
    Progress(f32),
}
enum QueuedAction {
    LaunchWeapon(Entity, Vec2),
    InsertIso(Entity, Iso2),
}

pub fn aiming(world: &mut World, window: &mut Window, cfg: &Config) {
    let needs_velocity: Vec<QueuedAction> = world
        .query::<(&Iso2, &mut Inventory, &PlayerControlled)>()
        .into_iter()
        // updates the weapon's position relative to the wielder,
        // if clicking, queues adding velocity to the weapon and unequips it.
        // if the weapon that's been equipped doesn't have an iso, queue adding one
        .filter_map(|(_, (wielder_iso, inv, _))| {
            let wep_ent = inv.equipped()?;
            let mut weapon = world.get_mut::<Weapon>(wep_ent).ok()?;
            let mut appearance = world.get_mut::<crate::graphics::Appearance>(wep_ent).ok()?;

            // physics temporaries
            let mouse = window.mouse();
            let delta = Unit::new_normalize(
                (wielder_iso.translation.vector + weapon.offset) - mouse.pos().into_vector(),
            );
            let (mut pos, rot, padding) = weapon.weapon_frame(delta, &cfg.keyframes);

            // apply the bottom padding to the Appearance
            appearance.alignment = crate::graphics::Alignment::Bottom(padding);

            // get the final position by applying the offset to the world pos
            pos += wielder_iso.translation.vector;

            // get the final rotation by converting it to a UnitComplex and adjusting
            let rot = UnitComplex::rotation_between_axis(&Unit::new_unchecked(Vec2::x()), &rot)
                * UnitComplex::new(-std::f32::consts::FRAC_PI_2);

            // get the weapon's current position if it has one,
            // otherwise get one inserted onto it ASAP.
            let mut wep_iso = match world.get_mut::<Iso2>(wep_ent) {
                Ok(iso) => iso,
                Err(_) => {
                    let mut new_pos = *wielder_iso;
                    new_pos.translation.vector = pos;
                    new_pos.rotation = rot;
                    return Some(QueuedAction::InsertIso(wep_ent, new_pos));
                }
            };

            // applying translation and rotation provided by weapon_frame
            wep_iso.translation.vector = pos;
            wep_iso.rotation = rot;

            // queue launch if clicking
            if weapon.can_launch() && mouse[MouseButton::Left].is_down() {
                weapon.launch();
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
