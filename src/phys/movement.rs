use crate::{draw, World};
use macroquad::{is_key_down, KeyCode};

#[derive(Debug, Clone, Copy)]
pub struct WalkAnimator {
    last_direction: na::Vector2<f32>,
}
impl Default for WalkAnimator {
    fn default() -> Self {
        Self {
            last_direction: na::zero(),
        }
    }
}
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct WalkDirectionArtConfig {
    pub side: draw::ArtHandle,
    pub down: draw::ArtHandle,
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
        .query_one::<(&mut draw::AnimationFrame, &mut draw::Looks)>(player.entity)
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
        player.walk_animator.last_direction = vel;

        looks.art = if vel.x.abs() < std::f32::EPSILON {
            config.player.direction_art.down
        } else {
            config.player.direction_art.side
        };
        looks.flip_x = vel.x < 0.0;

        Some(vel)
    } else {
        let ss = config.draw.get(looks.art).spritesheet?;
        if af.current_frame(ss) == ss.hold_at {
            af.0 -= 1;
            None
        } else {
            player.walk_animator.last_direction *= 0.92;
            Some(player.walk_animator.last_direction)
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
