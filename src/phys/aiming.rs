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
    pub speed: f32,
    timer: u16,
}
impl Weapon {
    pub fn new() -> Self {
        Self {
            offset: na::zero(),
            equip_time: 60,
            timer: 0,
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

    pub fn with_equip_time(mut self, time: u16) -> Self {
        self.equip_time = time;
        self
    }

    /// Moves timer forward. Retruns true if launching is possible.
    pub fn progress(&mut self) -> bool {
        if self.timer >= self.equip_time {
            true
        } else {
            self.timer += 1;
            false
        }
    }

    /// Resets the timer
    pub fn launch(&mut self) {
        self.timer = 0;
    }
}

enum QueuedAction {
    LaunchSpear(Entity, Vec2),
    InsertIso(Entity, Iso2),
}

pub fn aiming(world: &mut World, window: &mut Window) {
    let needs_velocity: Vec<QueuedAction> = world
        .query::<(&Iso2, &mut Inventory, &PlayerControlled)>()
        .into_iter()
        // updates the weapon's position relative to the wielder,
        // if clicking, queues adding velocity to the weapon and unequips it.
        // if the weapon that's been equipped doesn't have an iso, queue adding one
        .filter_map(|(_, (wielder_iso, inv, _))| {
            let wep_ent = inv.equipped()?;
            let mut weapon = world.get_mut::<Weapon>(wep_ent).ok()?;
            let can_launch = weapon.progress();

            // physics temporaries
            let wielder_loc = wielder_iso.translation.vector + weapon.offset;
            let mouse = window.mouse();
            let delta = Unit::new_normalize(wielder_loc - mouse.pos().into_vector());

            // the rotation the weapon should have
            let rotation =
                UnitComplex::rotation_between_axis(&Unit::new_unchecked(Vec2::x()), &delta)
                    * UnitComplex::new(-std::f32::consts::FRAC_PI_2);

            // get the weapon's current position if it has one,
            // otherwise get one inserted onto it ASAP.
            let mut wep_iso = match world.get_mut::<Iso2>(wep_ent) {
                Ok(iso) => iso,
                Err(_) => {
                    let mut new_pos = *wielder_iso;
                    new_pos.rotation = rotation;
                    new_pos.translation.vector = wielder_loc;
                    return Some(QueuedAction::InsertIso(wep_ent, new_pos));
                }
            };

            // rotate weapon to face mouse
            wep_iso.rotation = rotation;
            // put the weapon where the wielder is
            wep_iso.translation.vector = wielder_loc;

            // queue launch if clicking
            if can_launch && mouse[MouseButton::Left].is_down() {
                weapon.launch();
                inv.consume_equipped();
                Some(QueuedAction::LaunchSpear(
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
            LaunchSpear(wep_ent, launch_towards) => {
                world
                    .insert_one(wep_ent, super::Velocity(-launch_towards))
                    .expect("Couldn't insert velocity to launch spear!");
            }
            InsertIso(wep_ent, wep_iso) => {
                world
                    .insert_one(wep_ent, wep_iso)
                    .expect("Couldn't insert iso onto wep_ent!");
            }
        }
    });
}
