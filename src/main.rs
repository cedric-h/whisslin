use macroquad::*;

mod combat;
mod draw;
mod phys;
mod world;
pub use world::World;

#[macroquad::main("SlingChuck")]
async fn main() {
    let mut w = World::new();

    loop {
        w.update();
        draw::draw(&mut w);
        next_frame().await;
    }
}
