use crate::{phys::PhysHandle, world, World};
use macroquad::{drawing::Texture2D, *};
use std::{fmt, num::NonZeroUsize};

const ONE: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1) };

mod cam;
pub use cam::CedCam2D;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub zoom: f32,
    pub tile_size: f32,
    pub art: Vec<ArtConfig>,
}
impl Config {
    pub fn art(&self, file: &str) -> ArtHandle {
        ArtHandle(
            self.art
                .iter()
                .position(|a| a.file == file)
                .unwrap_or_else(|| panic!("no art by name of {}", file)),
        )
    }

    #[cfg(feature = "confui")]
    pub fn dev_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("zoom");
        ui.add(egui::DragValue::f32(&mut self.zoom).speed(0.005));
        ui.label("tile size");
        ui.add(egui::DragValue::f32(&mut self.tile_size).speed(0.005));

        ui.collapsing("Art", |ui| {
            for art in self.art.iter_mut() {
                ui.collapsing(&art.file.clone(), |ui| art.dev_ui(ui));
            }
        });
    }

    pub fn get(&self, art: ArtHandle) -> &ArtConfig {
        self.art
            .get(art.0)
            .unwrap_or_else(|| panic!("invalid art handle: {}", art))
    }

    pub fn camera(&self, iso: na::Isometry2<f32>) -> CedCam2D {
        CedCam2D {
            zoom: self.zoom,
            iso,
            flip_x: false,
        }
    }

    pub fn camera_x_flipped(&self, iso: na::Isometry2<f32>, flip_x: bool) -> CedCam2D {
        CedCam2D {
            flip_x,
            ..self.camera(iso)
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct ArtHandle(usize);

impl fmt::Display for ArtHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct Images {
    images: Vec<Texture2D>,
}
impl Images {
    pub async fn load(config: &world::Config) -> Self {
        let mut images = Vec::with_capacity(config.draw.art.len());

        clear_background(WHITE);
        draw_text("LOADING", 0.0, 0.0, 20.0, BLACK);
        next_frame().await;
        for (i, name) in config.draw.art.iter().enumerate() {
            clear_background(WHITE);

            draw_text(
                &format!(
                    "LOADING {}/{} ({:.3}%)",
                    i,
                    config.draw.art.len(),
                    (i as f32 / config.draw.art.len() as f32) * 100.0
                ),
                0.0,
                0.0,
                20.0,
                BLACK,
            );
            draw_text(&name.file, 20.0, 20.0, 20.0, DARKGRAY);
            images.push(load_texture(&name.file).await);
            next_frame().await;
        }

        Self { images }
    }

    pub fn get(&mut self, ah: ArtHandle) -> &Texture2D {
        unsafe { self.images.get_unchecked(ah.0) }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AnimationFrame(pub usize);
impl AnimationFrame {
    pub fn current_frame(self, ss: Spritesheet) -> usize {
        self.0 / ss.frame_rate as usize % ss.total.get()
    }

    pub fn at_holding_frame(self, ss: Spritesheet) -> bool {
        self.current_frame(ss) == ss.hold_at
    }
}

pub fn animate(World { ecs, .. }: &mut World) {
    for (_, AnimationFrame(af)) in ecs.query::<&mut AnimationFrame>().iter() {
        *af += 1;
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtConfig {
    pub file: String,
    pub scale: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spritesheet: Option<Spritesheet>,
}
impl ArtConfig {
    #[cfg(feature = "confui")]
    fn dev_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("file name");
        ui.add(egui::TextEdit::new(&mut self.file));

        ui.label("scale");
        ui.add(egui::DragValue::f32(&mut self.scale).speed(0.0001));

        let mut has_spritesheet = self.spritesheet.is_some();
        ui.checkbox("spritesheet", &mut has_spritesheet);
        ui.collapsing("spritesheet", |ui| {
            match (has_spritesheet, &mut self.spritesheet) {
                (false, ss @ Some(_)) => *ss = None,
                (true, None) => self.spritesheet = Some(Default::default()),
                (true, Some(ss)) => ss.dev_ui(ui),
                (false, None) => {}
            }
        });
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spritesheet {
    pub rows: NonZeroUsize,
    pub columns: NonZeroUsize,
    pub total: NonZeroUsize,
    pub frame_rate: usize,
    pub hold_at: usize,
}
impl Default for Spritesheet {
    fn default() -> Self {
        Self {
            rows: ONE,
            columns: ONE,
            total: ONE,
            frame_rate: 1,
            hold_at: 0,
        }
    }
}
impl Spritesheet {
    #[cfg(feature = "confui")]
    fn dev_ui(&mut self, ui: &mut egui::Ui) {
        let mut non_zero_drag = |label: &'static str, nz: &mut NonZeroUsize| {
            ui.label(label);

            let mut f = nz.get() as f32;
            ui.add(egui::DragValue::f32(&mut f));
            *nz = NonZeroUsize::new(f.round() as usize).unwrap_or(ONE)
        };
        non_zero_drag("rows", &mut self.rows);
        non_zero_drag("columns", &mut self.columns);
        non_zero_drag("total", &mut self.total);

        let mut usize_drag = |label: &'static str, u: &mut usize| {
            ui.label(label);

            let mut f = *u as f32;
            ui.add(egui::DragValue::f32(&mut f));
            *u = f as usize
        };
        usize_drag("frame rate", &mut self.frame_rate);
        usize_drag("hold at", &mut self.hold_at);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Looks {
    pub art: ArtHandle,
    pub z_offset: f32,
    pub bottom_offset: f32,
    pub scale: f32,
    pub flip_x: bool,
}
impl Looks {
    pub fn art(art: ArtHandle) -> Self {
        Looks {
            art,
            scale: 1.0,
            z_offset: 0.0,
            bottom_offset: 0.0,
            flip_x: false,
        }
    }
}

#[derive(Default)]
pub struct DrawState {
    sprites: Vec<(Looks, na::Isometry2<f32>, Option<AnimationFrame>)>,
}

pub fn draw(
    World {
        phys,
        ecs,
        config,
        player,
        map,
        images,
        draw_state,
        ..
    }: &mut World,
) {
    clear_background(Color([23, 138, 75, 255]));

    let player_iso = phys
        .collision_object(player.phys_handle)
        .unwrap()
        .position();

    let camera = config.draw.camera(player_iso.inverse());
    set_camera(camera);
    for tile in map.tiles.iter() {
        let image = images.get(tile.image);
        draw_texture_ex(
            *image,
            tile.translation.x,
            tile.translation.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(1.02, 1.085) * 2.0 * config.draw.tile_size),
                ..Default::default()
            },
        )
    }

    draw_state.sprites.extend(
        ecs.query::<(&Looks, &PhysHandle, Option<&AnimationFrame>)>()
            .iter()
            .filter_map(|(_, (&l, &h, af))| {
                Some((l, *phys.collision_object(h)?.position(), af.copied()))
            }),
    );

    draw_state.sprites.sort_unstable_by(|a, b| {
        fn f((looks, iso_a, _): &(Looks, na::Isometry2<f32>, Option<AnimationFrame>)) -> f32 {
            iso_a.translation.vector.y + looks.z_offset
        }

        f(a).partial_cmp(&f(b))
            .unwrap_or(std::cmp::Ordering::Greater)
    });

    for (looks, iso, anim_frame) in draw_state.sprites.drain(..) {
        let camera = config
            .draw
            .camera_x_flipped(player_iso.inverse() * iso, looks.flip_x);
        set_camera(camera);
        let art = config.draw.get(looks.art);
        let image = images.get(looks.art);
        let size = {
            let size = vec2(image.width(), image.height());
            match anim_frame.and(art.spritesheet) {
                Some(ss) => size / vec2(ss.columns.get() as f32, ss.rows.get() as f32),
                _ => size,
            }
        };
        let world_size = size * looks.scale * art.scale;
        draw_texture_ex(
            *image,
            world_size.x() / -2.0,
            world_size.y() / -2.0 - looks.bottom_offset,
            WHITE,
            DrawTextureParams {
                dest_size: Some(world_size),
                source: art.spritesheet.and_then(|ss| {
                    let af = anim_frame?.current_frame(ss);
                    let row = af / ss.columns.get();
                    let column = af % ss.columns.get();
                    Some(Rect {
                        x: column as f32 * size.x(),
                        y: row as f32 * size.y(),
                        w: size.x(),
                        h: size.y(),
                    })
                }),
                ..Default::default()
            },
        )
    }
}
