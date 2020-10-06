use crate::draw;
use glam::Vec2;

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub size: f32,
    pub border_thickness: f32,
    pub border_color: [u8; 4],
    pub art_handle: draw::ArtHandle,
    #[cfg(feature = "confui")]
    #[serde(skip, default)]
    pub dirty: bool,
}
impl Config {
    #[cfg(feature = "confui")]
    pub fn dev_ui(&mut self, ui: &mut egui::Ui) {
        let start = self.clone();

        ui.label("tile size");
        ui.add(egui::DragValue::f32(&mut self.size).speed(0.001));

        ui.label("border thickness");
        ui.add(egui::DragValue::f32(&mut self.border_thickness).speed(0.0001));

        self.dirty = *self != start;
    }
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub translation: Vec2,
    pub spritesheet_index: usize,
}

pub struct Map {
    pub tiles: Vec<Tile>,
}

impl Map {
    pub fn new(super::Config { draw, tile, .. }: &super::Config) -> Self {
        let index_to_translation = |x: i32, y: i32| -> Vec2 {
            let w: f32 = 3.0_f32.sqrt();
            let h: f32 = 2.0;

            Vec2::new(
                (x * 2 + (y & 1)) as f32 / 2.0 * w,
                ((3.0 / 4.0) * y as f32) * h,
            ) * (tile.size + tile.border_thickness)
        };
        let tile_count = draw.get(tile.art_handle).spritesheet.unwrap().total.get();

        Self {
            tiles: (-3..=3)
                .flat_map(|x| {
                    (-3..=3).map(move |y| Tile {
                        spritesheet_index: macroquad::rand::gen_range(0, tile_count),
                        translation: index_to_translation(x, y),
                    })
                })
                .collect(),
        }
    }
}
