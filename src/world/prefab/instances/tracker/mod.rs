use super::{Comp, InstanceConfig, InstanceKey, PrefabKey};
use crate::{phys, world};
use glam::Vec2;
#[cfg(feature = "confui")]
use super::Popup;

#[cfg(feature = "confui")]
mod overview_ui;
#[cfg(feature = "confui")]
pub use overview_ui::overview_ui;

#[cfg(feature = "confui")]
mod selector;

type ScannedTag = (usize, Vec2, InstanceKey);

#[derive(Default)]
/// Tracks all of the spawned prefab instances
/// so that we can reset them or clear them if need be.
pub struct Tracker {
    pub spawned: Vec<Tag>,

    #[cfg(feature = "confui")]
    selector: selector::Selector,

    #[cfg(feature = "confui")]
    /// Memory reserved for "instances near you" widget.
    scanner: Vec<ScannedTag>,

    #[cfg(feature = "confui")]
    /// Instances that need to be respawned when their old entities finally die.
    recycle_bin: Vec<InstanceKey>,

    #[cfg(feature = "confui")]
    pub(super) popup: Popup,

    #[cfg(feature = "confui")]
    resetting: bool,
}

impl Tracker {
    pub fn instances_of(&self, pf_key: PrefabKey) -> impl Iterator<Item = &Tag> {
        self.spawned.iter().filter(move |t| t.prefab_key == pf_key)
    }

    pub fn instances_of_mut(&mut self, pf_key: PrefabKey) -> impl Iterator<Item = &mut Tag> {
        self.spawned
            .iter_mut()
            .filter(move |t| t.prefab_key == pf_key)
    }

    /// Use this function to spawn Instances that aren't a part of the config.
    pub fn spawn_dynamic(
        &mut self,
        ecs: &mut hecs::World,
        phys: &mut phys::CollisionWorld,
        config: &world::Config,
        pf_key: PrefabKey,
        comps: &[Comp],
    ) -> Tag {
        let tag = config.prefab.spawn_instance(
            ecs,
            phys,
            &config.draw,
            pf_key,
            comps,
            InstanceSource::Dynamic,
        );
        self.spawned.push(tag.clone());
        tag
    }
}

#[derive(Clone)]
/// Contains all of the information necessary to keep tabs on a spawned prefab instance
pub struct Tag {
    /// Helps us keep track of if we need to recreate this Instance so it matches its Prefab.
    #[cfg(feature = "confui")]
    pub generation: usize,

    /// Helps us keep track of if we've started removing this Instance so we don't
    /// add it to the dead entity queue gratuitously
    #[cfg(feature = "confui")]
    pub killed: bool,

    #[cfg(feature = "confui")]
    pub selected: bool,

    pub prefab_key: PrefabKey,

    pub entity: hecs::Entity,

    /// Scripts frequently look up instances of prefabs,
    /// this helps us avoid allocating a new Ent for each entity each search.
    pub ent: Option<glsp::RRoot<world::script::Ent>>,

    paste: Option<usize>,

    source: InstanceSource,
}
impl Tag {
    pub fn new(prefab_key: PrefabKey, source: InstanceSource, entity: hecs::Entity) -> Tag {
        Tag {
            #[cfg(feature = "confui")]
            generation: 0,
            #[cfg(feature = "confui")]
            killed: false,
            #[cfg(feature = "confui")]
            selected: false,
            paste: None,
            prefab_key,
            source,
            ent: glsp::rroot(world::script::Ent(entity))
                .map_err(|e| glsp::eprn!("couldn't preallocate Ent: {}", e))
                .ok(),
            entity,
        }
    }

    pub fn instance_key(&self) -> Option<InstanceKey> {
        match self.source {
            InstanceSource::Config(k) => Some(k),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub enum InstanceSource {
    /// Instances spawned by scripts or otherwise not present in the Config.
    /// it prevents them from being saved with the "Save file" button or being
    /// shown in the developer UI.
    Dynamic,
    /// Instances recorded in and read from the Config.
    Config(InstanceKey),
}
