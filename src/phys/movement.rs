use crate::graphics::Appearance;
use crate::PhysHandle;
use crate::World;
use quicksilver::{geom::Vector, input::Key, lifecycle::Window};

pub struct PlayerControlled {
    pub speed: f32,
}

pub fn movement(world: &mut World, window: &mut Window) {
    let ecs = &world.ecs;
    let phys = &mut world.phys;

    #[rustfmt::skip]
    const KEYMAP: &'static [(Key, Vector)] = &[
        (Key::W, Vector { x:  0.0, y: -1.0 }),
        (Key::S, Vector { x:  0.0, y:  1.0 }),
        (Key::A, Vector { x: -1.0, y:  0.0 }),
        (Key::D, Vector { x:  1.0, y:  0.0 }),
    ];

    let move_vec = KEYMAP
        .iter()
        .fold(Vector::ZERO, |acc, (key, vec)| {
            if window.keyboard()[*key].is_down() {
                acc + *vec
            } else {
                acc
            }
        })
        .normalize();

    if move_vec.len2() > 0.0 {
        for (_, (h, &PlayerControlled { speed }, _appearance)) in ecs
            .query::<(&PhysHandle, &PlayerControlled, &mut Appearance)>()
            .iter()
        {
            (|| {
                let vel = move_vec.into_vector() * speed;

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
    }
}
