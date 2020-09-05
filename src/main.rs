use macroquad::*;

mod combat;
mod draw;
mod phys;
mod world;
pub use world::World;

#[macroquad::main("hackanoir")]
async fn main() {
    let mut w = World::new().await;

    loop {
        w.update();
        w.draw();
        next_frame().await;
    }
}
