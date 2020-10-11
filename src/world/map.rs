use crate::draw;
use glam::Vec2;

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub size: f32,
    pub border_thickness: f32,
    pub border_color: [u8; 4],
    tiles: fxhash::FxHashMap<(i32, i32), ()>,
    pub art_handle: draw::ArtHandle,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    pub dirty: bool,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    mouse_hot: bool,
    // #[cfg(feature = "confui")]
    // #[serde(skip)]
    // selected_tiles: fxhash::FxHashSet<(i32, i32)>,
}

#[cfg(feature = "confui")]
pub fn dev_ui(
    super::Game {
        config: super::Config { tile, draw, .. },
        player,
        phys,
        ..
    }: &mut super::Game,
    ui: &mut egui::Ui,
) -> Option<()> {
    use macroquad::*;
    let start = tile.clone();

    ui.label("tile size");
    ui.add(egui::DragValue::f32(&mut tile.size).speed(0.001));

    ui.label("border thickness");
    ui.add(egui::DragValue::f32(&mut tile.border_thickness).speed(0.0001));

    tile.dirty = *tile != start;

    // draw debug hexagon
    let index = {
        let player_pos = {
            let p = phys
                .collision_object(player.phys_handle)?
                .position()
                .translation
                .vector;
            Vec2::new(p.x, p.y)
        };
        let size = tile.size + tile.border_thickness;
        let offset = Vec2::new(0.1307 * size, size / -2.0);
        let index = translation_to_index(size, player_pos + draw.mouse_world() - offset);
        let p = index_to_translation(size, index) + offset;

        set_camera(draw.camera({
            let mut i = phys
                .collision_object(player.phys_handle)?
                .position()
                .inverse();
            i.translation.vector.y += draw.camera_move;
            i
        }));

        draw_hexagon(p.x(), p.y(), size, 0.01, true, RED, Color([0, 0, 0, 0]));

        let (x, y) = index;
        (x - 1, y)
    };

    if !ui.ctx().wants_mouse_input() && is_mouse_button_down(MouseButton::Left) {
        if !tile.mouse_hot {
            if tile.tiles.contains_key(&index) {
                tile.tiles.remove(&index);
            } else {
                tile.tiles.insert(index, ());
            }
            tile.dirty = true;
        }
        tile.mouse_hot = true;
    } else {
        tile.mouse_hot = false
    }

    Some(())
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub translation: Vec2,
    pub spritesheet_index: usize,
}

pub struct Map {
    pub tiles: Vec<Tile>,
}

/// square root of three
fn translation_to_index(tile_size: f32, t: Vec2) -> (i32, i32) {
    fn cube_round(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
        let (mut rx, mut ry, mut rz) = (x.round(), y.round(), z.round());

        let (x_diff, y_diff, z_diff) = ((rx - x).abs(), (ry - y).abs(), (rz - z).abs());

        if x_diff > y_diff && x_diff > z_diff {
            rx = -ry - rz;
        } else if y_diff > z_diff {
            ry = -rx - rz;
        } else {
            rz = -rx - ry;
        }

        (rx, ry, rz)
    }

    let q = (3.0_f32.sqrt() / 3.0 * t.x() - 1.0 / 3.0 * t.y()) / tile_size;
    let r = (2.0 / 3.0 * t.y()) / tile_size;

    let (y, x, _) = cube_round(q, r, -q - r);
    (x as i32, y as i32)
}

fn index_to_translation(tile_size: f32, (ri, qi): (i32, i32)) -> Vec2 {
    let (r, q) = (ri as f32, qi as f32);
    Vec2::new(3.0_f32.sqrt() * q + 3.0_f32.sqrt() / 2.0 * r, 3.0 / 2.0 * r) * tile_size
}

#[test]
fn tile_index_to_translation_and_back() {
    println!("starting ..");
    for x in -3..=3 {
        for y in -3..=3 {
            assert_eq!(
                (x, y),
                translation_to_index(1.0, index_to_translation(1.0, (x, y)))
            );
            println!("{} {} all good", x, y);
        }
    }
}

impl Map {
    pub fn new(super::Config { draw, tile, .. }: &super::Config) -> Self {
        let tile_count = draw.get(tile.art_handle).spritesheet.unwrap().total.get();

        Self {
            tiles: tile
                .tiles
                .iter()
                .map(|(&(x, y), &())| Tile {
                    spritesheet_index: macroquad::rand::gen_range(0, tile_count),
                    translation: index_to_translation(tile.size + tile.border_thickness, (x, y)),
                })
                .collect(),
        }
    }
}
