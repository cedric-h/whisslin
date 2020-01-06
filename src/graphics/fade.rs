/// Removes something after a given amount of frames.
/// Optionally also begins fading the transparency to 0 after a certain amount of time.
#[derive(Debug, serde::Deserialize)]
pub struct Fade {
    pub duration: usize,
    pub fade_start: usize,
}

impl Fade {
    pub fn no_visual(duration: usize) -> Self {
        Fade {
            duration,
            fade_start: duration,
        }
    }
}

pub fn fade(world: &mut crate::World) {
    let l8r = &mut world.l8r;
    let ecs = &world.ecs;

    for (fading_ent, (fade, appearance)) in &mut ecs.query::<(&mut Fade, &mut super::Appearance)>()
    {
        fade.duration -= 1;

        if fade.fade_start > fade.duration {
            appearance.transparency = Some(fade.duration as f32 / fade.fade_start as f32);
        }

        if fade.duration == 0 {
            l8r.despawn(fading_ent);
        }
    }
}
