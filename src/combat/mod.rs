use crate::{na, PhysHandle, Vec2};

pub mod health;
pub use health::Health;

/// Things with the Hurtful component remove Health from the Entities in their Contacts.
///
///
/// ```
/// let default_hurtful = Hurtful {
///     raw_damage: 1.0,
///     kind: HurtfulKind::Raw,
///     minimum_speed: 0.0
///     minimum_damage: 0
/// };
///
/// assert_eq!(default_hurtful, Hurtful::default())
/// ```
#[derive(serde::Deserialize, Debug, Clone)]
pub struct Hurtful {
    /// The damage before it is multipled by i.e. the speed as is the case if `HurtfulKind::Ram` is
    /// supplied.
    pub raw_damage: f32,
    /// Is this Entity always Hurtful, or is it only Hurtful when it's going at a certain speed?
    /// Or something else entirely?
    pub kind: HurtfulKind,
    /// If the Hurtful Entity gets a Force component and it goes below this value then when
    /// it collides with anything, no damage will be dealt. If the speed picks back up, then
    /// damage will be dealt again.
    ///
    /// Supplying 0.0 means that the Hurtful component will never be limited due to low speeds.
    /// This is the default value.
    pub minimum_speed: f32,
    /// Especially with HurtfulKind::Ram, it's easy to get *really close* to dealing some damage,
    /// but not quite. Here you can specify at least how much damage should be dealt.
    pub minimum_damage: usize,
}
impl Default for Hurtful {
    fn default() -> Self {
        Self {
            raw_damage: 1.0,
            kind: HurtfulKind::Raw,
            minimum_speed: 0.0,
            minimum_damage: 0,
        }
    }
}
impl Hurtful {
    fn damage(&self, speed: f32) -> Health {
        let calculated = (self.raw_damage * self.kind.damage_coefficient(speed)).round() as usize;
        Health::new(calculated.max(self.minimum_damage))
    }
}

/// A particle::Emitter component that is assigned to the Entity that receives damage when damage is dealt.
pub struct DamageReceivedParticleEmitters(pub Vec<crate::graphics::particle::Emitter>);

/// Control when your Entity is Hurtful
#[derive(serde::Deserialize, Debug, Clone)]
pub enum HurtfulKind {
    /// Do damage only if moving quickly and collision occurs with something.
    Ram {
        /// Signifies how much the speed should impact the damage.
        ///
        /// Supplying 0.0 here means that the weapon will always deal 0 damage.
        ///
        /// Supplying 1.0 here means that if the speed is 3.0, the damage dealt will be multiplied by 3.
        ///
        /// The resulting damage after the multiplication is rounded to the nearest integer,
        /// meaning that if you supply 0.49 as the speed damage coefficient and the speed is 1.0 tiles/frame,
        /// 0 damage will be dealt. (1.0 * 0.49 = 0.49, rounds down to 0)
        speed_damage_coefficient: f32,
    },
    /// Regardless of speed or any other factor, if collision occurs, damage is dealt.
    Raw,
}
impl HurtfulKind {
    fn damage_coefficient(&self, speed: f32) -> f32 {
        match &self {
            HurtfulKind::Raw => 1.0,
            HurtfulKind::Ram {
                speed_damage_coefficient,
            } => speed * speed_damage_coefficient,
        }
    }
}

pub fn hurtful_damage(world: &mut crate::World) {
    use crate::phys;
    use crate::phys::collision;

    let ecs = &world.ecs;
    let phys = &world.phys;
    let l8r = &mut world.l8r;

    for (_, (collision::Contacts(contacts), &h, hurtful, force)) in ecs
        .query::<(
            &collision::Contacts,
            &PhysHandle,
            &Hurtful,
            Option<&phys::Force>,
        )>()
        .iter()
    {
        let speed = force.map(|f| f.vec.magnitude()).unwrap_or(0.0);

        if speed < hurtful.minimum_speed {
            continue;
        }

        let hurtful_loc = match phys.collision_object(h) {
            Some(obj) => obj.position().translation.vector,
            _ => continue,
        };

        for &touched_ent in contacts.iter() {
            if let Ok(mut hp) = ecs.get_mut::<Health>(touched_ent) {
                *hp -= hurtful.damage(speed);

                (|| {
                    let particles = ecs
                        .get::<DamageReceivedParticleEmitters>(touched_ent)
                        .ok()?;
                    let touched_h = *ecs.get(touched_ent).ok()?;

                    let mut emitter_pos = *phys.collision_object(touched_h)?.position();

                    let touched_loc = emitter_pos.translation.vector;

                    emitter_pos.rotation = na::UnitComplex::rotation_between(
                        &na::Unit::new_normalize(
                            force
                                .map(|f| f.vec)
                                .unwrap_or_else(|| hurtful_loc - touched_loc),
                        ),
                        &Vec2::x_axis(),
                    );

                    for emitter in particles.0.iter().cloned() {
                        l8r.l8r(move |world| {
                            emitter.spawn_instance(world, emitter_pos);
                        });
                    }

                    Some(())
                })();
            }
        }
    }
}
