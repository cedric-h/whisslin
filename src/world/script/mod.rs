use super::Game;
use crate::{draw, world};
use glsp::prelude::*;

const DEFAULT_BEHAVIOR: &[u8] = compile!("src/world/script/default_behavior.glsp");

rdata! {
    #[derive(Copy, Clone, PartialEq)]
    pub struct V2(pub f32, pub f32);

    meths {
        "-": Self::sub,
        "+": Self::add,
        "/": Self::div,
        "*": Self::mul,
        get "y": Self::y,
        set "y": Self::set_y,
        get "x": Self::x,
        set "x": Self::set_x,
        "magn2": Self::magn2,
        "magn": Self::magn,
        "norm": Self::norm,
        "toward": Self::toward,
        "op-eq?": Self::op_eq,
    }
}
impl FromVal for V2 {
    fn from_val(val: &Val) -> GResult<Self> {
        if let Ok(x) = RRoot::<Self>::from_val(val) {
            return Ok(*x.borrow());
        }
        if let Ok(x) = Num::from_val(val) {
            return Ok(V2(x.into_f32(), x.into_f32()));
        }

        bail!("expected Vec2, received {}", val.a_type_name())
    }
}

impl V2 {
    fn add(&self, rest: &[Self]) -> Self {
        rest.iter()
            .fold(*self, |Self(x1, y1), Self(x2, y2)| Self(x1 + x2, y1 + y2))
    }

    fn sub(&self, rest: &[Self]) -> Self {
        if rest.len() == 0 {
            self.scale(-1.0)
        } else {
            rest.iter()
                .fold(*self, |Self(x1, y1), Self(x2, y2)| Self(x1 - x2, y1 - y2))
        }
    }

    fn mul(&self, rest: &[Self]) -> Self {
        rest.iter()
            .fold(*self, |Self(x1, y1), Self(x2, y2)| Self(x1 * x2, y1 * y2))
    }

    fn div(&self, rest: &[Self]) -> Self {
        rest.iter()
            .fold(*self, |Self(x1, y1), Self(x2, y2)| Self(x1 / x2, y1 / y2))
    }

    fn x(&mut self) -> f32 {
        self.0
    }

    fn set_x(&mut self, x: Num) {
        self.0 = x.into_f32();
    }

    fn y(&mut self) -> f32 {
        self.1
    }

    fn set_y(&mut self, x: Num) {
        self.1 = x.into_f32();
    }

    fn dot(&self, Self(x2, y2): Self) -> f32 {
        let Self(x1, y1) = *self;

        (x1 * x2) + (y1 * y2)
    }

    fn magn2(&self) -> f32 {
        self.dot(*self)
    }

    fn magn(&self) -> f32 {
        self.magn2().sqrt()
    }

    // more efficient scalar multiplication,
    // not exposed to glsp, used internally
    fn scale(self, scale: f32) -> Self {
        let Self(x1, y1) = self;
        Self(x1 * scale, y1 * scale)
    }

    // more efficient euler subtraction,
    // not exposed to glsp, used internally
    fn delta(self, Self(x2, y2): Self) -> Self {
        let Self(x1, y1) = self;
        Self(x1 - x2, y1 - y2)
    }

    fn norm(&self) -> V2 {
        self.scale(1.0 / self.magn())
    }

    fn toward(v: Self, x: Self, amnt: Option<Num>) -> Self {
        x.delta(v)
            .norm()
            .scale(amnt.map(|amt| amt.into_f32()).unwrap_or(1.0))
    }

    fn op_eq(&self, other: &Self) -> bool {
        self == other
    }
}

fn prefablib() -> GResult<()> {
    glsp::bind_rfn(
        "instances-of",
        rfn!(|prefab_name: Sym| -> GResult<_> {
            let Game {
                instance_tracker,
                config,
                ..
            } = &mut *Game::borrow_mut();
            let (pf_key, _) = config
                .prefab
                .by_name(&prefab_name.name())
                .ok_or_else(|| error!("no prefab with name {}", prefab_name))?;

            Ok(glsp::arr_from_iter(
                instance_tracker
                    .instances_of(pf_key)
                    .filter_map(|it| it.ent.as_ref()),
            ))
        }),
    )?;

    glsp::bind_rfn(
        "spawn-instance",
        rfn!(|prefab_name: Sym| -> GResult<RRoot<Ent>> {
            let Game {
                ecs,
                phys,
                instance_tracker,
                config,
                ..
            } = &mut *Game::borrow_mut();
            let (pf_key, _) = config
                .prefab
                .by_name(&prefab_name.name())
                .ok_or_else(|| error!("no prefab with name {}", prefab_name))?;

            instance_tracker
                .spawn_dynamic(ecs, phys, &config, pf_key, &vec![])
                .ent
                .ok_or_else(|| error!("Couldn't get Ent for newly spawned Instance"))
        }),
    )?;

    Ok(())
}

rdata! {
    /// A nice wrapper around a hecs::Entity for GameLisp to use.
    /// This assumes that the glsp::Runtime's Game Lib isn't being borrowed by anyone else.
    /// NOTE: Do not use it in contexts where that is not the case!
    #[derive(Copy, Clone, PartialEq)]
    pub struct Ent(pub hecs::Entity);

    meths {
        "move": Self::r#move,
        get "pos": Self::pos,
        set "pos": Self::set_pos,
        get "look-toward": Self::look_toward,
        set "look-toward": Self::set_look_toward,
        get "size": Self::size,
        set "size": Self::set_size,
        get "prefab": Self::prefab_name,
        get "anim-frame": Self::anim_frame,
        "kill": Self::kill,
        "message": Self::message,
        "op-eq?": Self::op_eq
    }
}
impl Ent {
    fn r#move(&self, V2(x, y): V2) -> GResult<()> {
        let Game { ecs, phys, .. } = &mut *Game::borrow_mut();

        ecs.get(self.0)
            .ok()
            .and_then(|h| phys.get_mut(*h))
            .map(|c| {
                let mut p = *c.position();
                p.translation.vector += na::Vector2::new(x, y);
                c.set_position(p);
            })
            .ok_or_else(|| error!("This Ent has no Position"))
    }

    fn set_pos(&self, V2(x, y): V2) -> GResult<()> {
        use world::prefab::{physical_from_comps, Comp};
        let p = na::Vector2::new(x, y);
        let Game {
            instance_tracker,
            config,
            ecs,
            phys,
            ..
        } = &mut *glsp::lib_mut();

        if let Some(c) = ecs.get(self.0).ok().and_then(|h| phys.get_mut(*h)) {
            c.set_position(na::Isometry2::new(p, 0.0));
        } else {
            let p = Comp::Position(p);
            let comps =
                std::iter::once(&p).chain(&self.prefab(&*config, &*instance_tracker)?.comps);

            physical_from_comps(ecs, phys, self.0, comps)
                .map_err(|e| error!("Couldn't make entity physical to set position: {}", e))?;
        }

        Ok(())
    }

    fn pos(&self) -> GResult<V2> {
        let Game { ecs, phys, .. } = &*glsp::lib();

        ecs.get(self.0)
            .ok()
            .and_then(|h| phys.collision_object(*h))
            .map(|c| c.position().translation.vector)
            .map(|v| V2(v.x, v.y))
            .ok_or_else(|| error!("This Ent has no position"))
    }

    fn prefab<'a>(
        &self,
        config: &'a world::Config,
        instance_tracker: &world::prefab::InstanceTracker,
    ) -> GResult<&'a world::prefab::PrefabConfig> {
        Ok(&config.prefab.fabs[instance_tracker
            .spawned
            .iter()
            .find(|x| x.entity == self.0)
            .map(|x| x.prefab_key)
            .ok_or_else(|| error!("This Ent has no prefab"))?])
    }

    fn prefab_name(&self) -> GResult<Sym> {
        let Game {
            instance_tracker,
            config,
            ..
        } = &*glsp::lib();
        glsp::sym(&self.prefab(config, instance_tracker)?.name)
    }

    fn message(&self, message: Val) {
        Intake::borrow_mut().messages.push((self.0, message));
    }

    fn kill(&self) {
        let Game { dead, .. } = &mut *glsp::lib_mut();
        dead.mark(self.0);
    }

    fn set_look_toward(&self, side: Sym) -> GResult<()> {
        let Game { ecs, .. } = &mut *glsp::lib_mut();
        let mut looks = ecs
            .get_mut::<draw::Looks>(self.0)
            .map_err(|e| error!("Couldn't get this Ent's looks: {}", e))?;

        looks.flip_x = if side == glsp::sym("left")? {
            true
        } else if side == glsp::sym("right")? {
            false
        } else {
            bail!("Expected either 'left or 'right")
        };

        Ok(())
    }

    fn look_toward(&self) -> GResult<Sym> {
        let Game { ecs, .. } = &*glsp::lib();
        let looks = ecs
            .get::<draw::Looks>(self.0)
            .map_err(|e| error!("Couldn't get this Ent's looks: {}", e))?;

        if looks.flip_x {
            glsp::sym("left")
        } else {
            glsp::sym("right")
        }
    }

    fn set_size(&self, num: Num) -> GResult<()> {
        let Game { ecs, .. } = &mut *glsp::lib_mut();
        let mut looks = ecs
            .get_mut::<draw::Looks>(self.0)
            .map_err(|e| error!("Couldn't get this Ent's looks: {}", e))?;

        looks.scale = num.into_f32();

        Ok(())
    }

    fn size(&self) -> GResult<f32> {
        let Game { ecs, .. } = &*glsp::lib();
        let looks = ecs
            .get::<draw::Looks>(self.0)
            .map_err(|e| error!("Couldn't get this Ent's looks: {}", e))?;

        Ok(looks.scale)
    }

    fn anim_frame(&self) -> GResult<usize> {
        let Game { ecs, config, .. } = &*glsp::lib();
        let mut q = ecs
            .query_one::<(&draw::AnimationFrame, &draw::Looks)>(self.0)
            .map_err(|e| error!("Couldn't borrow looks/animation frame: {}", e))?;
        let (af, looks) = q
            .get()
            .ok_or_else(|| error!("Couldn't get this Ent's looks and/or animation frame"))?;
        let ss = config
            .draw
            .get(looks.art)
            .spritesheet
            .ok_or_else(|| error!("This Ent isn't animated (no spritesheet)"))?;

        Ok(af.current_frame(ss))
    }

    fn op_eq(&self, other: &Self) -> bool {
        self == other
    }
}

syms! {
    pub struct Syms {
        update: "update",
        static_update: "static-update",
        collision: "collision",
        reload: "reload",
        on_message: "on-message",
        die: "die",
        init: "init",
    }
}

lib! {
    pub struct Intake {
        pub needs_script: Vec<(hecs::Entity, String)>,
        pub messages: Vec<(hecs::Entity, Val)>,
    }
}

impl Intake {
    pub fn new() -> Self {
        Self {
            needs_script: Vec::with_capacity(1000),
            messages: Vec::with_capacity(1000),
        }
    }
}

fn find_class<'a>(classes: &'a Vec<Root<Class>>, name: &str) -> Option<&'a Root<Class>> {
    classes
        .iter()
        .find(|c| c.name().filter(|n| *n.name() == *name).is_some())
}

fn class_name(class: &Root<Class>) -> String {
    class
        .name()
        .map(|n| n.name().to_string())
        .unwrap_or("Unknown".to_string())
}

fn log_meth_err(behavior: &Root<Obj>, meth: &Sym, r: GResult<Val>) {
    if let Err(e) = r {
        eprn!(
            "Couldn't call {} method on {} class: {}",
            meth,
            class_name(&behavior.class()),
            e
        )
    }
}
fn log_optional_meth_err(b: &Root<Obj>, m: &Sym, ro: GResult<Option<Val>>) {
    if let Some(r) = ro.transpose() {
        log_meth_err(b, m, r)
    }
}
fn log_optional_static_err(class: &Root<Class>, meth: &Sym, r: GResult<Option<Val>>) {
    if let Err(e) = r {
        eprn!("Couldn't call {} on {}: {}", meth, class_name(class), e);
    }
}

lib! {
    /// This struct is the bridge between when the Game is updating itself and
    /// when scripts are running, mutating the Game. These must be separated
    /// into separate phases so that the scripts can freely mutate the game
    /// without there being any open mutable references to the game.
    ///
    /// The Game can use this struct to insert new Scripts onto Entities,
    /// but it's not stored in Game so that Game doesn't have to be mutably borrowed when
    /// `init` is called on Scripts, so that `init` can mutate the world.
    pub struct Cache {
        syms: Syms,
        pub new_collisions: Vec<(hecs::Entity, hecs::Entity)>,
        classes: Vec<Root<Class>>,
        scripts: Vec<(Root<Obj>, RRoot<Ent>)>,
        intake: Intake,
    }
}

impl Cache {
    pub fn new(classes: &Val) -> GResult<Self> {
        prefablib()?;
        glsp::bind_rfn(
            "Vec2",
            rfn!(|x: Num, y: Num| V2(x.into_f32(), y.into_f32())),
        )?;

        Ok(Self {
            classes: FromVal::from_val(classes)?,
            syms: Syms::new().unwrap(),
            // optimistically assuming you aren't spawning more
            // than 1000 scripted entities in a single frame
            scripts: Vec::with_capacity(1000),
            new_collisions: Vec::with_capacity(1000),
            intake: Intake::new(),
        })
    }

    pub fn find_class<'a>(&'a self, name: &str) -> Option<&'a Root<Class>> {
        find_class(&self.classes, name)
    }

    /// This function should be called when hot-reloading occurs.
    #[cfg(feature = "confui")]
    pub fn reload(&mut self, new_classes_val: &Val) -> GResult<()> {
        let new_classes: Vec<Root<Class>> = FromVal::from_val(new_classes_val)?;
        let Self {
            classes,
            scripts,
            syms,
            ..
        } = self;

        for (behavior, ent) in scripts {
            let name = class_name(&behavior.class());

            let new_class = find_class(&new_classes, &name)
                .cloned()
                .unwrap_or_else(|| Self::default_behavior(&name));

            *behavior = match behavior.call_if_present(syms.reload, &(&ent, &new_class)) {
                Ok(Some(merged_behavior)) => merged_behavior,
                no_call => {
                    if let Err(e) = no_call {
                        eprn!("Couldn't reload {} class: {}", name, e);
                    }

                    glsp::call(&new_class, &[&ent]).unwrap_or_else(|e| {
                        eprn!("Couldn't make new {} class to reload: {}", name, e);
                        glsp::call(&Self::default_behavior(&name), &[&ent]).unwrap()
                    })
                }
            }
        }

        *classes = new_classes;

        Ok(())
    }

    pub fn default_behavior(missing_behavior: &str) -> Root<Class> {
        eprn!(
            "Couldn't find {}, had to use DefaultBehavior",
            missing_behavior
        );
        glsp::global("DefaultBehavior").unwrap_or_else(|_| {
            glsp::load_compiled(DEFAULT_BEHAVIOR).unwrap();
            glsp::global("DefaultBehavior").unwrap()
        })
    }

    pub fn update(&mut self) {
        std::mem::swap(&mut self.intake, &mut *Intake::borrow_mut());
        let Self {
            scripts,
            syms,
            classes,
            new_collisions,
            intake: Intake {
                needs_script,
                messages,
            },
            ..
        } = self;

        for class in classes.iter() {
            log_optional_static_err(
                class,
                &syms.static_update,
                class.call_if_present(&syms.static_update, &()),
            )
        }

        scripts.extend(needs_script.drain(..).filter_map(|(et, class_name)| {
            let class = find_class(&*classes, &class_name)
                .cloned()
                .unwrap_or_else(|| Self::default_behavior(&class_name));
            glsp::rroot(Ent(et))
                .and_then(|ent| Ok((glsp::call(&class, &(&ent,))?, ent)))
                .map_err(|e| eprn!("couldn't init behavior class: {}", e))
                .ok()
        }));

        for (behavior, ent) in scripts {
            let hecs_entity = ent.borrow().0;
            let ent = &*ent;

            for (_, message) in messages.iter().filter(|&&(e, _)| e == hecs_entity) {
                log_optional_meth_err(
                    behavior,
                    &syms.on_message,
                    behavior.call_if_present(&syms.on_message, &(ent, message)),
                );
            }

            for (_, collided_with) in new_collisions.iter().filter(|&&(e1, _)| e1 == hecs_entity) {
                log_optional_meth_err(
                    behavior,
                    &syms.collision,
                    behavior
                        .has_meth(&syms.collision)
                        .and_then(|has_collision| {
                            if has_collision {
                                let cw = glsp::rroot(Ent(*collided_with))?;
                                let _: Val = behavior.call(&syms.collision, &(ent, cw))?;
                            }
                            Ok(None)
                        }),
                );
            }

            log_meth_err(behavior, &syms.update, behavior.call(&syms.update, &[ent]));
        }

        new_collisions.clear();
        needs_script.clear();
        messages.clear();
    }

    pub fn cleanup(&mut self) {
        let Self { scripts, syms, .. } = self;

        for (behavior, ent) in
            scripts.drain_filter(|(_, e)| Game::borrow_mut().dead.is_marked(e.borrow().0))
        {
            log_optional_meth_err(
                &behavior,
                &syms.die,
                behavior.call_if_present(&syms.die, &[&ent]),
            );
            if let Err(e) = behavior.kill().and_then(|_| ent.free()) {
                eprn!(
                    "Couldn't kill {} behavior: {}",
                    class_name(&behavior.class()),
                    e
                );
            }
        }
    }
}
