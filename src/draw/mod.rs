use crate::{phys::PhysHandle, World};
use glam::{vec3, Mat4};
use macroquad::*;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub size: na::Vector2<f32>,
    pub color: macroquad::Color,
}
impl Default for Rect {
    fn default() -> Self {
        Rect {
            size: na::Vector2::new(1.0, 1.0),
            color: BLACK,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Looks {
    pub rect: Rect,
    pub bottom_offset: f32,
    pub flip_x: bool,
}
impl Looks {
    pub fn size(size: na::Vector2<f32>) -> Self {
        Looks {
            rect: Rect {
                size,
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

pub fn draw(
    World {
        phys,
        ecs,
        camera,
        player,
        map,
        ..
    }: &mut crate::World,
) {
    clear_background(BLACK);

    let player_iso = phys
        .collision_object(player.phys_handle)
        .unwrap()
        .position();

    camera.iso = player_iso.inverse();
    set_camera(*camera);
    for tile in map.tiles.iter() {
        draw_hexagon(
            tile.translation.x,
            tile.translation.y,
            crate::world::map::TILE_SIZE,
            0.075,
            true,
            BLACK,
            WHITE,
        )
    }

    for (looks, iso) in ecs
        .query::<(&Looks, &PhysHandle)>()
        .iter()
        .filter_map(|(_, (l, &h))| Some((l, phys.collision_object(h)?.position())))
    {
        camera.iso = player_iso.inverse() * *iso;
        set_camera(*camera);
        draw_rectangle(
            looks.rect.size.x / if looks.flip_x { 2.0 } else { -2.0 },
            looks.rect.size.y / -2.0 - looks.bottom_offset,
            looks.rect.size.x,
            looks.rect.size.y,
            looks.rect.color,
        )
    }
}

#[derive(Clone, Copy)]
pub struct CedCam2D {
    pub zoom: f32,
    pub iso: na::Isometry2<f32>,
}

impl Default for CedCam2D {
    fn default() -> CedCam2D {
        CedCam2D {
            zoom: 10.0,
            iso: na::Isometry2::identity(),
        }
    }
}

impl CedCam2D {
    pub fn with_zoom(zoom: f32) -> Self {
        CedCam2D {
            zoom,
            ..Default::default()
        }
    }

    /// Returns the screen space position for a 2D camera world space position
    pub fn world_to_screen(&self, point: na::Vector2<f32>) -> na::Vector2<f32> {
        let mat = self.matrix();
        let transform = mat.transform_point3(vec3(point.x, point.y, 0.0));

        na::Vector2::new(transform.x(), transform.y())
    }

    // Returns the world space position for a 2D camera screen space position
    pub fn screen_to_world(&self, point: na::Vector2<f32>) -> na::Vector2<f32> {
        let inv_mat = self.matrix().inverse();
        let transform = inv_mat.transform_point3(vec3(point.x, point.y, 0.0));

        na::Vector2::new(transform.x(), transform.y())
    }
}

impl Camera for CedCam2D {
    fn matrix(&self) -> glam::Mat4 {
        let Self { zoom, iso } = self;

        let (w, h) = (screen_width(), screen_height());
        Mat4::from_scale(vec3(1.0, -(w / h), 0.0) / *zoom)
            * Mat4::from_translation(vec3(
                iso.translation.vector.x,
                iso.translation.vector.y,
                0.0,
            ))
            * Mat4::from_axis_angle(vec3(0.0, 0.0, 1.0), iso.rotation.angle())
    }

    fn depth_enabled(&self) -> bool {
        false
    }

    fn render_pass(&self) -> Option<miniquad::RenderPass> {
        None
    }
}
