pub mod health;
pub use health::Health;

use crate::PhysHandle;

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
pub struct DamageReceivedParticleEmitter(pub crate::graphics::particle::Emitter);

/// Control when your Entity is Hurtful
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

    for (_, (collision::Contacts(contacts), &PhysHandle(h), hurtful, force)) in ecs
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

        for &touching_ent in contacts.iter() {
            if let Ok(mut hp) = ecs.get_mut::<Health>(touching_ent) {
                *hp -= hurtful.damage(speed);

                (|| {
                    let particles = ecs
                        .get::<DamageReceivedParticleEmitter>(touching_ent)
                        .ok()?;
                    let PhysHandle(touching_h) = *ecs.get::<PhysHandle>(touching_ent).ok()?;

                    let (_, _, _, contacts) = phys.contact_pair(h, touching_h, true)?;
                    let deepest = contacts
                        .deepest_contact()
                        .expect("no deepest contact!")
                        .contact;

                    let mut emitter = particles.0.clone();
                    emitter.offset_direction_bounds(deepest.normal);

                    let fade = crate::graphics::fade::Fade::no_visual(emitter.duration);

                    l8r.l8r(move |world| {
                        let emitter_ent = world.ecs.spawn((emitter, fade));
                        world.add_hitbox(
                            emitter_ent,
                            crate::Iso2::new(deepest.world1.coords, 0.0),
                            ncollide2d::shape::Cuboid::new(crate::Vec2::repeat(1.0)),
                            crate::CollisionGroups::new()
                                .with_membership(&[])
                                .with_whitelist(&[]),
                        );
                    });

                    Some(())
                })();
            }
        }
    }
}
