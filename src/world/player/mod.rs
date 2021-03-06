mod aiming;
pub use aiming::aiming;
mod movement;
pub use movement::movement;

use crate::{
    combat, draw,
    phys::{self, Collide, CollisionGroups, Cuboid, PhysHandle},
};

#[derive(Copy, Clone, Debug)]
pub enum PlayerState {
    Walking,
    Throwing,
    Standing,
}
impl PlayerState {
    pub fn is_walking(self) -> bool {
        matches!(self, PlayerState::Walking)
    }

    pub fn is_throwing(self) -> bool {
        matches!(self, PlayerState::Throwing)
    }

    pub fn is_standing(self) -> bool {
        matches!(self, PlayerState::Standing)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EachDirection<T> {
    // boy you turn me
    // inside
    // out
    // and 'round 'round
    up: T,
    side: T,
    down: T,
}
impl<T> EachDirection<T> {
    fn get(&self, d: Direction) -> &T {
        match d {
            Direction::Side => &self.side,
            Direction::Down => &self.down,
            Direction::Up => &self.up,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum Direction {
    Side,
    Up,
    Down,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectionConfig {
    art: draw::ArtHandle,
    weapon_in_front: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    directions: EachDirection<DirectionConfig>,
    weapon: aiming::WeaponConfig,
    speed: f32,
    stop_decay: f32,
}
impl Config {
    #[cfg(feature = "confui")]
    pub fn dev_ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Speed", |ui| {
            ui.label("speed");
            ui.add(egui::DragValue::f32(&mut self.speed).speed(0.005));
            ui.label("stop walk slowdown decay");
            ui.add(egui::DragValue::f32(&mut self.stop_decay).speed(0.005));
        });
        ui.collapsing("Weapon", |ui| self.weapon.dev_ui(ui));
    }
}

pub struct Player {
    pub state: PlayerState,
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
                    .with_membership(&[Collide::Weapon as usize])
                    .with_whitelist(&[Collide::Enemy as usize]),
                force_decay: 0.75,
                force_magnitude: 0.75,
                use_force_direction: true,
                // TODO: separate minimum_speed_to_knock_back
                minimum_speed: Some(0.05),
            },
        ));

        let ent = ecs.spawn((
            draw::Looks::art(config.player.directions.down.art),
            draw::AnimationFrame(3),
        ));
        Player {
            entity: ent,
            state: PlayerState::Walking,
            walk_animator: movement::WalkAnimator::default(),
            phys_handle: phys::phys_insert(
                ecs,
                phys,
                ent,
                na::Isometry::identity(),
                Cuboid::new(na::Vector2::new(0.6, 0.15) / 2.0),
                CollisionGroups::new().with_membership(&[phys::Collide::Player as usize]),
            ),
            weapon_entity: Some(wep_ent),
            wielder: aiming::Wielder::new(),
        }
    }
}
