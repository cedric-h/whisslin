use crate::{phys::PhysHandle, world, Game};
use fxhash::FxHashSet;
use hecs::Entity;

/// Entities with static collision aren't moved when other things run into them.
/// Instead, only the things that run into them will move.
///
/// Walls, fences, mountains, etc. can be considered to be CollisionStatic.
#[derive(serde::Deserialize, serde::Serialize, Default, Clone, PartialEq)]
pub struct CollisionStatic;

/// Assigning this component to an Entity allows you to get finer grained control
/// over what an Entity can collide with and be forced out of. The CollisionGroups
/// you pass to `.add_hitbox` control all possible collisions your shape can collide with.
///
/// These groups control only what bodies your Entity will be forced out of should they collide.
/// If these aren't supplied, the collision system will simply default to the CollisionGroups
/// supplied to `.add_hitbox`.
pub struct RigidGroups(pub super::CollisionGroups);
impl std::ops::Deref for RigidGroups {
    type Target = super::CollisionGroups;

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
pub struct Contacts {
    pub inner: FxHashSet<Entity>,
    force: na::Vector2<f32>,
}
impl Contacts {
    pub fn new() -> Self {
        Self {
            inner: FxHashSet::default(),
            force: na::zero(),
        }
    }
}
impl std::ops::Deref for Contacts {
    type Target = FxHashSet<Entity>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl std::ops::DerefMut for Contacts {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub fn collision(world: &mut Game) {
    let mut scripts = glsp::lib_mut::<world::script::Cache>();
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
            ($a:ident, $b:ident, $($for_each:tt)*) => {
                let mut $a = ent_from_handle($a);
                let mut $b = ent_from_handle($b);

                $($for_each)*;
                std::mem::swap(&mut $a, &mut $b);
                $($for_each)*;
            };
        };
        match event {
            &ContactEvent::Started(ent, other_ent) => {
                process_handle_pair!(ent, other_ent, {
                    if let Ok(mut contacts) = ecs.get_mut::<Contacts>(ent) {
                        contacts.insert(other_ent);
                    }
                    scripts.new_collisions.push((ent, other_ent));
                });
            }
            &ContactEvent::Stopped(ent, other_ent) => {
                process_handle_pair!(ent, other_ent, {
                    if let Ok(mut contacts) = ecs.get_mut::<Contacts>(ent) {
                        contacts.remove(&other_ent);
                    }
                });
            }
        }
    });

    for (
        collided_ent,
        (
            Contacts {
                inner: contacts,
                force,
            },
            &collided_h,
            rigid_groups,
        ),
    ) in ecs
        .query::<(&mut _, &_, Option<&RigidGroups>)>()
        .without::<CollisionStatic>()
        .iter()
    {
        for &other_ent in contacts.iter() {
            // if the recorded contact is with an entity that can't be found,
            // just ignore it, they've probably been deleted or something.
            if let Ok(other_h) = ecs.get(other_ent).map(|x| *x) {
                if let (Ok(other_rigid_groups), Some(rigid_groups)) =
                    (ecs.get::<RigidGroups>(other_ent), rigid_groups)
                {
                    if !rigid_groups.can_interact_with_groups(&other_rigid_groups) {
                        continue;
                    }
                };

                if let Some((l, _, _, contacts)) = phys.contact_pair(collided_h, other_h, true) {
                    let deepest = contacts.deepest_contact().unwrap().contact;
                    let mut normal = deepest.normal.into_inner() * deepest.depth;
                    if l == collided_h {
                        normal *= -1.0
                    }
                    *force += normal;
                }
            }
        }

        let obj = phys.get_mut(collided_h).unwrap_or_else(|| {
            panic!(
                "Contacted Entity[{:?}] has no Collision Object!",
                collided_ent
            )
        });

        *force *= 0.87;

        let mut iso = obj.position().clone();
        iso.translation.vector += *force;
        obj.set_position(iso);
    }
}

/// Remove the Collision Objects of dead Entities from the CollisionWorld
pub fn clear_dead_collision_objects(world: &mut Game) {
    let ecs = &world.ecs;
    let phys = &mut world.phys;

    phys.remove(
        world
            .dead
            .marks()
            .filter_map(|e| ecs.get::<PhysHandle>(e).ok().as_deref().copied())
            .collect::<Vec<PhysHandle>>()
            .as_slice(),
    );
}
