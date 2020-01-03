pub mod aiming;
pub mod collision;
pub mod movement;

use crate::{CollisionWorld, World};
use crate::{PhysHandle, Vec2};

/// DragTowards moves an Entity towards the supplied location (`goal_loc`) until the
/// Entity's Iso2's translation's `vector` is within the supplied speed (`speed`) of the
/// given location, at which point the DragTowards component is removed from the Entity
/// at the end of the next frame.
pub struct DragTowards {
    pub goal_loc: Vec2,
    pub speed: f32,
    speed_squared: f32,
}
impl DragTowards {
    pub fn new(goal_loc: Vec2, speed: f32) -> Self {
        Self {
            goal_loc,
            speed,
            speed_squared: speed.powi(2),
        }
    }
}

/// Chase moves an Entity's Iso2's translation's `vector` field towards another Entity's,
/// at the supplied rate (`speed`), removing the Chase component from the entity when
/// their positions are within `speed` of each other.
///
/// # Panics
/// This could potentially cause panics in the chase function if you have an Entity chase itself.
pub struct Chase {
    pub goal_ent: hecs::Entity,
    pub speed: f32,
    speed_squared: f32,
}
impl Chase {
    pub fn new(goal_ent: hecs::Entity, speed: f32) -> Self {
        Self {
            goal_ent,
            speed,
            speed_squared: speed.powi(2),
        }
    }
}

#[inline]
/// The result `.is_some()` if progress has been made wrt. the dragging,
/// and is `Some(true)` if the goal has been reached.
fn drag_goal(
    PhysHandle(h): PhysHandle,
    phys: &mut CollisionWorld,
    goal: &Vec2,
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

pub struct Velocity(Vec2);

pub fn velocity(world: &mut World) {
    let ecs = &world.ecs;
    let l8r = &mut world.l8r;
    let phys = &mut world.phys;

    for (_, (PhysHandle(h), &Velocity(vel))) in &mut world.ecs.query::<(&PhysHandle, &Velocity)>() {
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

    for (drag_ent, (hnd, drag)) in ecs.query::<(&PhysHandle, &DragTowards)>().iter() {
        // if the dragging is successful and the goal is reached...
        if let Some(true) = drag_goal(*hnd, phys, &drag.goal_loc, drag.speed, drag.speed_squared) {
            l8r.remove_one::<DragTowards>(drag_ent);
        }
    }
}

pub fn chase(world: &mut World) {
    let ecs = &world.ecs;
    //let l8r = &mut world.l8r;
    let phys = &mut world.phys;

    for (_, (hnd, chase)) in ecs.query::<(&PhysHandle, &Chase)>().iter() {
        (|| {
            let goal_loc = {
                let PhysHandle(goal_h) = *ecs.get::<PhysHandle>(chase.goal_ent).ok()?;
                phys.collision_object(goal_h)?.position().translation.vector
            };

            if drag_goal(*hnd, phys, &goal_loc, chase.speed, chase.speed_squared)? {
                //world.l8r.remove_one::<Chase>(chaser_ent);
            }

            Some(())
        })();
    }
}
