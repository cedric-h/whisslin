use nalgebra as na;
use quicksilver::geom::Vector;

pub type Vec2 = na::Vector2<f32>;
pub type Iso2 = na::Isometry2<f32>;
pub type CollisionWorld = ncollide2d::world::CollisionWorld<f32, hecs::Entity>;
pub type PhysHandle = ncollide2d::pipeline::CollisionObjectSlabHandle;

pub const DIMENSIONS: Vector = Vector { x: 480.0, y: 270.0 };
pub const TILE_SIZE: f32 = 16.0;
pub const SCALE: f32 = 4.0;

/// Collision Group Constants
pub mod collide {
    pub const PLAYER: usize = 1;
    pub const WEAPON: usize = 2;
    pub const ENEMY: usize = 3;
    pub const PARTICLE: usize = 4;

    /// Fences, Terrain, etc.
    pub const WORLD: usize = 5;
    pub const FARMABLE: usize = 6;

    // yeah
    pub const GUI: usize = 10;
    pub const PLANTING_CURSOR: usize = 11;
}
