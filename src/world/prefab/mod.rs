use slotmap::SlotMap;

pub mod instances;
pub use instances::{spawn_all_instances, Tracker as InstanceTracker};

mod comp;
pub use comp::{physical_from_comps, Comp};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Config {
    instances: SlotMap<InstanceKey, InstanceConfig>,
    pub fabs: SlotMap<PrefabKey, PrefabConfig>,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    removal_key: Option<PrefabKey>,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    popup: Popup,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    pastes: usize,
}

impl Config {
    pub fn by_name(&self, name: &str) -> Option<(PrefabKey, &PrefabConfig)> {
        self.fabs.iter().find(|(_, pf)| pf.name == name)
    }
}

slotmap::new_key_type! { pub struct InstanceKey; }

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceConfig {
    prefab_key: PrefabKey,
    comps: Vec<Comp>,
}

slotmap::new_key_type! { pub struct PrefabKey; }

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefabConfig {
    pub name: String,
    pub comps: Vec<Comp>,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    start_delete: bool,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    sure_delete: bool,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    dirty: bool,
    #[cfg(feature = "confui")]
    #[serde(skip)]
    generation: usize,
}

/// A state machine modelling who has control of the Prefab window
#[cfg(feature = "confui")]
pub enum Popup {
    AddComp {
        prefab_key: PrefabKey,
        comp: Comp,
    },
    AddPrefab {
        name: String,
    },
    /// No popups!
    Clear,
}
#[cfg(feature = "confui")]
impl Default for Popup {
    fn default() -> Self {
        Popup::Clear
    }
}

#[cfg(feature = "confui")]
pub fn dev_ui(ui: &mut egui::Ui, config: &mut crate::world::Config) {
    let Config { popup, fabs, .. } = &mut config.prefab;

    use Popup::*;
    match popup {
        Clear => return overview_ui(ui, config),
        &mut AddComp {
            prefab_key,
            ref mut comp,
        } => {
            ui.horizontal(|ui| {
                ui.label("Adding Comp to:");
                ui.label(&fabs[prefab_key].name);
            });
            comp.select_dev_ui(ui);
            if ui.button(format!("Add {}", comp)).clicked {
                fabs[prefab_key].comps.push(comp.clone());
                *popup = Clear;
            }
        }
        AddPrefab { name } => {
            ui.label("Adding Prefab...");
            ui.add(egui::TextEdit::new(name));
            if ui.button("Add").clicked {
                fabs.insert(PrefabConfig {
                    name: std::mem::take(name),
                    ..Default::default()
                });
                *popup = Clear;
            }
        }
    }

    if ui.button("Back").clicked {
        config.prefab.popup = Clear;
    }
}

#[cfg(feature = "confui")]
pub fn overview_ui(
    ui: &mut egui::Ui,
    crate::world::Config {
        prefab:
            Config {
                fabs,
                removal_key,
                popup,
                ..
            },
        draw,
        ..
    }: &mut crate::world::Config,
) {
    if ui.button("Add Prefab").clicked {
        *popup = Popup::AddPrefab {
            name: "Vase".to_string(),
        };
    }

    let mut fab_keys: Vec<PrefabKey> = fabs.keys().collect();
    fab_keys.sort_by_key(|&k| &fabs[k].name);

    for prefab_key in fab_keys {
        let PrefabConfig {
            name,
            comps,
            sure_delete,
            start_delete,
            dirty,
            generation,
        } = &mut fabs[prefab_key];

        ui.collapsing(&*name, |ui| {
            if *dirty {
                return;
            }

            let mut comp_removal_key: Option<usize> = None;
            for (i, c) in comps.iter_mut().enumerate() {
                let comp_name = c.to_string();
                ui.collapsing(&comp_name, |ui| {
                    *dirty = *dirty || c.edit_dev_ui(ui, draw);
                    if ui.button(format!("Remove {} {}", name, comp_name)).clicked {
                        comp_removal_key = Some(i);
                    }
                });
            }

            if let Some(i) = comp_removal_key {
                *dirty = true;
                comps.remove(i);
            }

            if *dirty {
                *generation += 1;
            }

            if ui.button("Add Comp").clicked {
                *popup = Popup::AddComp {
                    prefab_key,
                    comp: Comp::Health(1),
                };
            }

            if ui.button(format!("Remove {}", name)).clicked {
                if *start_delete && *sure_delete {
                    *removal_key = Some(prefab_key);
                } else {
                    *start_delete = true;
                }
            }

            if *start_delete {
                ui.checkbox("Are you sure?", sure_delete)
                    .tooltip_text("Click Remove again after checking me");
            }
        });
    }
}

#[cfg(feature = "confui")]
pub fn clear_removed_prefabs(world: &mut crate::Game) {
    if let Some(key) = world.config.prefab.removal_key.take() {
        instances::clear_prefab_instances(world, key);
        world.config.prefab.fabs.remove(key);
    }
}
