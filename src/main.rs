use macroquad::*;

mod combat;
mod draw;
mod phys;
mod world;
pub use world::World;

#[macroquad::main("hackanoir")]
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
