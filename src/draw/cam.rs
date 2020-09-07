use glam::{vec3, Mat4};
use macroquad::*;

#[derive(Clone, Copy)]
pub struct CedCam2D {
    pub zoom: f32,
    pub flip_x: bool,
    pub iso: na::Isometry2<f32>,
}

impl Default for CedCam2D {
    fn default() -> CedCam2D {
        CedCam2D {
            zoom: 10.0,
            flip_x: false,
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
        let mat = self.scale_matrix().inverse();
        let transform = mat.transform_point3(vec3(point.x, point.y, 0.0));

        na::Vector2::new(transform.x(), transform.y())
    }

    // Returns the world space position for a 2D camera screen space position
    pub fn screen_to_world(&self, point: na::Vector2<f32>) -> na::Vector2<f32> {
        let inv_mat = self.scale_matrix();
        let transform = inv_mat.transform_point3(vec3(point.x, point.y, 0.0));

        na::Vector2::new(transform.x(), transform.y())
    }

    fn scale_matrix(&self) -> glam::Mat4 {
        let Self { zoom, flip_x, .. } = *self;
        let (w, h) = (screen_width(), screen_height());
        Mat4::from_scale(vec3(if flip_x { -1.0 } else { 1.0 }, -(w / h), 1.0) / zoom)
    }
}

impl Camera for CedCam2D {
    fn matrix(&self) -> glam::Mat4 {
        self.scale_matrix()
            * Mat4::from_translation(vec3(
                self.iso.translation.vector.x,
                self.iso.translation.vector.y,
                0.0,
            ))
            * Mat4::from_axis_angle(vec3(0.0, 0.0, 1.0), self.iso.rotation.angle())
    }

    fn depth_enabled(&self) -> bool {
        false
    }

    fn render_pass(&self) -> Option<miniquad::RenderPass> {
        None
    }
}
