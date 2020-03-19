use crate::phys::face_cursor::face_cursor;
use crate::phys::{aiming, collision, movement};
use l8r::L8r;
use ncollide2d::pipeline::CollisionGroups;
use quicksilver::input::Key;
use quicksilver::lifecycle::Window;

use crate::combat;
use crate::core::*;
use crate::farm;
use crate::graphics::{self, particle};
use crate::gui;
use crate::phys;
use crate::props;
use crate::tilemap;
use crate::Game;

use super::GameState;

pub fn farming_enter(game: &mut Game, _window: &mut Window) {
    let world = &mut game.world;

    let config = std::rc::Rc::clone(&world.config);

    let _player = config.spawn(world);

    let spear_leaflet = config
        .items
        .get("spear_leaflet")
        .expect("no spear_leaflet in config")
        .spawn(world);

    world.add_hitbox(
        spear_leaflet,
        Iso2::translation(15.0, 8.5),
        ncollide2d::shape::Cuboid::new(Vec2::repeat(0.5)),
        CollisionGroups::new()
            .with_membership(&[])
            .with_whitelist(&[]),
    );

    for i in 0..4 {
        props::spawn_fence(world, Iso2::translation(8.0 + 2.0 * i as f32, 5.25));
    }

    // Tilemap stuffs
    tilemap::build_map_entities(world, "farm");
    farm::build_planting_cursor_entity(world);
}

pub fn farming_exit(game: &mut Game, _window: &mut Window) {
    let world = &mut game.world;
    tilemap::unload_map_entities(world);
    props::despawn_props(world);
}
pub fn farming_update(game: &mut Game, window: &mut Window) -> Option<GameState> {
    let world = &mut game.world;
    let gui = &mut game.gui;

    #[cfg(feature = "hot-config")]
    world.config.reload(world);

    graphics::fade::fade(world);

    movement::movement(world, window);
    phys::velocity(world);
    phys::chase(world);
    collision::collision(world);

    farm::growing(world);

    let mouse = window.mouse();
    let draggable_under_mouse = gui.draggable_under(mouse.pos(), world);
    if draggable_under_mouse.is_some() || gui.is_dragging() {
        gui.update_draggable_under_mouse(world, draggable_under_mouse, &mouse);
    } else {
        aiming::aiming(world, window);
        farm::planting(world, window);
    }

    combat::hurtful_damage(world);
    combat::health::remove_out_of_health(world);

    face_cursor(world, &window.mouse());

    let scheduled_world_edits: Vec<_> = world.l8r.drain(..).collect();
    L8r::now(scheduled_world_edits, world);

    game.particle_manager.emit_particles(world);

    let scheduled_world_edits: Vec<_> = world.l8r.drain(..).collect();
    L8r::now(scheduled_world_edits, world);

    particle::death::death_particles(world);
    phys::collision::clear_dead_collision_objects(world);
    particle::death::clear_dead(world);

    gui::inventory_events(world, &mut game.images);
    crate::items::inventory_inserts(world);

    if window.keyboard()[Key::N].is_down() {
        Some(GameState::COMBAT)
    } else {
        None
    }
}
