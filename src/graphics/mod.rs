#[allow(dead_code)]
pub mod colors;
pub mod images;
use images::ImageMap;
#[cfg(feature = "hitbox-outlines")]
mod hitbox_outlines;

use crate::{na, Iso2, Vec2};
use hecs::World;
use ncollide2d::shape::Cuboid;
use quicksilver::{
    geom::{Rectangle, Transform, Vector},
    graphics::{
        Background::{Col, Img},
        Color,
    },
    lifecycle::Window,
    Result,
};

pub enum Alignment {
    /// The center of the hitbox is the center of the sprite
    #[allow(dead_code)]
    Center,
    /// # Y axis
    /// The bottom of the sprite is aligned with the bottom of the hitbox.
    /// # X axis
    /// Centered.
    /// The value supplied is the offset from the bottom.
    Bottom(f32),
}
impl Default for Alignment {
    fn default() -> Self {
        Alignment::Bottom(0.0)
    }
}
impl Alignment {
    pub fn offset(&self, rect: &Rectangle) -> Vec2 {
        -1.0 * match &self {
            Alignment::Center => na::zero(),
            Alignment::Bottom(offset) => Vec2::new(0.0, rect.size.y / 2.0 + offset),
        }
    }
}

#[derive(Clone)]
pub enum AppearanceKind {
    #[allow(dead_code)]
    Color {
        color: Color,
        rectangle: Rectangle,
    },
    Image {
        name: String,
        scale: f32,
    },
}
impl AppearanceKind {
    pub fn image<S: Into<String>>(name: S) -> Self {
        AppearanceKind::Image {
            name: name.into(),
            scale: 1.0,
        }
    }

    /// I needed this for some very cursed reasons (game/src/items/mod.rs)
    pub fn name(&self) -> String {
        match &self {
            AppearanceKind::Color { .. } => String::from("Color"),
            AppearanceKind::Image { name, .. } => name.clone(),
        }
    }
}

pub struct Appearance {
    pub kind: AppearanceKind,
    pub alignment: Alignment,
    pub z_offset: f32,
    /// Render sprite flipped on X axis.
    pub flip_x: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            kind: AppearanceKind::Color {
                color: Color::RED,
                rectangle: Rectangle::new_sized(quicksilver::geom::Vector::ONE * 128),
            },
            alignment: Alignment::default(),
            z_offset: 0.0,
            flip_x: false,
        }
    }
}

pub fn render(window: &mut Window, world: &World, images: &mut ImageMap) -> Result<()> {
    window.clear(colors::DISCORD)?;

    let mut draw_query = world.query::<(&Appearance, Option<&Cuboid<f32>>, &Iso2)>();

    for (_, (appearance, cuboid, iso)) in draw_query.iter() {
        let rot = Transform::rotate(iso.rotation.angle().to_degrees());
        let loc = iso.translation.vector;

        match &appearance.kind {
            AppearanceKind::Color { color, rectangle: rect } => {
                let offset = appearance.alignment.offset(rect);

                let mut transform = Transform::translate(loc - (rect.size / 2.0).into_vector())
                    * rot
                    * Transform::translate(offset);
                if appearance.flip_x {
                    transform = transform * Transform::scale((-1, 1));
                }

                window.draw_ex(
                    rect,
                    Col(*color),
                    transform,
                    loc.y + offset.y + appearance.z_offset,
                );
            }
            AppearanceKind::Image { name, scale } => {
                images
                    .get_mut(name)
                    .unwrap_or_else(|| panic!("Couldn't find an image with name: {}", name))
                    .execute(|img| {
                        let mut rect = img.area();
                        rect.size *= *scale;
                        let offset = appearance.alignment.offset(&rect);

                        let mut transform = Transform::translate(loc - (rect.size / 2.0).into_vector())
                            * rot
                            * Transform::translate(offset);
                        if appearance.flip_x {
                            transform = transform * Transform::scale((-1, 1));
                        }

                        window.draw_ex(
                            &rect,
                            Img(img),
                            transform,
                            loc.y + offset.y + appearance.z_offset,
                        );
                        Ok(())
                    })?;
            }
        }

        #[cfg(feature = "hitbox-outlines")]
        {
            if let Some(c) = cuboid {
                hitbox_outlines::debug_lines(window, c, iso, 1.0);
            }
        }
    }

    #[cfg(feature = "hitbox-outlines")]
    for (_, (_, cuboid, iso)) in draw_query.iter() {
        if let Some(c) = cuboid {
            hitbox_outlines::debug_lines(window, c, iso, 0.18);
        }
    }

    Ok(())
}
