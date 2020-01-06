use crate::World;
use crate::{PhysHandle, Vec2};
use fxhash::FxHashSet;
use hecs::Entity;

/// Entities with static collision aren't moved when other things run into them.
/// Instead, only the things that run into them will move.
///
/// Walls, fences, mountains, etc. can be considered to be CollisionStatic.
pub struct CollisionStatic;

/// Assigning this component to an Entity allows you to get finer grained control
/// over what an Entity can collide with and be forced out of. The CollisionGroups
/// you pass to `.add_hitbox` control all possible collisions your shape can collide with.
///
/// These groups control only what bodies your Entity will be forced out of should they collide.
/// If these aren't supplied, the collision system will simply default to the CollisionGroups
/// supplied to `.add_hitbox`.
pub struct RigidGroups(pub crate::CollisionGroups);
impl std::ops::Deref for RigidGroups {
    type Target = crate::CollisionGroups;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for RigidGroups {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Records all of the other entities this entity is touching
pub struct Contacts(pub FxHashSet<Entity>);
impl Contacts {
    pub fn new() -> Self {
        Self(FxHashSet::default())
    }
}
impl std::ops::Deref for Contacts {
    type Target = FxHashSet<Entity>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for Contacts {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub fn collision(world: &mut World) {
    let ecs = &mut world.ecs;
    let phys = &mut world.phys;

    phys.clear_events();
    phys.update();

    phys.contact_events().into_iter().for_each(|event| {
        use ncollide2d::pipeline::ContactEvent;
        let ent_from_handle = |h| {
            *phys
                .collision_object(h)
                .unwrap_or_else(|| {
                    panic!(
                        "Collision started for {:?} but no collision_object with this handle!",
                        h
                    );
                })
                .data()
        };
        macro_rules! process_handle_pair {
            ($a:ident, $b:ident, $for_each:expr) => {
                let ent_a = ent_from_handle($a);
                let ent_b = ent_from_handle($b);

                //std::array::IntoIter::new([(ent_a, ent_b), (ent_b, ent_a)])
                vec![(ent_a, ent_b), (ent_b, ent_a)]
                    .into_iter()
                    .filter_map(|(ent, other_ent)| {
                        Some((
                            ecs.get_mut::<Contacts>(ent).ok()?
                            /*.unwrap_or_else(|e| {
                                panic!(
                                    "Entity[{:?}] was collided with but has no Contacts: {}",
                                    ent, e
                                )
                            })*/,
                            other_ent,
                        ))
                    })
                    .for_each($for_each);
            };
        };
        match event {
            &ContactEvent::Started(a, b) => {
                process_handle_pair!(a, b, |(mut contacts, other_ent)| {
                    contacts.insert(other_ent);
                });
            }
            &ContactEvent::Stopped(a, b) => {
                process_handle_pair!(a, b, |(mut contacts, other_ent)| {
                    contacts.remove(&other_ent);
                });
            }
        }
    });

    for (collided_ent, (Contacts(contacts), &PhysHandle(collided_h), rigid_groups)) in ecs
        .query::<(&_, &_, Option<&RigidGroups>)>()
        .without::<CollisionStatic>()
        .iter()
    {
        let mut contacted_displacement = Vec2::zeros();

        for &other_ent in contacts.iter() {
            // if the recorded contact is with an entity that can't be found,
            // just ignore it, they've probably been deleted or something.
            if let Ok(PhysHandle(other_h)) = ecs.get(other_ent).map(|x| *x) {
                if let (Ok(other_rigid_groups), Some(rigid_groups)) =
                    (ecs.get::<RigidGroups>(other_ent), rigid_groups)
                {
                    if !rigid_groups.can_interact_with_groups(&other_rigid_groups) {
                        continue;
                    }
                };

                if let Some((_, _, _, contacts)) = phys.contact_pair(collided_h, other_h, true) {
                    let deepest = contacts.deepest_contact().unwrap().contact;
                    contacted_displacement -= deepest.normal.into_inner() * deepest.depth;
                }
            }
        }

        let obj = phys.get_mut(collided_h).unwrap_or_else(|| {
            panic!(
                "Contacted Entity[{:?}] has no Collision Object!",
                collided_ent
            )
        });

        let mut iso = obj.position().clone();
        iso.translation.vector += contacted_displacement;
        obj.set_position(iso);
    }
}

/// Remove the Collision Objects of dead Entities from the CollisionWorld
pub fn clear_dead_collision_objects(world: &mut World) {
    let ecs = &world.ecs;
    let phys = &mut world.phys;

    phys.remove(
        &ecs.query::<(&PhysHandle, &crate::Dead)>()
            .iter()
            .map(|(_, (&PhysHandle(h), _))| h)
            .collect::<Vec<_>>(),
    );
}
