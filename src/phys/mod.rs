pub mod aiming;
pub mod collision;
pub mod movement;

use crate::{Iso2, Vec2};

pub struct Velocity(Vec2);

pub fn velocity(world: &mut hecs::World) {
    for (_, (iso, &Velocity(vel))) in &mut world.query::<(&mut Iso2, &Velocity)>() {
        iso.translation.vector += vel;
    }
}
