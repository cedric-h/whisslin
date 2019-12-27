use hecs::{Entity, World};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoAppearance(Entity),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoAppearance(e) => write!(
                f,
                "You attempted to insert an item[{:?}], but it didn't have an appearance!",
                e
            ),
        }
    }
}

impl std::error::Error for Error {}

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

    pub fn new_with(items: &[Entity], world: &World) -> Result<Self, Error> {
        let mut s = Self::new();

        for item in items.iter() {
            s.insert(*item, world)?;
        }

        Ok(s)
    }

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
    }

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

    /// Gets the appearance to use for HashMap indexing
    pub fn insert(&mut self, ent: Entity, world: &World) -> Result<(), Error> {
        world
            .get::<crate::graphics::Appearance>(ent)
            .map(|appearance| self.insert_raw(appearance.kind.name().into(), ent))
            .map_err(|_| Error::NoAppearance(ent))
    }

    pub fn insert_raw(&mut self, key: String, val: hecs::Entity) {
        self.slots.entry(key).or_insert(vec![]).push(val);
    }
}
