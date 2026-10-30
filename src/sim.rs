use crate::presentation::Sfx;
// Fixed-tick native simulation. Physics are a new implementation, not yet cycle-exact.
use crate::{
    assets::{Bitmap, ObjectInfo},
    level::{HEIGHT, Level, WIDTH},
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State {
    Walk,
    Fall,
    Climb,
    Hoist,
    Shrug,
    OhNo,
    Explode,
    Splat,
    Drown,
    Burn,
    Exit,
    Float,
    Block,
    Build,
    Bash,
    Mine,
    Dig,
    Dead,
    Saved,
}
#[derive(Clone, Debug)]
pub struct Lem {
    pub x: i32,
    pub y: i32,
    pub dir: i32,
    pub state: State,
    pub age: u32,
    pub fall: i32,
    pub climber: bool,
    pub floater: bool,
    pub bomb: Option<u32>,
    pub bricks: u32,
}
#[derive(Clone)]
pub struct World {
    pub level: Level,
    pub terrain: Bitmap,
    pub lems: Vec<Lem>,
    pub tick: u32,
    pub released: usize,
    pub saved: usize,
    pub skills: [usize; 8],
    pub release: usize,
    pub events: Vec<Sfx>,
    pub info: Vec<ObjectInfo>,
    pub dirty: bool,
    pub nuke: bool,
}
impl World {
    pub fn new(level: Level, terrain: Bitmap, info: Vec<ObjectInfo>) -> Self {
        let skills = level.skills;
        let release = level.release;
        Self {
            level,
            terrain,
            lems: vec![],
            tick: 0,
            released: 0,
            saved: 0,
            skills,
            release,
            events: vec![],
            info,
            dirty: true,
            nuke: false,
        }
    }
    pub fn solid(&self, x: i32, y: i32) -> bool {
        if !(0..WIDTH as i32).contains(&x) || !(0..HEIGHT as i32).contains(&y) {
            return false;
        }
        self.terrain.rgba[(y as usize * WIDTH + x as usize) * 4 + 3] != 0
    }
    fn steel(&self, x: i32, y: i32) -> bool {
        self.level
            .steel
            .iter()
            .any(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
    }
    fn edit(&mut self, x: i32, y: i32, fill: bool) -> bool {
        if !(0..WIDTH as i32).contains(&x)
            || !(0..HEIGHT as i32).contains(&y)
            || (!fill && self.steel(x, y))
        {
            return false;
        }
        let o = (y as usize * WIDTH + x as usize) * 4;
        let was = self.terrain.rgba[o + 3] != 0;
        if fill {
            self.terrain.rgba[o..o + 4].copy_from_slice(&[204, 119, 0, 255]);
        } else {
            self.terrain.rgba[o..o + 4].fill(0);
        }
        self.dirty |= was != fill;
        was != fill
    }
    fn cut(&mut self, x: i32, y: i32, w: i32, h: i32) -> bool {
        let mut changed = false;
        for yy in y..y + h {
            for xx in x..x + w {
                changed |= self.edit(xx, yy, false);
            }
        }
        changed
    }
    fn hard_rect(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        (y..y + h).any(|yy| (x..x + w).any(|xx| self.steel(xx, yy)))
    }
    pub fn finished(&self) -> bool {
        self.tick >= self.level.minutes as u32 * 60 * 17
            || (self.released == self.level.total
                && self
                    .lems
                    .iter()
                    .all(|l| matches!(l.state, State::Dead | State::Saved)))
    }
    pub fn assign(&mut self, index: usize, skill: usize) -> bool {
        if skill >= 8 || self.skills[skill] == 0 || self.finished() {
            self.events.push(Sfx::Reject);
            return false;
        }
        let Some(l) = self.lems.get_mut(index) else {
            return false;
        };
        if matches!(
            l.state,
            State::Dead
                | State::Saved
                | State::OhNo
                | State::Explode
                | State::Splat
                | State::Drown
                | State::Burn
                | State::Exit
        ) {
            self.events.push(Sfx::Reject);
            return false;
        }
        let ok = match skill {
            0 => {
                if l.climber {
                    false
                } else {
                    l.climber = true;
                    true
                }
            }
            1 => {
                if l.floater {
                    false
                } else {
                    l.floater = true;
                    true
                }
            }
            2 => {
                if l.bomb.is_some() {
                    false
                } else {
                    l.bomb = Some(85);
                    true
                }
            }
            _ if matches!(
                l.state,
                State::Fall | State::Float | State::Climb | State::Hoist | State::Block
            ) =>
            {
                false
            }
            3 => {
                l.state = State::Block;
                l.age = 0;
                true
            }
            4 => {
                l.state = State::Build;
                l.bricks = 12;
                l.age = 0;
                true
            }
            5 => {
                l.state = State::Bash;
                l.age = 0;
                true
            }
            6 => {
                l.state = State::Mine;
                l.age = 0;
                true
            }
            7 => {
                l.state = State::Dig;
                l.age = 0;
                true
            }
            _ => false,
        };
        if ok {
            self.skills[skill] -= 1;
            self.events.push(Sfx::Assign);
        } else {
            self.events.push(Sfx::Reject);
        }
        ok
    }
    fn explode(&mut self, l: &mut Lem) {
        for y in -14i32..=10 {
            for x in -12i32..=12 {
                if x * x + y * y < 170 {
                    self.edit(l.x + x, l.y + y, false);
                }
            }
        }
        l.state = State::Explode;
        l.age = 0;
        l.bomb = None;
        self.events.push(Sfx::Explosion);
    }
    pub fn step(&mut self) {
        if self.finished() {
            return;
        }
        self.tick += 1;
        let interval = ((99 - self.release.min(99)) / 2 + 4) as u32;
        if !self.nuke
            && self.tick > 40
            && self.released < self.level.total
            && (self.tick - 41).is_multiple_of(interval)
        {
            let entrances = self
                .level
                .objects
                .iter()
                .filter(|o| o.id == 1)
                .collect::<Vec<_>>();
            if !entrances.is_empty() {
                let e = entrances[self.released % entrances.len()];
                self.lems.push(Lem {
                    x: e.x + 24,
                    y: e.y + 16,
                    dir: 1,
                    state: State::Fall,
                    age: 0,
                    fall: 0,
                    climber: false,
                    floater: false,
                    bomb: None,
                    bricks: 0,
                });
                self.released += 1;
                if self.released == 1 {
                    self.events.push(Sfx::LetsGo);
                }
            }
        }
        let blockers = self
            .lems
            .iter()
            .filter(|l| l.state == State::Block)
            .map(|l| (l.x, l.y))
            .collect::<Vec<_>>();
        for i in 0..self.lems.len() {
            let mut l = self.lems[i].clone();
            if matches!(l.state, State::Dead | State::Saved) {
                continue;
            }
            l.age += 1;
            if self.nuke
                && l.bomb.is_none()
                && !matches!(
                    l.state,
                    State::OhNo
                        | State::Explode
                        | State::Splat
                        | State::Drown
                        | State::Burn
                        | State::Exit
                )
            {
                l.bomb = Some(85 + i as u32 * 3);
            }
            if let Some(b) = l.bomb {
                if b == 0 {
                    l.bomb = None;
                    l.age = 0;
                    if matches!(l.state, State::Fall | State::Float) {
                        self.explode(&mut l);
                    } else {
                        l.state = State::OhNo;
                    }
                } else {
                    l.bomb = Some(b - 1);
                }
            }
            if matches!(
                l.state,
                State::OhNo
                    | State::Explode
                    | State::Splat
                    | State::Drown
                    | State::Burn
                    | State::Exit
            ) {
                match l.state {
                    State::OhNo => {
                        if l.age == 5 {
                            self.events.push(Sfx::OhNo);
                        }
                        if l.age >= 16 {
                            self.explode(&mut l);
                        }
                    }
                    State::Explode if l.age >= 4 => l.state = State::Dead,
                    State::Exit if l.age >= 8 => l.state = State::Saved,
                    State::Splat | State::Drown if l.age >= 16 => l.state = State::Dead,
                    State::Burn if l.age >= 14 => l.state = State::Dead,
                    _ => {}
                }
                self.lems[i] = l;
                continue;
            }
            if l.state != State::Dead {
                let ground = self.solid(l.x, l.y + 1);
                if !ground && matches!(l.state, State::Walk | State::Block | State::Bash) {
                    l.state = State::Fall;
                    l.fall = 0;
                    l.age = 0;
                }
                match l.state {
                    State::Walk => {
                        if blockers.iter().any(|(x, y)| {
                            (*x - l.x) * l.dir > 0 && (*x - l.x).abs() < 7 && (*y - l.y).abs() < 6
                        }) {
                            l.dir = -l.dir;
                        }
                        let mut rise = 0;
                        while rise < 7 && self.solid(l.x + l.dir, l.y - rise) {
                            rise += 1;
                        }
                        if rise >= 7 {
                            if l.climber {
                                l.state = State::Climb;
                                l.age = 0;
                            } else {
                                l.dir = -l.dir;
                            }
                        } else {
                            l.x += l.dir;
                            l.y -= rise;
                            let mut drop = 0;
                            while drop < 4 && !self.solid(l.x, l.y + 1) {
                                l.y += 1;
                                drop += 1;
                            }
                            if drop == 4 {
                                l.state = State::Fall;
                                l.fall = 4;
                                l.age = 0;
                            }
                        }
                    }
                    State::Fall | State::Float => {
                        if l.floater && l.fall > 16 && l.state != State::Float {
                            l.state = State::Float;
                            l.age = 0;
                        }
                        let speed = if l.state == State::Float {
                            if l.age.is_multiple_of(2) { 1 } else { 0 }
                        } else {
                            3
                        };
                        for _ in 0..speed {
                            if self.solid(l.x, l.y + 1) {
                                if l.fall > 60 && l.state != State::Float {
                                    l.state = State::Splat;
                                    l.age = 0;
                                    self.events.push(Sfx::Splat);
                                } else {
                                    l.state = State::Walk;
                                    l.age = 0;
                                }
                                break;
                            }
                            l.y += 1;
                            l.fall += 1;
                        }
                    }
                    State::Climb => {
                        if self.solid(l.x, l.y - 9) {
                            l.dir = -l.dir;
                            l.state = State::Fall;
                            l.fall = 0;
                        } else if !self.solid(l.x + l.dir, l.y - 7) {
                            l.state = State::Hoist;
                            l.age = 0;
                        } else {
                            l.y -= 1;
                        }
                    }
                    State::Hoist => {
                        if l.age < 5 {
                            l.y -= 2;
                        }
                        if l.age >= 8 {
                            l.x += l.dir * 2;
                            l.state = State::Walk;
                            l.age = 0;
                        }
                    }
                    State::Shrug => {
                        if l.age >= 8 {
                            l.state = State::Walk;
                            l.age = 0;
                        }
                    }
                    State::Build => {
                        if self.solid(l.x + l.dir * 3, l.y - 10) {
                            l.state = State::Walk;
                            l.age = 0;
                        } else {
                            if l.age % 16 == 9 {
                                for dx in 0..6 {
                                    self.edit(l.x + l.dir * dx, l.y - 1, true);
                                }
                            }
                            if l.age % 16 == 10 && l.bricks <= 3 {
                                self.events.push(Sfx::BuilderWarning);
                            }
                            if l.age.is_multiple_of(16) {
                                l.y -= 1;
                                l.x += l.dir * 2;
                                l.bricks = l.bricks.saturating_sub(1);
                                if l.bricks == 0 {
                                    l.state = State::Shrug;
                                    l.age = 0;
                                }
                            }
                        }
                    }
                    State::Dig => {
                        if l.age.is_multiple_of(8) {
                            let changed = self.cut(l.x - 4, l.y, 9, 2);
                            if changed {
                                l.y += 1;
                            } else if !self.solid(l.x, l.y + 1) {
                                l.state = State::Fall;
                                l.fall = 0;
                            } else {
                                if self.hard_rect(l.x - 4, l.y, 9, 2) {
                                    self.events.push(Sfx::Reject);
                                }
                                l.state = State::Walk;
                            }
                        }
                    }
                    State::Bash => {
                        if l.age.is_multiple_of(4) {
                            let x = if l.dir > 0 { l.x } else { l.x - 5 };
                            if self.cut(x, l.y - 8, 6, 9) {
                                l.x += l.dir * 2;
                            } else {
                                if self.hard_rect(x, l.y - 8, 6, 9) {
                                    self.events.push(Sfx::Reject);
                                }
                                l.state = State::Walk;
                            }
                        }
                    }
                    State::Mine if l.age.is_multiple_of(8) => {
                        let x = if l.dir > 0 { l.x - 1 } else { l.x - 8 };
                        if self.cut(x, l.y - 5, 10, 8) {
                            l.x += l.dir * 2;
                            l.y += 1;
                        } else {
                            if self.hard_rect(x, l.y - 5, 10, 8) {
                                self.events.push(Sfx::Reject);
                            }
                            l.state = State::Walk;
                        }
                    }
                    _ => {}
                }
                if l.y >= HEIGHT as i32 + 10 || l.x < 0 || l.x >= WIDTH as i32 {
                    l.state = State::Dead;
                    self.events.push(Sfx::FallOut);
                }
                if !matches!(l.state, State::Dead | State::Block | State::Splat) {
                    for obj in &self.level.objects {
                        let Some(m) = self.info.iter().find(|m| m.id == obj.id) else {
                            continue;
                        };
                        let x = obj.x + m.trigger_x;
                        let y = obj.y + m.trigger_y;
                        if l.x >= x
                            && l.x < x + m.trigger_w
                            && l.y + 1 >= y
                            && l.y + 1 < y + m.trigger_h
                        {
                            match m.effect {
                                1 => {
                                    l.state = State::Exit;
                                    l.age = 0;
                                    l.bomb = None;
                                    self.saved += 1;
                                    self.events.push(Sfx::Rescue);
                                    break;
                                }
                                4..=6 => {
                                    l.state = if m.effect == 5 {
                                        State::Drown
                                    } else if m.effect == 6 {
                                        State::Burn
                                    } else {
                                        State::Dead
                                    };
                                    l.age = 0;
                                    l.bomb = None;
                                    self.events.push(if m.effect == 5 {
                                        Sfx::Drown
                                    } else if m.effect == 6 {
                                        Sfx::Burn
                                    } else {
                                        Sfx::FallOut
                                    });
                                    break;
                                }
                                7 if l.dir < 0 => {
                                    if matches!(l.state, State::Bash | State::Mine) {
                                        l.state = State::Walk;
                                        self.events.push(Sfx::Reject);
                                    }
                                }
                                8 if l.dir > 0 => {
                                    if matches!(l.state, State::Bash | State::Mine) {
                                        l.state = State::Walk;
                                        self.events.push(Sfx::Reject);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            self.lems[i] = l;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> World {
        let l = Level {
            number: 0,
            name: "Test".into(),
            rating: "Fun".into(),
            style: 0,
            special: 0,
            total: 1,
            required: 1,
            minutes: 1,
            release: 99,
            skills: [1; 8],
            scroll: 0,
            objects: vec![],
            steel: vec![],
        };
        let mut w = World::new(l, Bitmap::blank(WIDTH, HEIGHT), vec![]);
        for x in 0..WIDTH {
            w.edit(x as i32, 100, true);
        }
        w.lems.push(Lem {
            x: 100,
            y: 99,
            dir: 1,
            state: State::Walk,
            age: 0,
            fall: 0,
            climber: false,
            floater: false,
            bomb: None,
            bricks: 0,
        });
        w.released = 1;
        w
    }
    #[test]
    fn skill_consumed_once() {
        let mut w = world();
        assert!(w.assign(0, 0));
        assert!(!w.assign(0, 0));
        assert_eq!(w.skills[0], 0);
    }
    #[test]
    fn blocker_turns_walkers() {
        let mut w = world();
        let mut b = w.lems[0].clone();
        b.x = 105;
        b.state = State::Block;
        w.lems.push(b);
        w.step();
        assert_eq!(w.lems[0].dir, -1);
    }
    #[test]
    fn steel_survives_bomb() {
        let mut w = world();
        w.level.steel.push([90, 100, 20, 2]);
        w.lems[0].bomb = Some(0);
        for _ in 0..22 {
            w.step();
        }
        assert_eq!(w.events.iter().filter(|e| **e == Sfx::Explosion).count(), 1);
        assert!(w.solid(100, 100));
    }
    #[test]
    fn long_fall_kills() {
        let mut w = world();
        w.lems[0].y = 10;
        w.lems[0].state = State::Fall;
        for _ in 0..40 {
            w.step();
        }
        assert_eq!(w.lems[0].state, State::Splat);
        for _ in 0..17 {
            w.step();
        }
        assert_eq!(w.lems[0].state, State::Dead);
    }
    #[test]
    fn lets_go_only_for_first_release() {
        let mut w = world();
        w.lems.clear();
        w.released = 0;
        w.level.total = 3;
        w.level.objects.push(crate::level::Placement {
            id: 1,
            x: 100,
            y: 0,
            flags: 0,
        });
        let mut voices = 0;
        for _ in 0..200 {
            w.step();
            voices += w.events.drain(..).filter(|e| *e == Sfx::LetsGo).count();
        }
        assert_eq!(w.released, 3);
        assert_eq!(voices, 1);
    }
    #[test]
    fn each_skill_has_one_assignment_sound() {
        for skill in 0..8 {
            let mut w = world();
            assert!(w.assign(0, skill));
            assert_eq!(w.events, vec![Sfx::Assign]);
            assert_eq!(w.skills[skill], 0);
        }
    }
    #[test]
    fn bomber_has_oh_no_then_explosion_once() {
        let mut w = world();
        w.assign(0, 2);
        let mut saw_oh_no = false;
        let mut saw_explosion = false;
        for _ in 0..120 {
            w.step();
            saw_oh_no |= w.lems[0].state == State::OhNo;
            saw_explosion |= w.lems[0].state == State::Explode;
        }
        assert!(saw_oh_no && saw_explosion);
        assert_eq!(w.events.iter().filter(|e| **e == Sfx::OhNo).count(), 1);
        assert_eq!(w.events.iter().filter(|e| **e == Sfx::Explosion).count(), 1);
        assert_eq!(w.lems[0].state, State::Dead);
    }
    #[test]
    fn builder_warns_for_last_three_steps_then_shrugs() {
        let mut w = world();
        w.assign(0, 4);
        for _ in 0..192 {
            w.step();
        }
        assert_eq!(
            w.events
                .iter()
                .filter(|e| **e == Sfx::BuilderWarning)
                .count(),
            3
        );
        assert_eq!(w.lems[0].state, State::Shrug);
        for _ in 0..8 {
            w.step();
        }
        assert_eq!(w.lems[0].state, State::Walk);
    }
    #[test]
    fn climber_uses_hoist_at_top_of_wall() {
        let mut w = world();
        for y in 80..100 {
            w.edit(101, y, true);
        }
        w.assign(0, 0);
        let mut hoisted = false;
        for _ in 0..40 {
            w.step();
            hoisted |= w.lems[0].state == State::Hoist;
        }
        assert!(hoisted);
    }
    #[test]
    fn steel_stops_excavators_with_rejection_sound() {
        for skill in [5, 6, 7] {
            let mut w = world();
            for y in 90..100 {
                for x in 100..110 {
                    w.edit(x, y, true);
                }
            }
            w.level.steel.push([90, 90, 30, 12]);
            w.assign(0, skill);
            for _ in 0..32 {
                w.step();
            }
            assert!(w.events.contains(&Sfx::Reject), "skill {skill}");
        }
    }
}
