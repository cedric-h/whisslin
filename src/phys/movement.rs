use macroquad::{is_key_down, KeyCode};

pub fn movement(world: &mut crate::World) {
    let phys = &mut world.phys;

    #[rustfmt::skip]
    let keymap = [
        (KeyCode::W, -na::Vector2::y()),
        (KeyCode::S,  na::Vector2::y()),
        (KeyCode::A, -na::Vector2::x()),
        (KeyCode::D,  na::Vector2::x()),
    ];

    let move_vec = keymap
        .iter()
        .filter(|(key, _)| is_key_down(*key))
        .fold(na::Vector2::zeros(), |acc, (_, vec)| acc + *vec)
        .normalize();

    if move_vec.magnitude_squared() > 0.0 {
        let vel = move_vec * 0.1;

        let obj = phys
            .get_mut(world.player.phys_handle)
            .expect("player no phys");
        let mut iso = obj.position().clone();
        iso.translation.vector += vel;
        obj.set_position_with_prediction(iso.clone(), {
            iso.translation.vector += vel;
            iso
        });
    }
}
