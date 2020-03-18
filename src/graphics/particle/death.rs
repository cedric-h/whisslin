use crate::core::*;
use crate::graphics;
use crate::World;

pub struct Dead;
pub struct DeathParticleEmitters(pub Vec<graphics::particle::Emitter>);

pub fn death_particles(world: &mut World) {
    let ecs = &world.ecs;
    let phys = &world.phys;
    let l8r = &mut world.l8r;

    for (_, (_, h, particles)) in &mut ecs.query::<(&Dead, &PhysHandle, &DeathParticleEmitters)>() {
        (|| {
            let mut iso = Iso2::identity();
            iso.translation = phys.collision_object(*h)?.position().translation;

            for emitter in particles.0.iter().cloned() {
                l8r.l8r(move |world| {
                    emitter.spawn_instance(world, iso);
                });
            }

            Some(())
        })();
    }
}

pub fn clear_dead(world: &mut World) {
    let to_kill = world
        .ecs
        .query::<&Dead>()
        .iter()
        .map(|(ent, _)| ent)
        .collect::<Vec<hecs::Entity>>();

    to_kill.into_iter().for_each(|ent| {
        world
            .ecs
            .despawn(ent)
            .unwrap_or_else(|e| panic!("Couldn't kill Dead[{:?}]: {}", ent, e))
    });
}
