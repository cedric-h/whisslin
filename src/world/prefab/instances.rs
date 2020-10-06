#[cfg(feature = "confui")]
use super::Comp;
use super::{comp::spawn_comps, Config, InstanceKey, PrefabKey};
use crate::{draw, phys, world, Game};

impl Config {
    /// Returns Tag to be tracked in an InstanceTracker
    fn spawn_all_config_instances<'a>(
        &'a self,
        ecs: &'a mut hecs::World,
        phys: &'a mut phys::CollisionWorld,
        draw_config: &'a draw::Config,
    ) -> impl ExactSizeIterator<Item = Tag> + 'a {
        self.instances
            .iter()
            .map(move |(k, _)| self.spawn_config_instance(ecs, phys, draw_config, k))
    }

    fn spawn_config_instance(
        &self,
        ecs: &mut hecs::World,
        phys: &mut phys::CollisionWorld,
        draw_config: &draw::Config,
        instance_key: InstanceKey,
    ) -> Tag {
        let &super::InstanceConfig {
            prefab_key,
            ref comps,
        } = &self.instances[instance_key];
        self.spawn_instance(
            ecs,
            phys,
            draw_config,
            prefab_key,
            comps,
            InstanceSource::Config(instance_key),
        )
    }

    fn spawn_instance(
        &self,
        ecs: &mut hecs::World,
        phys: &mut phys::CollisionWorld,
        draw_config: &draw::Config,
        prefab_key: PrefabKey,
        comps: &[Comp],
        source: InstanceSource,
    ) -> Tag {
        let entity = spawn_comps(
            ecs,
            phys,
            draw_config,
            comps
                .iter()
                .chain(self.fabs[prefab_key].comps.iter())
                .cloned(),
        );
        Tag {
            generation: 0,
            killed: false,
            prefab_key,
            source,
            ent: glsp::rroot(world::script::Ent(entity))
                .map_err(|e| glsp::eprn!("couldn't preallocate Ent: {}", e))
                .ok(),
            entity,
        }
    }
}

#[derive(Default)]
/// Tracks all of the spawned prefab instances
/// so that we can reset them or clear them if need be.
pub struct Tracker {
    pub spawned: Vec<Tag>,
    #[cfg(feature = "confui")]
    /// Memory reserved for "instances near you" widget.
    scanner: Vec<(usize, na::Vector2<f32>, InstanceKey)>,
    #[cfg(feature = "confui")]
    /// Instances that need to be respawned when their old entities finally die.
    recycle_bin: Vec<InstanceKey>,
    #[cfg(feature = "confui")]
    popup: Popup,
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

#[cfg(feature = "confui")]
pub fn clear_prefab_instances(
    Game {
        instance_tracker: Tracker { spawned, .. },
        dead,
        ..
    }: &mut Game,
    key: PrefabKey,
) {
    for tag in spawned.drain_filter(|t| t.prefab_key == key) {
        dead.mark(tag.entity);
    }
}

#[cfg(feature = "confui")]
enum Popup {
    AddComp {
        instance_key: InstanceKey,
        comp: Comp,
    },
    AddInstance {
        prefab_key: PrefabKey,
    },
    /// No popups, show the overview!
    Clear,
}
#[cfg(feature = "confui")]
impl Default for Popup {
    fn default() -> Self {
        Popup::Clear
    }
}

#[cfg(feature = "confui")]
pub fn dev_ui(ui: &mut egui::Ui, world: &mut Game) -> Option<()> {
    use Popup::*;
    match &mut world.instance_tracker.popup {
        Clear => {
            overview_ui(ui, world);
            return Some(());
        }
        &mut AddComp {
            instance_key,
            ref mut comp,
        } => {
            let super::Config {
                instances, fabs, ..
            } = &mut world.config.prefab;

            ui.horizontal(|ui| {
                ui.label("Adding Comp to Instance of:");
                ui.label(&fabs[instances[instance_key].prefab_key].name);
            });
            comp.select_dev_ui(ui);
            if ui.button(format!("Add {}", comp)).clicked {
                instances[instance_key].comps.push(comp.clone());
                world.instance_tracker.popup = Clear;
            }
        }
        AddInstance { prefab_key } => {
            let Game {
                ecs,
                phys,
                player,
                config: crate::world::Config { prefab, .. },
                ..
            } = world;

            ui.label("Adding Instance...");
            for (key, pf) in prefab.fabs.iter() {
                ui.radio_value(pf.name.clone(), prefab_key, key);
            }
            if ui.button("Add").clicked {
                let instance_key = prefab.instances.insert(super::InstanceConfig {
                    prefab_key: *prefab_key,
                    comps: vec![Comp::Position({
                        fn y_only(mut v: na::Vector2<f32>) -> na::Vector2<f32> {
                            v.x = 0.0;
                            v
                        }

                        // find the foot of the player
                        let p_foot = {
                            let obj = phys.collision_object(player.phys_handle)?;
                            let player_pos = obj.position().translation.vector;
                            let &phys::Cuboid { half_extents, .. } = obj.shape().as_shape()?;
                            player_pos + y_only(half_extents)
                        };
                        // find the Instance's hitbox dimensions, if any
                        let half_extents = prefab.fabs[*prefab_key]
                            .comps
                            .iter()
                            .find_map(|c| match c {
                                &Comp::Hitbox(hb) => Some(y_only(hb)),
                                _ => None,
                            })
                            .unwrap_or_else(|| na::Vector2::zeros());

                        // put the object at the player's feet offset by the object's hitbox size
                        p_foot + half_extents
                    })],
                });
                world
                    .instance_tracker
                    .spawned
                    .push(prefab.spawn_config_instance(
                        ecs,
                        phys,
                        &world.config.draw,
                        instance_key,
                    ));
                world.instance_tracker.popup = Clear;
            }
        }
    }

    if ui.button("Back").clicked {
        world.instance_tracker.popup = Clear;
    }

    Some(())
}

#[cfg(feature = "confui")]
pub fn overview_ui(
    ui: &mut egui::Ui,
    Game {
        config: crate::world::Config { prefab, draw, .. },
        ecs,
        phys,
        instance_tracker:
            Tracker {
                spawned,
                scanner,
                recycle_bin,
                popup,
                resetting,
            },
        player,
        dead,
        ..
    }: &mut Game,
) -> Option<()> {
    if ui.button("Reset Instances").clicked {
        *resetting = true;
        for tag in &*spawned {
            dead.mark(tag.entity);
        }
    }

    if *resetting {
        if !spawned.iter().any(|t| ecs.contains(t.entity)) {
            spawned.clear();
            *resetting = false;
            spawned.extend(prefab.spawn_all_config_instances(ecs, phys, draw));
        }
    }

    if ui.button("Add Instance").clicked {
        if let Some(prefab_key) = prefab.fabs.keys().next() {
            *popup = Popup::AddInstance { prefab_key };
        }
    }

    scanner.extend(spawned.iter().enumerate().filter_map(|(i, t)| {
        Some((
            i,
            phys.collision_object(*ecs.get(t.entity).ok()?)?
                .position()
                .translation
                .vector,
            // We don't want to show developers instances spawned via scripts,
            // only ones that are part of the map. Devs should go to their scripts
            // to modify instances spawned via scripts.
            t.instance_key()?,
        ))
    }));

    let player_pos = phys
        .collision_object(player.phys_handle)?
        .position()
        .translation
        .vector;
    scanner.sort_by(|(_, a, _), (_, b, _)| {
        let a_dist = (a - player_pos).magnitude();
        let b_dist = (b - player_pos).magnitude();

        a_dist
            .partial_cmp(&b_dist)
            .unwrap_or(std::cmp::Ordering::Greater)
    });

    let mut removal_key: Option<(InstanceKey, hecs::Entity)> = None;
    for (tag_index, _, instance_key) in scanner.drain(..) {
        let tag = &mut spawned[tag_index];
        ui.label(&prefab.fabs[tag.prefab_key].name);

        let mut dirty = false;
        let mut comp_removal_index: Option<usize> = None;
        for (i, c) in prefab.instances[instance_key].comps.iter_mut().enumerate() {
            let comp_name = c.to_string();
            ui.collapsing(&comp_name, |ui| {
                dirty = dirty || c.edit_dev_ui(ui, draw);
                if ui.button(format!("Remove {}", comp_name)).clicked {
                    comp_removal_index = Some(i);
                }
            });
        }

        if let Some(i) = comp_removal_index {
            dirty = true;
            prefab.instances[instance_key].comps.remove(i);
        }

        if ui.button("Add Comp").clicked {
            *popup = Popup::AddComp {
                instance_key,
                comp: Comp::Health(1),
            };
        }

        if ui
            .button(format!(
                "Remove {} Instance",
                &prefab.fabs[tag.prefab_key].name
            ))
            .clicked
        {
            removal_key = Some((instance_key, tag.entity));
        }

        if dirty {
            // out with the old!
            tag.killed = true;
            dead.mark(tag.entity);
            recycle_bin.push(instance_key);
        }
    }

    recycle_bin.drain_filter(|instance_key| {
        if let Some(tag) = spawned
            .iter_mut()
            .find(|tag| tag.instance_key() == Some(*instance_key))
            .filter(|t| !ecs.contains(t.entity))
        {
            // in with the new!
            *tag = {
                let mut new_tag = prefab.spawn_config_instance(ecs, phys, draw, *instance_key);
                new_tag.generation = tag.generation;
                new_tag
            };
            true
        } else {
            false
        }
    });

    if let Some((key, entity)) = removal_key {
        dead.mark(entity);
        prefab.instances.remove(key);
    }

    Some(())
}

#[derive(Clone)]
#[allow(dead_code)]
/// Contains all of the information necessary to keep tabs on a spawned prefab instance
pub struct Tag {
    /// Helps us keep track of if we need to recreate this Instance so it matches its Prefab.
    pub generation: usize,
    /// Helps us keep track of if we've started removing this Instance so we don't
    /// add it to the dead entity queue gratuitously
    pub killed: bool,
    pub prefab_key: PrefabKey,
    pub entity: hecs::Entity,
    /// Scripts frequently look up instances of prefabs,
    /// this helps us avoid allocating a new Ent for each entity each search.
    pub ent: Option<glsp::RRoot<world::script::Ent>>,
    source: InstanceSource,
}
impl Tag {
    fn instance_key(&self) -> Option<InstanceKey> {
        match self.source {
            InstanceSource::Config(k) => Some(k),
            _ => None,
        }
    }
}
#[derive(Clone, Copy)]
enum InstanceSource {
    /// Instances spawned by scripts or otherwise not present in the Config.
    /// it prevents them from being saved with the "Save file" button or being
    /// shown in the developer UI.
    Dynamic,
    /// Instances recorded in and read from the Config.
    Config(InstanceKey),
}

/// Stops recording entities that are marked "dead"
pub fn clear_dead(
    Game {
        dead,
        instance_tracker,
        ..
    }: &mut Game,
) {
    instance_tracker
        .spawned
        .drain_filter(|tag| !tag.killed && dead.is_marked(tag.entity));
}

/// Respawns instances of prefabs that are marked "dirty"
#[cfg(feature = "confui")]
pub fn keep_fresh(
    Game {
        dead,
        ecs,
        phys,
        instance_tracker,
        config,
        ..
    }: &mut Game,
) {
    // find all dirty prefabs
    config
        .prefab
        .fabs
        .iter()
        .filter(|(_, pf)| pf.dirty)
        // respawn each instance of this prefab the tracker knows about
        // and is out of sync with our prefab
        .filter(|(pf_key, pf)| {
            instance_tracker
                .instances_of_mut(*pf_key)
                .filter(|t| t.generation != pf.generation)
                // for now we can only reload entities stored in the actual config
                .filter_map(|t| Some((t.instance_key()?, t)))
                .all(|(instance_key, t)| {
                    if !t.killed {
                        t.killed = true;
                        // out with the old!
                        dead.mark(t.entity);
                    } else if !ecs.contains(t.entity) {
                        // in with the new!
                        *t = config.prefab.spawn_config_instance(
                            ecs,
                            phys,
                            &config.draw,
                            instance_key,
                        );
                        t.generation = pf.generation;
                        return true;
                    }
                    false
                })
        })
        .map(|(i, _)| i)
        .next()
        .map(|i| config.prefab.fabs[i].dirty = false);
}

pub fn spawn_all_instances(
    Game {
        phys,
        ecs,
        instance_tracker,
        config: world::Config { draw, prefab, .. },
        ..
    }: &mut Game,
) {
    instance_tracker
        .spawned
        .extend(prefab.spawn_all_config_instances(ecs, phys, draw));
}
