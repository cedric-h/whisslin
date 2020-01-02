pub mod aiming;
pub mod collision;
pub mod movement;

use crate::World;
use crate::{Iso2, Vec2};

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
fn drag_goal(loc: &mut Vec2, goal: &Vec2, speed: f32, speed_squared: f32) -> bool {
    let delta = *loc - *goal;
    if delta.magnitude_squared() < speed_squared {
        *loc = *goal;
        true
    } else {
        *loc -= delta.normalize() * speed;
        false
    }
}

pub struct Velocity(Vec2);

pub fn velocity(world: &mut World) {
    for (_, (iso, &Velocity(vel))) in &mut world.ecs.query::<(&mut Iso2, &Velocity)>() {
        iso.translation.vector += vel;
    }

    for (drag_ent, (iso, drag)) in world.ecs.query::<(&mut Iso2, &DragTowards)>().iter() {
        if drag_goal(
            &mut iso.translation.vector,
            &drag.goal_loc,
            drag.speed,
            drag.speed_squared,
        ) {
            world.l8r.remove_one::<DragTowards>(drag_ent);
        }
    }
}

pub fn chase(world: &mut World) {
    for (_, (iso, chase)) in world.ecs.query::<(&mut Iso2, &Chase)>().iter() {
        let goal_iso = match unsafe { world.ecs.get_unchecked::<Iso2>(chase.goal_ent) } {
            Ok(iso) => iso,
            Err(_) => continue,
        };
        if drag_goal(
            &mut iso.translation.vector,
            &goal_iso.translation.vector,
            chase.speed,
            chase.speed_squared,
        ) {
            //world.l8r.remove_one::<Chase>(chaser_ent);
        }
    }
}
