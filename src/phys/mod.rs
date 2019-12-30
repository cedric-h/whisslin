pub mod aiming;
pub mod collision;
pub mod movement;

use crate::World;
use crate::{Iso2, Vec2};

pub struct DragTowards {
    pub goal: Vec2,
    pub speed: f32,
    speed_squared: f32,
}
impl DragTowards {
    pub fn new(goal: Vec2, speed: f32) -> Self {
        Self {
            goal,
            speed,
            speed_squared: speed.powi(2),
        }
    }
}

pub struct Velocity(Vec2);

pub fn velocity(world: &mut World) {
    for (_, (iso, &Velocity(vel))) in &mut world.ecs.query::<(&mut Iso2, &Velocity)>() {
        iso.translation.vector += vel;
    }

    for (drag_ent, (iso, drag)) in world.ecs.query::<(&mut Iso2, &DragTowards)>().iter() {
        let delta = iso.translation.vector - drag.goal;
        if delta.magnitude_squared() < drag.speed_squared {
            iso.translation.vector = drag.goal;
            world.l8r.remove_one::<DragTowards>(drag_ent);
        } else {
            iso.translation.vector -= delta.normalize() * drag.speed;
        }
    }
}
