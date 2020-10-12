use super::{Comp, InstanceConfig, InstanceKey, Tracker};
use crate::{world, Game};
use glam::Vec2;

/// Applies an action, then saves it.
fn do_save(game: &mut Game, cursor_pos: Vec2, mut a: Action) {
    a.apply(game, cursor_pos);
    game.instance_tracker.selector.stack.push(a);
}

/// Undoes an action, then saves it.
fn undo_save(game: &mut Game, cursor_pos: Vec2, mut a: Action) {
    a.unapply(game, cursor_pos);
    game.instance_tracker.selector.z_stack.push(a);
}

struct MouseLock {
    at: Vec2,
    pending_action: Action,
}

#[cfg(feature = "confui")]
#[derive(Default)]
pub struct Selector {
    /// Stack of actions, for Undo
    stack: Vec<Action>,

    /// Stack of actions, for Redo
    z_stack: Vec<Action>,

    /// Copy buffer,
    clipboard: Vec<(InstanceConfig, Vec2)>,

    state: State,
}

enum State {
    BoxSelect {
        select_start: Option<Vec2>,
    },
    Free {
        mouse_lock: Option<MouseLock>,
        select_sealed: bool,
    },
}
impl State {
    fn free() -> Self {
        State::Free {
            mouse_lock: None,
            select_sealed: false,
        }
    }
}
impl Default for State {
    fn default() -> Self {
        Self::free()
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    Select(hecs::Entity),
    Deselect(hecs::Entity),
    GroupSelect(Vec<hecs::Entity>),
    GroupDeselect(Vec<hecs::Entity>),
    Delete(Vec<(InstanceKey, InstanceConfig)>),
    Paste {
        id: usize,
        selected_before: Vec<hecs::Entity>,
        clipboard: Vec<(InstanceConfig, Vec2)>,
    },
    Move(Vec2),
    Smush {
        toward: Vec2,
        by: Vec2,
    },
}

#[derive(Copy, Clone, Debug)]
enum Step {
    Back,
    Forward,
}

impl Action {
    fn apply(&mut self, game: &mut Game, cursor_pos: Vec2) {
        self.walk(game, cursor_pos, Step::Forward);
    }

    fn unapply(&mut self, game: &mut Game, cursor_pos: Vec2) {
        self.walk(game, cursor_pos, Step::Back);
    }

    fn walk(&mut self, game: &mut Game, cursor_pos: Vec2, step: Step) {
        use Action::*;
        use Step::*;

        let Game {
            ecs,
            phys,
            config: world::Config { draw, prefab, .. },
            dead,
            instance_tracker: Tracker { spawned, .. },
            ..
        } = game;
        macro_rules! selected {
            () => {
                spawned.iter_mut().filter(|t| t.selected)
            };
        }

        macro_rules! move_pos {
            ( $e:ident, $($w:tt)* ) => { {
                for tag in selected!() {
                    if let Some(c) = ecs.get(tag.entity).ok().and_then(|h| phys.get_mut(*h)) {
                        let mut $e = *c.position();
                        $e = $($w)*;
                        c.set_position($e);

                        if let Some(ik) = tag.instance_key() {
                            let (mut found_pos, mut found_rot) = (false, false);
                            let iso = c.position();
                            let rot = iso.rotation.angle();
                            let pos = iso.translation.vector;
                            let comps = &mut prefab.instances[ik].comps;

                            for comp in comps.iter_mut() {
                                match comp {
                                    Comp::Position(v) => {
                                        *v = pos;
                                        found_pos = true;
                                    }
                                    Comp::Angle(x) => {
                                        *x = rot;
                                        found_rot = true;
                                    }
                                    _ => {}
                                }
                            }

                            if !found_pos {
                                comps.push(Comp::Position(pos));
                            }
                            if !found_rot {
                                comps.push(Comp::Angle(rot));
                            }
                        }
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
            GroupSelect(ents) => {
                for ent in ents {
                    Select(*ent).walk(game, cursor_pos, step);
                }
            }
            &mut Deselect(e) => {
                spawned
                    .iter_mut()
                    .find(|t| t.entity == e)
                    .map(|t| t.selected = matches!(step, Back));
            }
            GroupDeselect(ents) => {
                for ent in ents {
                    Deselect(*ent).walk(game, cursor_pos, step);
                }
            }
            Delete(delets) => match step {
                Forward => {
                    for (ik, _) in delets {
                        for tag in spawned.iter().filter(|t| t.instance_key() == Some(*ik)) {
                            dead.mark(tag.entity);
                        }
                        prefab.instances.remove(*ik);
                    }
                }
                Back => {
                    for (_, instance_config) in delets {
                        let ik = prefab.instances.insert(instance_config.clone());
                        spawned.push(prefab.spawn_config_instance(ecs, phys, draw, ik));
                    }
                }
            },
            Paste {
                id,
                selected_before,
                clipboard,
            } => match step {
                Forward => {
                    selected_before.clear();
                    selected_before.extend(selected!().map(|t| {
                        t.selected = false;
                        t.entity
                    }));

                    spawned.extend(clipboard.iter().map(|(instance, delta)| {
                        let ik = prefab.instances.insert({
                            let mut inst = instance.clone();
                            inst.comps.drain_filter(|c| matches!(c, Comp::Position(_)));
                            inst.comps.push(Comp::Position({
                                let (x, y) = (*delta + cursor_pos).into();
                                na::Vector2::new(x, y)
                            }));
                            inst
                        });
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

pub fn dev_ui(ui: &mut egui::Ui, game: &mut Game, cursor_pos: Vec2) {
    game.ignore_inputs.mouse = true;

    copy_paste(game, cursor_pos);
    undo_redo(game, cursor_pos);

    show_selected(&game.instance_tracker);

    let mut state = std::mem::take(&mut game.instance_tracker.selector.state);
    match &mut state {
        State::Free {
            select_sealed,
            mouse_lock,
        } => {
            use macroquad::*;

            add_selections(ui, game, cursor_pos, select_sealed);
            manage_selections(ui, game, cursor_pos, mouse_lock);

            if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::B) {
                state = State::BoxSelect { select_start: None };
            }
        }
        State::BoxSelect { select_start } => {
            if box_select(ui, game, cursor_pos, select_start) {
                state = State::free();
            }
        }
    };
    game.instance_tracker.selector.state = state;
}

/// Returns `true` to relinquish control.
fn box_select(
    ui: &mut egui::Ui,
    game: &mut Game,
    cursor_pos: Vec2,
    select_start: &mut Option<Vec2>,
) -> bool {
    use macroquad::*;
    use Action::*;

    if is_key_down(KeyCode::Escape) {
        return true;
    }

    match select_start {
        None => {
            draw_rectangle(cursor_pos.x(), cursor_pos.y(), -0.250, 0.018, RED);
            draw_rectangle(cursor_pos.x(), cursor_pos.y(), 0.250, 0.018, RED);
            draw_rectangle(cursor_pos.x(), cursor_pos.y(), 0.018, -0.250, RED);
            draw_rectangle(cursor_pos.x(), cursor_pos.y(), 0.018, 0.250, RED);

            if !ui.ctx().wants_mouse_input() && is_mouse_button_down(MouseButton::Left) {
                *select_start = Some(cursor_pos);
            }
        }
        Some(start) => {
            let min = start.min(cursor_pos);
            let size = start.max(cursor_pos) - min;
            draw_rectangle_lines(min.x(), min.y(), size.x(), size.y(), 0.1, RED);

            if !is_mouse_button_down(MouseButton::Left) {
                let mut over_ents: Vec<(bool, hecs::Entity)> = {
                    let Tracker {
                        scanner, spawned, ..
                    } = &game.instance_tracker;

                    scanner
                        .iter()
                        .filter(|&&(_, pos, _)| {
                            let delta = pos - min;
                            delta.abs().cmple(size).all() && delta.cmpge(Vec2::zero()).all()
                        })
                        .map(|&(ti, _, _)| (spawned[ti].selected, spawned[ti].entity))
                        .collect()
                };

                if over_ents.iter().any(|(selected, _)| *selected) {
                    do_save(
                        game,
                        cursor_pos,
                        GroupDeselect(
                            over_ents
                                .drain_filter(|(selected, _)| *selected)
                                .map(|(_, e)| e)
                                .collect(),
                        ),
                    );
                }

                if !over_ents.is_empty() {
                    do_save(
                        game,
                        cursor_pos,
                        GroupSelect(over_ents.drain(..).map(|(_, e)| e).collect()),
                    );
                }

                return true;
            }
        }
    }

    false
}

fn copy_paste(game: &mut Game, cursor_pos: Vec2) {
    use macroquad::*;

    fn selected_to_clipboard(
        Game {
            instance_tracker:
                Tracker {
                    selector,
                    scanner,
                    spawned,
                    ..
                },
            config: world::Config { prefab, .. },
            ..
        }: &mut Game,
        cursor_pos: Vec2,
    ) {
        selector.clipboard.clear();
        selector.clipboard.extend(
            scanner
                .iter()
                .filter(|&(t, _, _)| spawned[*t].selected)
                .map(|&(_, p, ik)| (prefab.instances[ik].clone(), p - cursor_pos)),
        );
    }

    if is_key_down(KeyCode::LeftControl) {
        if is_key_pressed(KeyCode::C) {
            selected_to_clipboard(game, cursor_pos);
        }

        if is_key_pressed(KeyCode::X) {
            selected_to_clipboard(game, cursor_pos);
            delete_selected(game, cursor_pos);
        }

        if is_key_pressed(KeyCode::V) {
            do_save(
                game,
                cursor_pos,
                Action::Paste {
                    id: game.config.prefab.pastes,
                    selected_before: vec![],
                    clipboard: game.instance_tracker.selector.clipboard.clone(),
                },
            )
        }
    }
}

fn undo_redo(game: &mut Game, cursor_pos: Vec2) {
    use macroquad::*;

    if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::Z) {
        if is_key_down(KeyCode::LeftShift) {
            if let Some(a) = game.instance_tracker.selector.z_stack.pop() {
                do_save(game, cursor_pos, a);
            }
        } else {
            if let Some(a) = game.instance_tracker.selector.stack.pop() {
                undo_save(game, cursor_pos, a);
            } else {
                glsp::eprn!("Nothing to undo!")
            }
        }
    }
}

fn add_selections(ui: &mut egui::Ui, game: &mut Game, cursor_pos: Vec2, select_sealed: &mut bool) {
    use macroquad::*;
    use Action::*;

    if let Some(&(tag_index, p, _)) = game
        .instance_tracker
        .scanner
        .first()
        .filter(|&&(_, p, _)| (p - cursor_pos).length_squared() < 0.04)
    {
        draw_circle_lines(p.x(), p.y(), 0.025, 0.025, RED);

        if !ui.ctx().wants_mouse_input()
            && is_mouse_button_down(MouseButton::Left)
            && !is_key_down(KeyCode::LeftShift)
            && !is_key_down(KeyCode::LeftControl)
        {
            if !*select_sealed {
                do_save(game, cursor_pos, {
                    let tag = &game.instance_tracker.spawned[tag_index];

                    if tag.selected {
                        Deselect(tag.entity)
                    } else {
                        Select(tag.entity)
                    }
                });
            }
            *select_sealed = true;
        } else {
            *select_sealed = false;
        }
    }

    if is_key_down(KeyCode::LeftControl) && is_key_down(KeyCode::A) {
        game.ignore_inputs.keyboard = true;
    }

    if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::A) {
        if game.instance_tracker.selected().count() == 0 {
            do_save(
                game,
                cursor_pos,
                GroupSelect(
                    game.instance_tracker
                        .spawned
                        .iter()
                        .map(|t| t.entity)
                        .collect(),
                ),
            );
        } else {
            do_save(
                game,
                cursor_pos,
                GroupDeselect(game.instance_tracker.selected().map(|t| t.entity).collect()),
            );
        }
    }
}

fn show_selected(
    Tracker {
        scanner, spawned, ..
    }: &Tracker,
) {
    use macroquad::*;

    for &(ti, p, _) in scanner {
        if spawned[ti].selected {
            draw_circle_lines(p.x(), p.y(), 0.05, 0.025, BLUE);
        }
    }
}

fn delete_selected(game: &mut Game, cursor_pos: Vec2) {
    do_save(
        game,
        cursor_pos,
        Action::Delete(
            game.instance_tracker
                .selected()
                .filter_map(|t| t.instance_key())
                .map(|k| (k, game.config.prefab.instances[k].clone()))
                .collect(),
        ),
    );
}

fn manage_selections(
    ui: &mut egui::Ui,
    game: &mut Game,
    cursor_pos: Vec2,
    mouse_lock: &mut Option<MouseLock>,
) -> Option<Vec2> {
    use macroquad::*;
    use Action::*;

    let (count, sum) = {
        let Tracker {
            scanner, spawned, ..
        } = &mut game.instance_tracker;
        scanner
            .iter()
            .filter(|(t, _, _)| spawned[*t].selected)
            .fold((0, Vec2::zero()), |(count, a), &(_, p, _)| {
                (count + 1, a + p)
            })
    };

    if count == 0 {
        return None;
    }
    let average = sum / count as f32;

    draw_rectangle(average.x(), average.y(), 0.350, 0.032, MAGENTA);
    draw_rectangle(average.x(), average.y(), 0.032, -0.350, ORANGE);

    if is_key_pressed(KeyCode::Backspace) {
        delete_selected(game, cursor_pos);
    }

    if !ui.ctx().wants_mouse_input() && is_mouse_button_down(MouseButton::Left) {
        if (average - cursor_pos).length_squared() < 0.04 && mouse_lock.is_none() {
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
                *mouse_lock = Some(MouseLock {
                    at: cursor_pos,
                    pending_action,
                });
            }
        }
    } else if let Some(lock) = mouse_lock.take() {
        game.instance_tracker
            .selector
            .stack
            .push(lock.pending_action);
    }

    if let Some(lock) = mouse_lock {
        lock.pending_action.unapply(game, cursor_pos);
        let delta = cursor_pos - lock.at;
        match &mut lock.pending_action {
            Move(by) => *by = delta,
            Smush { by, .. } => *by = delta,
            _ => unreachable!(),
        };
        lock.pending_action.apply(game, cursor_pos);
    }

    Some(average)
}
