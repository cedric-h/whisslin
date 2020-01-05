pub mod health;

/// Things with the Hurtful component remove Health from the Entities in their Contacts.
pub struct Hurtful;

pub fn hurtful_damage(world: &mut crate::World) {
    use crate::phys::collision;
    use health::Health;

    let ecs = &world.ecs;

    for (_, (collision::Contacts(contacts), _)) in
        ecs.query::<(&collision::Contacts, &Hurtful)>().iter()
    {
        for &touching_ent in contacts.iter() {
            if let Ok(mut hp) = ecs.get_mut::<Health>(touching_ent) {
                *hp -= Health::new(1);
            }
        }
    }
}
