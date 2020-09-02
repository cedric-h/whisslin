use crate::{
    combat::{self, aiming},
    draw,
    phys::{self, collide, movement, CollisionGroups, Cuboid, PhysHandle},
};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub keyframes: Vec<aiming::KeyFrame>,
    pub direction_art: movement::WalkDirectionArtConfig,
    pub weapon: aiming::Weapon,
    pub speed: f32,
}
impl Config {
    /*
    pub fn dev_ui(&mut self) {
        use macroquad::*;

        set_default_camera();
        draw_window(
            hash!(),
            vec2(400.0, 200.0),
            vec2(320.0, 400.0),
            WindowParams {
                label: "Shop".to_string(),
                close_button: false,
                ..Default::default()
            },
            |ui| {
                let mut speed_str = self.speed.to_string();
                ui.input_field(hash!(), "player speed", &mut speed_str);
                if let Ok(speed) = speed_str.parse::<f32>() {
                    self.speed = speed;
                }
            }
        );
    }*/
}

pub struct Player {
    pub entity: hecs::Entity,
    pub phys_handle: PhysHandle,
    pub weapon_entity: Option<hecs::Entity>,
    pub wielder: aiming::Wielder,
    pub walk_animator: movement::WalkAnimator,
}
impl Player {
    pub fn new(
        ecs: &mut hecs::World,
        phys: &mut phys::CollisionWorld,
        config: &super::Config,
    ) -> Self {
        let wep_ent = ecs.spawn((
            draw::Looks::art(config.draw.art("baseball.png")),
            combat::Hurtful {
                minimum_speed: 0.05,
                raw_damage: 1.0,
                minimum_damage: 1,
                kind: combat::HurtfulKind::Ram {
                    speed_damage_coefficient: 1.0,
                },
            },
            phys::KnockBack {
                groups: CollisionGroups::new()
                    .with_membership(&[collide::WEAPON])
                    .with_whitelist(&[collide::ENEMY]),
                force_decay: 0.75,
                force_magnitude: 0.75,
                use_force_direction: true,
                // TODO: separate minimum_speed_to_knock_back
                minimum_speed: Some(0.05),
            },
        ));

        let ent = ecs.spawn((
            draw::Looks::art(config.player.direction_art.down),
            draw::AnimationFrame(3),
        ));
        Player {
            entity: ent,
            walk_animator: movement::WalkAnimator::default(),
            phys_handle: phys::phys_insert(
                ecs,
                phys,
                ent,
                na::Isometry::identity(),
                Cuboid::new(na::Vector2::new(0.7, 0.3) / 2.0),
                CollisionGroups::new().with_membership(&[phys::collide::PLAYER]),
            ),
            weapon_entity: Some(wep_ent),
            wielder: aiming::Wielder::new(),
        }
    }
}
