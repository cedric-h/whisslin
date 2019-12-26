use crate::graphics::Appearance;
use crate::Iso2;
use hecs::World;
use quicksilver::{geom::Vector, input::Key, lifecycle::Window};

pub struct PlayerControlled {
    pub speed: f32
}

pub fn movement(world: &mut World, window: &mut Window) {
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
        for (_, (iso, &PlayerControlled { speed }, appearance)) in world
            .query::<(&mut Iso2, &PlayerControlled, &mut Appearance)>()
            .iter()
        {
            iso.translation.vector += move_vec.into_vector() * speed;
            appearance.flip_x = move_vec.x > 0.0;
        }
    }
}
