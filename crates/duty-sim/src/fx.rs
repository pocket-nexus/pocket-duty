//! What a round leaves in the air and on a wall: tracers, flashes, dust, the
//! hole. The reference draws these with thousands of particles and decals cut
//! to the wall's triangles (`web/src/fx/`); a handheld draws a few dozen
//! squares from one small picture. Which effect an event makes, its colours
//! and its seconds follow the reference's tables (`fx/impacts.js`,
//! `fx/muzzle.js`, `fx/tracers.js`).
//!
//! The device asks for vertices each frame (`build`): squares turned to the
//! eye for what hangs in the air, squares laid on the surface for holes.

use crate::event::Event;
use crate::math::*;
use crate::rng::Rng;
use crate::tune::surface;

/// A vertex in the GE's component order: texture, colour, position.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct FxVertex {
    pub uv: [f32; 2],
    /// Red, green, blue, and how opaque.
    pub colour: [u8; 4],
    pub pos: [f32; 3],
}

/// The picture's four shapes: where each tile starts.
mod tile {
    pub const DOT: [f32; 2] = [0.0, 0.0];
    pub const PUFF: [f32; 2] = [0.5, 0.0];
    pub const FLASH: [f32; 2] = [0.0, 0.5];
    pub const HOLE: [f32; 2] = [0.5, 0.5];
}

#[derive(Clone, Copy, Debug)]
struct Mote {
    tile: [f32; 2],
    pos: V3,
    vel: V3,
    /// Lies on a surface facing this way; zero for one that turns to the eye.
    normal: V3,
    age: f32,
    life: f32,
    /// Half its side at birth and at death, metres.
    size: (f32, f32),
    colour: [u8; 4],
    /// Adds its light to what is behind it.
    glow: bool,
    /// How fast it falls, metres per second squared.
    weight: f32,
}

const NONE: Mote = Mote {
    tile: tile::DOT,
    pos: V3::ZERO,
    vel: V3::ZERO,
    normal: V3::ZERO,
    age: 0.0,
    life: 0.0,
    size: (0.0, 0.0),
    colour: [0; 4],
    glow: false,
    weight: 0.0,
};

#[derive(Clone, Copy, Debug, Default)]
struct Tracer {
    from: V3,
    to: V3,
    age: f32,
}

pub const MOTES: usize = 72;
pub const HOLES: usize = 40;
pub const TRACERS: usize = 12;
/// Vertices `build` writes at most: four a square.
pub const VERTICES: usize = (MOTES + HOLES + TRACERS) * 4;

pub struct Fx {
    motes: [Mote; MOTES],
    holes: [Mote; HOLES],
    next_hole: usize,
    tracers: [Tracer; TRACERS],
    rng: Rng,
}

impl Default for Fx {
    fn default() -> Self {
        Fx::new()
    }
}

impl Fx {
    pub fn new() -> Fx {
        Fx {
            motes: [NONE; MOTES],
            holes: [NONE; HOLES],
            next_hole: 0,
            tracers: [Tracer {
                age: 1.0,
                ..Default::default()
            }; TRACERS],
            rng: Rng::new(0x0f0f_1234),
        }
    }

    pub fn clear(&mut self) {
        *self = Fx::new();
    }

    fn mote(&mut self, mote: Mote) {
        // A free place, or the one nearest its end.
        let mut at = 0;
        let mut most = -1.0f32;
        for (i, m) in self.motes.iter().enumerate() {
            let spent = if m.life <= 0.0 { 2.0 } else { m.age / m.life };
            if spent > most {
                most = spent;
                at = i;
            }
        }
        self.motes[at] = mote;
    }

    fn hole(&mut self, pos: V3, normal: V3, half: f32, life: f32, alpha: u8) {
        self.holes[self.next_hole] = Mote {
            tile: tile::HOLE,
            pos: pos + normal * 0.012,
            normal,
            age: 0.0,
            life,
            size: (half, half),
            colour: [16, 14, 12, alpha],
            ..NONE
        };
        self.next_hole = (self.next_hole + 1) % HOLES;
    }

    /// What an event of the match leaves. `eye`, `ahead`, `right` and `up` are the player's view:
    /// his own muzzle is below and beside it.
    pub fn on_event(&mut self, event: &Event, eye: V3, ahead: V3, right: V3, up: V3) {
        match *event {
            Event::Shot {
                soldier,
                from,
                to,
                tracer,
                ..
            } => {
                let muzzle = if soldier.is_some() {
                    from
                } else {
                    eye + ahead * 0.75 + right * 0.085 - up * 0.075
                };
                if tracer && (to - muzzle).len2() > 4.0 {
                    let mut oldest = 0;
                    for (i, t) in self.tracers.iter().enumerate() {
                        if t.age > self.tracers[oldest].age {
                            oldest = i;
                        }
                    }
                    self.tracers[oldest] = Tracer {
                        from: muzzle,
                        to,
                        age: 0.0,
                    };
                }
                if soldier.is_some() {
                    // A soldier's muzzle: a flash for a twentieth of a second (`fx/muzzle.js:105-473`).
                    let half = self.rng.range(0.16, 0.24);
                    {
                        let mote = Mote {
                            tile: tile::FLASH,
                            pos: muzzle,
                            life: 0.055,
                            size: (half, half * 1.3),
                            colour: [255, 214, 150, 235],
                            glow: true,
                            ..NONE
                        };
                        self.mote(mote);
                    }
                }
            }
            Event::Impact {
                pos,
                normal,
                surface: s,
            } => {
                // What each surface throws up (`fx/impacts.js:156-916`): its dust's colour, how much, and sparks off metal.
                let (dust, half, life, lift) = match s {
                    surface::METAL => ([52, 50, 47, 150], 0.10, 0.3, 0.6),
                    surface::WOOD => ([118, 88, 54, 170], 0.14, 0.4, 0.9),
                    surface::DIRT => ([92, 70, 50, 190], 0.22, 0.7, 1.4),
                    surface::SAND => ([170, 146, 104, 190], 0.24, 0.75, 1.4),
                    11 => ([190, 163, 122, 180], 0.17, 0.5, 1.0),
                    _ => ([146, 122, 90, 180], 0.17, 0.5, 1.0),
                };
                let drift = v3(self.rng.signed() * 0.3, 0.25, self.rng.signed() * 0.3);
                {
                    let mote = Mote {
                        tile: tile::PUFF,
                        pos: pos + normal * 0.08,
                        vel: normal * lift + drift,
                        life,
                        size: (half * 0.45, half * 1.6),
                        colour: dust,
                        ..NONE
                    };
                    self.mote(mote);
                }
                if s == surface::METAL {
                    for _ in 0..3 {
                        let spray = (normal
                            + v3(self.rng.signed(), self.rng.signed(), self.rng.signed()) * 0.8)
                            .norm();
                        {
                            let mote = Mote {
                                tile: tile::DOT,
                                pos: pos + normal * 0.03,
                                vel: spray * self.rng.range(4.5, 9.0),
                                life: self.rng.range(0.18, 0.4),
                                size: (0.022, 0.008),
                                colour: [255, 196, 96, 255],
                                glow: true,
                                weight: 14.0,
                                ..NONE
                            };
                            self.mote(mote);
                        }
                    }
                }
                let ground = matches!(s, surface::DIRT | surface::SAND);
                let half = if ground {
                    self.rng.range(0.045, 0.075)
                } else {
                    self.rng.range(0.024, 0.04)
                };
                self.hole(pos, normal, half, if ground { 60.0 } else { 90.0 }, 200);
            }
            Event::Struck { pos, .. } => {
                // (`fx/impacts.js:793-841`)
                let spray = (eye - pos).norm() * 0.6
                    + v3(self.rng.signed() * 0.4, 0.5, self.rng.signed() * 0.4);
                {
                    let mote = Mote {
                        tile: tile::PUFF,
                        pos,
                        vel: spray,
                        life: self.rng.range(0.3, 0.5),
                        size: (0.05, 0.22),
                        colour: [128, 14, 11, 200],
                        weight: 2.0,
                        ..NONE
                    };
                    self.mote(mote);
                }
            }
            Event::Fell { pos, .. } => {
                let mote = Mote {
                    tile: tile::PUFF,
                    pos: pos + v3(0.0, 0.15, 0.0),
                    vel: v3(0.0, 0.5, 0.0),
                    life: 0.9,
                    size: (0.25, 0.8),
                    colour: [150, 128, 96, 110],
                    ..NONE
                };
                self.mote(mote);
            }
            Event::Blast { pos } => {
                // (`fx/explosions.js`): the flash, the smoke that follows it up, the scorch it leaves.
                {
                    let mote = Mote {
                        tile: tile::FLASH,
                        pos: pos + v3(0.0, 0.6, 0.0),
                        life: 0.14,
                        size: (1.6, 4.2),
                        colour: [255, 208, 140, 255],
                        glow: true,
                        ..NONE
                    };
                    self.mote(mote);
                }
                for _ in 0..7 {
                    let out = v3(
                        self.rng.signed() * 2.2,
                        self.rng.range(1.2, 3.4),
                        self.rng.signed() * 2.2,
                    );
                    {
                        let mote = Mote {
                            tile: tile::PUFF,
                            pos: pos + v3(0.0, 0.5, 0.0) + out * 0.2,
                            vel: out,
                            life: self.rng.range(1.2, 2.2),
                            size: (0.6, 2.3),
                            colour: [46, 42, 38, 210],
                            weight: -0.4,
                            ..NONE
                        };
                        self.mote(mote);
                    }
                }
                for _ in 0..6 {
                    let spray = v3(
                        self.rng.signed(),
                        self.rng.range(0.4, 1.3),
                        self.rng.signed(),
                    )
                    .norm();
                    {
                        let mote = Mote {
                            tile: tile::DOT,
                            pos: pos + v3(0.0, 0.3, 0.0),
                            vel: spray * self.rng.range(6.0, 14.0),
                            life: self.rng.range(0.3, 0.6),
                            size: (0.04, 0.012),
                            colour: [255, 170, 80, 255],
                            glow: true,
                            weight: 16.0,
                            ..NONE
                        };
                        self.mote(mote);
                    }
                }
                self.hole(pos, V3::UP, 1.6, 120.0, 150);
            }
            Event::Bounce { pos } => {
                let mote = Mote {
                    tile: tile::PUFF,
                    pos,
                    vel: v3(0.0, 0.4, 0.0),
                    life: 0.35,
                    size: (0.06, 0.2),
                    colour: [150, 128, 96, 120],
                    ..NONE
                };
                self.mote(mote);
            }
            _ => {}
        }
    }

    /// Ages everything by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        for m in self.motes.iter_mut().chain(self.holes.iter_mut()) {
            if m.life <= 0.0 {
                continue;
            }
            m.age += dt;
            if m.age >= m.life {
                m.life = 0.0;
                continue;
            }
            m.vel.y -= m.weight * dt;
            // The air takes a mote's speed; a hole has none.
            m.vel = m.vel * exp(-2.2 * dt);
            m.pos += m.vel * dt;
        }
        for t in &mut self.tracers {
            t.age += dt;
        }
    }

    /// Writes the squares of what shows at the places `within` takes: first those that cover what
    /// is behind them, then those that add their light. Returns how many vertices of each.
    /// Positions are counted from `eye`.
    pub fn build(
        &self,
        eye: V3,
        right: V3,
        up: V3,
        within: impl Fn(V3) -> bool,
        out: &mut [FxVertex],
    ) -> (usize, usize) {
        let mut n = 0;
        let mut square = |n: &mut usize, m: &Mote| {
            if *n + 4 > out.len() || m.life <= 0.0 || !within(m.pos) {
                return;
            }
            let part = m.age / m.life;
            let half = lerp(m.size.0, m.size.1, part);
            // Opaque at once, then gone by its end; a hole stays whole until its last fifth.
            let fade = if m.normal.len2() > 0.5 {
                saturate((1.0 - part) * 5.0)
            } else {
                saturate((1.0 - part) * 1.6)
            };
            let colour = [
                m.colour[0],
                m.colour[1],
                m.colour[2],
                (m.colour[3] as f32 * fade) as u8,
            ];
            let (a, b) = if m.normal.len2() > 0.5 {
                let a = if abs(m.normal.y) < 0.9 {
                    m.normal.cross(V3::UP).norm()
                } else {
                    m.normal.cross(v3(1.0, 0.0, 0.0)).norm()
                };
                (a, m.normal.cross(a))
            } else {
                (right, up)
            };
            let centre = m.pos - eye;
            for (k, (sx, sy)) in [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)]
                .into_iter()
                .enumerate()
            {
                let p = centre + a * (sx * half) + b * (sy * half);
                out[*n + k] = FxVertex {
                    uv: [
                        m.tile[0] + 0.02 + (sx * 0.5 + 0.5) * 0.46,
                        m.tile[1] + 0.02 + (0.5 - sy * 0.5) * 0.46,
                    ],
                    colour,
                    pos: [p.x, p.y, p.z],
                };
            }
            *n += 4;
        };
        for m in self
            .holes
            .iter()
            .chain(self.motes.iter().filter(|m| !m.glow))
        {
            square(&mut n, m);
        }
        let covering = n;
        for m in self.motes.iter().filter(|m| m.glow) {
            square(&mut n, m);
        }
        // A tracer: a streak along the round's way for a tenth of a second, wider with distance so
        // that it stays a pixel wide far off (`fx/tracers.js`).
        for t in self.tracers.iter().filter(|t| t.age < 0.1) {
            if n + 4 > out.len() {
                break;
            }
            let along = t.to - t.from;
            let length = along.len();
            let dir = along * (1.0 / max(length, 1e-3));
            // The streak runs down the way at the reference's 340 m/s, 12 m long at most.
            let head = min(length, t.age * 340.0 + 6.0);
            let tail = max(0.0, head - 12.0);
            let (p0, p1) = (t.from + dir * tail - eye, t.from + dir * head - eye);
            if !within(eye + p0.lerp(p1, 0.5)) {
                continue;
            }
            let side = dir.cross(p0.lerp(p1, 0.5)).norm_or(up);
            let alpha = (220.0 * saturate(1.0 - t.age * 10.0)) as u8;
            for (k, (p, s, u)) in [
                (p0, -1.0, 0.0),
                (p0, 1.0, 0.46),
                (p1, 1.0, 0.46),
                (p1, -1.0, 0.0),
            ]
            .into_iter()
            .enumerate()
            {
                let w = max(0.012, p.len() * 0.0035);
                let q = p + side * (s * w);
                out[n + k] = FxVertex {
                    uv: [0.02 + u, 0.25],
                    colour: [255, 150, 60, alpha],
                    pos: [q.x, q.y, q.z],
                };
            }
            n += 4;
        }
        (covering, n - covering)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_into_a_wall_leaves_dust_and_a_hole() {
        let mut fx = Fx::new();
        let (eye, ahead, right, up) = (
            v3(0.0, 1.6, 0.0),
            v3(0.0, 0.0, -1.0),
            v3(1.0, 0.0, 0.0),
            V3::UP,
        );
        fx.on_event(
            &Event::Impact {
                pos: v3(0.0, 1.5, -8.0),
                normal: v3(0.0, 0.0, 1.0),
                surface: 0,
            },
            eye,
            ahead,
            right,
            up,
        );
        fx.on_event(
            &Event::Shot {
                soldier: None,
                weapon: 0,
                from: eye,
                to: v3(0.0, 1.5, -8.0),
                tracer: true,
            },
            eye,
            ahead,
            right,
            up,
        );
        let mut out = [FxVertex::default(); VERTICES];
        fx.step(1.0 / 30.0);
        let (covering, glowing) = fx.build(eye, right, up, |_| true, &mut out);
        assert_eq!(
            (covering, glowing),
            (8, 4),
            "a hole and a puff, and the tracer"
        );
        // The hole lies on the wall, a little off it.
        assert!(out[..4]
            .iter()
            .all(|v| (v.pos[2] + 8.0 - 0.012).abs() < 1e-3));
        for _ in 0..30 {
            fx.step(1.0 / 30.0);
        }
        let (covering, glowing) = fx.build(eye, right, up, |_| true, &mut out);
        assert_eq!(
            (covering, glowing),
            (4, 0),
            "the dust and the tracer are gone, the hole stays"
        );
    }
}
