use crate::{
    draw,
    phys::{self, PhysHandle},
    world, World,
};
use macroquad::*;

/// Instead of processing rotations as `UnitComplex`es,
/// this function treats them as `na::Vector2`s, for ease of lerping
/// among a host of other factors.
#[derive(Debug)]
pub struct KeyFrame {
    pub time: f32,
    pub pos: na::Vector2<f32>,
    pub rot: na::Unit<na::Vector2<f32>>,
    pub bottom_offset: f32,
}
impl KeyFrame {
    fn into_iso2(self) -> na::Isometry2<f32> {
        na::Isometry2::from_parts(
            na::Translation2::from(self.pos),
            na::UnitComplex::rotation_between_axis(
                &na::Unit::new_unchecked(-na::Vector2::y()),
                &self.rot,
            ),
        )
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
    /// TODO: A way to leave this stage (without firing).
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

#[derive(Clone)]
pub struct Weapon {
    // positioning
    pub offset: na::Vector2<f32>,
    pub bottom_offset: f32,

    // animations
    pub equip_time: u16,
    pub readying_time: u16,

    // projectile
    pub force_magnitude: f32,
    /// Range [0, 1] unless you want your Weapon to get exponentially faster each frame.
    pub force_decay: f32,
    pub hitbox_size: na::Vector2<f32>,
    pub hitbox_groups: phys::CollisionGroups,
    pub prelaunch_groups: phys::CollisionGroups,
    pub boomerang: bool,

    // side effects
    pub player_knock_back_force: f32,
    pub player_knock_back_decay: f32,
}
impl Default for Weapon {
    fn default() -> Self {
        Self {
            // positioning
            offset: na::zero(),
            bottom_offset: 0.0,

            // timing
            equip_time: 60,
            readying_time: 60,

            // projectile
            hitbox_size: na::Vector2::new(1.0, 1.0),
            hitbox_groups: {
                phys::CollisionGroups::new()
                    .with_membership(&[phys::collide::WEAPON])
                    .with_whitelist(&[phys::collide::WORLD, phys::collide::ENEMY])
            },
            prelaunch_groups: {
                phys::CollisionGroups::new()
                    .with_membership(&[phys::collide::WEAPON])
                    .with_blacklist(&[phys::collide::PLAYER, phys::collide::ENEMY])
            },
            force_magnitude: 1.0,
            force_decay: 1.0,
            boomerang: false,

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
        mouse_delta: na::Unit<na::Vector2<f32>>,
        state: WielderState,
        keyframes: &[KeyFrame],
    ) -> Option<KeyFrame> {
        // the implied last frame of the reloading animtion,
        // pointing towards the mouse.
        let mut last = KeyFrame {
            time: 1.0,
            pos: self.offset,
            rot: mouse_delta,
            bottom_offset: self.bottom_offset,
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
                last.bottom_offset *= 1.0 - (timer as f32) / (self.readying_time as f32);
                Some(last)
            }
            WielderState::Readied | WielderState::Shooting => {
                last.bottom_offset = 0.0;
                Some(last)
            }
        }
    }

    fn reloading_animation_frame(
        mut prog: f32,
        keyframes: &[KeyFrame],
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
            bottom_offset: lf.bottom_offset + (rf.bottom_offset - lf.bottom_offset) * prog,
        }
    }
}

// updates the weapon's position relative to the wielder,
// if clicking, queues adding velocity to the weapon and unequips it.
// if the weapon that's been equipped doesn't have an iso, queue adding one
pub fn aiming(world: &mut World) -> Option<()> {
    let World {
        ecs,
        l8r,
        phys,
        camera,
        player:
            world::Player {
                entity: wielder_ent,
                phys_handle: wielder_h,
                weapon: player_weapon,
                wielder,
            },
        ..
    } = world;

    let wielder_iso = phys.collision_object(*wielder_h)?.position();

    let wep_ent = player_weapon.clone()?;
    let mut weapon = ecs.get_mut::<Weapon>(wep_ent).ok()?;

    // physics temporaries
    let mouse = {
        let (mouse_x, mouse_y) = mouse_position();
        let x = -(mouse_x - screen_width() / 2.0);
        let y = mouse_y - screen_height() / 2.0;
        camera.iso = na::Isometry2::translation(weapon.offset.x, weapon.offset.y);
        camera.world_to_screen(na::Vector2::new(x, y))
    };
    let delta = -na::Unit::new_normalize(mouse);

    let from_rot = |rot| {
        na::Unit::new_normalize(
            na::UnitComplex::from_angle(rot).transform_vector(&na::Vector2::x()),
        )
    };
    let keyframes = vec![
        KeyFrame {
            time: 0.0,
            pos: na::Vector2::new(-0.2, -0.4),
            rot: from_rot(-25.0),
            bottom_offset: -0.5,
        },
        KeyFrame {
            time: 0.2,
            pos: na::Vector2::new(0.5, -0.8),
            rot: from_rot(-45.0),
            bottom_offset: -0.4,
        },
        KeyFrame {
            time: 0.4,
            pos: na::Vector2::new(0.6, -0.9),
            rot: from_rot(-200.0),
            bottom_offset: -0.6,
        },
        KeyFrame {
            time: 0.6,
            pos: na::Vector2::new(0.0, -0.7),
            rot: from_rot(-350.0),
            bottom_offset: -0.3,
        },
        KeyFrame {
            time: 0.7,
            pos: na::Vector2::new(0.0, -0.7),
            rot: from_rot(25.0),
            bottom_offset: 0.2,
        },
    ];
    wielder.advance_state(is_mouse_button_down(MouseButton::Left), &weapon);
    let frame = weapon.animation_frame(delta, wielder.state, &keyframes)?;

    // updating the weapon's looks
    {
        let mut wep_looks = ecs.get_mut::<draw::Looks>(wep_ent).ok()?;
        wep_looks.bottom_offset = frame.bottom_offset;
        //wep_looks.flip_x = wielder_looks.flip_x;
    }

    // handle positioning
    let mut frame_iso = frame.into_iso2();
    frame_iso.translation.vector += wielder_iso.translation.vector;

    // get and modify if possible or just insert the weapon's current position
    let wep_h = *ecs.get::<PhysHandle>(wep_ent)
        .map_err(|_| {
            let groups = weapon.prelaunch_groups.clone();
            let size = weapon.hitbox_size.clone();
            l8r.l8r(move |world| {
                world.add_hitbox(
                    wep_ent,
                    frame_iso,
                    ncollide2d::shape::Cuboid::new(size),
                    groups,
                );
            })
        })
        .ok()?;
    let wep_obj = phys.get_mut(wep_h)?;
    wep_obj.set_position(frame_iso);

    if let WielderState::Reloading { timer: 0 } = wielder.state {
        wep_obj.set_collision_groups(weapon.prelaunch_groups);
    }

    // fire the spear if the wielder state indicates to do so!
    if wielder.shooting() {
        // cut off ties between weapon/player
        if !weapon.boomerang {
            *player_weapon = None;
        }

        // side effect! (knockback)
        l8r.insert_one(
            *wielder_ent,
            phys::Force::new(
                delta.into_inner() * -weapon.player_knock_back_force,
                weapon.player_knock_back_decay,
            ),
        );

        // the spear needs to go forward and run into things now.
        //
        // damage isn't configured here because the spear was Hurtful the entire time,
        // it's only now even able to collide with things.
        wep_obj.set_collision_groups(weapon.hitbox_groups);

        l8r.insert_one(
            wep_ent,
            // the no clear is important for not knocking back things later
            phys::Force::new_no_clear(
                delta.into_inner() * weapon.force_magnitude,
                weapon.force_decay,
            ),
        );
    }

    Some(())
}
