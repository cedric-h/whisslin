#![feature(drain_filter)]
use macroquad::*;

mod combat;
mod draw;
mod phys;
mod world;
pub use world::{Game, World};

#[macroquad::main("hexagolm")]
async fn main() {
    let mut w = World::new().await;

    #[cfg(feature = "confui")]
    loop {
        w.draw();
        w.update();
        next_frame().await;
    }

    #[cfg(not(feature = "confui"))]
    loop {
        w.update();
        w.draw();
        next_frame().await;
    }
}
