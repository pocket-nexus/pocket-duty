//! What happened in a step, for whoever draws it, sounds it and writes it on the screen. The
//! simulation only records; nothing it does later depends on who read the list.

use crate::math::V3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// The player's foot came down.
    Footstep {
        surface: u8,
        running: bool,
    },
    Jump,
    /// The player landed at this speed, metres a second.
    Land {
        speed: f32,
        surface: u8,
    },
    /// A round left a muzzle: the player's (`soldier` is `None`) or a soldier's. `to` is where
    /// it stopped; `tracer` rounds are drawn on their way.
    Shot {
        soldier: Option<u8>,
        weapon: u8,
        from: V3,
        to: V3,
        tracer: bool,
    },
    /// A round met the world.
    Impact {
        pos: V3,
        normal: V3,
        surface: u8,
    },
    /// The trigger fell on an empty chamber.
    Dry,
    /// A change of magazine reached a step: 0 began, 1 magazine out, 2 magazine in, 3 done.
    Reload {
        step: u8,
    },
    /// Another weapon came up.
    Draw {
        weapon: u8,
    },
    /// The player's round hit a soldier: where, in which zone (`tune::soldier::ZONE_*`), and
    /// whether it was the last one he took.
    Struck {
        soldier: u8,
        zone: u8,
        killed: bool,
        pos: V3,
    },
    /// The player was hit: how hard, and where it came from.
    Hurt {
        amount: f32,
        from: V3,
    },
    /// The player was killed by this soldier.
    Down {
        soldier: Option<u8>,
    },
    /// The player is back on his feet.
    Up,
    /// A soldier calls out: 0 he has seen the player, 1 he changes his magazine, 2 he is hit, 3 he dies.
    Bark {
        soldier: u8,
        kind: u8,
        pos: V3,
    },
    /// A soldier fell.
    Fell {
        soldier: u8,
        pos: V3,
    },
    /// A grenade left a hand, struck the ground, went off.
    Thrown {
        pos: V3,
    },
    Bounce {
        pos: V3,
    },
    Blast {
        pos: V3,
    },
    /// A round passed close to the player's head.
    Whizz {
        pos: V3,
    },
    /// The match is over.
    Over {
        won: bool,
    },
}

/// A step's events, oldest first. One step cannot make more than this; what would not fit is dropped.
#[derive(Clone, Copy)]
pub struct Events {
    list: [Event; Events::ROOM],
    count: usize,
}

impl Default for Events {
    fn default() -> Self {
        Events {
            list: [Event::Jump; Events::ROOM],
            count: 0,
        }
    }
}

impl Events {
    pub const ROOM: usize = 64;

    pub fn push(&mut self, event: Event) {
        if self.count < Events::ROOM {
            self.list[self.count] = event;
            self.count += 1;
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }

    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        self.list[..self.count].iter()
    }
}
