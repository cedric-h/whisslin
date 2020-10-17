use super::Game;
use crate::{draw, phys, world};
use glsp::prelude::*;

const DEFAULT_BEHAVIOR: &[u8] = compile!("src/world/script/default_behavior.glsp");

/// Scripts use Tags to find specific entities.
pub type TagEnts = smallvec::SmallVec<[(RRoot<Ent>, Option<Sym>); 64]>;
pub type EntTags = smallvec::SmallVec<[(Sym, Option<Sym>); 64]>;
pub struct TagBank {
    tags: fxhash::FxHashMap<Sym, TagEnts>,
    ents: fxhash::FxHashMap<hecs::Entity, EntTags>,
}
impl TagBank {
    pub fn new() -> Self {
        use {fxhash::FxBuildHasher, std::collections::HashMap};
        Self {
            tags: HashMap::with_capacity_and_hasher(1000, FxBuildHasher::default()),
            ents: HashMap::with_capacity_and_hasher(1000, FxBuildHasher::default()),
        }
    }

    pub fn deposit(&mut self, et: hecs::Entity, tags: &[(String, String)]) {
        let ent = match glsp::rroot(Ent(et)) {
            Ok(o) => o,
            Err(e) => {
                eprn!("Couldn't preallocate Ent for Tag: {}", e);
                return;
            }
        };

        for (strtag, strval) in tags {
            macro_rules! symmify {
                ( $s:ident ) => {
                    match glsp::sym($s) {
                        Ok(x) => x,
                        Err(e) => {
                            eprn!("Couldn't symmify {}: {}", $s, e);
                            continue;
                        }
                    }
                };
            }

            if strtag.len() == 0 {
                continue;
            }
            let tag = symmify!(strtag);
            let val = match strval.len() {
                0 => None,
                _ => Some(symmify!(strval)),
            };

            self.tags.entry(tag).or_default().push((ent.clone(), val));
            self.ents.entry(et).or_default().push((tag, val));
        }
    }
}

pub fn cleanup_tags(Game { tag_bank, dead, .. }: &mut Game) {
    let TagBank { ents, tags, .. } = tag_bank;

    dead.marks()
        .filter_map(|e| Some((e, ents.remove(&e)?)))
        .flat_map(|(e, e_tags)| e_tags.into_iter().map(move |(e_tag, _)| (e, e_tag)))
        .for_each(|(e, e_tag)| {
            tags.get_mut(&e_tag)
                .map(|t| t.retain(|(t, _)| t.borrow().0 != e));
        })
}

rdata! {
    #[derive(Copy, Clone, PartialEq, Debug)]
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
        "lerp": Self::lerp,
        "slerp": Self::slerp,
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

    fn lerp(&mut self, Self(x2, y2): Self, tn: Num) -> Self {
        let t = tn.into_f32();
        let Self(x1, y1) = *self;
        fn lerp(x: f32, y: f32, t: f32) -> f32 {
            x + ((y - x) * t)
        }
        Self(lerp(x1, x2, t), lerp(y1, y2, t))
    }

    fn slerp(&self, q0: Self, tn: Num) -> Self {
        const MU: f32 = 1.85298109240830;
        const U: [f32; 8] = [
            1.0 / (1.0 * 3.0),
            1.0 / (2.0 * 5.0),
            1.0 / (3.0 * 7.0),
            1.0 / (4.0 * 9.0),
            1.0 / (5.0 * 11.0),
            1.0 / (6.0 * 13.0),
            1.0 / (7.0 * 15.0),
            MU / (8.0 * 17.0),
        ];
        const V: [f32; 8] = [
            1.0 / 3.0,
            2.0 / 5.0,
            3.0 / 7.0,
            4.0 / 9.0,
            5.0 / 11.0,
            6.0 / 13.0,
            7.0 / 15.0,
            MU * 8.0 / 17.0,
        ];

        let q1 = *self;
        let t = tn.into_f32();
        let xm1 = q0.dot(q1) - 1.0;
        let d = 1.0 - t;
        let t_pow2 = t * t;
        let d_pow2 = d * d;

        let mut ts = [0.0; 8];
        let mut ds = [0.0; 8];
        for i in (0..7).rev() {
            ts[i] = (U[i] * t_pow2 - V[i]) * xm1;
            ds[i] = (U[i] * d_pow2 - V[i]) * xm1;
        }

        let f0 = t
            * (1.0
                + ts[0]
                    * (1.0
                        + ts[1]
                            * (1.0
                                + ts[2]
                                    * (1.0
                                        + ts[3]
                                            * (1.0
                                                + ts[4]
                                                    * (1.0
                                                        + ts[5]
                                                            * (1.0 + ts[6] * (1.0 + ts[7]))))))));

        let f1 = d
            * (1.0
                + ds[0]
                    * (1.0
                        + ds[1]
                            * (1.0
                                + ds[2]
                                    * (1.0
                                        + ds[3]
                                            * (1.0
                                                + ds[4]
                                                    * (1.0
                                                        + ds[5]
                                                            * (1.0 + ds[6] * (1.0 + ds[7]))))))));

        q0.scale(f0).plus(q1.scale(f1))
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

    // more efficient euler subtraction,
    // not exposed to glsp, used internally
    fn plus(self, Self(x2, y2): Self) -> Self {
        let Self(x1, y1) = self;
        Self(x1 + x2, y1 + y2)
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
        "ent-tagged",
        rfn!(|tag: Sym| -> GResult<(RRoot<Ent>, Option<Sym>)> {
            let Game { tag_bank, .. } = &mut *Game::borrow_mut();
            let vault = tag_bank
                .tags
                .get(&tag)
                .ok_or_else(|| error!("No such tag {}", tag))?;
            match (vault.len(), vault.first()) {
                (0, _) => bail!("No ents tagged {}", tag),
                (1, Some(a)) => Ok(a.clone()),
                _ => bail!("More than one ent tagged {}", tag),
            }
        }),
    )?;
    glsp::bind_rfn(
        "all-tagged",
        rfn!(|tag: Sym| -> GResult<Root<Arr>> {
            let Game { tag_bank, .. } = &mut *Game::borrow_mut();
            match tag_bank.tags.get(&tag) {
                None => Ok(glsp::arr()),
                Some(vault) => glsp::arr_from_iter(vault.clone().iter()),
            }
        }),
    )?;
    glsp::bind_rfn(
        "all-tagged-with-val",
        rfn!(|tag: Sym, val: Sym| -> GResult<Root<Arr>> {
            let Game { tag_bank, .. } = &mut *Game::borrow_mut();
            match tag_bank.tags.get(&tag) {
                None => Ok(glsp::arr()),
                Some(vault) => glsp::arr_from_iter(
                    vault
                        .clone()
                        .iter()
                        .filter(|(_, v)| *v == Some(val))
                        .map(|(e, _)| e),
                ),
            }
        }),
    )?;
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
                tag_bank,
                config,
                ..
            } = &mut *Game::borrow_mut();
            let (pf_key, _) = config
                .prefab
                .by_name(&prefab_name.name())
                .ok_or_else(|| error!("no prefab with name {}", prefab_name))?;

            instance_tracker
                .spawn_dynamic(ecs, phys, tag_bank, &config, pf_key, &vec![])
                .ent
                .ok_or_else(|| error!("Couldn't get Ent for newly spawned Instance"))
        }),
    )?;

    Ok(())
}

#[test]
fn slerp() {
    use glam::Vec2;

    let q0 = V2(0.0, 1.0);
    let q1 = V2(1.0, 0.0);

    assert_eq!(q0.slerp(q1, Num::Flo(0.0)), q0);
    assert_eq!(q0.slerp(q1, Num::Flo(1.0)), q1);
    let V2(x, y) = q1.slerp(q0, Num::Flo(0.5));
    assert!(
        Vec2::new(x, y).abs_diff_eq(Vec2::one().normalize(), 0.005,),
        "Expected: ~({}, {}), got: {:?}",
        x,
        y,
        Vec2::one().normalize()
    );
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
        get "rot": Self::rot,
        set "rot": Self::set_rot,
        get "force": Self::force,
        set "force": Self::set_force,
        get "look-toward": Self::look_toward,
        set "look-toward": Self::set_look_toward,
        get "size": Self::size,
        set "size": Self::set_size,
        get "prefab": Self::prefab_name,
        get "anim-frame": Self::anim_frame,
        "toggle-collision-whitelist": Self::toggle_collision_whitelist,
        "tagval": Self::tagval,
        "has-tag": Self::has_tag,
        "kill": Self::kill,
        "message": Self::message,
        "op-eq?": Self::op_eq
    }
}

macro_rules! collider {
    ( $ecs:ident, $phys:ident, $($et:tt)* ) => {
        $ecs.get($($et)*)
            .ok()
            .and_then(|h| $phys.get_mut(*h))
            .ok_or_else(|| error!("This Entity has no position."))
    };
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
        let Game { ecs, phys, .. } = &mut *glsp::lib_mut();
        let v = collider!(ecs, phys, self.0)?.position().translation.vector;
        Ok(V2(v.x, v.y))
    }

    fn set_rot(&self, rot: f32) -> GResult<()> {
        let Game { ecs, phys, .. } = &mut *glsp::lib_mut();
        let c = collider!(ecs, phys, self.0)?;
        let v = c.position().translation.vector;
        c.set_position(na::Isometry2::new(v, rot));
        Ok(())
    }

    fn rot(&self) -> GResult<f32> {
        let Game { ecs, phys, .. } = &mut *glsp::lib_mut();
        Ok(collider!(ecs, phys, self.0)?.position().rotation.angle())
    }

    fn toggle_collision_whitelist(
        &self,
        collide: phys::Collide,
        desired_state: Option<bool>,
    ) -> GResult<bool> {
        let Game { ecs, phys, .. } = &mut *glsp::lib_mut();

        let c = collider!(ecs, phys, self.0)?;
        let mut groups = *c.collision_groups();
        let state = desired_state.unwrap_or_else(|| !groups.is_member_of(collide as usize));
        groups.modify_whitelist(collide as usize, state);
        c.set_collision_groups(groups);

        Ok(state)
    }

    fn force(&self) -> V2 {
        let Game { ecs, .. } = &*glsp::lib();
        let force = ecs.get::<phys::Force>(self.0);

        match force {
            Ok(f) => V2(f.vec.x, f.vec.y),
            Err(_) => V2(0.0, 0.0),
        }
    }

    fn set_force(&self, (V2(x, y), decay): (V2, f32)) -> GResult<()> {
        let Game { ecs, .. } = &mut *glsp::lib_mut();

        ecs.get_mut::<phys::Force>(self.0)
            .map(|mut f| f.vec = na::Vector2::new(x, y))
            .or_else(|_| {
                ecs.insert_one(
                    self.0,
                    phys::Force::new_no_clear(na::Vector2::new(x, y), decay),
                )
                .map_err(|e| error!("Couldn't set force on Ent {:#?}: {}", self.0, e))
            })
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

    fn tag(&self, tag: Sym) -> GResult<Option<Sym>> {
        let Game { tag_bank, .. } = &*glsp::lib();
        Ok(tag_bank
            .ents
            .get(&self.0)
            .ok_or_else(|| error!("No tags for this Ent!"))?
            .iter()
            .find(|(t, _)| *t == tag)
            .and_then(|(_, v)| v.clone()))
    }

    fn tagval(&self, tag: Sym) -> GResult<Sym> {
        self.tag(tag)?
            .ok_or_else(|| error!("Ent doesn't have this tag!"))
    }

    fn has_tag(&self, tag: Sym) -> GResult<bool> {
        Ok(self.tag(tag)?.is_some())
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
        message: "message",
        death: "death",
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

/// Calls a Glsp object, logging failure to eprn if it occurs.
macro_rules! call {
    ( $class:ident :: $meth:ident $( . $rest:ident )* ( $($arg:tt)* ) ) => {
        let meth = $meth$(.$rest )*;
        call! { $class, meth, $class.call_if_present(meth, &($($arg)*)) }
    };

    ( $inst:ident . $meth:ident $( . $rest:ident )* ( $($arg:tt)* ) ) => {
        let class = $inst.class();
        let meth = $meth$(.$rest )*;
        call! { class, meth, $inst.call_if_present(meth, &($($arg)*)) }
    };

    ( $class:ident, $meth:ident, $($ro:tt)* ) => {
        let ro: GResult<Option<Val>> = $($ro)*;
        if let Err(e) = ro {
            eprn!(
                "Couldn't call {} method on {} class: {}",
                $meth,
                class_name(&$class),
                e
            )
        }
    };
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
        glsp::bind_global("pi", std::f32::consts::PI)?;
        glsp::bind_rfn("lerp", rfn!(|x: Num, y: Num, t: Num| x + ((y - x) * t)))?;
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
            call! { class::syms.static_update() }
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
                call! { behavior.syms.message(ent, message) }
            }

            for (_, collided_with) in new_collisions.iter().filter(|&&(e1, _)| e1 == hecs_entity) {
                let class = behavior.class();
                let collision = &syms.collision;
                let ro = behavior
                    .has_meth(&syms.collision)
                    .and_then(|has_collision| {
                        if has_collision {
                            let cw = glsp::rroot(Ent(*collided_with))?;
                            let _: Val = behavior.call(&syms.collision, &(ent, cw))?;
                        }
                        Ok(None)
                    });
                call!(class, collision, ro);
            }

            call! { behavior.syms.update(ent,) }
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
            call! { behavior.syms.death(&ent,) }

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
