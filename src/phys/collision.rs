use crate::Iso2;
use hecs::World;
use ncollide2d::shape::Cuboid;

pub struct CollisionStatic;

pub fn collision(world: &mut World) {
    use ncollide2d::query::contact;
    world
        .query::<&Cuboid<f32>>()
        .into_iter()
        .filter(|(id, _)| world.get::<CollisionStatic>(*id).is_err())
        // get the ents that need to be moved
        .filter_map(|(id, cuboid)| {
            let iso = world.get::<Iso2>(id).ok()?;
            for (o_id, (o_iso, o_cuboid)) in world.query::<(&Iso2, &Cuboid<f32>)>().iter() {
                if id != o_id {
                    if let Some(c) = contact(&iso, cuboid, o_iso, o_cuboid, 0.0) {
                        return Some((id, c.normal.into_inner() * c.depth));
                    }
                }
            }
            None
        })
        .for_each(|(collided_id, normal)| {
            let mut iso = world.get_mut::<Iso2>(collided_id).unwrap_or_else(|e| {
                panic!(
                    "Collided[{:?}] had Iso2 in last iterator method, but can't get Iso2 now: {:?}",
                    collided_id, e
                )
            });
            iso.translation.vector -= normal;
        });
}
