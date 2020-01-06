use crate::{na, Iso2, PhysHandle, Vec2};
use na::Unit;
use rand::distributions::uniform::Uniform;

fn unit_vector_to_unit_complex(vec: Unit<Vec2>) -> na::UnitComplex<f32> {
    na::UnitComplex::rotation_between(&vec, &Vec2::x_axis())
}

/// Creates a tuple suitable for use as an Emitter's `direction_bounds` field.
pub fn direction_bounds_from_degrees(a: f32, b: f32) -> (Unit<Vec2>, Unit<Vec2>) {
    fn convert_one(angle: f32) -> Unit<Vec2> {
        na::UnitComplex::from_angle(angle.to_radians()) * Vec2::x_axis()
    }

    (convert_one(a), convert_one(b))
}

/// Generates some particles at the location of the Entity this Component is associated
/// with for the given duration, sending them off in a direction specified by the bounds.
#[derive(Clone, Debug)]
pub struct Emitter {
    /// For how many frames should this Particle Emitter emit particles?
    pub duration: usize,

    /// Between what two directions should the generated particles.
    /// If None, a completely random direction is supplied.
    pub direction_bounds: Option<(Unit<Vec2>, Unit<Vec2>)>,

    /// A Uniform used each frame to generate a value indicating how many particles should be emitted
    /// at the end of that frame.
    pub particle_count: Uniform<usize>,

    // particle configuration
    pub force_magnitude: Uniform<f32>,
    pub force_decay: Uniform<f32>,
    pub particle_duration: Uniform<usize>,
    pub particle_duration_fade: Uniform<usize>,
    pub color: [Uniform<f32>; 4],
    pub size: [Uniform<f32>; 2],
    /// If true, the value generated for the particle's size on the x axis
    /// will also be used for its size on the y axis.
    pub square: bool,
}
impl Default for Emitter {
    fn default() -> Self {
        Self {
            duration: 1,
            particle_count: (1..=1).into(),
            direction_bounds: None,

            // the particles themselves
            force_decay: (0.75..=0.75).into(),
            force_magnitude: (1.0..=1.0).into(),
            particle_duration: (100..=100).into(),
            particle_duration_fade: (25..=25).into(),
            color: [
                (0.2..1.0).into(),
                (0.0..=0.0).into(),
                (0.0..=0.0).into(),
                (1.0..=1.0).into(),
            ],
            size: [(0.1..0.4).into(), (0.1..0.4).into()],
            square: false,
        }
    }
}
impl Emitter {
    /// Offset the direction in which the particles will be emitted.
    ///
    /// For example, if you have a set of direction bounds generated from the angles -15 and 15 i.e.
    /// ```rs
    /// let my_bounds = particle::direction_bounds_from_degrees(-15.0, 15.0);
    /// ```
    /// and you want to orient these bounds to point towards something at (0.5, 0.5) so that the
    /// particles fly towards it give or take as much as 15 degrees, then you could call this
    /// method on an Emitter instantiated with those bounds like so:
    /// ```rs
    /// use nalgebra as na;
    ///
    /// let my_emitter = Emitter {
    ///     direction_bounds: my_bounds,
    ///     .. Default::default()
    /// };
    /// my_emitter.offset_direction_bounds(na::Unit::new_normalized(na::Vector2<f32>::repeat(0.5)));
    /// ```
    ///
    /// Has no effect if no `direction_bounds` field is found on the Emitter.
    pub fn offset_direction_bounds(&mut self, dir: Unit<Vec2>) {
        let offset = unit_vector_to_unit_complex(dir);
        let offset_one = |vec| unit_vector_to_unit_complex(vec) * offset * Vec2::x_axis();

        self.direction_bounds
            .map(|(a, b)| (offset_one(a), offset_one(b)));
    }

    /// Generate a Unit<Vec2> pointing in a direction somewhere between the two values stored in
    /// `self.direction_bounds`.
    ///
    /// Returns a completely random direction if no `direction_bounds` field is present on this Emitter.
    fn generate_direction(&self, rng: &mut rand::rngs::ThreadRng) -> Unit<Vec2> {
        use rand::Rng;

        self.direction_bounds
            .map(|(a, b)| a.slerp(&b, rng.gen_range(0.0, 1.0)))
            .unwrap_or_else(|| {
                na::UnitComplex::from_angle(rng.gen_range(0.0, std::f32::consts::PI * 2.0))
                    * Vec2::x_axis()
            })
    }
}

/// Stores state needed across frames of particle generation.
pub struct Manager {
    rng: rand::rngs::ThreadRng,
}
impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}
impl Manager {
    fn new() -> Self {
        Self {
            rng: rand::thread_rng(),
        }
    }

    /// Intended to be called every frame.
    ///
    /// Schedules the creation of Particle Entities for the end of the next frame.
    /// Updates Emitters and removes them if their duration ends.
    pub fn emit_particles(&mut self, world: &mut crate::World) {
        let ecs = &world.ecs;
        let phys = &world.phys;
        let l8r = &mut world.l8r;

        for (emitter_ent, (&PhysHandle(h), emitter)) in
            &mut ecs.query::<(&PhysHandle, &mut Emitter)>()
        {
            emitter.duration -= 1;

            // schedule the removal of the component at the end of the frame if its time is up.
            if emitter.duration == 0 {
                l8r.remove_one::<Emitter>(emitter_ent);
            }

            let emitter_translation = {
                phys.collision_object(h)
                    .unwrap_or_else(|| {
                        panic!(
                            "particle::Emitter[{:?}] has no Collision Object on handle[{:?}]!",
                            emitter_ent, h
                        )
                    })
                    .position()
                    .translation
            };

            use rand::distributions::Distribution;
            let rng = &mut self.rng;
            let particle_count = emitter.particle_count.sample(rng);

            for _ in 0..particle_count {
                use crate::{collide, graphics, phys};

                let size = if emitter.square {
                    Vec2::repeat(emitter.size[0].sample(rng))
                } else {
                    Vec2::new(emitter.size[0].sample(rng), emitter.size[1].sample(rng))
                };
                let dir = emitter.generate_direction(rng);

                let particle_components = (
                    graphics::Appearance {
                        kind: graphics::AppearanceKind::Color {
                            color: quicksilver::graphics::Color {
                                r: emitter.color[0].sample(rng),
                                g: emitter.color[1].sample(rng),
                                b: emitter.color[2].sample(rng),
                                a: emitter.color[3].sample(rng),
                            },
                            rectangle: quicksilver::geom::Rectangle::new_sized(size),
                        },
                        alignment: graphics::Alignment::Center,
                        z_offset: -10.0,
                        ..Default::default()
                    },
                    phys::Force::new(
                        *dir * emitter.force_magnitude.sample(rng),
                        emitter.force_decay.sample(rng),
                    ),
                    graphics::fade::Fade {
                        duration: emitter.particle_duration.sample(rng),
                        fade_start: emitter.particle_duration_fade.sample(rng),
                    },
                );

                l8r.l8r(move |world: &mut crate::World| {
                    let particle = world.ecs.spawn(particle_components);
                    world.add_hitbox(
                        particle,
                        Iso2::from_parts(emitter_translation, unit_vector_to_unit_complex(dir)),
                        ncollide2d::shape::Cuboid::new(size),
                        crate::CollisionGroups::new()
                            .with_membership(&[collide::PARTICLE])
                            .with_whitelist(&[collide::WORLD]),
                    );
                });
            }
        }
    }
}
