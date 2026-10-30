//! Associations verified against the game's 68000 renderer and sound call sites.
use crate::sim::{Lem, State};
use serde::Serialize;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Sfx {
    LetsGo,
    Rescue,
    Splat,
    Explosion,
    Assign,
    Reject,
    BuilderWarning,
    OhNo,
    FallOut,
    Drown,
    Burn,
}
#[derive(Serialize)]
pub struct SoundSpec {
    pub name: &'static str,
    pub command: u16,
    pub call_address: usize,
}
pub const SOUNDS: [SoundSpec; 11] = [
    SoundSpec {
        name: "lets_go",
        command: 0x0e16,
        call_address: 0x0a1e,
    },
    SoundSpec {
        name: "rescue",
        command: 0x0c1b,
        call_address: 0x6fd0,
    },
    SoundSpec {
        name: "splat",
        command: 0x1319,
        call_address: 0x5c20,
    },
    SoundSpec {
        name: "explosion",
        command: 0x101d,
        call_address: 0x6364,
    },
    SoundSpec {
        name: "assign",
        command: 0x1317,
        call_address: 0x12d6,
    },
    SoundSpec {
        name: "reject",
        command: 0x0c1a,
        call_address: 0x13d2,
    },
    SoundSpec {
        name: "builder_warning",
        command: 0x061c,
        call_address: 0x6834,
    },
    SoundSpec {
        name: "oh_no",
        command: 0x0e18,
        call_address: 0x627c,
    },
    SoundSpec {
        name: "fall_out",
        command: 0x1a1e,
        call_address: 0x6394,
    },
    SoundSpec {
        name: "drown",
        command: 0x100a,
        call_address: 0x6f42,
    },
    SoundSpec {
        name: "burn",
        command: 0x1108,
        call_address: 0x6f86,
    },
];
#[derive(Clone, Copy, Serialize)]
pub struct Animation {
    pub name: &'static str,
    pub address: usize,
    pub w: usize,
    pub h: usize,
    pub planes: usize,
    pub frames: usize,
    pub left_offset: usize,
}
pub const ANIMATIONS: [Animation; 17] = [
    Animation {
        name: "walk",
        address: 0xcf46,
        w: 16,
        h: 10,
        planes: 2,
        frames: 9,
        left_offset: 540,
    },
    Animation {
        name: "fall",
        address: 0xd37e,
        w: 16,
        h: 10,
        planes: 2,
        frames: 4,
        left_offset: 240,
    },
    Animation {
        name: "dig",
        address: 0xd55e,
        w: 16,
        h: 14,
        planes: 3,
        frames: 16,
        left_offset: 0,
    },
    Animation {
        name: "climb",
        address: 0xdc5e,
        w: 16,
        h: 12,
        planes: 2,
        frames: 8,
        left_offset: 576,
    },
    Animation {
        name: "hoist",
        address: 0xe0de,
        w: 16,
        h: 12,
        planes: 2,
        frames: 8,
        left_offset: 576,
    },
    Animation {
        name: "build",
        address: 0xe75e,
        w: 16,
        h: 13,
        planes: 3,
        frames: 16,
        left_offset: 1664,
    },
    Animation {
        name: "splat",
        address: 0xf45e,
        w: 16,
        h: 10,
        planes: 2,
        frames: 16,
        left_offset: 0,
    },
    Animation {
        name: "block",
        address: 0xf81e,
        w: 16,
        h: 10,
        planes: 2,
        frames: 16,
        left_offset: 0,
    },
    Animation {
        name: "bash",
        address: 0xfbde,
        w: 16,
        h: 10,
        planes: 3,
        frames: 32,
        left_offset: 2560,
    },
    Animation {
        name: "float",
        address: 0x10fde,
        w: 16,
        h: 16,
        planes: 3,
        frames: 8,
        left_offset: 1024,
    },
    Animation {
        name: "mine",
        address: 0x117de,
        w: 16,
        h: 13,
        planes: 3,
        frames: 24,
        left_offset: 2496,
    },
    Animation {
        name: "drown",
        address: 0x12b5e,
        w: 16,
        h: 10,
        planes: 2,
        frames: 16,
        left_offset: 0,
    },
    Animation {
        name: "burn",
        address: 0x1318e,
        w: 16,
        h: 14,
        planes: 4,
        frames: 14,
        left_offset: 0,
    },
    Animation {
        name: "exit",
        address: 0x12f1e,
        w: 16,
        h: 13,
        planes: 2,
        frames: 8,
        left_offset: 0,
    },
    Animation {
        name: "shrug",
        address: 0x13936,
        w: 16,
        h: 10,
        planes: 2,
        frames: 8,
        left_offset: 480,
    },
    Animation {
        name: "oh_no",
        address: 0x13cf6,
        w: 16,
        h: 10,
        planes: 2,
        frames: 16,
        left_offset: 0,
    },
    Animation {
        name: "explosion",
        address: 0xe55e,
        w: 32,
        h: 32,
        planes: 3,
        frames: 1,
        left_offset: 0,
    },
];
pub fn frame(l: &Lem) -> Option<(&'static Animation, usize, usize)> {
    let name = match l.state {
        State::Walk => "walk",
        State::Fall => "fall",
        State::Climb => "climb",
        State::Hoist => "hoist",
        State::Float => "float",
        State::Block => "block",
        State::Build => "build",
        State::Bash => "bash",
        State::Mine => "mine",
        State::Dig => "dig",
        State::Shrug => "shrug",
        State::OhNo => "oh_no",
        State::Explode => "explosion",
        State::Splat => "splat",
        State::Drown => "drown",
        State::Burn => "burn",
        State::Exit => "exit",
        State::Dead | State::Saved => return None,
    };
    let a = ANIMATIONS
        .iter()
        .find(|a| a.name == name)
        .expect("known animation");
    let frame = if l.state == State::Float {
        // Opening then the oscillating canopy sequence from the movement table.
        let opening = [0, 2, 3, 5, 5, 5, 5, 5, 5];
        let looped = [6, 7, 7, 6, 5, 4, 4, 5];
        let age = l.age as usize;
        if age < opening.len() {
            opening[age]
        } else {
            looped[(age - opening.len()) % looped.len()]
        }
    } else {
        l.age as usize % if l.state == State::Walk { 8 } else { a.frames }
    };
    Some((a, usize::from(a.left_offset > 0 && l.dir < 0), frame))
}

/// Runtime instructions selecting each sequence; checked against the user's Code file.
pub const ANIMATION_CALLS: [(&str, usize); 17] = [
    ("walk", 0x2612),
    ("fall", 0x5ab8),
    ("dig", 0x1340),
    ("climb", 0x63a6),
    ("hoist", 0x6ade),
    ("build", 0x1294),
    ("block", 0x1080),
    ("bash", 0x121a),
    ("float", 0x5bd2),
    ("mine", 0x1180),
    ("shrug", 0x6980),
    ("oh_no", 0x62f2),
    ("exit", 0x6fa6),
    ("burn", 0x6f5a),
    ("drown", 0x6f16),
    ("splat", 0x5c14),
    ("explosion", 0x44dc),
];
