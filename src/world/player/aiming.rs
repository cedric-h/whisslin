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
pub struct Keyframe {
    pub time: f32,
    pub pos: na::Vector2<f32>,
    pub rot: Rot,
    pub bottom_offset: f32,
    #[cfg(feature = "confui")]
    #[serde(skip, default)]
    removal_checkbox_checked: bool,
    #[cfg(feature = "confui")]
    #[serde(skip, default)]
    removal_checkbox_out: bool,
}

#[cfg(feature = "confui")]
pub enum KeyframeDevUiEvent {
    Remove,
}

impl Keyframe {
    fn into_iso2(self) -> na::Isometry2<f32> {
        na::Isometry2::new(self.pos, self.rot.0)
    }

    #[cfg(feature = "confui")]
    pub fn dev_ui(&mut self, ui: &mut egui::Ui) -> Option<KeyframeDevUiEvent> {
        if ui.button("remove keyframe").clicked {
            if !self.removal_checkbox_out {
                self.removal_checkbox_out = true;
            } else if self.removal_checkbox_checked {
                return Some(KeyframeDevUiEvent::Remove);
            }
        }
        if self.removal_checkbox_out {
            ui.checkbox("Are you sure?", &mut self.removal_checkbox_checked)
                .tooltip_text("Click the 'remove keyframe' button again after checking me.");
        }

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

        None
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum WielderState {
    /// Sit and think about how you just wasted that last weapon.
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

    /// Moves timers forward, changes state when necessary
    fn advance_state(
        &mut self,
        mouse_down: bool,
        weapon: &WeaponConfig,
        readying_animation_length: u16,
    ) {
        use WielderState::*;

        self.state = match self.state {
            Reloading { mut timer } => {
                timer += 1;
                if timer >= weapon.reload_time {
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
                } else if timer >= readying_animation_length {
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
            Shooting => Reloading { timer: 0 },
        };
    }

    fn shooting(&self) -> bool {
        self.state == WielderState::Shooting
    }
}

fn weapon_hitbox_groups() -> phys::CollisionGroups {
    phys::CollisionGroups::new()
        .with_membership(&[phys::collide::WEAPON])
        .with_whitelist(&[phys::collide::WORLD, phys::collide::ENEMY])
}
fn weapon_prelaunch_groups() -> phys::CollisionGroups {
    phys::CollisionGroups::new()
        .with_membership(&[phys::collide::WEAPON])
        .with_blacklist(&[phys::collide::PLAYER, phys::collide::ENEMY])
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WeaponConfig {
    // positioning
    offset: na::Vector2<f32>,
    bottom_offset: f32,

    // timing
    reload_time: u16,

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

    keyframes: Vec<Keyframe>,
    animation_art: draw::ArtHandle,
}
impl WeaponConfig {
    /// # Input
    /// Takes a unit vector representing the delta
    /// between the player's world position and the mouse.
    /// (These are used to generate the implied last frame, i.e.
    /// where the spear points at the mouse)
    /// Also takes the keyframes from the game's configuration files.
    ///
    /// # Output
    /// This function returns a Keyframe representing how
    /// the weapon should be oriented at this point in time.
    ///
    /// However, if the weapon shouldn't be given a position at all
    /// (so that it remains unrendered) None is returned.
    fn animation_frame(
        &self,
        mouse_delta: na::Unit<na::Vector2<f32>>,
        state: WielderState,
        readying_animation_length: u16,
    ) -> Option<Keyframe> {
        // the implied last frame of the readying animtion,
        // pointing towards the mouse.
        let mut last = Keyframe {
            time: 1.0,
            pos: self.offset,
            rot: Rot(mouse_delta.angle(&na::Vector2::x())),
            bottom_offset: self.bottom_offset,
            #[cfg(feature = "confui")]
            removal_checkbox_checked: false,
            #[cfg(feature = "confui")]
            removal_checkbox_out: false,
        };

        // read timers
        Some(match state {
            WielderState::Reloading { .. } | WielderState::Loaded => return None,
            WielderState::Readying { timer } => self.readying_animation_frame(
                (timer as f32) / (readying_animation_length as f32),
                &last,
            ),
            WielderState::Readied | WielderState::Shooting => {
                last.rot.0 = 0.0;
                last.bottom_offset = 0.0;
                last
            }
        })
    }

    fn readying_animation_frame(&self, mut prog: f32, last: &Keyframe) -> Keyframe {
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

        Keyframe {
            time: prog,
            pos: lf.pos.lerp(&rf.pos, prog),
            rot: Rot::from_unit(lf.rot.as_unit().slerp(&rf.rot.into(), prog)),
            bottom_offset: lf.bottom_offset + (rf.bottom_offset - lf.bottom_offset) * prog,
            #[cfg(feature = "confui")]
            removal_checkbox_checked: false,
            #[cfg(feature = "confui")]
            removal_checkbox_out: false,
        }
    }

    #[cfg(feature = "confui")]
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
            ui.label("reload time");
            let mut et = self.reload_time as f32;
            ui.add(egui::DragValue::f32(&mut et));
            self.reload_time = et.round() as u16;
        });

        ui.collapsing("Projectile", |ui| {
            ui.label("hitbox size");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::f32(&mut self.hitbox_size.x).speed(0.01));
                ui.add(egui::DragValue::f32(&mut self.hitbox_size.y).speed(0.01));
            });

            ui.label("force magnitude");
            ui.add(egui::DragValue::f32(&mut self.force_magnitude).speed(0.01));

            ui.add(egui::Slider::f32(&mut self.force_decay, 0.0..=1.0).text("force decay"));

            ui.checkbox("boomerang", &mut self.boomerang)
                .tooltip_text("do you automatically get this weapon back after having thrown it?");
        });

        ui.collapsing("Side Effects", |ui| {
            ui.label("player knock back force");
            ui.add(egui::DragValue::f32(&mut self.player_knock_back_force).speed(0.01));

            ui.add(
                egui::Slider::f32(&mut self.player_knock_back_decay, 0.0..=1.0)
                    .text("player knock back decay")
            );
        });

        ui.collapsing("Keyframes", |ui| {
            let dead_index: Option<usize> = self
                .keyframes
                .iter_mut()
                .enumerate()
                .filter_map(|(i, kf)| {
                    ui
                        .collapsing(format!("keyframe {}", i), |ui| match kf.dev_ui(ui) {
                            Some(KeyframeDevUiEvent::Remove) => Some(i),
                            None => None,
                        })
                        .and_then(|x| x)
                })
                // there can only ever be one removed per frame, so ...
                .next();

            if let Some(i) = dead_index {
                self.keyframes.remove(i);
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
                state: player_state,
                weapon_entity,
                wielder,
                walk_animator,
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
        let cam = draw_config.camera(na::Isometry2::translation(weapon.offset.x, weapon.offset.y).inverse());
        cam.world_to_screen(na::Vector2::new(x, y))
    };
    let delta = -na::Unit::new_normalize(mouse);
    let mouse_down = is_mouse_button_down(MouseButton::Left);

    let readying_animation_length = match draw_config.get(weapon.animation_art).spritesheet {
        Some(ss) => (ss.total.get() * ss.frame_rate) as u16 - 2,
        None => 10,
    };

    // updating the wielder's looks if throwing should be in control
    let wielder_flipped = {
        let mut looks = ecs.get_mut::<draw::Looks>(*wielder_ent).ok()?;

        let frame = match wielder.state {
            WielderState::Readying { timer } => Some(timer),
            WielderState::Readied => Some(readying_animation_length),
            _ => None,
        };
        if let Some(f) = frame {
            *player_state = super::PlayerState::Throwing;
            looks.art = weapon.animation_art;
            if let Ok(mut af) = ecs.get_mut::<draw::AnimationFrame>(*wielder_ent) {
                af.0 = f.into();
            }
            looks.flip_x = delta.x < 0.0;

            // if we're leaving these states it's important to give animation control back to walking
            if !mouse_down {
                walk_animator.direction = super::Direction::Side;
                *player_state = super::PlayerState::Walking;
            }
        };

        looks.flip_x
    };

    wielder.advance_state(mouse_down, &weapon, readying_animation_length);
    let frame = weapon.animation_frame(delta, wielder.state, readying_animation_length)?;

    // updating the weapon's looks
    {
        let mut wep_looks = ecs.get_mut::<draw::Looks>(wep_ent).ok()?;
        wep_looks.bottom_offset = frame.bottom_offset;
    }

    // handle positioning
    let mut frame_iso = frame.into_iso2();
    if wielder_flipped {
        frame_iso.translation.vector.x *= -1.0;
    }
    frame_iso.translation.vector += wielder_iso.translation.vector;
    let wep_h = *ecs.get::<PhysHandle>(wep_ent).ok().or_else(|| {
        let groups = weapon.prelaunch_groups.clone();
        let shape = ncollide2d::shape::Cuboid::new(weapon.hitbox_size.clone());
        l8r.l8r(move |world| drop(world.add_hitbox(wep_ent, frame_iso, shape, groups)));
        None
    })?;

    let wep_obj = phys.get_mut(wep_h)?;
    wep_obj.set_position(frame_iso);

    // fire the spear if the wielder state indicates to do so!
    if wielder.shooting() {
        // cut off ties between weapon/player
        if !weapon.boomerang {
            *weapon_entity = None;
        }

        // let walking regain control of animating the wielder
        walk_animator.direction = super::Direction::Side;
        *player_state = super::PlayerState::Walking;

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
