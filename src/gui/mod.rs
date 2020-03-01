use crate::World;
use crate::{graphics, items, phys};
use crate::{Iso2, PhysHandle, Vec2};
use hecs::Entity;
use quicksilver::geom::{Rectangle, Vector};
use quicksilver::input::{Mouse, MouseButton};

struct Draggable;

/// Draggables that also have the Docking component will return to a given location when released.
#[derive(Clone, Copy)]
struct Docking {
    home: Vec2,
    speed: f32,
}
impl Docking {
    fn new(home: Vec2, speed: f32) -> Self {
        Self { home, speed }
    }

    /// Starts sending an entity back towards their home location at the end of the next frame.
    fn dock(&self, docking_ent: Entity, l8r: &mut l8r::L8r<crate::World>) {
        l8r.insert_one(docking_ent, phys::DragTowards::new(self.home, self.speed));
    }
}

/// ItemSlot structs correspond to inventory items and dump their contents out on release.
#[derive(Debug)]
struct ItemSlot {
    /// The type of item this ItemSlot contains (None if it's still empty)
    item_name: Option<String>,
    /// The Entity for the image that shows what item_name this slot stores.
    icon_ent: Entity,
    /// The Entity for the text that indicates how many items are held in this slot.
    /// This Entity should have a Counter component.
    counter_ent: Entity,
    /// The InventoryWindow that owns this ItemSlot
    parent: Entity,
}

type EntityAndSlot<'a> = (Entity, hecs::Ref<'a, ItemSlot>);

/// This component goes on the Entity which has the real items::Inventory component,
/// and helps update its GUI when Inventory events happen.
pub struct InventoryWindow {
    /// The Entity of the main GUI window, all of the slots are positioned relative to it.
    window: Entity,
    /// That special slot that holds the thing they're currently using
    equipped_slot: Entity,
    /// The other slots that hold the other kinds of items they have
    loose_slots: Vec<Entity>,
}
impl InventoryWindow {
    /// Takes out_ent out of the inventory and puts in_ent into the same category of slots
    /// as out_ent was in. This means that this method works for equipped_slots and loose_slots,
    /// and whichever of those two out_ent was, in_ent will become.
    fn swap_in_out(&mut self, in_ent: Entity, out_ent: Entity) {
        if out_ent == self.equipped_slot {
            self.equipped_slot = in_ent;
        } else {
            self.loose_slots.retain(|x| *x != out_ent);
            self.loose_slots.push(in_ent);
        }
    }

    /// Iterates over all of the slots in the inventory and compares them to the provided name.
    /// Returns the entity and slot, if any, that houses items of the name provided.
    fn find_item_slot<'a>(
        &'a self,
        ecs: &'a hecs::World,
        item_name: &'a str,
    ) -> Option<EntityAndSlot<'a>> {
        self.occupied_slots(&ecs)
            // unwrap safe because occupied_slots
            .find(|(_, item_slot)| item_slot.item_name.as_ref().unwrap() == item_name)
    }

    /// All of the slots and their Entity in an InventoryWindow that are already storing some type of item.
    ///
    /// The opposite of .empty_slots().
    ///
    /// Iteration order: see slots()
    fn occupied_slots<'a>(
        &'a self,
        ecs: &'a hecs::World,
    ) -> impl Iterator<Item = EntityAndSlot> + 'a {
        self.slots(&ecs)
            .filter(|(_, slot)| slot.item_name.is_some())
    }

    /// All of the slots and their Entity in an InventoryWindow that aren't yet storing some type of item.
    ///
    /// The opposite of .occupied_slots().
    ///
    /// Iteration order: see slots()
    fn empty_slots<'a>(&'a self, ecs: &'a hecs::World) -> impl Iterator<Item = EntityAndSlot> + 'a {
        self.slots(&ecs)
            .filter(|(_, slot)| slot.item_name.is_none())
    }

    /// All of the slots in an InventoryWindow, loose or equipped.
    ///
    /// Iteration order: first all of the loose slots, starting with the top left. Finally, the equipped
    /// slot is tacked onto the end.
    fn slots<'a>(&'a self, ecs: &'a hecs::World) -> impl Iterator<Item = EntityAndSlot> + 'a {
        self.loose_slots
            .iter()
            .chain(std::iter::once(&self.equipped_slot))
            .map(move |item_ent| {
                (
                    *item_ent,
                    ecs.get::<ItemSlot>(*item_ent).unwrap_or_else(|_| {
                        panic!(
                            concat!(
                                "Entity[{:?}] stored as one of InventoryWindow[{:?}]'s slots,",
                                "but no ItemSlot component!"
                            ),
                            item_ent, self.window
                        )
                    }),
                )
            })
    }
}

/// How many of a type of item a player has.
pub struct Counter(usize);
impl Counter {
    fn make_graphics_appearance(&self, parent: Entity) -> graphics::Appearance {
        graphics::Appearance {
            kind: graphics::AppearanceKind::Text {
                text: self.0.to_string(),
                style: quicksilver::graphics::FontStyle::new(20.0, graphics::colors::DISCORD),
            },
            alignment: graphics::Alignment::relative(parent, graphics::Alignment::Center),
            z_offset: 130.0,
            ..Default::default()
        }
    }
}

fn slot_icon_graphics_appearance(
    slot_ent: Entity,
    item_name: &str,
    images: &mut graphics::images::ImageMap,
) -> graphics::Appearance {
    // make an icon for the slot
    let scale = {
        let mut scale: Vec2 = crate::na::zero();
        images
            .get_mut(item_name)
            .unwrap()
            .execute(|image| {
                scale = image.area().size.into_vector();
                Ok(())
            })
            .unwrap();
        (16.0 / scale.x) * 0.8
    };

    graphics::Appearance {
        kind: graphics::AppearanceKind::Image {
            name: item_name.to_string(),
            scale,
        },
        alignment: graphics::Alignment::relative(slot_ent, graphics::Alignment::TopLeft),
        z_offset: 130.0,
        ..Default::default()
    }
}

pub fn build_inventory_gui_entities(world: &mut World, parent: Entity) -> InventoryWindow {
    use ncollide2d::shape::Cuboid;

    let window = {
        let size = Vec2::new(10.0, 6.0);

        let window = world.ecs.spawn((
            Draggable,
            graphics::Appearance {
                kind: graphics::AppearanceKind::Color {
                    color: graphics::colors::DISCORD,
                    rectangle: Rectangle::new_sized(size),
                },
                alignment: graphics::Alignment::TopLeft,
                z_offset: 100.0,
                ..Default::default()
            },
            #[cfg(feature = "hot-config")]
            crate::config::ReloadWithConfig,
        ));

        world.add_hitbox(
            window,
            Iso2::translation(19.0, 1.0),
            Cuboid::new(size / 2.0),
            crate::CollisionGroups::new()
                .with_membership(&[crate::collide::GUI])
                .with_whitelist(&[]),
        );

        window
    };

    // a thin line that cuts across the GUI, deliminating sections.
    let hr = |world: &mut World, x: f32, y: f32| {
        let size = Vec2::new(9.0, 0.125);

        let hr = world.ecs.spawn((
            graphics::Appearance {
                kind: graphics::AppearanceKind::Color {
                    color: graphics::colors::LIGHT_SLATE_GRAY,
                    rectangle: Rectangle::new_sized(size),
                },
                alignment: graphics::Alignment::relative(window, graphics::Alignment::TopLeft),
                z_offset: 110.0,
                ..Default::default()
            },
            #[cfg(feature = "hot-config")]
            crate::config::ReloadWithConfig,
        ));

        world.add_hitbox(
            hr,
            Iso2::translation(x, y - (0.125 / 2.0)),
            Cuboid::new(size / 2.0),
            crate::CollisionGroups::new()
                .with_membership(&[crate::collide::GUI])
                .with_whitelist(&[]),
        );

        hr
    };

    // these guys aren't actually given real appearances until an item
    // is put in the slots they are associated with.
    let blank_icon = |world: &mut World| {
        let size = Vec2::new(0.8, 0.8);

        let icon = world.ecs.spawn((
            #[cfg(feature = "hot-config")]
            crate::config::ReloadWithConfig,
        ));

        world.add_hitbox(
            icon,
            Iso2::translation(0.1, 0.1),
            Cuboid::new(size / 2.0),
            crate::CollisionGroups::new()
                .with_membership(&[crate::collide::GUI])
                .with_whitelist(&[]),
        );

        icon
    };

    let blank_counter = |world: &mut World| {
        let size = Vec2::new(0.2, 0.2);

        let blank_counter = world.ecs.spawn((
            Counter(0),
            #[cfg(feature = "hot-config")]
            crate::config::ReloadWithConfig,
        ));

        world.add_hitbox(
            blank_counter,
            Iso2::translation(1.4, 0.4),
            Cuboid::new(size / 2.0),
            crate::CollisionGroups::new()
                .with_membership(&[crate::collide::GUI])
                .with_whitelist(&[]),
        );

        blank_counter
    };

    let slot = |world: &mut World, x: f32, y: f32| {
        let size = Vec2::new(2.0, 1.0);

        let icon_ent = blank_icon(world);
        let counter_ent = blank_counter(world);

        let slot = world.ecs.spawn((
            Docking::new(Vec2::new(x, y), 0.4),
            ItemSlot {
                item_name: None,
                icon_ent,
                counter_ent,
                parent,
            },
            Draggable,
            graphics::Appearance {
                kind: graphics::AppearanceKind::Color {
                    color: graphics::colors::LIGHT_SLATE_GRAY,
                    rectangle: Rectangle::new_sized(size),
                },
                alignment: graphics::Alignment::relative(window, graphics::Alignment::TopLeft),
                z_offset: 120.0,
                ..Default::default()
            },
            #[cfg(feature = "hot-config")]
            crate::config::ReloadWithConfig,
        ));

        world.add_hitbox(
            slot,
            Iso2::translation(x, y),
            Cuboid::new(size / 2.0),
            crate::CollisionGroups::new()
                .with_membership(&[crate::collide::GUI])
                .with_whitelist(&[]),
        );

        slot
    };

    hr(world, 0.5, 0.5);
    hr(world, 0.5, 2.5);

    let equipped_slot = { slot(world, 1.25, 1.0) };

    let mut loose_slots = vec![];
    for y in 0..2 {
        for x in 0..3 {
            loose_slots.push(slot(world, 3.0 * (x as f32) + 1.0, 1.5 * (y as f32) + 3.0));
        }
    }

    InventoryWindow {
        window,
        equipped_slot,
        loose_slots,
    }
}

// when an item is inserted into an inventory and the entity with the inventory
// also has an InventoryWindow component associated with it
//
// 1) if there are currently no slots storing this type of item, we need to
//      find an applicable empty one and give it the appropriate icon
//
// 2) update the applicable counters on the slots
pub fn try_slot_insert<'a>(
    inv_ent: Entity,
    item_name: &str,
    ecs: &hecs::World,
    l8r: &mut l8r::L8r<crate::World>,
    images: &mut graphics::images::ImageMap,
) -> Option<()> {
    // early return because it's perfectly fine for an Inventory not to have an InventoryWindow,
    // but we only want to update the InventoryWindow if it does have one.
    let inv_window = ecs.get::<InventoryWindow>(inv_ent).ok()?;

    // find the first slot with the same name, or if that doesn't
    // work just grab the first slot that's empty.
    let (slot_ent, new_slot) = inv_window
        .find_item_slot(&ecs, item_name)
        .map(|ent_and_slot| (ent_and_slot, false))
        .or_else(|| inv_window.empty_slots(ecs).next().map(|slot| (slot, true)))
        // early return here because if there's no slot reserved for an entity of this
        // type and there are no empty slots...  then just give up! no space for this item!
        .map(|((ent, _slot), new_slot)| (ent, new_slot))?;

    let mut item_slot = ecs.get_mut::<ItemSlot>(slot_ent).unwrap();

    // popping open an empty slot means updating the entity's item_slot's item_name
    // and assigning a icon appearance to its icon_ent.
    if new_slot {
        item_slot.item_name = Some(item_name.to_string());

        l8r.insert_one(
            item_slot.icon_ent,
            slot_icon_graphics_appearance(slot_ent, item_name, images),
        );
    }

    // update counter value and appearance
    let mut counter = ecs
        .get_mut::<Counter>(item_slot.counter_ent)
        .unwrap_or_else(|_| {
            panic!(
                "ItemSlot[{:?}] of Inventory[{:?}] Counter[{:?}] has no counter component!",
                slot_ent, inv_ent, item_slot.counter_ent
            )
        });
    counter.0 += 1;
    l8r.insert_one(
        item_slot.counter_ent,
        counter.make_graphics_appearance(slot_ent),
    );

    Some(())
}

/// Attempts to swap two ItemSlot entities, having each dock to the location
/// previously occupied by the other and changing their parents records of which
/// ItemSlot holds the equipped, if necessary.
///
/// May return early if the entities don't have Docking or ItemSlot components.
///
/// Panics if either ItemSlot's record of who their parent points to an invalid entity
/// (one without an InventoryWindow component).
fn try_swap_slot_ents(
    left_ent: Entity,
    right_ent: Entity,
    ecs: &hecs::World,
    l8r: &mut l8r::L8r<crate::World>,
) -> Option<()> {
    let left_docking = *ecs.get::<Docking>(left_ent).ok()?;
    let right_docking = *ecs.get::<Docking>(right_ent).ok()?;

    {
        let mut right_docking = ecs.get_mut::<Docking>(right_ent).unwrap();

        *right_docking = left_docking;
        right_docking.dock(right_ent, l8r);
    }

    {
        let mut left_docking = ecs.get_mut::<Docking>(left_ent).unwrap();

        *left_docking = right_docking;
        right_docking.dock(left_ent, l8r);
    }

    let left_parent = ecs.get::<ItemSlot>(left_ent).ok()?.parent;
    let left_equipped = {
        ecs.get::<InventoryWindow>(left_parent)
            .unwrap_or_else(|_| {
                panic!(
                    "ItemSlot[{:?}]'s parent[{:?}] has no inventory window!",
                    left_ent, left_parent
                )
            })
            .equipped_slot
            == left_ent
    };
    let right_parent = ecs.get::<ItemSlot>(right_ent).ok()?.parent;
    let right_equipped = {
        ecs.get::<InventoryWindow>(right_parent)
            .unwrap_or_else(|_| {
                panic!(
                    "ItemSlot[{:?}]'s parent[{:?}] has no inventory window!",
                    right_ent, right_parent
                )
            })
            .equipped_slot
            == right_ent
    };

    if left_equipped {
        ecs.get_mut::<InventoryWindow>(left_parent)
            .unwrap()
            .swap_in_out(right_ent, left_ent);
    }
    if right_equipped {
        ecs.get_mut::<InventoryWindow>(right_parent)
            .unwrap()
            .swap_in_out(left_ent, right_ent);
    }

    Some(())
}

pub fn inventory_events(world: &mut World, images: &mut graphics::images::ImageMap) {
    let ecs = &world.ecs;
    let l8r = &mut world.l8r;

    for (_, (items::InventoryInsert(inv_ent), item_appearance)) in
        &mut ecs.query::<(&items::InventoryInsert, &graphics::Appearance)>()
    {
        try_slot_insert(*inv_ent, item_appearance.kind.name(), ecs, l8r, images);
    }

    // reflecting the equipping of an item in the gui is as simple as swapping the positions of the slots.
    for (inv_ent, inv_equip) in &mut ecs.query::<&items::InventoryEquip>() {
        // NOTE: this could bug out if you like equipped an item while dragging a slot
        // around to drop it somewhere else? or not.

        let (swap_left, swap_right) = {
            let inv_window = match ecs.get::<InventoryWindow>(inv_ent) {
                Ok(w) => w,
                Err(_) => continue,
            };
            (
                match inv_equip
                    .0
                    .as_ref()
                    .and_then(|t| inv_window.find_item_slot(&ecs, t))
                    .map(|(ent, _)| ent)
                {
                    Some(ent) => ent,
                    None => continue,
                },
                inv_window.equipped_slot,
            )
        };

        try_swap_slot_ents(swap_left, swap_right, ecs, l8r);
    }

    for (inv_ent, (_, inv_window)) in
        &mut ecs.query::<(&items::InventoryConsumeEquipped, &mut InventoryWindow)>()
    {
        let equipped_ent = inv_window.equipped_slot;
        let slot = ecs.get::<ItemSlot>(equipped_ent).unwrap_or_else(|_| {
            panic!(
                "No ItemSlot component on equipped_slot[{:?}] on Inventory[{:?}]!",
                equipped_ent, inv_ent
            )
        });
        let mut counter = ecs
            .get_mut::<Counter>(slot.counter_ent)
            .unwrap_or_else(|_| {
                panic!(
                    "No Counter component on Inventory[{:?}]'s ItemSlot[{:?}]'s counter_ent[{:?}]",
                    inv_ent, equipped_ent, slot.counter_ent
                )
            });
        counter.0 -= 1;

        if counter.0 > 0 {
            l8r.insert_one(
                slot.counter_ent,
                counter.make_graphics_appearance(equipped_ent),
            );
        } else {
            l8r.remove_one::<graphics::Appearance>(slot.counter_ent);
            l8r.remove_one::<graphics::Appearance>(slot.icon_ent);
        }
    }
}

#[derive(Default)]
pub struct GuiState {
    last_mouse_down_pos: Option<Vec2>,
    dragging_ent: Option<Entity>,
}
impl GuiState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging_ent.is_some()
    }

    pub fn draggable_under(&self, mouse: Vector, world: &World) -> Option<Entity> {
        world
            .ecs
            .query::<(&Draggable, &graphics::Appearance, &PhysHandle)>()
            .iter()
            .filter_map(|(gui_ent, (_, appearance, &PhysHandle(h)))| {
                let iso = world.phys.collision_object(h)?.position();
                let rect = match appearance.kind {
                    graphics::AppearanceKind::Color { rectangle, .. } => rectangle,
                    _ => unreachable!(),
                };
                let size = rect.size.into_vector();

                // top left
                let tl = appearance.alignment.offset(&rect, world) + iso.translation.vector
                    - (size / 2.0);
                // bottom right
                let br = tl + size;

                if tl.x < mouse.x && mouse.x < br.x && tl.y < mouse.y && mouse.y < br.y {
                    Some((gui_ent, appearance.z_offset))
                } else {
                    None
                }
            })
            .filter(|(ent, _)| self.dragging_ent.map(|drag| drag != *ent).unwrap_or(true))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(e, _)| e)
    }

    pub fn update_draggable_under_mouse(
        &mut self,
        world: &mut World,
        draggable_under: Option<Entity>,
        mouse: &Mouse,
    ) {
        let ecs = &world.ecs;
        let l8r = &mut world.l8r;
        let phys = &mut world.phys;

        let mouse_down = mouse[MouseButton::Left].is_down();

        let drag_me = self.dragging_ent.filter(|_| mouse_down).or(draggable_under);

        if let (true, Some(entity)) = (mouse_down, drag_me) {
            let mouse_pos = mouse.pos().into_vector();

            if let Some(last) = self.last_mouse_down_pos {
                let PhysHandle(h) = *ecs.get_mut::<PhysHandle>(entity).unwrap();
                let obj = phys.get_mut(h).unwrap();

                let mut iso = obj.position().clone();
                let offset = last - iso.translation.vector;
                iso.translation.vector = mouse_pos - offset;
                obj.set_position(iso);
            }
            self.last_mouse_down_pos = Some(mouse_pos);
            self.dragging_ent = Some(entity);
        } else {
            // if they're releasing what they've been dragging over another entity,
            if let (Some(released_ent), Some(under_ent)) = (self.dragging_ent, draggable_under) {
                Self::handle_drag_drop(ecs, l8r, under_ent, released_ent);

                self.dragging_ent = None;
            }
            // if there isn't a second ent that we're dropping on top of, however,
            // the item slot was released over the void, we need to drop the items.
            else if let Some(_released_ent) = self.dragging_ent {
            }
            self.last_mouse_down_pos = None;
        };
    }

    fn handle_drag_drop(
        ecs: &hecs::World,
        l8r: &mut l8r::L8r<crate::World>,
        // the entity that is under what was being dragged, the ent in the "drop zone"
        drop_ent: Entity,
        // the entity that was being dragged and is now being released over something else.
        drag_ent: Entity,
    ) -> Option<()> {
        // if it was released over another item slot, we need to swap the slots.
        // anything else just zips the item slot back on home.

        // Returns true if the child_ent is the equipped slot in their parent InventoryWindow.
        let equipped = |child_ent, child_slot: &hecs::Ref<ItemSlot>| -> bool {
            let parent = child_slot.parent;

            let parent_inventory = ecs.get::<InventoryWindow>(parent).unwrap_or_else(|_| {
                panic!(
                    "ItemSlot[{:?}]'s parent[{:?}] has no inventory window!",
                    child_ent, parent
                )
            });

            parent_inventory.equipped_slot == child_ent
        };

        // both are slots, so they need to be swapped!
        if let (Ok(drop_slot), Ok(drag_slot)) =
            (ecs.get::<ItemSlot>(drop_ent), ecs.get::<ItemSlot>(drag_ent))
        {
            if equipped(drop_ent, &drop_slot) {
                l8r.insert_one(
                    drop_slot.parent,
                    items::InventoryEquip(drag_slot.item_name.clone()),
                );
            }
            if equipped(drag_ent, &drag_slot) {
                l8r.insert_one(
                    drag_slot.parent,
                    items::InventoryEquip(drop_slot.item_name.clone()),
                );
            }

            try_swap_slot_ents(drop_ent, drag_ent, ecs, l8r);
        }
        // if they were dropped on top of some other gui element, but that gui element isn't
        // an ItemSlot, we can just send the draggable back home.
        else if let Ok(docking) = ecs.get::<Docking>(drag_ent) {
            // send it back home
            docking.dock(drag_ent, l8r);
        }

        Some(())
    }
}
