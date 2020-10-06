use crate::{
    combat, draw,
    phys::{self, PhysHandle},
    world,
};
use std::fmt;

pub(super) fn spawn_comps(
    ecs: &mut hecs::World,
    phys: &mut phys::CollisionWorld,
    draw_config: &draw::Config,
    prefab: impl Iterator<Item = Comp>,
) -> hecs::Entity {
    use Comp::*;

    let mut b = hecs::EntityBuilder::new();

    let mut pm = PhysMake::default();
    let mut script_name: Option<String> = Default::default();

    for comp in prefab {
        match comp {
            Art(ah) => {
                b.add(draw::Looks::art(ah));
                if draw_config.get(ah).spritesheet.is_some() {
                    b.add(draw::AnimationFrame(0));
                }
            }
            DeathAnimation(ah) => {
                b.add(draw::DeathAnimation::new(ah));
            }
            Health(amount) => {
                b.add(combat::Health::new(amount));
            }
            Position(_) | Angle(_) | Collision(_) | Hitbox(_) => pm.apply_comp(&comp),
            Script(name) => script_name = Some(name),
        }
    }

    let e = ecs.spawn(b.build());

    let _ = pm.build(ecs, phys, e);

    if let Some(name) = script_name {
        glsp::lib_mut::<world::script::Intake>()
            .needs_script
            .push((e, name));
    }

    e
}

#[derive(Default)]
struct PhysMake {
    position: Option<na::Vector2<f32>>,
    angle: Option<f32>,
    collision: Option<phys::Collisionship>,
    hitbox: Option<na::Vector2<f32>>,
}
impl PhysMake {
    fn apply_comp(&mut self, comp: &Comp) {
        use Comp::*;
        match comp {
            &Position(p) => self.position = Some(p),
            &Angle(a) => self.angle = Some(a),
            Collision(c) => self.collision = Some(c.clone()),
            &Hitbox(hb) => self.hitbox = Some(hb),
            _ => {}
        }
    }

    fn build(
        self,
        ecs: &mut hecs::World,
        phys: &mut phys::CollisionWorld,
        e: hecs::Entity,
    ) -> Result<PhysHandle, &'static str> {
        let pos = self.position.ok_or_else(|| "No Position")?;
        let coll = self.collision.ok_or_else(|| "No Collision Mask")?;
        let hb = self.hitbox.ok_or_else(|| "No Hitbox")?;
        let angle = self.angle.unwrap_or(0.0);

        Ok(phys::phys_insert(
            ecs,
            phys,
            e,
            na::Isometry2::new(pos, angle),
            phys::Cuboid::new(hb / 2.0),
            coll.into(),
        ))
    }
}

pub fn physical_from_comps<'a>(
    ecs: &mut hecs::World,
    phys: &mut phys::CollisionWorld,
    e: hecs::Entity,
    comps: impl Iterator<Item = &'a Comp>,
) -> Result<PhysHandle, &'static str> {
    comps
        .fold(PhysMake::default(), |mut pm, c| {
            pm.apply_comp(c);
            pm
        })
        .build(ecs, phys, e)
}

#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq)]
pub enum Comp {
    Art(draw::ArtHandle),
    DeathAnimation(draw::ArtHandle),
    Health(usize),
    Position(na::Vector2<f32>),
    Angle(f32),
    Collision(phys::Collisionship),
    Hitbox(na::Vector2<f32>),
    Script(String),
}
#[cfg(feature = "confui")]
impl fmt::Display for Comp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(feature = "confui")]
impl Comp {
    pub fn name(&self) -> &'static str {
        use Comp::*;
        match self {
            Art(_) => "Art",
            DeathAnimation(_) => "Death Animation",
            Health(_) => "Health",
            Position(_) => "Position",
            Angle(_) => "Angle",
            Collision(_) => "Collision",
            Hitbox(_) => "Hitbox",
            Script(_) => "Script",
        }
    }

    pub fn edit_dev_ui(&mut self, ui: &mut egui::Ui, draw: &mut draw::Config) -> bool {
        let mut dirty = false;

        use Comp::*;
        match self {
            Art(ah) => return draw.select_handle_dev_ui(ui, ah),
            DeathAnimation(ah) => return draw.select_handle_dev_ui(ui, ah),
            Health(hp_u) => {
                let mut hp = *hp_u as f32;
                ui.add(egui::DragValue::f32(&mut hp));
                if hp != *hp_u as f32 {
                    dirty = true;
                }
                *hp_u = hp as usize;
            }
            Position(p) => {
                ui.horizontal(|ui| {
                    let pp = *p;
                    ui.add(egui::DragValue::f32(&mut p.x).speed(0.001));
                    ui.add(egui::DragValue::f32(&mut p.y).speed(0.001));
                    if pp != *p {
                        dirty = true
                    }
                });
            }
            Angle(a) => {
                let pa = *a;
                ui.add(egui::DragValue::f32(a));
                if pa != *a {
                    dirty = true
                }
            }
            Collision(col) => return col.dev_ui(ui),
            Hitbox(hb) => {
                ui.horizontal(|ui| {
                    let phb = *hb;
                    ui.add(egui::DragValue::f32(&mut hb.x).speed(0.001));
                    ui.add(egui::DragValue::f32(&mut hb.y).speed(0.001));
                    if phb != *hb {
                        dirty = true
                    }
                });
            }
            Script(name) => {
                let before_len = name.len();
                ui.add(egui::TextEdit::new(name));
                match glsp::lib_mut::<world::script::Cache>().find_class(name) {
                    Some(_) => {
                        if before_len != name.len() {
                            dirty = true;
                        }
                        ui.label("Set!")
                    }
                    None => ui.label(format!(
                        "Using DefaultBehavior, couldn't find a {} class",
                        name
                    )),
                };
            }
        }

        dirty
    }

    pub fn select_dev_ui(&mut self, ui: &mut egui::Ui) {
        use Comp::*;

        let defaults = unsafe {
            [
                Art(draw::ArtHandle::new_unchecked(0)),
                DeathAnimation(draw::ArtHandle::new_unchecked(0)),
                Health(1),
                Position(na::zero()),
                Angle(0.0),
                Collision(phys::Collisionship::default()),
                Hitbox(na::zero()),
                Script("IntroSlime".to_string()),
            ]
        };

        for d in defaults.iter().cloned() {
            ui.radio_value(d.name(), self, d);
        }
    }
}
