use crate::{Iso2, World};
use hecs::Entity;
use std::collections::HashMap;

/// Entities with this component get inserted into the Inventory
/// component of the given Entity at the end of each frame.
/// Similar to InventoryEquip, this happens after l8r.now(),
/// so inserting this component using l8r still works.
///
/// As soon as it's processed, this component is removed from the entity it affected.
pub struct InventoryInsert(pub Entity);

/// Switch the Inventory component of the entity this component is
/// associated with over to the given type of item at the end of the
/// next frame.
///
/// The name of the type of item is supplied a string, the same string
/// as is used for the item's image file.
///
/// Similar to InventoryInsert, this happens after l8r.now(),
/// so inserting this component using l8r still works.
///
/// As soon as it's processed, this component is removed from the entity it affected.
pub struct InventoryEquip<'name>(pub &'name str);

/// NOTE: this function is designed to be run after l8r.now(), but it also
/// runs its own l8r.now() at the end of its execution so as to run some
/// commands it schedules to l8r for convenience.
pub fn inventory_inserts(world: &mut World) {
    // manually splitting the borrow to appease rustc
    let ecs = &world.ecs;
    let l8r = &mut world.l8r;

    for (item_ent, (InventoryInsert(inv_ent), item_appearance)) in
        &mut ecs.query::<(&InventoryInsert, &crate::graphics::Appearance)>()
    {
        let mut inventory = ecs.get_mut::<Inventory>(*inv_ent).unwrap_or_else(|_| {
            panic!(
                "Attempted InventoryInsert({:?}) for entity[{:?}] lacking Inventory!",
                item_ent, inv_ent
            )
        });

        // now that it's becoming an item, we don't want it to have a position.
        // Removing the position ensures that it's not rendered or collided with
        // or any of that other icky stuff.
        l8r.remove_one::<Iso2>(item_ent);

        /*
        println!(
            "inserting item {:?} named {} on {:?}",
            item_ent,
            item_appearance.kind.name(),
            inv_ent
        );*/

        inventory.insert(item_appearance.kind.name().to_string(), item_ent);

        // this component is basically an event, it happens once then we can get rid of it.
        l8r.remove_one::<InventoryInsert>(item_ent);
    }

    for (item_ent, (&InventoryEquip(item_name), inventory)) in
        &mut ecs.query::<(&InventoryEquip, &mut Inventory)>()
    {
        let top_item_ent = inventory
            .slots
            .get_mut(item_name)
            .and_then(|items| items.pop())
            .unwrap_or_else(|| {
                panic!(
                    "Attempted to equip {} for Inventory[{:?}] but no items of that type!",
                    item_name, item_ent
                )
            });

        inventory.equipped = Some((top_item_ent, item_name.to_string()));

        l8r.remove_one::<InventoryEquip>(item_ent);
    }

    let scheduled_world_edits = world.l8r.drain();
    crate::L8r::now(scheduled_world_edits, world);
}

#[derive(Default, Debug)]
pub struct Inventory {
    // the strings correspond to the appearance of the item
    // it's quite dumb, because something's behavior shouldn't be
    // linked to its appearance, (and in this case the behavior
    // that this is linked to is item stacking) but...
    // works for now
    slots: HashMap<String, Vec<Entity>>,

    // the type of the equipped thing is also stored
    equipped: Option<(Entity, String)>,
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    /*
    pub fn new_with(items: &[Entity], world: &World) -> Result<Self, Error> {
        let mut s = Self::new();

        for item in items.iter() {
            s.insert(*item, world)?;
        }

        Ok(s)
    }*/

    /*
    pub fn with_equip(mut self, e: Entity, world: &World) -> Self {
        self.equipped = Some((
            e,
            world
                .get::<crate::graphics::Appearance>(e)
                .expect("Can't add item without appearance!")
                .kind
                .name()
                .into(),
        ));
        self
    }*/

    /*
    pub fn equip(&mut self, e: Entity) {
        self.equipped = Some(e);
    }*/

    pub fn equipped(&self) -> Option<Entity> {
        self.equipped.as_ref().map(|(e, _)| *e)
    }

    /// Deequips the equipped item, but equips another item
    /// of the same type to take its place, if one is available.
    ///
    /// # Panics
    /// Panics if there is no equipped.
    pub fn consume_equipped(&mut self) {
        let eq = self
            .equipped
            .as_ref()
            .expect("Can't consume equipped; doesn't exist!");
        self.equipped = self
            .slots
            .get_mut(&eq.1)
            .filter(|items| !items.is_empty())
            .map(|items| (items.remove(0), eq.1.clone()));
    }

    /*
    /// Gets the appearance to use for HashMap indexing
    pub fn insert(&mut self, ent: Entity, world: &World) -> Result<(), Error> {
        world
            .get::<crate::graphics::Appearance>(ent)
            .map(|appearance| self.insert_raw(appearance.kind.name().into(), ent))
            .map_err(|_| Error::NoAppearance(ent))
    }*/

    fn insert(&mut self, key: String, val: hecs::Entity) {
        self.slots.entry(key).or_insert(vec![]).push(val);
    }
}
