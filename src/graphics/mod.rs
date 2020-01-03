#[allow(dead_code)]
pub mod colors;
pub mod images;
pub mod sprite_sheet;
use images::ImageMap;
#[cfg(feature = "hitbox-outlines")]
mod hitbox_outlines;

use crate::config::Config;
use crate::World;
use crate::{na, Iso2, Vec2};
use crate::{DIMENSIONS, TILE_SIZE};
use hecs::Entity;
use ncollide2d::shape::Cuboid;
use quicksilver::{
    geom::{Rectangle, Transform, Vector},
    graphics::{
        Background::{Col, Img},
        Color, Font, FontStyle, Image, View,
    },
    lifecycle::{Asset, Window},
    Result,
};

#[derive(Debug)]
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
    TopLeft,
    /// Some other entity's position + some other arbitrary alignment.
    Relative(Entity, Box<Alignment>),
}
impl Default for Alignment {
    fn default() -> Self {
        Alignment::Bottom(0.0)
    }
}
impl Alignment {
    pub fn relative(entity: Entity, alignment: Alignment) -> Self {
        Alignment::Relative(entity, Box::new(alignment))
    }

    fn parent_location(&self, world: &World) -> Option<Vec2> {
        if let Alignment::Relative(ent, _) = self {
            use crate::PhysHandle;

            let mut parent_location = world
                .ecs
                .get::<PhysHandle>(*ent)
                .ok()
                .map(|x| *x)
                .and_then(|PhysHandle(h)| world.phys.collision_object(h))
                .map(|obj| obj.position().translation.vector)
                .unwrap_or_else(|| {
                    panic!(
                        concat!(
                            "Relative positioning requested for Entity[{:?}], ",
                            "but no CollisionObject could be found for it."
                        ),
                        ent
                    )
                });

            if let Ok(parent_appearance) = world.ecs.get::<Appearance>(*ent) {
                if let Some(location) = parent_appearance.alignment.parent_location(world) {
                    parent_location += location;
                }
            }

            Some(parent_location)
        } else {
            None
        }
    }

    pub fn offset(&self, rect: &Rectangle, world: &World) -> Vec2 {
        -1.0 * match &self {
            Alignment::Center => na::zero(),
            Alignment::Bottom(offset) => Vec2::new(0.0, rect.size.y / 2.0 + offset),
            Alignment::TopLeft => (rect.size / -2.0).into_vector(),
            Alignment::Relative(_, alignment) => {
                let mut offset = alignment.offset(rect, world);

                if let Some(location) = self.parent_location(world) {
                    // TODO: actually look up the rect
                    offset += location;
                }

                -1.0 * offset
            }
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
    Text {
        text: String,
        style: FontStyle,
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
    /// then I used it for sprite sheets too, I'm going to hell
    pub fn name(&self) -> &str {
        match &self {
            AppearanceKind::Image { name, .. } => &name,
            _ => unreachable!(),
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
                rectangle: Rectangle::new_sized(Vector::ONE),
            },
            alignment: Alignment::default(),
            z_offset: 0.0,
            flip_x: false,
        }
    }
}

pub fn render(
    window: &mut Window,
    world: &World,
    images: &mut ImageMap,
    font: &mut Asset<Font>,
    cfg: &Config,
) -> Result<()> {
    window.set_view(View::new(Rectangle::new_sized(DIMENSIONS / TILE_SIZE)));
    window.clear(colors::DISCORD)?;

    #[allow(unused_variables)]
    let mut render_one = |appearance: &Appearance,
                          sheet_index: Option<&sprite_sheet::Index>,
                          iso: &Iso2,
                          cuboid: Option<&Cuboid<f32>>|
     -> Result<()> {
        let rot = Transform::rotate(iso.rotation.angle().to_degrees());
        let loc = iso.translation.vector;

        match &appearance.kind {
            AppearanceKind::Color {
                color,
                rectangle: rect,
            } => {
                let offset = appearance.alignment.offset(rect, world);

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
            other => {
                let mut execute = |img: &Image, mut rect: Rectangle, scale: f32| {
                    rect.size *= scale / 16.0;
                    let offset = appearance.alignment.offset(&rect, world);

                    let mut transform = Transform::translate(loc - (rect.size / 2.0).into_vector())
                        * rot
                        * Transform::translate(offset);
                    if appearance.flip_x {
                        transform = transform * Transform::scale((-1, 1));
                    }

                    window.draw_ex(
                        &rect,
                        Img(&img),
                        transform,
                        loc.y + offset.y + appearance.z_offset,
                    );

                    Ok(())
                };
                match other {
                    AppearanceKind::Image { name, scale } => {
                        images
                            .get_mut(name)
                            .unwrap_or_else(|| panic!("Couldn't find an image with name: {}", name))
                            .execute(|src| {
                                let (img, rect) = if let (Some(entry), Some(index)) =
                                    (cfg.sprite_sheets.get(name), sheet_index)
                                {
                                    (
                                        src.subimage(Rectangle::new(
                                            entry.frame_size.component_mul(&index.0),
                                            entry.frame_size,
                                        )),
                                        Rectangle::new_sized(entry.frame_size),
                                    )
                                } else {
                                    (src.clone(), src.area())
                                };
                                execute(&img, rect, *scale)
                            })?;
                    }
                    AppearanceKind::Text { text, style } => {
                        font.execute(|font| {
                            let img = font.render(text.as_str(), &style)?;
                            execute(&img, img.area(), 1.0)
                        })?;
                    }
                    _ => unreachable!(),
                }
            }
        }

        #[cfg(feature = "hitbox-outlines")]
        {
            if let Some(c) = cuboid {
                hitbox_outlines::debug_lines(window, c, iso, 1.0);
            }
        }

        Ok(())
    };

    for (_, (appearance, sheet_index, iso)) in world
        .ecs
        .query::<(&Appearance, Option<&sprite_sheet::Index>, &Iso2)>()
        .iter()
    {
        render_one(appearance, sheet_index, iso, None)?;
    }
    for (appearance, sheet_index, iso, cuboid) in world.phys.objects.iter().filter_map(|(_, obj)| {
            let ent = *obj.data();
            Some((
                world.ecs.get::<Appearance>(ent).ok()?,
                world.ecs.get::<sprite_sheet::Index>(ent).ok(),
                obj.position(),
                Some(obj.shape().as_shape::<Cuboid<f32>>().unwrap_or_else(|| {
                    panic!(
                        "Physical Entity[{:?}] is found in world.phys, but has shape other than Cuboid!",
                        ent,
                    )
                }))
            ))
        }) {
            // if let to avoid some weird hecs::Ref not coercing issues
            if let Some(sheet_index) = sheet_index {
                render_one(&appearance, Some(&sheet_index), iso, cuboid)
            } else {
                render_one(&appearance, None, iso, cuboid)
            }?;
        }

    #[cfg(feature = "hitbox-outlines")]
    for (_, (cuboid, iso)) in &mut world.ecs.query::<(&Cuboid<f32>, &Iso2)>() {
        hitbox_outlines::debug_lines(window, cuboid, iso, 0.18);
    }

    Ok(())
}
