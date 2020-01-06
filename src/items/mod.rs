use crate::World;
use fxhash::FxHashMap;
use hecs::Entity;

/// Entities with this component get inserted into the Inventory
/// component of the given Entity at the end of the next frame.
///
/// Similar to InventoryEquip, this happens after l8r.now(),
/// so inserting this component using l8r still works.
///
/// As soon as it's processed, this component is removed from the entity it affected.
pub struct InventoryInsert(pub Entity);

/// Entities with this component have their equipped item deleted and another of the same type
/// equipped take its place (if one is available) at the end of the next frame.
///
/// # Panics
/// Panics if the inventory entity this is associated with has nothing equipped.
///
/// Similar to InventoryEquip, this happens after l8r.now(),
/// so inserting this component using l8r still works.
///
/// As soon as it's processed, this component is removed from the entity it affected.
pub struct InventoryConsumeEquipped;

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
pub struct InventoryEquip(pub Option<String>);

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

        inventory.insert(item_appearance.kind.name().to_string(), item_ent, l8r);

        // this component is basically an event, it happens once then we can get rid of it.
        l8r.remove_one::<InventoryInsert>(item_ent);
    }

    // TODO: find some way to abuse L8r to keep (ConsumeEquipped/Equip)s in order and to allow
    // multiples of them.

    for (inv_ent, (inv_equip, inventory)) in &mut ecs.query::<(&InventoryEquip, &mut Inventory)>() {
        let item_name_to_equip = inv_equip.0.as_ref();

        // if there's something equipped right now we want to throw it back in the stack for the
        // type of item it is.
        if let Some((equipped_ent, item_name)) = inventory.equipped.take() {
            println!("deequipping {:?}", &item_name);
            inventory.insert(item_name, equipped_ent, l8r);
        }

        // handling equipping whatever new thing we're supposed to equip
        if let Some(item_name) = item_name_to_equip {
            let top_item_ent = inventory.equip_named(item_name).unwrap_or_else(|| {
                panic!(
                    "Attempted to equip {} for Inventory[{:?}] but no items of that type!",
                    item_name, inv_ent
                )
            });

            println!("equipping {:?}", item_name);
            inventory.equipped = Some((top_item_ent, item_name.to_string()));
        }
        // if we're equipping nothing, however, we take our equipped item, we put
        // it *back* in our slot for it, and then we record the lack of an equipped item.

        l8r.remove_one::<InventoryEquip>(inv_ent);
    }

    for (inv_ent, (_, inventory)) in &mut ecs.query::<(&InventoryConsumeEquipped, &mut Inventory)>()
    {
        inventory.consume_equipped();
        l8r.remove_one::<InventoryConsumeEquipped>(inv_ent);
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
    slots: FxHashMap<String, Vec<Entity>>,

    // the type of the equipped thing is also stored
    pub equipped: Option<(Entity, String)>,
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn equipped_ent(&self) -> Option<Entity> {
        self.equipped.as_ref().map(|(e, _)| *e)
    }

    /// Returns:
    /// an Option that contains the equipped entity if there was one to equip.
    fn consume_equipped(&mut self) -> Option<Entity> {
        let (_, equipped_item_name) = self
            .equipped
            .take()
            .expect("Can't consume equipped; nothing's equipped!");

        self.equip_named(equipped_item_name)
    }

    /// Finds the stack of items with this name, pops one off of the top and equips it.
    fn equip_named<S: Into<String>>(&mut self, name: S) -> Option<Entity> {
        let name = name.into();

        let popped = self
            .slots
            .get_mut(&name)
            .filter(|items| !items.is_empty())?
            .pop()
            // probably safe because we just checked to see if it was empty
            .unwrap();
        self.equipped = Some((popped, name));

        Some(popped)
    }

    fn insert(&mut self, item_name: String, item_ent: hecs::Entity, l8r: &mut crate::L8r) {
        // now that it's becoming an item, we want to yank it out of the physics world.
        // doing that ensures that it's not rendered or collided with
        // or any of that other icky stuff.
        l8r.l8r(move |world| {
            if let Ok(crate::PhysHandle(h)) = world.ecs.remove_one(item_ent) {
                world.phys.remove(&[h])
            }
        });

        /*
        println!(
            "inserting item {:?} named {} on {:?}",
            item_ent,
            item_name,
            inv_ent
        );*/

        self.slots.entry(item_name).or_insert(vec![]).push(item_ent);
    }
}
