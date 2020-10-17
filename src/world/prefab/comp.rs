use crate::{
    combat, draw,
    phys::{self, PhysHandle},
    world::{self, script},
};
use std::fmt;

pub(super) fn spawn_comps(
    ecs: &mut hecs::World,
    phys: &mut phys::CollisionWorld,
    tag_bank: &mut script::TagBank,
    draw_config: &draw::Config,
    prefab: impl Iterator<Item = Comp>,
) -> hecs::Entity {
    use Comp::*;

    let mut b = hecs::EntityBuilder::new();

    let mut pm = PhysMake::default();
    let mut script_name: Option<String> = Default::default();
    let mut tags = vec![];
    let mut art = None;
    let mut z_offset = None;

    for comp in prefab {
        match comp {
            Art(ah) => art = Some(ah),
            ZOffset(z) => z_offset = Some(z),
            DeathAnimation(ah) => {
                b.add(draw::DeathAnimation::new(ah));
            }
            Tags(t) => tags.extend(t),
            Health(amount) => {
                b.add(combat::Health::new(amount));
            }
            Position(_) | Angle(_) | Collision(_) | Hitbox(_) => pm.apply_comp(&comp),
            Script(name) => script_name = Some(name),
        }
    }

    if let Some(ah) = art {
        let mut looks = draw::Looks::art(ah);
        if let Some(z) = z_offset {
            looks.z_offset = z;
        }
        b.add(looks);
        if draw_config.get(ah).spritesheet.is_some() {
            b.add(draw::AnimationFrame(0));
        }
    }

    let e = ecs.spawn(b.build());

    let _ = pm.build(ecs, phys, e);

    if let Some(name) = script_name {
        glsp::lib_mut::<world::script::Intake>()
            .needs_script
            .push((e, name));
    }

    tag_bank.deposit(e, &tags);

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
        let (c_static, groups) = coll.into();

        if let Some(c_static) = c_static {
            if let Err(e) = ecs.insert_one(e, c_static) {
                glsp::eprn!("Couldn't add CollisionStatic: {}", e);
            }
        }

        Ok(phys::phys_insert(
            ecs,
            phys,
            e,
            na::Isometry2::new(pos, angle),
            phys::Cuboid::new(hb / 2.0),
            groups,
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

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq)]
pub enum Comp {
    Art(draw::ArtHandle),
    ZOffset(f32),
    DeathAnimation(draw::ArtHandle),
    Tags(Vec<(String, String)>),
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
            ZOffset(_) => "Z Offset",
            DeathAnimation(_) => "Death Animation",
            Tags(_) => "Tags",
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
            ZOffset(z) => {
                let pz = *z;
                ui.add(egui::DragValue::f32(z).speed(0.001));
                if pz != *z {
                    dirty = true
                }
            }
            DeathAnimation(ah) => return draw.select_handle_dev_ui(ui, ah),
            Health(hp_u) => {
                let mut hp = *hp_u as f32;
                ui.add(egui::DragValue::f32(&mut hp));
                if hp != *hp_u as f32 {
                    dirty = true;
                }
                *hp_u = hp as usize;
            }
            Tags(tags) => {
                let mut i = 0;
                tags.drain_filter(|(tag, val)| {
                    i += 1;
                    ui.horizontal(|ui| {
                        let tag_len_before = tag.len();
                        let tag_focus =
                            ui.add(egui::TextEdit::new(tag).id((i, "tag"))).has_kb_focus;
                        if tag_focus && tag_len_before != tag.len() {
                            dirty = true;
                        }

                        let val_len_before = val.len();
                        let val_focus =
                            ui.add(egui::TextEdit::new(val).id((i, "val"))).has_kb_focus;
                        if val_focus && val_len_before != val.len() {
                            dirty = true;
                        }

                        macroquad::is_key_pressed(macroquad::KeyCode::Backspace)
                            && tag_len_before == 0
                    })
                    .0
                });

                if ui.button("Add Tag").clicked {
                    tags.push(("Example".to_string(), String::new()));
                }
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
                ui.add(egui::DragValue::f32(a).speed(0.001));
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
                ZOffset(0.0),
                DeathAnimation(draw::ArtHandle::new_unchecked(0)),
                Tags(vec![]),
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
