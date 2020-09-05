use crate::{
    draw,
    phys::{self, PhysHandle},
    world, World,
};
use macroquad::*;

#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Rot(pub f32);

impl Rot {
    fn as_unit(self) -> na::Unit<na::Vector2<f32>> {
        na::Unit::new_normalize(
            na::UnitComplex::from_angle(self.0).transform_vector(&na::Vector2::x()),
        )
    }

    fn from_unit(unit: na::Unit<na::Vector2<f32>>) -> Self {
        Rot(unit.angle(&na::Vector2::x()))
    }
}

impl Into<na::Unit<na::Vector2<f32>>> for Rot {
    fn into(self) -> na::Unit<na::Vector2<f32>> {
        self.as_unit()
    }
}

/// Instead of processing rotations as `UnitComplex`es,
/// this function treats them as `na::Vector2`s, for ease of lerping
/// among a host of other factors.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyFrame {
    pub time: f32,
    pub pos: na::Vector2<f32>,
    pub rot: Rot,
    pub bottom_offset: f32,
}
impl KeyFrame {
    fn into_iso2(self) -> na::Isometry2<f32> {
        na::Isometry2::new(self.pos, self.rot.0)
    }

    pub fn dev_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("time");
        ui.add(egui::DragValue::f32(&mut self.time).speed(0.01));

        ui.label("bottom offset");
        ui.add(egui::DragValue::f32(&mut self.bottom_offset).speed(0.01));

        ui.add(egui::Slider::f32(&mut self.rot.0, -180.0..=180.0).text("rotation"));

        ui.label("position");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::f32(&mut self.pos.x).speed(0.01));
            ui.add(egui::DragValue::f32(&mut self.pos.y).speed(0.01));
        });
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

pub fn weapon_hitbox_groups() -> phys::CollisionGroups {
    phys::CollisionGroups::new()
        .with_membership(&[phys::collide::WEAPON])
        .with_whitelist(&[phys::collide::WORLD, phys::collide::ENEMY])
}
pub fn weapon_prelaunch_groups() -> phys::CollisionGroups {
    phys::CollisionGroups::new()
        .with_membership(&[phys::collide::WEAPON])
        .with_blacklist(&[phys::collide::PLAYER, phys::collide::ENEMY])
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Weapon {
    // positioning
    offset: na::Vector2<f32>,
    bottom_offset: f32,

    // animations
    equip_time: u16,
    readying_time: u16,

    // projectile
    force_magnitude: f32,
    /// Range [0, 1] unless you want your Weapon to get exponentially faster each frame.
    force_decay: f32,
    hitbox_size: na::Vector2<f32>,
    boomerang: bool,
    #[serde(skip, default = "weapon_hitbox_groups")]
    hitbox_groups: phys::CollisionGroups,
    #[serde(skip, default = "weapon_prelaunch_groups")]
    prelaunch_groups: phys::CollisionGroups,

    // side effects
    player_knock_back_force: f32,
    player_knock_back_decay: f32,

    keyframes: Vec<KeyFrame>,
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
        &self,
        mouse_delta: na::Unit<na::Vector2<f32>>,
        state: WielderState,
    ) -> Option<KeyFrame> {
        // the implied last frame of the reloading animtion,
        // pointing towards the mouse.
        let mut last = KeyFrame {
            time: 1.0,
            pos: self.offset,
            rot: Rot(mouse_delta.angle(&na::Vector2::x())),
            bottom_offset: self.bottom_offset,
        };

        // read timers
        Some(match state {
            WielderState::Summoning { .. } => return None,
            WielderState::Reloading { timer } => {
                self.reloading_animation_frame((timer as f32) / (self.equip_time as f32), &last)
            }
            WielderState::Loaded => last,
            WielderState::Readying { timer } => {
                last.bottom_offset *= 1.0 - (timer as f32) / (self.readying_time as f32);
                last
            }
            WielderState::Readied | WielderState::Shooting => {
                last.bottom_offset = 0.0;
                last
            }
        })
    }

    fn reloading_animation_frame(&self, mut prog: f32, last: &KeyFrame) -> KeyFrame {
        let mut frames = self.keyframes.iter();

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
            rot: Rot::from_unit(lf.rot.as_unit().slerp(&rf.rot.into(), prog)),
            bottom_offset: lf.bottom_offset + (rf.bottom_offset - lf.bottom_offset) * prog,
        }
    }

    pub fn dev_ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Default Position", |ui| {
            ui.label("bottom offset");
            ui.add(egui::DragValue::f32(&mut self.bottom_offset).speed(0.01));

            ui.label("position");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::f32(&mut self.offset.x).speed(0.01));
                ui.add(egui::DragValue::f32(&mut self.offset.y).speed(0.01));
            });
        });

        ui.collapsing("Timing", |ui| {
            ui.label("readying time");
            let mut rt = self.readying_time as f32;
            ui.add(egui::DragValue::f32(&mut rt));
            self.readying_time = rt.round() as u16;

            ui.label("equip time");
            let mut et = self.equip_time as f32;
            ui.add(egui::DragValue::f32(&mut et));
            self.equip_time = et.round() as u16;
        });

        ui.collapsing("Projectile", |ui| {
            ui.label("hitbox size");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::f32(&mut self.hitbox_size.x).speed(0.01));
                ui.add(egui::DragValue::f32(&mut self.hitbox_size.y).speed(0.01));
            });

            ui.label("force magnitude");
            ui.add(egui::DragValue::f32(&mut self.force_magnitude).speed(0.01));

            ui.label("force decay");
            ui.add(egui::DragValue::f32(&mut self.force_decay).speed(0.01));

            ui.checkbox("boomerang", &mut self.boomerang)
                .tooltip_text("do you automatically get this weapon back after having thrown it?");
        });

        ui.collapsing("Side Effects", |ui| {
            ui.label("player knock back force");
            ui.add(egui::DragValue::f32(&mut self.player_knock_back_force).speed(0.01));

            ui.label("player knock back decay");
            ui.add(egui::DragValue::f32(&mut self.player_knock_back_decay).speed(0.01));
        });

        ui.collapsing("Keyframes", |ui| {
            for (i, kf) in self.keyframes.iter_mut().enumerate() {
                ui.collapsing(format!("keyframe {}", i), |ui| kf.dev_ui(ui));
            }
        });
    }
}

// updates the weapon's position relative to the wielder,
// if clicking, queues adding velocity to the weapon and unequips it.
// if the weapon that's been equipped doesn't have an iso, queue adding one
pub fn aiming(
    World {
        ecs,
        l8r,
        phys,
        config:
            world::Config {
                player: world::player::Config { weapon, .. },
                draw: draw_config,
            },
        player:
            world::Player {
                entity: wielder_ent,
                phys_handle: wielder_h,
                weapon_entity,
                wielder,
                ..
            },
        ..
    }: &mut World,
) -> Option<()> {
    let wielder_iso = phys.collision_object(*wielder_h)?.position();
    let wep_ent = weapon_entity.clone()?;

    // physics temporaries
    let mouse = {
        let (mouse_x, mouse_y) = mouse_position();
        let x = -(mouse_x - screen_width() / 2.0);
        let y = mouse_y - screen_height() / 2.0;
        let cam = draw_config.camera(na::Isometry2::translation(weapon.offset.x, weapon.offset.y));
        cam.world_to_screen(na::Vector2::new(x, y))
    };
    let delta = -na::Unit::new_normalize(mouse);

    wielder.advance_state(is_mouse_button_down(MouseButton::Left), &weapon);
    let frame = weapon.animation_frame(delta, wielder.state)?;

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
    let wep_h = *ecs
        .get::<PhysHandle>(wep_ent)
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
            *weapon_entity = None;
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
