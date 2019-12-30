use crate::graphics;
use crate::graphics::colors;
use crate::graphics::images::ImageMap;
use crate::phys::DragTowards;
use crate::World;
use crate::{Iso2, Vec2};
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
    fn dock(&self, docking_ent: Entity, l8r: &mut crate::L8r) {
        l8r.insert_one(docking_ent, DragTowards::new(self.home, self.speed));
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
}

type EntityAndSlot<'a> = (Entity, hecs::Ref<'a, ItemSlot>);

/// This component goes on the Entity which has the real crate::items::Inventory component,
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
                style: quicksilver::graphics::FontStyle::new(20.0, colors::DISCORD),
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
    images: &mut ImageMap,
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

pub fn build_inventory_gui_entities(world: &mut World) -> InventoryWindow {
    let ecs = &mut world.ecs;

    let window = ecs.spawn((
        Draggable,
        Iso2::translation(19.0, 1.0),
        graphics::Appearance {
            kind: graphics::AppearanceKind::Color {
                color: colors::DISCORD,
                rectangle: Rectangle::new_sized((10, 6)),
            },
            alignment: graphics::Alignment::TopLeft,
            z_offset: 100.0,
            ..Default::default()
        },
    ));

    // a thin line that cuts across the GUI, deliminating sections.
    let hr = |x: f32, y: f32| {
        (
            Iso2::translation(x, y - (0.125 / 2.0)),
            graphics::Appearance {
                kind: graphics::AppearanceKind::Color {
                    color: colors::LIGHT_SLATE_GRAY,
                    rectangle: Rectangle::new_sized((9, 0.125)),
                },
                alignment: graphics::Alignment::relative(window, graphics::Alignment::TopLeft),
                z_offset: 110.0,
                ..Default::default()
            },
        )
    };

    let slot = |x: f32, y: f32, icon_ent: Entity, counter_ent: Entity| {
        (
            Iso2::translation(x, y),
            Docking::new(Vec2::new(x, y), 0.4),
            ItemSlot {
                item_name: None,
                icon_ent,
                counter_ent,
            },
            Draggable,
            graphics::Appearance {
                kind: graphics::AppearanceKind::Color {
                    color: colors::LIGHT_SLATE_GRAY,
                    rectangle: Rectangle::new_sized((2, 1)),
                },
                alignment: graphics::Alignment::relative(window, graphics::Alignment::TopLeft),
                z_offset: 120.0,
                ..Default::default()
            },
        )
    };

    // these guys aren't actually given real appearances until an item
    // is put in the slots they are associated with.
    // position relative: slot top left
    let blank_icon = || (Iso2::translation(0.1, 0.1),);
    // position relative: slot center
    let blank_counter = || (Iso2::translation(1.4, 0.4), Counter(0));

    ecs.spawn(hr(0.5, 0.5));
    ecs.spawn(hr(0.5, 2.5));

    let equipped_slot = {
        let slot = slot(
            1.25,
            1.0,
            ecs.spawn(blank_icon()),
            ecs.spawn(blank_counter()),
        );
        ecs.spawn(slot)
    };

    let mut loose_slots = vec![];
    for y in 0..2 {
        for x in 0..3 {
            let slot = slot(
                3.0 * (x as f32) + 1.0,
                1.5 * (y as f32) + 3.0,
                ecs.spawn(blank_icon()),
                ecs.spawn(blank_counter()),
            );
            loose_slots.push(ecs.spawn(slot));
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
    l8r: &mut crate::L8r,
    images: &mut ImageMap,
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

pub fn inventory_events(world: &mut World, images: &mut ImageMap) {
    let ecs = &world.ecs;
    let l8r = &mut world.l8r;

    use crate::items;
    for (_, (items::InventoryInsert(inv_ent), item_appearance)) in
        &mut ecs.query::<(&items::InventoryInsert, &crate::graphics::Appearance)>()
    {
        try_slot_insert(*inv_ent, item_appearance.kind.name(), ecs, l8r, images);
    }

    // reflecting the equipping of an item in the gui is as simple as swapping the positions of the slots.
    for (_, (&items::InventoryEquip(equipped_type), inv_window)) in
        &mut ecs.query::<(&items::InventoryEquip, &mut InventoryWindow)>()
    {
        (|| {
            let new_equip_ent = dbg!(inv_window.find_item_slot(&ecs, equipped_type)?.0);
            let old_equip_ent = dbg!(inv_window.equipped_slot);

            // NOTE: this could bug out if you like equipped an item while dragging a slot
            // around to drop it somewhere else? or not.

            let old_equip_docking = *ecs.get::<Docking>(old_equip_ent).ok()?;
            let new_equip_docking = *ecs.get::<Docking>(new_equip_ent).ok()?;

            {
                let mut new_equip_docking = ecs.get_mut::<Docking>(new_equip_ent).ok()?;

                *new_equip_docking = old_equip_docking;
                new_equip_docking.dock(new_equip_ent, l8r);
            }

            {
                let mut old_equip_docking = ecs.get_mut::<Docking>(old_equip_ent).ok()?;

                *old_equip_docking = new_equip_docking;
                old_equip_docking.dock(old_equip_ent, l8r);
            }

            Some(())
        })();
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
            .query::<(&Draggable, &graphics::Appearance, &Iso2)>()
            .iter()
            .filter_map(|(gui_ent, (_, appearance, iso))| {
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
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(e, _)| e)
    }

    pub fn update_draggable_under_mouse(
        &mut self,
        world: &mut World,
        new_draggable: Option<Entity>,
        mouse: &Mouse,
    ) {
        let mouse_down = mouse[MouseButton::Left].is_down();

        let drag_me = self.dragging_ent.filter(|_| mouse_down).or(new_draggable);

        if let (true, Some(entity)) = (mouse_down, drag_me) {
            let mouse_pos = mouse.pos().into_vector();

            if let Some(last) = self.last_mouse_down_pos {
                let mut iso = world.ecs.get_mut::<Iso2>(entity).unwrap();
                let offset = last - iso.translation.vector;
                iso.translation.vector = mouse_pos - offset;
            }
            self.last_mouse_down_pos = Some(mouse_pos);
            self.dragging_ent = Some(entity);
        } else {
            // if they're releasing something they've been dragging,
            // we need to register an event for that so we can process it.
            if let Some(entity) = self.dragging_ent {
                if let Ok(docking) = world.ecs.get::<Docking>(entity) {
                    docking.dock(entity, &mut world.l8r);
                }
                self.dragging_ent = None;
            }
            self.last_mouse_down_pos = None;
        };
    }
}
