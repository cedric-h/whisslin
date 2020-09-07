use crate::draw;

#[derive(Debug, Clone)]
pub struct Tile {
    pub translation: na::Vector2<f32>,
    pub spritesheet_index: usize,
}

pub struct Map {
    pub tiles: Vec<Tile>,
}

impl Map {
    pub fn new(config: &super::Config) -> Self {
        let index_to_translation = |x: i32, y: i32| -> na::Vector2<f32> {
            let w: f32 = 3.0_f32.sqrt();
            let h: f32 = 2.0;

            na::Vector2::new(
                (x * 2 + (y & 1)) as f32 / 2.0 * w,
                ((3.0 / 4.0) * y as f32) * h,
            ) * config.draw.tile_size
        };

        Self {
            tiles: (-3..=3)
                .flat_map(|x| (-3..=3).map(move |y| Tile {
					spritesheet_index: macroquad::rand::gen_range(0, 3),
					translation: index_to_translation(x, y),
				}))
                .collect(),
        }
    }
}
