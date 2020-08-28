pub struct Tile {
    pub translation: na::Vector2<f32>,
}

pub struct Map {
    pub tiles: Vec<Tile>,
}

pub const TILE_SIZE: f32 = 1.5;

fn index_to_offset(x: usize, y: usize) -> na::Vector2<f32> {
    let w: f32 = 3.0_f32.sqrt();
    let h: f32 = 2.0;

    na::Vector2::new(
        (x * 2 + (y & 1)) as f32 / 2.0 * w,
        ((3.0 / 4.0) * y as f32) * h,
    ) * TILE_SIZE
}

impl Map {
    pub fn new() -> Self {
        Self {
            tiles: (0..10)
                .flat_map(move |x| {
                    (0..5).map(move |y| Tile {
                        translation: index_to_offset(x, y),
                    })
                })
                .collect(),
        }
    }
}
