use super::Direction;
use crate::{
    draw::{AnimationFrame, Looks},
    World,
};
use macroquad::{is_key_down, KeyCode};

#[derive(Debug, Clone, Copy)]
pub struct WalkAnimator {
    direction: Direction,
    last_move: na::Vector2<f32>,
}
impl Default for WalkAnimator {
    fn default() -> Self {
        Self {
            direction: Direction::Down,
            last_move: na::zero(),
        }
    }
}

pub fn movement(
    World {
        phys,
        ecs,
        player,
        config,
        ..
    }: &mut World,
) -> Option<()> {
    let mut query = ecs
        .query_one::<(&mut AnimationFrame, &mut Looks)>(player.entity)
        .ok()?;
    let (af, looks) = query.get()?;

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

    let vel = if move_vec.magnitude_squared() > 0.0 {
        let vel = move_vec * config.player.speed;
        player.walk_animator.last_move = vel;

        let new_direction = match (vel.x.abs() > std::f32::EPSILON, vel.y < 0.0) {
            (true, _) => Direction::Side,
            (_, true) => Direction::Up,
            _ => Direction::Down,
        };

        if new_direction != player.walk_animator.direction {
            player.walk_animator.direction = new_direction;
            let direction_config = config.player.directions.get(new_direction);
            looks.art = direction_config.art;
            if let Some(mut wep_looks) = player
                .weapon_entity
                .and_then(|e| ecs.get_mut::<Looks>(e).ok())
            {
                wep_looks.z_offset = (direction_config.weapon_in_front as u8 * 10) as f32;
            }
        }
        looks.flip_x = vel.x < 0.0;

        Some(vel)
    } else {
        let ss = config.draw.get(looks.art).spritesheet?;
        if af.current_frame(ss) == ss.hold_at {
            af.0 -= 1;
            None
        } else {
            player.walk_animator.last_move *= config.player.stop_decay;
            Some(player.walk_animator.last_move)
        }
    }?;

    let obj = phys.get_mut(player.phys_handle).expect("player no phys");
    let mut iso = obj.position().clone();
    iso.translation.vector += vel;
    obj.set_position_with_prediction(iso.clone(), {
        iso.translation.vector += vel;
        iso
    });

    Some(())
}
