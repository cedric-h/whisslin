/// A measure of hit points.
///
/// Symbolically represents how close something is to dying.
///
/// If this value reaches 0, the Entity associated with it is deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(from = "usize")]
pub enum Health {
    Points(usize),
    Dead,
}
#[allow(dead_code)]
impl Health {
    pub fn new(val: usize) -> Self {
        Health::Points(val)
    }

    pub fn is_dead(&self) -> bool {
        match self {
            Health::Dead => true,
            _ => false,
        }
    }

    pub fn is_alive(&self) -> bool {
        !self.is_dead()
    }

    pub fn points(&self) -> Option<usize> {
        match self {
            Health::Points(val) => Some(*val),
            Health::Dead => None,
        }
    }

    pub fn points_ref(&self) -> Option<&usize> {
        match self {
            Health::Points(val) => Some(val),
            Health::Dead => None,
        }
    }

    pub fn points_mut(&mut self) -> Option<&mut usize> {
        match self {
            Health::Points(val) => Some(val),
            Health::Dead => None,
        }
    }
}
impl std::ops::Deref for Health {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        const NO_HEALTH: &'static usize = &0;
        self.points_ref().unwrap_or(NO_HEALTH)
    }
}
impl std::ops::SubAssign for Health {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}
impl std::ops::Sub for Health {
    type Output = Health;

    fn sub(self, other: Self) -> Self {
        self.checked_sub(*other)
            .filter(|&x| x != 0)
            .map(|x| Health::Points(x))
            .unwrap_or(Health::Dead)
    }
}
impl std::ops::Add for Health {
    type Output = Health;

    fn add(self, other: Self) -> Self {
        self.checked_add(*other)
            .filter(|&x| x != 0)
            .filter(|_| !self.is_dead())
            .map(|x| Health::Points(x))
            .unwrap_or(Health::Dead)
    }
}
impl std::ops::AddAssign for Health {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}
impl From<usize> for Health {
    fn from(num: usize) -> Self {
        match num {
            0 => Health::Dead,
            x => Health::Points(x)
        }
    }
}

#[test]
fn health_sub() {
    assert!((Health::Dead - Health::Dead).is_dead());
    assert!((Health::new(20) - Health::Dead).is_alive());

    let mut my_health = Health::new(10);

    assert!((my_health - Health::Dead).is_alive());

    assert!((my_health - Health::new(20)).is_dead());

    assert_eq!(my_health - Health::new(3), Health::new(7));

    my_health -= Health::new(std::usize::MAX);

    assert!(my_health.is_dead());

    assert!((my_health + Health::new(1)).is_dead());

    my_health = Health::new(5);

    my_health -= Health::new(1);
    assert_eq!(my_health, Health::new(4));

    my_health -= Health::new(4);
    assert!(my_health.is_dead());
}

#[test]
fn health_add() {
    assert!((Health::Dead + Health::Dead).is_dead());
    assert!((Health::new(20) + Health::Dead).is_alive());

    let mut my_health = Health::Dead;
    assert!(my_health.is_dead());

    assert!((my_health + Health::Dead).is_dead());
    assert!((my_health + Health::new(20)).is_dead());

    my_health += Health::new(std::usize::MAX);
    assert!(my_health.is_dead());

    my_health = Health::new(1);

    assert_eq!(my_health + Health::new(1), Health::new(2));

    my_health += Health::new(1);
    assert_eq!(my_health, Health::new(2));
}

#[test]
fn health_misc() {
    let my_health = Health::Dead;
    assert_eq!(*my_health, 0);

    assert!(*my_health < 4);
    assert!(*my_health < *Health::new(4));
}

/// Gives things with 0 health the Dead component.
pub fn remove_out_of_health(world: &mut crate::World) {
    let ecs = &world.ecs;
    let l8r = &mut world.l8r;

    for (ent, &health) in ecs.query::<&Health>().iter() {
        if health.is_dead() {
            l8r.insert_one(ent, crate::Dead);
        }
    }
}
