pub mod collision;

pub type CollisionWorld = ncollide2d::world::CollisionWorld<f32, hecs::Entity>;
pub type PhysHandle = ncollide2d::pipeline::CollisionObjectSlabHandle;
pub use ncollide2d::{pipeline::CollisionGroups, shape::Cuboid};

use crate::Game;
use glsp::FromVal;

/// Collision Group Constants
#[derive(serde::Deserialize, serde::Serialize, Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u32)]
pub enum Collide {
    Player,
    Weapon,
    Enemy,
    /// Fences, Terrain, etc.
    World,
    Creature,
}
impl FromVal for Collide {
    fn from_val(val: &glsp::Val) -> glsp::GResult<Self> {
        let sym = glsp::Sym::from_val(val)?;
        Ok(match &*sym.name() {
            "Player" => Self::Player,
            "Weapon" => Self::Weapon,
            "Enemy" => Self::Enemy,
            "World" => Self::World,
            "Creature" => Self::Creature,
            _ => glsp::bail!("Not a valid Collision marker: {}", sym),
        })
    }
}

#[cfg(feature = "confui")]
const ALL_COLLIDE: &[Collide] = {
    use Collide::*;
    &[Player, Weapon, Enemy, World, Creature]
};

/// A collision relationship :P
#[derive(serde::Deserialize, serde::Serialize, Default, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Collisionship {
    collision_static: Option<collision::CollisionStatic>,

    #[serde(default)]
    pub blacklist: std::collections::HashSet<Collide>,

    #[cfg(feature = "confui")]
    #[serde(skip)]
    adding_blacklist: Option<Collide>,

    #[serde(default)]
    pub whitelist: std::collections::HashSet<Collide>,

    #[cfg(feature = "confui")]
    #[serde(skip)]
    adding_whitelist: Option<Collide>,

    #[serde(default)]
    pub membership: std::collections::HashSet<Collide>,

    #[cfg(feature = "confui")]
    #[serde(skip)]
    adding_membership: Option<Collide>,
}
impl Collisionship {
    #[cfg(feature = "confui")]
    /// Returns `true` if "dirty" i.e. meaningful outward-facing changes to the data occured.
    pub fn dev_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;

        let mut stat = self.collision_static.is_some();
        if ui.checkbox("Immovable", &mut stat).clicked {
            self.collision_static = if stat {
                Some(collision::CollisionStatic)
            } else {
                None
            };
            dirty = true;
        }

        fn list_edit(
            ui: &mut egui::Ui,
            title: &str,
            adding: &mut Option<Collide>,
            list: &mut std::collections::HashSet<Collide>,
            dirty: &mut bool,
        ) {
            ui.collapsing(title, |ui| {
                *adding = adding.take().and_then(|mut to_add| {
                    for collide in ALL_COLLIDE.iter() {
                        ui.radio_value(format!("{:?}", collide), &mut to_add, *collide);
                    }
                    if ui.button("Add").clicked {
                        *dirty = true;
                        list.insert(to_add);
                        None
                    } else if ui.button("Back").clicked {
                        None
                    } else {
                        Some(to_add)
                    }
                });
                if adding.is_none() {
                    let mut to_remove: Option<Collide> = None;
                    for collide in list.iter() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{:?}", collide));
                            if ui.button("Remove").clicked {
                                to_remove = Some(collide.clone());
                            }
                        });
                    }
                    if ui.button("Add").clicked {
                        *adding = Some(Collide::World);
                    }
                    if let Some(c) = to_remove {
                        *dirty = true;
                        list.remove(&c);
                    }
                }
            });
        }
        list_edit(
            ui,
            "Membership",
            &mut self.adding_membership,
            &mut self.membership,
            &mut dirty,
        );
        list_edit(
            ui,
            "Whitelist",
            &mut self.adding_whitelist,
            &mut self.whitelist,
            &mut dirty,
        );
        list_edit(
            ui,
            "Blacklist",
            &mut self.adding_blacklist,
            &mut self.blacklist,
            &mut dirty,
        );

        dirty
    }

    pub fn into_groups(self) -> CollisionGroups {
        let (_, groups): (Option<collision::CollisionStatic>, CollisionGroups) = self.into();
        groups
    }
}
impl Into<(Option<collision::CollisionStatic>, CollisionGroups)> for Collisionship {
    fn into(self) -> (Option<collision::CollisionStatic>, CollisionGroups) {
        let Self {
            blacklist,
            whitelist,
            membership,
            ..
        } = self;
        let m = |l: std::collections::HashSet<Collide>| {
            l.into_iter().map(|c| c as usize).collect::<Vec<_>>()
        };
        (
            self.collision_static,
            CollisionGroups::new()
                .with_membership(&m(membership))
                .with_whitelist(&m(whitelist))
                .with_blacklist(&m(blacklist)),
        )
    }
}

pub fn phys_components(
    phys: &mut CollisionWorld,
    entity: hecs::Entity,
    iso: na::Isometry2<f32>,
    cuboid: Cuboid<f32>,
    groups: CollisionGroups,
) -> (PhysHandle, collision::Contacts) {
    let (h, _) = phys.add(
        iso,
        ncollide2d::shape::ShapeHandle::new(cuboid),
        groups,
        ncollide2d::pipeline::GeometricQueryType::Contacts(0.0, 0.0),
        entity,
    );
    (h, collision::Contacts::new())
}

pub fn phys_insert(
    ecs: &mut hecs::World,
    phys: &mut CollisionWorld,
    entity: hecs::Entity,
    iso: na::Isometry2<f32>,
    cuboid: Cuboid<f32>,
    groups: CollisionGroups,
) -> PhysHandle {
    let comps = phys_components(phys, entity, iso, cuboid, groups);
    let h = comps.0;
    ecs.insert(entity, comps).unwrap_or_else(|e| {
        panic!(
            "Couldn't add comps for Entity[{:?}] for phys[handle: {:?}] insertion: {}",
            entity, h, e
        )
    });
    h
}

pub fn phys_remove(
    ecs: &mut hecs::World,
    phys: &mut CollisionWorld,
    entity: hecs::Entity,
    h: PhysHandle,
) {
    phys.remove(&[h]);
    ecs.remove::<(collision::Contacts, PhysHandle)>(entity)
        .unwrap_or_else(|e| {
            panic!(
                "Couldn't remove Contacts and PhysHandle for Entity[{:?}] when removing phys: {}",
                entity, e
            )
        });
}

/// DragTowards moves an Entity towards the supplied location (`goal_loc`) until the
/// Entity's Iso2's translation's `vector` is within the supplied speed (`speed`) of the
/// given location, at which point the DragTowards component is removed from the Entity
/// at the end of the next frame.
pub struct DragTowards {
    pub goal_loc: na::Vector2<f32>,
    pub speed: f32,
    speed_squared: f32,
}
impl DragTowards {
    pub fn new(goal_loc: na::Vector2<f32>, speed: f32) -> Self {
        Self {
            goal_loc,
            speed,
            speed_squared: speed.powi(2),
        }
    }
}

/// Chase moves an Entity's Iso2's translation's `vector` field towards another Entity's,
/// at the supplied rate (`speed`), removing the Chase component from the entity when
/// their positions are within `speed` of each other (if `remove_when_reached` is true).
///
/// # Panics
/// This will panic if either entity doesn't have `PhysHandle`s/`CollisionObject`s.
/// Having an Entity chase itself might work but I wouldn't recommend it.
pub struct Chase {
    pub goal_ent: hecs::Entity,
    pub speed: f32,
    pub remove_when_reached: bool,
    speed_squared: f32,
}
impl Chase {
    /// Continues chasing even when the goal entity is reached.
    pub fn determined(goal_ent: hecs::Entity, speed: f32) -> Self {
        Self {
            goal_ent,
            speed,
            remove_when_reached: false,
            speed_squared: speed.powi(2),
        }
    }
}

/// LurchChase applies a force to an entity, in the direction of another entity (`goal_ent`),
/// whenever no forces are found on the entity.
///
/// # Panics
/// This will panic if either entity doesn't have `PhysHandle`s/`CollisionObject`s.
/// Having an Entity chase itself might work but I wouldn't recommend it.
pub struct LurchChase {
    pub goal_ent: hecs::Entity,
    pub magnitude: f32,
    pub decay: f32,
}
impl LurchChase {
    /// Continues chasing even when the goal entity is reached.
    pub fn new(goal_ent: hecs::Entity, magnitude: f32, decay: f32) -> Self {
        Self {
            goal_ent,
            magnitude,
            decay,
        }
    }
}

/// Entities with a Charge component will go forward in the direction they are facing,
/// at the designated speed.
pub struct Charge {
    pub speed: f32,
}
impl Charge {
    pub fn new(speed: f32) -> Self {
        Self { speed }
    }
}

pub struct KnockBack {
    pub groups: CollisionGroups,
    pub force_decay: f32,
    pub force_magnitude: f32,
    /// Whether or not the direction of any Force affecting the object should be used
    /// for the direction for the knock back.
    pub use_force_direction: bool,
    /// If the entity has a Force and the magnitude of the Force's `vec` field isn't at least
    /// this high, no knockback will be administered.
    pub minimum_speed: Option<f32>,
}

/// A Force is applied to an Entity every frame and decays a bit,
/// eventually reaching 0 and being removed. Unlike a Velocity, a Force
/// is only temporary, eventually fading away.
#[derive(Clone)]
pub struct Force {
    pub vec: na::Vector2<f32>,
    /// Domain [0, 1] unless you want the velocity to increase exponentially :thinking:
    pub decay: f32,
    /// Whether or not to remove the component from the entity when the Force isn't really
    /// having an effect any more.
    pub clear: bool,
}
impl Force {
    /// A new Force that is cleared when the velocity decays down to extremely small decimals.
    pub fn new(vec: na::Vector2<f32>, decay: f32) -> Self {
        Self {
            vec,
            decay,
            clear: true,
        }
    }
    /// A new Force that is NOT cleared when the velocity decays down to extremely small decimals.
    pub fn new_no_clear(vec: na::Vector2<f32>, decay: f32) -> Self {
        Self {
            vec,
            decay,
            clear: false,
        }
    }
}

/// Sphereically interpolates the rotation of an Entity with this component towards the
/// position of the Entity provided.
///
/// # Panics
/// This will panic if either entity doesn't have `PhysHandle`s/`CollisionObject`s.
/// Having an Entity look at itself might work but I wouldn't recommend it.
pub struct LookChase {
    pub look_at_ent: hecs::Entity,
    pub speed: f32,
}
impl LookChase {
    pub fn new(look_at_ent: hecs::Entity, speed: f32) -> Self {
        Self { look_at_ent, speed }
    }
}

#[inline]
/// The result `.is_some()` if progress has been made wrt. the dragging,
/// and is `Some(true)` if the goal has been reached.
fn drag_goal(
    h: PhysHandle,
    phys: &mut CollisionWorld,
    goal: &na::Vector2<f32>,
    speed: f32,
    speed_squared: f32,
) -> Option<bool> {
    let obj = phys.get_mut(h)?;
    let mut iso = obj.position().clone();
    let loc = &mut iso.translation.vector;

    let delta = *loc - *goal;
    Some(if delta.magnitude_squared() < speed_squared {
        *loc = *goal;
        obj.set_position_with_prediction(iso, iso);
        true
    } else {
        let vel = delta.normalize() * speed;
        *loc -= vel;
        obj.set_position_with_prediction(iso.clone(), {
            iso.translation.vector -= vel;
            iso
        });
        false
    })
}

pub struct Velocity(na::Vector2<f32>);

/// Also applies Forces and KnockBack.
pub fn velocity(world: &mut Game) {
    let ecs = &world.ecs;
    let l8r = &mut world.l8r;
    let phys = &mut world.phys;

    for (_, (h, &Velocity(vel))) in &mut world.ecs.query::<(&PhysHandle, &Velocity)>() {
        (|| {
            let obj = phys.get_mut(*h)?;
            let mut iso = obj.position().clone();
            iso.translation.vector += vel;
            obj.set_position_with_prediction(iso.clone(), {
                iso.translation.vector += vel;
                iso
            });

            Some(())
        })();
    }

    for (ent, (&h, knock_back, contacts, force)) in
        &mut world
            .ecs
            .query::<(&_, &KnockBack, &collision::Contacts, Option<&Force>)>()
    {
        if let (Some(force), Some(minimum_speed)) = (force, knock_back.minimum_speed) {
            if force.vec.magnitude() < minimum_speed {
                continue;
            }
        }

        let loc = phys
            .collision_object(h)
            .unwrap_or_else(|| {
                panic!(
                    "Entity[{:?}] has PhysHandle[{:?}] but no Collision Object!",
                    ent, h
                )
            })
            .position()
            .translation
            .vector;

        for &o_ent in contacts.iter() {
            (|| {
                ecs.get::<collision::CollisionStatic>(o_ent).err()?;
                let o_h = *ecs.get::<PhysHandle>(o_ent).ok()?;
                /*.unwrap_or_else(|e| panic!(
                    "Entity[{:?}] stored in Contacts[{:?}] but no PhysHandle: {}",
                    o_ent, ent, e
                ));*/
                let o_obj = phys.collision_object(o_h)?;
                /*
                .unwrap_or_else(|| panic!(
                    "Entity[{:?}] stored in Contacts[{:?}] with PhysHandle[{:?}] but no Collision Object!",
                    ent, o_ent, o_h
                ));*/

                if knock_back
                    .groups
                    .can_interact_with_groups(o_obj.collision_groups())
                {
                    let delta = force
                        .map(|f| f.vec)
                        .filter(|_| knock_back.use_force_direction)
                        .unwrap_or_else(|| o_obj.position().translation.vector - loc)
                        .normalize();

                    l8r.insert_one(
                        o_ent,
                        Force::new(delta * knock_back.force_magnitude, knock_back.force_decay),
                    );
                }

                Some(())
            })();
        }
    }

    for (force_ent, (&h, force)) in &mut world.ecs.query::<(&PhysHandle, &mut Force)>() {
        (|| {
            let obj = phys.get_mut(h)?;
            let mut iso = obj.position().clone();

            iso.translation.vector += force.vec;

            force.vec *= force.decay;

            obj.set_position_with_prediction(iso.clone(), {
                iso.translation.vector += force.vec;
                iso
            });

            if force.clear && force.vec.magnitude_squared() < 0.0005 {
                l8r.remove_one::<Force>(force_ent);
            }

            Some(())
        })();
    }

    for (drag_ent, (hnd, drag)) in ecs.query::<(&PhysHandle, &DragTowards)>().iter() {
        // if the dragging is successful and the goal is reached...
        if let Some(true) = drag_goal(*hnd, phys, &drag.goal_loc, drag.speed, drag.speed_squared) {
            l8r.remove_one::<DragTowards>(drag_ent);
        }
    }
}

/// Note: Also does the calculations for LurchChase, LookChase, and Charge
pub fn chase(world: &mut Game) {
    let ecs = &world.ecs;
    let l8r = &mut world.l8r;
    let phys = &mut world.phys;

    let loc_of_ent = |goal_ent, phys: &mut CollisionWorld| -> Option<na::Vector2<f32>> {
        let goal_h = *ecs.get::<PhysHandle>(goal_ent).ok()?;
        Some(phys.collision_object(goal_h)?.position().translation.vector)
    };

    for (chaser_ent, (hnd, chase)) in ecs.query::<(&PhysHandle, &Chase)>().iter() {
        (|| {
            let goal_loc = loc_of_ent(chase.goal_ent, phys)?;

            let within_range = drag_goal(*hnd, phys, &goal_loc, chase.speed, chase.speed_squared)?;
            if within_range && chase.remove_when_reached {
                l8r.remove_one::<Chase>(chaser_ent);
            }

            Some(())
        })();
    }

    for (chaser_ent, (_, lurch)) in ecs
        .query::<hecs::Without<Force, (&PhysHandle, &LurchChase)>>()
        .iter()
    {
        (|| {
            let goal_loc = loc_of_ent(lurch.goal_ent, phys)?;
            let chaser_loc = loc_of_ent(chaser_ent, phys)?;

            let delta = (goal_loc - chaser_loc).normalize();
            l8r.insert_one(chaser_ent, Force::new(delta * lurch.magnitude, lurch.decay));

            Some(())
        })();
    }

    for (_, (&h, &Charge { speed })) in ecs.query::<(&PhysHandle, &Charge)>().iter() {
        (|| {
            let obj = phys.get_mut(h)?;
            let mut iso = obj.position().clone();

            iso.translation.vector -= iso.rotation * -na::Vector2::y() * speed;

            obj.set_position(iso);

            Some(())
        })();
    }

    for (_, (&h, look_chase)) in ecs.query::<(&PhysHandle, &LookChase)>().iter() {
        (|| {
            let look_at_loc = loc_of_ent(look_chase.look_at_ent, phys)?;

            let obj = phys.get_mut(h)?;
            let mut iso = obj.position().clone();

            let delta = na::Unit::new_normalize(iso.translation.vector - look_at_loc);
            let current = na::Unit::new_unchecked(iso.rotation * na::Vector2::x());

            iso.rotation *=
                na::UnitComplex::from_angle(look_chase.speed * delta.dot(&current).signum());

            obj.set_position(iso);

            Some(())
        })();
    }
}
