use crate::CollisionWorld;
use crate::PhysHandle;
use nalgebra::base::Unit;
use quicksilver::input::Mouse;

pub struct FacesCursor;

pub fn face_cursor(
    mouse: &Mouse,
    collision_world: &CollisionWorld,
    appearance: &mut crate::graphics::Appearance,
    phys_handle: &PhysHandle,
) {
    let &PhysHandle(actual_handle) = phys_handle;
    let iso2 = collision_world
        .collision_object(actual_handle)
        .unwrap()
        .position();

    let difference = Unit::new_normalize(mouse.pos().into_vector() - (iso2.translation.vector));

    appearance.flip_x = difference.x < 0.0;
}
