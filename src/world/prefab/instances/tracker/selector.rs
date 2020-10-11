use super::{Comp, InstanceConfig, Tag, Tracker};
use crate::{world, Game};
use glam::Vec2;

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

    mouse_lock: Option<MouseLock>,

    select_sealed: bool,
}

#[derive(Clone, Debug)]
pub enum Action {
    Select(hecs::Entity),
    Deselect(hecs::Entity),
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

enum Step {
    Back,
    Forward,
}

impl Action {
    pub fn apply(&mut self, game: &mut Game, cursor_pos: Vec2) {
        self.drive(game, cursor_pos, Step::Forward);
    }

    pub fn unapply(&mut self, game: &mut Game, cursor_pos: Vec2) {
        self.drive(game, cursor_pos, Step::Back);
    }

    fn drive(
        &mut self,
        Game {
            ecs,
            phys,
            config: world::Config { draw, prefab, .. },
            dead,
            instance_tracker: Tracker { spawned, .. },
            ..
        }: &mut Game,
        cursor_pos: Vec2,
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

pub fn copy_paste(game: &mut Game, cursor_pos: Vec2) {
    use macroquad::*;

    if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::C) {
        let Game {
            instance_tracker:
                Tracker {
                    selector,
                    scanner,
                    spawned,
                    ..
                },
            config: world::Config { prefab, .. },
            ..
        } = game;

        selector.clipboard.clear();
        selector.clipboard.extend(
            scanner
                .iter()
                .filter(|&(t, _, _)| spawned[*t].selected)
                .map(|&(_, p, ik)| (prefab.instances[ik].clone(), p - cursor_pos)),
        );
    }

    if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::V) {
        let mut paste = Action::Paste {
            id: game.config.prefab.pastes,
            selected_before: vec![],
            clipboard: game.instance_tracker.selector.clipboard.clone(),
        };
        paste.apply(game, cursor_pos);
        game.instance_tracker.selector.stack.push(paste);
    }
}

pub fn undo_redo(selector: &mut Selector, game: &mut Game, cursor_pos: Vec2) {
    use macroquad::*;

    if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::Z) {
        if is_key_down(KeyCode::LeftShift) {
            if let Some(mut m) = selector.z_stack.pop() {
                m.apply(game, cursor_pos);
                selector.stack.push(m);
            }
        } else {
            if let Some(mut m) = selector.stack.pop() {
                m.unapply(game, cursor_pos);
                selector.z_stack.push(m);
            } else {
                glsp::eprn!("Nothing to undo!")
            }
        }
    }
}

pub fn add_selections(
    ui: &mut egui::Ui,
    selector: &mut Selector,
    game: &mut Game,
    cursor_pos: Vec2,
) {
    use macroquad::*;

    if let Some(&(tag_index, p, _)) = game
        .instance_tracker
        .scanner
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
                let &Tag {
                    selected, entity, ..
                } = &game.instance_tracker.spawned[tag_index];
                let mut s = if selected {
                    Deselect(entity)
                } else {
                    Select(entity)
                };
                s.apply(game, cursor_pos);
                selector.stack.push(s);
            }
            selector.select_sealed = true;
        } else {
            selector.select_sealed = false;
        }
    }
}

pub fn manage_selections(
    ui: &mut egui::Ui,
    selector: &mut Selector,
    game: &mut Game,
    cursor_pos: Vec2,
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
                draw_circle_lines(p.x(), p.y(), 0.05, 0.025, BLUE);
                (count + 1, a + p)
            })
    };

    if count == 0 {
        return None;
    }
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
