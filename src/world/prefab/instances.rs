use super::{comp::spawn_comps, Comp, Config, InstanceConfig, InstanceKey, PrefabKey};
use crate::{
    draw, phys,
    world::{self, Dead},
    Game,
};
#[cfg(feature = "confui")]
use glam::Vec2;

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
}

type ScannedTag = (usize, Vec2, InstanceKey);

#[derive(Default)]
/// Tracks all of the spawned prefab instances
/// so that we can reset them or clear them if need be.
pub struct Tracker {
    pub spawned: Vec<Tag>,

    #[cfg(feature = "confui")]
    selector: Selector,

    #[cfg(feature = "confui")]
    /// Memory reserved for "instances near you" widget.
    scanner: Vec<ScannedTag>,

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

struct MouseLock {
    at: Vec2,
    pending_action: Action,
}

#[cfg(feature = "confui")]
#[derive(Default)]
struct Selector {
    /// Stack of actions, for Undo
    stack: Vec<Action>,

    /// Stack of actions, for Redo
    z_stack: Vec<Action>,

    /// Copy buffer,
    clipboard: Vec<(InstanceConfig, Vec2)>,

    mouse_lock: Option<MouseLock>,

    select_sealed: bool,
}

#[derive(Clone, Debug)]
enum Action {
    Select(hecs::Entity),
    Paste {
        id: usize,
        selected_before: Vec<hecs::Entity>,
        clipboard: Vec<(InstanceConfig, Vec2)>,
    },
    Deselect(hecs::Entity),
    Move(Vec2),
    Smush {
        toward: Vec2,
        by: Vec2,
    },
}

enum Step {
    Back,
    Forward,
}

impl Action {
    fn drive(
        &mut self,
        ecs: &mut hecs::World,
        phys: &mut phys::CollisionWorld,
        prefab: &mut super::Config,
        draw: &mut draw::Config,
        cursor_pos: Vec2,
        dead: &mut Dead,
        spawned: &mut Vec<Tag>,
        step: Step,
    ) {
        use Action::*;
        use Step::*;

        macro_rules! move_pos {
            ( $e:ident, $($w:tt)* ) => { {
                for t in spawned.iter().filter(|t| t.selected) {
                    if let Some(c) = ecs.get(t.entity).ok().and_then(|h| phys.get_mut(*h)) {
                        let mut $e = *c.position();
                        $e = $($w)*;
                        c.set_position($e);
                    }
                }
            } }
        }

        macro_rules! move_trans {
            ( $p:ident, $($w:tt)* ) => { {
                move_pos!(c, {
                    let av = c.translation.vector;
                    let $p = Vec2::new(av.x, av.y);
                    let gv = $($w)*;
                    c.translation.vector.x = gv.x();
                    c.translation.vector.y = gv.y();
                    c
                })
            } }
        }

        match self {
            &mut Select(e) => {
                spawned
                    .iter_mut()
                    .find(|t| t.entity == e)
                    .map(|t| t.selected = matches!(step, Forward));
            }
            &mut Deselect(e) => {
                spawned
                    .iter_mut()
                    .find(|t| t.entity == e)
                    .map(|t| t.selected = matches!(step, Back));
            }
            Paste {
                id,
                selected_before,
                clipboard,
            } => match step {
                Forward => {
                    selected_before.clear();
                    for t in spawned.iter_mut() {
                        selected_before.push(t.entity);
                        t.selected = false;
                    }

                    spawned.extend(clipboard.iter().map(|(instance, delta)| {
                        let ik = prefab.instances.insert(instance.clone());
                        prefab.instances[ik].comps.push(Comp::Position({
                            let (x, y) = (*delta + cursor_pos).into();
                            na::Vector2::new(x, y)
                        }));
                        let mut tag = prefab.spawn_config_instance(ecs, phys, draw, ik);
                        tag.selected = true;
                        tag.paste = Some(*id);
                        tag
                    }));

                    prefab.pastes += 1;
                }
                Back => {
                    for (e, ik) in spawned.iter().filter_map(|t| {
                        t.paste.filter(|p| p == id)?;
                        Some((t.entity, t.instance_key()?))
                    }) {
                        dead.mark(e);
                        prefab.instances.remove(ik);
                    }

                    for e in selected_before.iter().copied() {
                        if let Some(t) = spawned.iter_mut().find(|t| t.entity == e) {
                            t.selected = true;
                        }
                    }
                }
            },
            &mut Move(by) => match step {
                Forward => move_trans!(p, p + by),
                Back => move_trans!(p, p - by),
            },
            &mut Smush { toward, by } => match step {
                Forward => move_trans!(p, p + (p - toward) * by),
                Back => move_trans!(p, (p + toward * by) / (Vec2::one() + by)),
            },
        }
    }
}

#[test]
fn smush() {
    let p = Vec2::new(10.0_f32, 1.0);
    let origin = Vec2::one();
    let scale = Vec2::one() * 0.1;

    let smush = p + (p - origin) * scale;
    let unsmush = (p + origin * scale) / (Vec2::one() + scale);

    assert_eq!(unsmush, p);
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
                selector,
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
    use macroquad::*;

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

    let cursor_pos = {
        let mouse = draw.mouse_world();
        let player = phys
            .collision_object(player.phys_handle)?
            .position()
            .translation
            .vector;

        mouse + Vec2::new(player.x, player.y)
    };
    scanner.sort_by(|&(_, a, _), &(_, b, _)| {
        let a_dist = (a - cursor_pos).length_squared();
        let b_dist = (b - cursor_pos).length_squared();

        a_dist
            .partial_cmp(&b_dist)
            .unwrap_or(std::cmp::Ordering::Greater)
    });

    /*
    set_default_camera();
    draw_text(
        &selector.stack.len().to_string(),
        100.0,
        100.0,
        100.0,
        BLACK,
    );
    for (i, m) in selector.stack.iter().enumerate() {
        use Action::*;
        draw_text(
            match m {
                Select(_) => "Select",
                Deselect(_) => "Deselect",
                Move(_) => "Move",
                Smush(_) => "Smush",
            },
            0.0,
            30.0 + 20.0 * i as f32,
            18.0,
            BLACK,
        );
    }*/

    set_camera(draw.camera({
        let mut i = phys
            .collision_object(player.phys_handle)?
            .position()
            .inverse();
        i.translation.vector.y += draw.camera_move;
        i
    }));

    macro_rules! drive {
        ( $t:expr, $m:ident ) => {{
            $m.drive(ecs, phys, prefab, draw, cursor_pos, dead, spawned, $t);
        }};
    }
    macro_rules! apply {
        ( $($w:tt)* ) => { {
            #[allow(unused_mut)]
            let mut m = $($w)*;
            drive!(Step::Forward, m);
            m
        } }
    }
    macro_rules! unapply {
        ( $($w:tt)* ) => { {
            #[allow(unused_mut)]
            let mut m = $($w)*;
            drive!(Step::Back, m);
            m
        } }
    }

    if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::C) {
        selector.clipboard.clear();
        selector.clipboard.extend(
            scanner
                .iter()
                .filter(|&(t, _, _)| spawned[*t].selected)
                .map(|&(_, p, ik)| (prefab.instances[ik].clone(), p - cursor_pos)),
        );
    }

    if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::V) {
        selector.stack.push(apply!(Action::Paste {
            id: prefab.pastes,
            selected_before: vec![],
            clipboard: selector.clipboard.clone()
        }));
    }

    if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::Z) {
        if is_key_down(KeyCode::LeftShift) {
            if let Some(m) = selector.z_stack.pop() {
                selector.stack.push(apply!(m));
            }
        } else {
            if let Some(m) = selector.stack.pop() {
                selector.z_stack.push(unapply!(m));
            } else {
                glsp::eprn!("Nothing to undo!")
            }
        }
    }

    if let Some(&(tag_index, p, _)) = scanner
        .first()
        .filter(|&&(_, p, _)| (p - cursor_pos).length_squared() < 0.04)
    {
        use Action::*;
        draw_circle_lines(p.x(), p.y(), 0.025, 0.025, RED);

        if !ui.ctx().wants_mouse_input()
            && is_mouse_button_down(MouseButton::Left)
            && !is_key_down(KeyCode::LeftShift)
            && !is_key_down(KeyCode::LeftControl)
        {
            if !selector.select_sealed {
                let tag = &spawned[tag_index];
                selector.stack.push(apply!(if tag.selected {
                    Deselect(tag.entity)
                } else {
                    Select(tag.entity)
                }));
            }
            selector.select_sealed = true;
        } else {
            selector.select_sealed = false;
        }
    }

    let (count, sum) = scanner
        .iter()
        .filter(|(t, _, _)| spawned[*t].selected)
        .fold((0, Vec2::zero()), |(count, a), &(_, p, _)| {
            draw_circle_lines(p.x(), p.y(), 0.05, 0.025, BLUE);
            (count + 1, a + p)
        });

    if count != 0 {
        use Action::*;
        let average = sum / count as f32;

        draw_rectangle(average.x(), average.y(), 0.350, 0.032, MAGENTA);
        draw_rectangle(average.x(), average.y(), 0.032, -0.350, ORANGE);

        if !ui.ctx().wants_mouse_input() && is_mouse_button_down(MouseButton::Left) {
            if (average - cursor_pos).length_squared() < 0.04 && selector.mouse_lock.is_none() {
                let action = if is_key_down(KeyCode::LeftShift) {
                    Some(Move(Vec2::zero()))
                } else if is_key_down(KeyCode::LeftControl) {
                    Some(Smush {
                        toward: cursor_pos,
                        by: Vec2::zero(),
                    })
                } else {
                    None
                };

                if let Some(pending_action) = action {
                    selector.mouse_lock = Some(MouseLock {
                        at: cursor_pos,
                        pending_action,
                    });
                }
            }
        } else if let Some(lock) = selector.mouse_lock.take() {
            selector.stack.push(lock.pending_action);
        }

        if let Some(lock) = &mut selector.mouse_lock {
            unapply!(&mut lock.pending_action);
            let delta = cursor_pos - lock.at;
            match &mut lock.pending_action {
                Move(by) => *by = delta,
                Smush { by, .. } => *by = delta,
                Paste { .. } | Select(_) | Deselect(_) => unreachable!(),
            };
            apply!(&mut lock.pending_action);
        }
    }

    let mut removal_key: Option<(InstanceKey, hecs::Entity)> = None;
    for (tag_index, _, instance_key) in scanner.drain(..) {
        let tag = &mut spawned[tag_index];
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
