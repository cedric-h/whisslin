use super::{selector, Comp, InstanceKey, Popup, Tracker};
use crate::{world, Game};
use glam::Vec2;

pub fn overview_ui(ui: &mut egui::Ui, game: &mut Game) -> Option<()> {
    reset_ui(ui, game);

    let cursor_pos = {
        let Game {
            phys,
            player,
            config: world::Config { draw, .. },
            ..
        } = game;

        let player_iso = phys.collision_object(player.phys_handle)?.position();

        macroquad::set_camera(draw.camera({
            let mut i = player_iso.inverse();
            i.translation.vector.y += draw.camera_move;
            i
        }));

        let mouse = draw.mouse_world();
        let player = player_iso.translation.vector;
        mouse + Vec2::new(player.x, player.y)
    };

    scan(game, cursor_pos);

    selector::copy_paste(game, cursor_pos);

    let mut selector = std::mem::take(&mut game.instance_tracker.selector);
    selector::undo_redo(&mut selector, game, cursor_pos);
    selector::add_selections(ui, &mut selector, game, cursor_pos);
    selector::manage_selections(ui, &mut selector, game, cursor_pos);
    game.instance_tracker.selector = selector;

    show_selected(ui, game);
    recycle(game);

    Some(())
}

fn reset_ui(
    ui: &mut egui::Ui,
    Game {
        ecs,
        phys,
        instance_tracker: Tracker {
            resetting, spawned, ..
        },
        dead,
        config: world::Config { prefab, draw, .. },
        ..
    }: &mut Game,
) {
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
}

fn scan(
    Game {
        instance_tracker: Tracker {
            scanner, spawned, ..
        },
        ecs,
        phys,
        ..
    }: &mut Game,
    cursor_pos: Vec2,
) {
    scanner.extend(spawned.iter().enumerate().filter_map(|(i, t)| {
        Some((
            i,
            {
                let v = phys
                    .collision_object(*ecs.get(t.entity).ok()?)?
                    .position()
                    .translation
                    .vector;
                Vec2::new(v.x, v.y)
            },
            // We don't want to show developers instances spawned via scripts,
            // only ones that are part of the map. Devs should go to their scripts
            // to modify instances spawned via scripts.
            t.instance_key()?,
        ))
    }));

    scanner.sort_by(|&(_, a, _), &(_, b, _)| {
        let a_dist = (a - cursor_pos).length_squared();
        let b_dist = (b - cursor_pos).length_squared();

        a_dist
            .partial_cmp(&b_dist)
            .unwrap_or(std::cmp::Ordering::Greater)
    });
}

fn show_selected(
    ui: &mut egui::Ui,
    Game {
        dead,
        config: world::Config { prefab, draw, .. },
        instance_tracker:
            Tracker {
                spawned,
                scanner,
                popup,
                recycle_bin,
                ..
            },
        ..
    }: &mut Game,
) {
    if ui.button("Add Instance").clicked {
        if let Some(prefab_key) = prefab.fabs.keys().next() {
            *popup = Popup::AddInstance { prefab_key };
        }
    }

    let mut removal_key: Option<(InstanceKey, hecs::Entity)> = None;
    for (tag_index, _, instance_key) in scanner.drain(..) {
        let tag = &mut spawned[tag_index];
        if !tag.selected {
            continue;
        }
        ui.label(&prefab.fabs[tag.prefab_key].name);

        let mut dirty = false;
        let mut comp_removal_index: Option<usize> = None;
        let comps = match prefab.instances.get_mut(instance_key) {
            Some(i) => &mut i.comps,
            None => continue,
        };
        for (i, c) in comps.iter_mut().enumerate() {
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

    if let Some((key, entity)) = removal_key {
        dead.mark(entity);
        prefab.instances.remove(key);
    }
}

fn recycle(
    Game {
        ecs,
        phys,
        config: world::Config { prefab, draw, .. },
        instance_tracker:
            Tracker {
                recycle_bin,
                spawned,
                ..
            },
        ..
    }: &mut Game,
) {
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
}
