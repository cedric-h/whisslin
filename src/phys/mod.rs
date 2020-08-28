pub mod collision;
pub mod movement;

pub type CollisionWorld = ncollide2d::world::CollisionWorld<f32, hecs::Entity>;
pub type PhysHandle = ncollide2d::pipeline::CollisionObjectSlabHandle;
pub use ncollide2d::{pipeline::CollisionGroups, shape::Cuboid};

use crate::World;

/// Collision Group Constants
pub mod collide {
    pub const PLAYER: usize = 1;
    pub const WEAPON: usize = 2;
    pub const ENEMY: usize = 3;

    /// Fences, Terrain, etc.
    pub const WORLD: usize = 5;
}

pub fn phys_insert(
    ecs: &mut hecs::World,
    phys: &mut CollisionWorld,
    entity: hecs::Entity,
    iso: na::Isometry2<f32>,
    cuboid: Cuboid<f32>,
    groups: CollisionGroups,
) -> PhysHandle {
    let (h, _) = phys.add(
        iso,
        ncollide2d::shape::ShapeHandle::new(cuboid),
        groups,
        ncollide2d::pipeline::GeometricQueryType::Contacts(0.0, 0.0),
        entity,
    );
    ecs.insert_one(entity, collision::Contacts::new())
        .unwrap_or_else(|e| {
            panic!(
                "Couldn't insert Contacts for Entity[{:?}] when adding hitbox: {}",
                entity, e
            )
        });
    ecs.insert_one(entity, h).unwrap_or_else(|e| {
        panic!(
            "Couldn't insert PhysHandle[{:?}] for Entity[{:?}] to add hitbox: {}",
            h, entity, e
        )
    });

    h
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
pub fn velocity(world: &mut World) {
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

/// Note: Also does the calculations for LookChase and Charge
pub fn chase(world: &mut World) {
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
