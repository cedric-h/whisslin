use super::{comp::spawn_comps, Comp, Config, InstanceConfig, InstanceKey, PrefabKey};
use crate::{draw, phys, world, Game};

mod tracker;
pub use tracker::{InstanceSource, Tag, Tracker};

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
        let &InstanceConfig {
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
        Tag::new(prefab_key, source, entity)
    }
}

#[cfg(feature = "confui")]
pub fn clear_prefab_instances(
    Game {
        instance_tracker: tracker::Tracker { spawned, .. },
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
            tracker::overview_ui(ui, world);
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
                let instance_key = prefab.instances.insert(InstanceConfig {
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

/// Stops recording entities that are marked "dead"
pub fn clear_dead(
    Game {
        dead,
        instance_tracker: trk,
        ..
    }: &mut Game,
) {
    #[cfg(feature = "confui")]
    trk.spawned
        .drain_filter(|tag| !tag.killed && dead.is_marked(tag.entity));

    #[cfg(not(feature = "confui"))]
    trk.spawned.drain_filter(|tag| dead.is_marked(tag.entity));
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
