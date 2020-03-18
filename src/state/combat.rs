use super::GameState;
use crate::combat;
use crate::core::*;
use crate::farm;
use crate::graphics::{self, particle};
use crate::gui;
use crate::gui::InventoryWindow;
use crate::items;
use crate::phys;
use crate::phys::{aiming, collision, movement};
use crate::props;
use crate::tilemap;
use crate::Game;
use l8r::L8r;
use nalgebra as na;
use ncollide2d::pipeline::CollisionGroups;
use ncollide2d::shape::Cuboid;
use quicksilver::lifecycle::Window;

pub fn combat_enter(game: &mut Game, _window: &mut Window) {
    let world = &mut game.world;

    let config = std::rc::Rc::clone(&world.config);

    let world = &mut game.world;
    //
    let (player, (_player_loc, _)) = world
        .ecs
        .query::<(&PhysHandle, &InventoryWindow)>()
        .iter()
        .next()
        .unwrap();

    let player_loc = (|| {
        let h = *world.ecs.get::<PhysHandle>(player).ok()?;
        Some(
            world
                .phys
                .collision_object(h)?
                .position()
                .translation
                .vector,
        )
    })()
    .unwrap();

    const ENEMY_COUNT: usize = 4;
    for i in 0..ENEMY_COUNT {
        let angle = (std::f32::consts::PI * 2.0 / (ENEMY_COUNT as f32)) * (i as f32);
        let loc = player_loc + na::UnitComplex::from_angle(angle) * Vec2::repeat(5.0);
        let base_group = CollisionGroups::new().with_membership(&[collide::ENEMY]);
        let knock_back_not_collide = [collide::ENEMY, collide::PLAYER];

        let bread = world.ecs.spawn((
            graphics::Appearance {
                kind: graphics::AppearanceKind::image("sandwich"),
                alignment: graphics::Alignment::Center,
                ..Default::default()
            },
            combat::health::Health::new(10),
            combat::DamageReceivedParticleEmitters(vec![config.particles["blood_splash"].clone()]),
            particle::death::DeathParticleEmitters(
                vec![config.particles["arterial_spray"].clone()],
            ),
            phys::collision::RigidGroups(base_group.with_blacklist(&knock_back_not_collide)),
            phys::Charge::new(0.05),
            phys::LookChase::new(player, 0.025),
            phys::KnockBack {
                groups: base_group.with_whitelist(&knock_back_not_collide),
                force_decay: 0.75,
                force_magnitude: 0.2,
                use_force_direction: false,
                minimum_speed: None,
            },
        ));
        world.add_hitbox(
            bread,
            Iso2::new(loc, angle),
            Cuboid::new(Vec2::new(1.0, 1.0) / 2.0),
            base_group,
        );
    }
    tilemap::build_map_entities(world, "combat");
}
pub fn combat_exit(game: &mut Game, _window: &mut Window) {
    let world = &mut game.world;
    tilemap::unload_map_entities(world);
    props::despawn_props(world);
}
pub fn combat_update(game: &mut Game, window: &mut Window) -> Option<GameState> {
    let world = &mut game.world;
    let gui = &mut game.gui;

    #[cfg(feature = "hot-config")]
    world.config.reload(&mut world);

    graphics::fade::fade(world);

    movement::movement(world, window);
    phys::velocity(world);
    phys::chase(world);
    collision::collision(world);

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

    let scheduled_world_edits: Vec<_> = world.l8r.drain(..).collect();
    L8r::now(scheduled_world_edits, world);

    game.particle_manager.emit_particles(world);

    let scheduled_world_edits: Vec<_> = world.l8r.drain(..).collect();
    L8r::now(scheduled_world_edits, world);

    particle::death::death_particles(world);
    phys::collision::clear_dead_collision_objects(world);
    particle::death::clear_dead(world);

    gui::inventory_events(world, &mut game.images);
    items::inventory_inserts(world);

    None
}
