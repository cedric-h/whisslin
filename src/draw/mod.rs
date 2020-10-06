use crate::{
    phys::{self, PhysHandle},
    world, Game,
};
use macroquad::{drawing::Texture2D, *};
use ncollide2d::shape::Cuboid;
use std::{fmt, num::NonZeroUsize};

const ONE: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1) };
mod cam;
pub use cam::CedCam2D;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub zoom: f32,
    pub camera_move: f32,
    pub art: Vec<ArtConfig>,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    art_search: String,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    popup: Popup,
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
        match &mut self.popup {
            Popup::Clear => {
                ui.label("zoom");
                ui.add(egui::DragValue::f32(&mut self.zoom).speed(0.001));

                ui.label("camera move");
                ui.add(egui::DragValue::f32(&mut self.camera_move).speed(0.01));

                ui.collapsing("Art", |ui| {
                    let mut removal_index: Option<usize> = None;
                    for (i, art) in self.art.iter_mut().enumerate() {
                        use ArtConfigDevUiRequest::*;
                        ui.collapsing(&art.file.clone(), |ui| match art.dev_ui(ui) {
                            Remove => removal_index = Some(i),
                            NoRequest => {}
                        });
                    }
                    if let Some(i) = removal_index {
                        self.art.remove(i);
                    }

                    if ui.button("Add Art").clicked {
                        self.popup = Popup::AddArt {
                            file: "vase.png".to_string(),
                        };
                    }
                });
            }
            Popup::AddArt { file } => {
                ui.label("Image File for new Art");
                ui.add(egui::TextEdit::new(file));

                if std::path::Path::new("art/").join(&mut *file).exists() {
                    if ui.button("Add Art").clicked {
                        self.art.push(ArtConfig {
                            file: std::mem::take(file),
                            scale: self.art.first().map(|a| a.scale).unwrap_or(1.0),
                            spritesheet: None,
                            align: Default::default(),
                        });
                    }
                } else {
                    ui.add(
                        egui::Label::new(format!("./art/{} does not exist", file))
                            .text_color(egui::color::RED),
                    );
                }

                if ui.button("Back").clicked {
                    self.popup = Popup::Clear;
                }
            }
        }
    }

    #[cfg(feature = "confui")]
    /// Returns `true` if "dirty" i.e. meaningful outward-facing changes to the data occured.
    pub fn select_handle_dev_ui(&mut self, ui: &mut egui::Ui, current: &mut ArtHandle) -> bool {
        ui.label("Art Search");
        ui.add(egui::TextEdit::new(&mut self.art_search));

        self.art
            .iter()
            .enumerate()
            .map(|(i, art)| (ArtHandle(i), &art.file))
            .filter(|(_, f)| f.starts_with(&self.art_search))
            .any(|(ah, art)| ui.radio_value(art, current, ah).clicked)
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

/// A state machine modelling who has control of the Config window
#[cfg(feature = "confui")]
enum Popup {
    AddArt {
        file: String,
    },
    /// No popups!
    Clear,
}
#[cfg(feature = "confui")]
impl Default for Popup {
    fn default() -> Self {
        Popup::Clear
    }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ArtHandle(usize);

impl ArtHandle {
    pub const unsafe fn new_unchecked(u: usize) -> Self {
        ArtHandle(u)
    }
}

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
            images.push(load_texture(&format!("art/{}", name.file)).await);
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
        self.0 / ss.frame_rate.get() % ss.total.get()
    }

    pub fn at_holding_frame(self, ss: Spritesheet) -> bool {
        self.current_frame(ss) == ss.hold_at
    }
}

pub fn animate(Game { ecs, .. }: &mut Game) {
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
    #[serde(default, skip_serializing_if = "Align::is_bottom")]
    pub align: Align,
}
impl ArtConfig {
    #[cfg(feature = "confui")]
    fn dev_ui(&mut self, ui: &mut egui::Ui) -> ArtConfigDevUiRequest {
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

        if ui.button("Remove").clicked {
            return ArtConfigDevUiRequest::Remove;
        }

        ArtConfigDevUiRequest::NoRequest
    }
}
#[cfg(feature = "confui")]
/// ArtConfig's dev_ui method uses this to request that things outside of
/// the purview of a single ArtConfig are manipulated.
enum ArtConfigDevUiRequest {
    /// Remove this ArtConfig from the Config.
    Remove,
    /// No change necessary.
    NoRequest,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Align {
    Center,
    Bottom,
}
impl Default for Align {
    fn default() -> Self {
        Align::Bottom
    }
}
impl Align {
    pub fn is_bottom(&self) -> bool {
        matches!(self, Align::Bottom)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spritesheet {
    pub rows: NonZeroUsize,
    pub columns: NonZeroUsize,
    pub total: NonZeroUsize,
    pub frame_rate: NonZeroUsize,
    pub hold_at: usize,
}
impl Default for Spritesheet {
    fn default() -> Self {
        Self {
            rows: ONE,
            columns: ONE,
            total: ONE,
            frame_rate: ONE,
            hold_at: 0,
        }
    }
}
impl Spritesheet {
    /// Coords are in terms of tiles, not pixels.
    /// Multiply by tile texture size for pixel coords.
    fn coords(self, af: usize) -> glam::Vec2 {
        let row = af / self.columns.get();
        let column = af % self.columns.get();
        vec2(column as f32, row as f32)
    }

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
        non_zero_drag("frame rate", &mut self.frame_rate);

        let mut usize_drag = |label: &'static str, u: &mut usize| {
            ui.label(label);

            let mut f = *u as f32;
            ui.add(egui::DragValue::f32(&mut f));
            *u = f as usize
        };
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

#[derive(Debug, Copy, Clone)]
/// A Component that animates an entity's death, should it die.
pub struct DeathAnimation {
    art: ArtHandle,
}
impl DeathAnimation {
    pub fn new(art: ArtHandle) -> Self {
        Self { art }
    }
}

/// A Component that is active on Ghost entities as they animate a death.
/// The bool should start out as false and be set to true if the animation has begun playing.
pub struct AnimatingDeath(bool);

/// Ghost entities are spawned to play death animations.
pub fn insert_ghosts(
    Game {
        phys,
        l8r,
        ecs,
        dead,
        ..
    }: &mut Game,
) {
    for (death_anim, iso, half_extents) in dead.marks().filter_map(
        |e| -> Option<(DeathAnimation, na::Isometry2<f32>, na::Vector2<f32>)> {
            let (&death_anim, &h) = ecs.query_one::<(&_, &_)>(e).ok()?.get()?;
            let obj = phys.collision_object(h)?;
            let Cuboid { half_extents, .. } = obj.shape().as_shape()?;
            Some((death_anim, *obj.position(), *half_extents))
        },
    ) {
        l8r.l8r(move |Game { ecs, phys, .. }| {
            let ghost = ecs.spawn((
                Looks::art(death_anim.art),
                AnimationFrame(0),
                AnimatingDeath(false),
            ));
            phys::phys_insert(
                ecs,
                phys,
                ghost,
                iso,
                Cuboid::new(half_extents),
                phys::CollisionGroups::new().with_whitelist(&[]),
            );
        });
    }
}

/// Once ghosts play their death animations, they have no reason to exist.
pub fn clear_ghosts(
    Game {
        ecs, config, dead, ..
    }: &mut Game,
) {
    for (e, (AnimatingDeath(started), af, looks)) in
        ecs.query::<(&mut _, &AnimationFrame, &Looks)>().iter()
    {
        let ss = config.draw.get(looks.art).spritesheet.unwrap();
        let cf = af.current_frame(ss);

        match cf {
            1 => *started = true,
            0 if *started => dead.mark(e),
            _ => {}
        }
    }
}

#[derive(Default)]
pub struct DrawState {
    sprites: Vec<SpriteData>,
}
type SpriteData = (
    Looks,
    na::Isometry2<f32>,
    na::Vector2<f32>,
    Option<AnimationFrame>,
);

pub fn draw(
    Game {
        phys,
        ecs,
        config,
        player,
        map,
        images,
        draw_state,
        ..
    }: &mut Game,
) {
    clear_background(Color([23, 138, 75, 255]));

    let player_iso_inverse = {
        let mut i = phys
            .collision_object(player.phys_handle)
            .unwrap()
            .position()
            .inverse();
        i.translation.vector.y += config.draw.camera_move;
        i
    };

    let camera = config.draw.camera(player_iso_inverse);
    set_camera(camera);
    let tile_image = images.get(config.tile.art_handle);
    let tile_ss = config.draw.get(config.tile.art_handle).spritesheet.unwrap();
    let tile_image_size = {
        let size = vec2(tile_image.width(), tile_image.height());
        size / vec2(tile_ss.columns.get() as f32, tile_ss.rows.get() as f32)
    };

    // draw the tile images
    for tile in map.tiles.iter() {
        draw_texture_ex(
            *tile_image,
            tile.translation.x(),
            tile.translation.y(),
            WHITE,
            DrawTextureParams {
                dest_size: Some(Vec2::one() * config.tile.size * 2.0),
                source: {
                    let coords = tile_ss.coords(tile.spritesheet_index) * tile_image_size;
                    Some(Rect {
                        x: coords.x(),
                        y: coords.y(),
                        w: tile_image_size.x(),
                        h: tile_image_size.y(),
                    })
                },
                ..Default::default()
            },
        )
    }

    draw_state.sprites.extend(
        ecs.query::<(&Looks, &PhysHandle, Option<&AnimationFrame>)>()
            .iter()
            .filter_map(|(_, (&l, &h, af))| {
                let o = phys.collision_object(h)?;
                let half_extents = o.shape().as_shape::<Cuboid<f32>>().unwrap().half_extents;
                Some((l, *o.position(), half_extents, af.copied()))
            }),
    );

    draw_state.sprites.sort_unstable_by(|a, b| {
        fn f((looks, iso_a, _, _): &SpriteData) -> f32 {
            iso_a.translation.vector.y + looks.z_offset
        }

        f(a).partial_cmp(&f(b))
            .unwrap_or(std::cmp::Ordering::Greater)
    });

    for (looks, iso, half_size, anim_frame) in draw_state.sprites.drain(..) {
        let camera = config
            .draw
            .camera_x_flipped(player_iso_inverse * iso, looks.flip_x);
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
            match art.align {
                Align::Bottom => -world_size.y() + half_size.y - looks.bottom_offset,
                Align::Center => world_size.y() / -2.0 - looks.bottom_offset,
            },
            WHITE,
            DrawTextureParams {
                dest_size: Some(world_size),
                source: art.spritesheet.and_then(|ss| {
                    let coords = ss.coords(anim_frame?.current_frame(ss)) * size;
                    Some(Rect {
                        x: coords.x(),
                        y: coords.y(),
                        w: size.x(),
                        h: size.y(),
                    })
                }),
                ..Default::default()
            },
        )
    }

    #[cfg(feature = "confui")]
    if config.draw_debug {
        for obj in ecs
            .query::<&PhysHandle>()
            .iter()
            .filter_map(|(_, &h)| phys.collision_object(h))
        {
            let half = obj.shape().as_shape::<Cuboid<f32>>().unwrap().half_extents;
            let size = half * 2.0;
            let pos = -half;

            let camera = config.draw.camera(player_iso_inverse * obj.position());
            set_camera(camera);

            draw_rectangle_lines(pos.x, pos.y, size.x, size.y, 0.01, RED);
        }
    }
}
