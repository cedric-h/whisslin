use crate::CollisionWorld;
use crate::PhysHandle;
use crate::Vec2;
use crate::{graphics, World};
use nalgebra::base::Unit;
use quicksilver::input::Mouse;
pub struct FacesCursor;

pub fn face_cursor_each(
    mouse_pos: Vec2,
    collision_world: &CollisionWorld,
    appearance: &mut crate::graphics::Appearance,
    actual_handle: &PhysHandle,
) {
    let iso2 = collision_world
        .collision_object(*actual_handle)
        .unwrap()
        .position();

    let difference = Unit::new_normalize(mouse_pos - (iso2.translation.vector));

    appearance.flip_x = difference.x < 0.0;
}

pub fn face_cursor(world: &mut World, mouse: &Mouse) {
    for (_, (appearance, phys_handle, _)) in world
        .ecs
        .query::<(&mut graphics::Appearance, &PhysHandle, &FacesCursor)>()
        .iter()
    {
        face_cursor_each(
            mouse.pos().into_vector(),
            &world.phys,
            appearance,
            phys_handle,
        );
    }
}
