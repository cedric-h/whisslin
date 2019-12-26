use crate::Iso2;
use ncollide2d::shape::Cuboid;
use quicksilver::geom::{Rectangle, Transform, Vector};
use quicksilver::graphics::{Background::Col, Color};
use quicksilver::lifecycle::Window;

pub fn debug_lines(window: &mut Window, cuboid: &Cuboid<f32>, iso: &Iso2, alpha: f32) {
    let mut debug_line = quicksilver::geom::Line::new((0.0, 0.0), (0.0, 0.0)).with_thickness(0.04);

    let pos = Transform::translate(iso.translation.vector);
    let rot = Transform::rotate(iso.rotation.angle().to_degrees());
    let rect = Rectangle::from_cuboid(Vector::ZERO, cuboid);

    const LINES: &'static [((f32, f32), (f32, f32))] = &[
        ((0.0, 0.0), (1.0, 0.0)),
        ((0.0, 0.0), (0.0, 1.0)),
        ((0.0, 1.0), (1.0, 1.0)),
        ((1.0, 0.0), (1.0, 1.0)),
        ((0.0, 0.0), (1.0, 1.0)),
        ((0.0, 1.0), (1.0, 0.0)),
    ];

    for (a, b) in LINES.iter() {
        debug_line.a = rect.size.times(*a) + rect.pos;
        debug_line.b = rect.size.times(*b) + rect.pos;
        window.draw_ex(
            &debug_line,
            Col(Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: alpha,
            }),
            pos * rot,
            (alpha - 0.5) * -1000.0,
        );
    }
}
