//! The player: how the pad moves him, what the eye does, and his health.
//!
//! The rules and their numbers are the reference's (`web/src/player/`): the
//! speeds by stance and direction, the acceleration toward a wished velocity,
//! the jump with its buffer and its moment of grace off a ledge, the slide out
//! of a sprint, the climb onto what is too tall to step on, health that comes
//! back after a few seconds unhurt. What a handheld's pad cannot ask for is
//! left out: leaning, and lying prone.

use crate::body::{Body, Near};
use crate::event::{Event, Events};
use crate::level::Level;
use crate::math::*;
use crate::tune::player::*;
use crate::tune::{DT, GRAVITY, SURFACES};

/// The pad as the simulation reads it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// The walk: right and forward, -1…1.
    pub mx: f32,
    pub my: f32,
    /// The look: right and up, -1…1.
    pub lx: f32,
    pub ly: f32,
    /// `btn` bits held.
    pub buttons: u32,
}

pub mod btn {
    pub const FIRE: u32 = 1;
    pub const ADS: u32 = 2;
    pub const JUMP: u32 = 4;
    pub const CROUCH: u32 = 8;
    pub const RELOAD: u32 = 16;
    pub const NEXT: u32 = 32;
    pub const MODE: u32 = 64;
    pub const SPRINT: u32 = 128;
}

/// A spring that carries a kick and lets it go: part of a kick displaces the spring, the rest
/// fades by itself (`web/src/player/springs.js:149-177`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Kick {
    x: f32,
    v: f32,
    rest: f32,
}

impl Kick {
    pub fn add(&mut self, amount: f32, share: f32) {
        self.x += amount * (1.0 - share);
        self.rest += amount * share;
    }

    /// `hz` and `damping` are the spring's; `tau` the seconds the rest fades in.
    pub fn step(&mut self, hz: f32, damping: f32, tau: f32, dt: f32) {
        let w = TAU * hz;
        // Three pieces a step: the spring is stiff for one.
        let h = dt / 3.0;
        for _ in 0..3 {
            self.v += (-w * w * self.x - 2.0 * damping * w * self.v) * h;
            self.x += self.v * h;
        }
        self.rest *= exp(-dt / tau);
    }

    pub fn value(&self) -> f32 {
        self.x + self.rest
    }
}

/// A climb onto a ledge: from where, to where, and how far along it is. Off a floor onto a
/// ladder under its edge it is the same carry, ending with him on that ladder.
#[derive(Clone, Copy, Debug)]
pub struct Climb {
    from: V3,
    to: V3,
    at: f32,
    time: f32,
    onto: Option<u16>,
}

#[derive(Clone, Copy, Debug)]
pub struct Player {
    pub body: Body,
    pub yaw: f32,
    pub pitch: f32,
    pub crouched: bool,
    pub sprinting: bool,
    /// Seconds a slide has left, and the way it goes.
    pub sliding: f32,
    slide_dir: V3,
    slide_cooldown: f32,
    pub climb: Option<Climb>,
    /// The ladder he is on (`Level::links`), and the seconds before he takes hold of one again.
    pub ladder: Option<u16>,
    ladder_wait: f32,
    pub health: f32,
    pub dead: bool,
    since_hurt: f32,
    /// The eye's height over the feet, settling after a change of stance, and what the eye still
    /// owes a step the feet took at once.
    eye: f32,
    step_debt: f32,
    /// Where the walk's stride is, in turns of a full two-step cycle, and how much of the bob shows.
    stride: f32,
    bob: f32,
    /// What the weapon's kick adds to the view: pitch, yaw, and a push back along it.
    pub kick: [Kick; 2],
    pub punch: Kick,
    /// How far the sights are up, 0…1, linear in time: the weapons raise it.
    pub ads: f32,
    coyote: f32,
    jump_buffer: f32,
    jump_cooldown: f32,
    held: u32,
    /// How long the look has been held, for a button's turn to gather speed.
    look_time: f32,
    /// Radians a second the look turns at full deflection, the setting's; whether up is down; and
    /// whether the look comes from an analog stick (which needs no gathering).
    pub turn: f32,
    pub invert: bool,
    pub analog_look: bool,
    /// What the aim's help does to the look this step: how much it slows it, 0…1 of its speed.
    pub look_scale: f32,
}

impl Player {
    pub fn new(feet: V3, yaw: f32) -> Player {
        Player {
            body: Body::new(feet, RADIUS, HEIGHT[0], STEP[0]),
            yaw,
            pitch: 0.0,
            crouched: false,
            sprinting: false,
            sliding: 0.0,
            slide_dir: V3::ZERO,
            slide_cooldown: 0.0,
            climb: None,
            ladder: None,
            ladder_wait: 0.0,
            health: HEALTH,
            dead: false,
            since_hurt: 100.0,
            eye: EYE[0],
            step_debt: 0.0,
            stride: 0.0,
            bob: 0.0,
            kick: [Kick::default(); 2],
            punch: Kick::default(),
            ads: 0.0,
            coyote: 0.0,
            jump_buffer: 0.0,
            jump_cooldown: 0.0,
            held: 0,
            look_time: 0.0,
            turn: 2.2,
            invert: false,
            analog_look: false,
            look_scale: 1.0,
        }
    }

    /// The level horizontal speed.
    pub fn speed(&self) -> f32 {
        self.body.vel.flat().len()
    }

    /// One step of `DT`. `can_ads`: the weapon lets the sights be raised.
    /// `near` is the triangles about his body, his own from step to step.
    pub fn step(&mut self, level: &Level, near: &mut Near, input: &Input, events: &mut Events) {
        let pressed = input.buttons & !self.held;
        self.held = input.buttons;
        if self.dead {
            // (he falls if he was in the air, and then lies)
            self.body.vel = v3(0.0, if self.body.grounded { 0.0 } else { self.body.vel.y - GRAVITY * DT }, 0.0);
            self.body.advance(level, near, DT);
            self.eye = ease(self.eye, 0.35, 6.0, DT);
            self.ladder = None;
            return;
        }
        self.look(input);
        for timer in [
            &mut self.coyote,
            &mut self.jump_buffer,
            &mut self.jump_cooldown,
            &mut self.slide_cooldown,
            &mut self.ladder_wait,
        ] {
            *timer = max(0.0, *timer - DT);
        }
        if let Some(mut climb) = self.climb {
            // Carried along the climb: up first, then onto the ledge. Onto a ladder: out first, then down.
            climb.at += DT / climb.time;
            let t = min(climb.at, 1.0);
            let (first, then) = (smoothstep(0.0, 0.7, t), smoothstep(0.35, 1.0, t));
            let (rise, reach) = if climb.to.y >= climb.from.y { (first, then) } else { (then, first) };
            self.body.pos = v3(
                lerp(climb.from.x, climb.to.x, reach),
                lerp(climb.from.y, climb.to.y, rise),
                lerp(climb.from.z, climb.to.z, reach),
            );
            self.body.vel = V3::ZERO;
            self.climb = (t < 1.0).then_some(climb);
            if t >= 1.0 {
                self.body.grounded = climb.onto.is_none();
                self.body.vel = if climb.onto.is_none() { heading(self.yaw) * 1.8 } else { V3::ZERO };
                self.ladder = climb.onto;
            }
            self.settle();
            return;
        }

        let forward = heading(self.yaw);
        let right = v3(cos(self.yaw), 0.0, -sin(self.yaw));
        // The wish: sideways a little slower than ahead, backwards slower still.
        let (sx, sz) = (
            input.mx * STRAFE_SCALE,
            if input.my >= 0.0 {
                input.my
            } else {
                input.my * BACK_SCALE
            },
        );
        let raw = sqrt(input.mx * input.mx + input.my * input.my);
        let wish_len = min(1.0, sqrt(sx * sx + sz * sz));
        let wish = (forward * sz + right * sx).norm();
        // ---- a ladder: he hangs on it, and the stick toward its wall is up
        if self.hang(level, near, pressed, wish * wish_len) {
            self.settle();
            return;
        }
        let grounded = self.body.grounded;
        let speed = self.speed();

        // ---- stance
        if pressed & btn::CROUCH != 0 {
            if self.sprinting
                && grounded
                && speed >= SLIDE_START
                && self.slide_cooldown <= 0.0
                && self.sliding <= 0.0
            {
                // Out of a sprint a crouch is a slide.
                self.sliding = SLIDE_TIME;
                self.slide_dir = self.body.vel.flat().norm_or(forward)
                    * max(SLIDE_MIN_ENTRY, min(SLIDE_ENTRY, speed * 1.3));
                self.set_stance(true);
            } else if self.crouched {
                if self.body.fits(level, HEIGHT[0]) {
                    self.set_stance(false);
                }
            } else {
                self.set_stance(true);
            }
        }
        // ---- sprint: the stick all the way and mostly ahead, or the button, on the feet and with the sights down
        let ahead = raw > 0.45 && input.my / max(raw, 1e-3) > SPRINT_FORWARD;
        let asked = (raw > 0.92 || input.buttons & btn::SPRINT != 0) && ahead;
        self.sprinting = asked
            && self.sliding <= 0.0
            && self.ads < 0.3
            && (grounded || self.sprinting)
            && (!self.crouched || self.body.fits(level, HEIGHT[0]));
        if self.sprinting && self.crouched {
            self.set_stance(false);
        }

        // ---- jump
        if pressed & btn::JUMP != 0 {
            self.jump_buffer = JUMP_BUFFER;
        }
        if grounded {
            self.coyote = COYOTE;
        }
        let mut jumped = false;
        if self.jump_buffer > 0.0
            && self.jump_cooldown <= 0.0
            && self.coyote > 0.0
            && (!self.crouched || self.body.fits(level, HEIGHT[0]))
        {
            if self.sliding > 0.0 {
                // A jump ends a slide and keeps most of its speed.
                self.body.vel =
                    self.slide_dir.norm() * min(self.slide_dir.len(), SPRINT_SPEED * 1.06);
                self.sliding = 0.0;
                self.slide_cooldown = SLIDE_COOLDOWN;
            }
            self.set_stance(false);
            // (half a step's fall more: the height is then the reference's at this step's length)
            self.body.vel.y = sqrt(2.0 * GRAVITY * JUMP_APEX) + GRAVITY * DT * 0.5;
            self.body.grounded = false;
            self.jump_buffer = 0.0;
            self.coyote = 0.0;
            self.jump_cooldown = JUMP_COOLDOWN;
            jumped = true;
            events.push(Event::Jump);
        }

        // ---- the walk
        if self.sliding > 0.0 {
            self.sliding -= DT;
            let friction = SURFACES[(self.body.surface as usize).min(SURFACES.len() - 1)].friction;
            let mut sp = self.slide_dir.len();
            sp = sp * exp(-0.75 * DT) - 0.85 * DT;
            sp -= sp * clamp(friction - 0.55, 0.0, 0.8) * 0.62 * DT;
            // The stick steers it a little.
            let steered = (self.slide_dir.norm() + right * (input.mx * 2.6 * DT)).norm();
            self.slide_dir = steered * max(sp, 0.0);
            self.body.vel.x = self.slide_dir.x;
            self.body.vel.z = self.slide_dir.z;
            if self.sliding <= 0.0 || sp < SLIDE_EXIT || !grounded {
                self.sliding = 0.0;
                self.slide_cooldown = SLIDE_COOLDOWN;
            }
        } else if grounded {
            let mut top = if self.sprinting {
                SPRINT_SPEED
            } else {
                SPEED[self.crouched as usize]
            };
            if !self.sprinting {
                top *= lerp(1.0, ADS_SCALE, self.ads);
            }
            let target = wish * (top * wish_len);
            let friction = SURFACES[(self.body.surface as usize).min(SURFACES.len() - 1)].friction;
            let rate = (if raw < 0.02 {
                STOP_DECEL
            } else if top * wish_len < 0.92 * speed {
                GROUND_DECEL
            } else {
                GROUND_ACCEL
            }) * clamp(friction + 0.08, 0.75, 1.05);
            let d = target - self.body.vel.flat();
            let step = min(d.len(), rate * DT);
            let d = d.norm() * step;
            self.body.vel.x += d.x;
            self.body.vel.z += d.z;
        } else if wish_len > 0.0 {
            // In the air the stick adds speed only along the wish, and only up to a walk's part.
            let along = self.body.vel.dot(wish);
            let add = clamp(AIR_SPEED_CAP * wish_len - along, 0.0, AIR_ACCEL * DT);
            self.body.vel += wish * add;
        }
        if !self.body.grounded {
            self.body.vel.y = max(self.body.vel.y - GRAVITY * DT, -TERMINAL);
        }

        // ---- a ledge to climb: the jump held, pushing ahead, and something too tall to step on in the way
        if input.buttons & btn::JUMP != 0
            && input.my > 0.4
            && self.sliding <= 0.0
            && (self.body.blocked || !self.body.grounded)
            && self.body.vel.y < 1.5
        {
            self.climb = self.ledge(level, forward);
            if self.climb.is_some() {
                self.set_stance(false);
                return;
            }
        }

        let before = self.body.pos.y;
        self.body.advance(level, near, DT);
        // A step the feet took at once the eye takes over a moment.
        if self.body.grounded && grounded && !jumped {
            self.step_debt = clamp(self.step_debt - (self.body.pos.y - before), -0.5, 0.5);
        }
        if self.body.landed > 0.0 {
            events.push(Event::Land {
                speed: self.body.landed,
                surface: self.body.surface,
            });
            if self.body.landed > FALL_SAFE {
                self.hurt(
                    (self.body.landed - FALL_SAFE) * FALL_DAMAGE,
                    self.body.pos,
                    events,
                );
            }
            // The eye dips with the landing.
            self.step_debt = clamp(
                self.step_debt - min(0.063, 0.006 * self.body.landed),
                -0.5,
                0.5,
            );
        }

        // ---- the stride: a footstep each half turn
        if self.body.grounded && self.sliding <= 0.0 {
            let length = STRIDE[self.crouched as usize] * if self.sprinting { 1.28 } else { 1.0 };
            let before = self.stride;
            self.stride += self.speed() * DT / (2.0 * length);
            if floor(self.stride * 2.0) != floor(before * 2.0) {
                events.push(Event::Footstep {
                    surface: self.body.surface,
                    running: self.speed() >= 5.4,
                });
            }
            if self.stride >= 1.0 {
                self.stride -= 1.0;
            }
        }
        self.settle();
    }

    /// The eye's own motion and the body's repair, every step.
    fn settle(&mut self) {
        let target = if self.sliding > 0.0 {
            EYE[1] - 0.1
        } else {
            EYE[self.crouched as usize]
        };
        self.eye = ease(self.eye, target, 1.0 / EYE_TAU, DT);
        self.step_debt = ease(self.step_debt, 0.0, 14.0, DT);
        let moving = self.body.grounded && self.sliding <= 0.0 && self.climb.is_none();
        let weight = if moving {
            min(1.55, powf(self.speed() / SPEED[0], 0.85)) * lerp(1.0, 0.22, self.ads)
        } else {
            0.0
        };
        self.bob = ease(self.bob, weight, 9.0, DT);
        self.kick[0].step(9.5, 0.5, 0.28, DT);
        self.kick[1].step(10.26, 0.56, 0.28, DT);
        self.punch.step(12.0, 0.62, 0.2, DT);
        // ---- health
        self.since_hurt += DT;
        if self.since_hurt > REGEN_DELAY && self.health < HEALTH {
            self.health = min(
                HEALTH,
                self.health
                    + REGEN_RATE * saturate((self.since_hurt - REGEN_DELAY) / REGEN_RAMP) * DT,
            );
        }
    }

    fn set_stance(&mut self, crouched: bool) {
        self.crouched = crouched;
        self.body.height = HEIGHT[crouched as usize];
        self.body.step = STEP[crouched as usize];
    }

    fn look(&mut self, input: &Input) {
        let amount = sqrt(input.lx * input.lx + input.ly * input.ly);
        self.look_time = if amount > 0.05 {
            self.look_time + DT
        } else {
            0.0
        };
        // A button is all or nothing: its turn starts gently and gathers speed. A stick's own
        // travel does that, by a curve.
        let gather = if self.analog_look {
            powf(min(amount, 1.0), 1.4) / max(amount, 1e-3)
        } else {
            lerp(0.42, 1.0, saturate(self.look_time / 0.3))
        };
        let rate = self.turn * gather * lerp(1.0, ADS_LOOK, self.ads) * self.look_scale * DT;
        self.yaw = wrap_angle(self.yaw - input.lx * rate);
        let up = if self.invert { -input.ly } else { input.ly };
        self.pitch = clamp(self.pitch + up * rate * 0.72, -PITCH_LIMIT, PITCH_LIMIT);
    }

    /// A ledge ahead to climb onto: its top between a knee's and a reach's height, with room to
    /// stand on it (`web/src/player/mantle.js:63-166`, without the vault over).
    fn ledge(&self, level: &Level, forward: V3) -> Option<Climb> {
        let feet = self.body.pos;
        // Something stands in the way at the knee or at the chest.
        let wall = [0.5, 1.2].iter().any(|h| {
            level
                .ray(
                    v3(feet.x, feet.y + h, feet.z),
                    forward,
                    RADIUS + MANTLE_REACH,
                )
                .is_some()
        });
        if !wall {
            return None;
        }
        let over = feet + forward * (RADIUS + 0.45);
        let hit = level.ray(
            v3(over.x, feet.y + 2.3, over.z),
            v3(0.0, -1.0, 0.0),
            2.3 - MANTLE_MIN,
        )?;
        let top = feet.y + 2.3 - hit.t;
        if hit.n.y < 0.62 || top - feet.y > MANTLE_MAX {
            return None;
        }
        // Room for a crouched head there, and nothing over the player's own head on the way up.
        let stand = v3(over.x, top, over.z);
        if level
            .solid
            .occluded(stand + v3(0.0, 0.1, 0.0), V3::UP, HEIGHT[1], 0)
            || level.solid.occluded(
                v3(feet.x, feet.y + 1.0, feet.z),
                V3::UP,
                top - feet.y + 0.9,
                0,
            )
        {
            return None;
        }
        Some(Climb {
            from: feet,
            to: stand,
            at: 0.0,
            time: lerp(
                0.38,
                0.82,
                saturate((top - feet.y - 0.72) / (MANTLE_MAX - 0.72)),
            ),
            onto: None,
        })
    }

    /// A ladder, which this game adds (`Level::links`). He takes hold of one within reach of its
    /// line by pushing at its wall, or by stepping off the floor over it; from that floor, where
    /// something stands on the edge, pushing at it carries him over and onto the ladder. On it the
    /// stick toward the wall is up and away from it is down, the jump lets go, its foot is left
    /// by walking, and near its top he is carried onto the floor. Returns whether the ladder had
    /// this step.
    fn hang(&mut self, level: &Level, near: &mut Near, pressed: u32, wish: V3) -> bool {
        let feet = self.body.pos;
        let ends = |link: &crate::level::Link| (link.foot(), link.top(), (link.top() - link.foot()).flat().norm_or(V3::ZERO));
        let held = self.ladder.map(|i| i as usize).filter(|i| *i < level.links.len());
        self.ladder = None;
        if held.is_none() && self.ladder_wait > 0.0 {
            return false;
        }
        let found = held.or_else(|| {
            level.links.iter().position(|link| {
                let (foot, top, wall) = ends(link);
                let falling = !self.body.grounded && self.body.vel.y < 0.0 && feet.y > foot.y + LADDER_HEAD;
                feet.y > foot.y - 0.2
                    && (foot - feet).flat().len() < LADDER_REACH
                    && (falling && feet.y < top.y || wish.dot(wall) > 0.3 && feet.y < top.y - LADDER_HEAD)
            })
        });
        let Some(index) = found else {
            // On the floor at a ladder's top, pushing out at what stands on the edge.
            let over = level.links.iter().position(|link| {
                let (_, top, wall) = ends(link);
                self.body.grounded
                    && self.body.blocked
                    && abs(feet.y - top.y) < 0.3
                    && (top - feet).flat().len() < 0.8
                    && wish.dot(wall) < -0.5
            });
            if let Some(index) = over {
                let (foot, top, _) = ends(&level.links[index]);
                self.climb = Some(Climb {
                    from: feet,
                    to: v3(foot.x, top.y - LADDER_HEAD - 0.3, foot.z),
                    at: 0.0,
                    time: 0.5,
                    onto: Some(index as u16),
                });
                self.set_stance(false);
            }
            return over.is_some();
        };
        let (foot, top, wall) = ends(&level.links[index]);
        let (push, off) = (wish.dot(wall), (foot - feet).flat());
        if pressed & btn::JUMP != 0 || off.len() > LADDER_REACH + 0.35 {
            // Let go: off it backwards, and no hold on it again for a moment.
            self.ladder_wait = 0.5;
            self.body.vel = wall * -2.2;
            return false;
        }
        if self.body.grounded && push <= 0.3 && feet.y < foot.y + 0.3 {
            return false;
        }
        if push > 0.3 && feet.y >= top.y - LADDER_HEAD - 0.02 {
            self.climb = Some(Climb { from: feet, to: top, at: 0.0, time: 0.4, onto: None });
            return true;
        }
        self.ladder = Some(index as u16);
        self.sprinting = false;
        self.sliding = 0.0;
        let rate = if push > 0.3 {
            LADDER_SPEED
        } else if push < -0.3 {
            -LADDER_SPEED
        } else {
            0.0
        };
        // (up no further than where the carry onto the floor takes him)
        let rate = if rate > 0.0 { min(rate, max(0.0, top.y - LADDER_HEAD - feet.y) / DT) } else { rate };
        let pull = off * min(6.0, 1.5 / max(off.len(), 1e-3));
        self.body.vel = v3(pull.x, rate, pull.z);
        self.body.grounded = false;
        self.body.advance(level, near, DT);
        true
    }

    /// Takes `amount` of damage that came from `from`.
    pub fn hurt(&mut self, amount: f32, from: V3, events: &mut Events) {
        if self.dead || amount <= 0.0 {
            return;
        }
        self.health -= amount;
        self.since_hurt = 0.0;
        events.push(Event::Hurt { amount, from });
        // The view flinches with the hit (`web/src/player/health.js:101-115`).
        let severity = saturate(amount / 45.0);
        self.kick[0].add((0.6 + 1.9 * severity) * 0.012, 0.34);
        if self.health <= 0.0 {
            self.health = 0.0;
            self.dead = true;
            self.sliding = 0.0;
            self.sprinting = false;
        }
    }

    /// Back on the feet at a place, whole.
    pub fn place(&mut self, feet: V3, yaw: f32) {
        let (turn, invert, analog) = (self.turn, self.invert, self.analog_look);
        *self = Player::new(feet, yaw);
        self.turn = turn;
        self.invert = invert;
        self.analog_look = analog;
    }

    /// Where the eye is, without the bob.
    pub fn head(&self) -> V3 {
        v3(
            self.body.pos.x,
            self.body.pos.y + self.eye + self.step_debt,
            self.body.pos.z,
        )
    }

    /// The eye: where it is, its yaw, pitch and roll, and how far its field of view is from the
    /// widest (1) to the sights' (`ADS_FOV`).
    pub fn eye(&self) -> (V3, f32, f32, f32, f32) {
        let turn = self.stride * TAU + PI * 0.5;
        let right = v3(cos(self.yaw), 0.0, -sin(self.yaw));
        // A figure of eight in step with the feet (`web/src/player/camera.js:316-343`).
        let bob = right * (sin(turn) * BOB[0] * self.bob)
            + v3(0.0, sin(2.0 * turn) * BOB[1] * self.bob, 0.0);
        let back = forward(self.yaw, self.pitch) * (-self.punch.value());
        let pitch = clamp(self.pitch + self.kick[0].value(), -PITCH_LIMIT, PITCH_LIMIT);
        let roll = -sin(turn) * 0.0073 * self.bob;
        let zoom = lerp(1.0, ADS_FOV, smoothstep(0.0, 1.0, self.ads))
            * if self.sprinting { SPRINT_FOV } else { 1.0 };
        (
            self.head() + bob + back,
            self.yaw + self.kick[1].value(),
            pitch,
            roll,
            zoom,
        )
    }
}

fn powf(x: f32, y: f32) -> f32 {
    if x <= 0.0 {
        0.0
    } else {
        exp(y * libm::logf(x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::tests::yard;

    fn run(player: &mut Player, level: &Level, input: Input, seconds: f32, events: &mut Events) {
        let mut near = Near::new();
        for _ in 0..(seconds * 60.0) as u32 {
            player.step(level, &mut near, &input, events);
        }
    }

    #[test]
    fn the_walk_reaches_the_reference_speeds() {
        let yard = yard();
        let level = yard.level();
        let mut events = Events::default();
        // Facing -Z from the middle of the yard, where nothing stands for 20 m.
        let mut p = Player::new(v3(0.0, 0.0, -1.0), 0.0);
        run(
            &mut p,
            &level,
            Input {
                my: 0.8,
                ..Default::default()
            },
            1.0,
            &mut events,
        );
        assert!(abs(p.speed() - 4.57 * 0.8) < 0.05, "a walk: {}", p.speed());
        run(
            &mut p,
            &level,
            Input {
                my: 1.0,
                ..Default::default()
            },
            1.0,
            &mut events,
        );
        assert!(
            abs(p.speed() - 7.01) < 0.05 && p.sprinting,
            "the stick all the way sprints: {}",
            p.speed()
        );
        run(&mut p, &level, Input::default(), 0.4, &mut events);
        assert!(p.speed() < 0.01, "and stops: {}", p.speed());
        assert!(events.iter().any(|e| matches!(e, Event::Footstep { .. })));
    }

    #[test]
    fn a_jump_reaches_its_height() {
        let yard = yard();
        let level = yard.level();
        let mut events = Events::default();
        let mut p = Player::new(v3(0.0, 0.0, -1.0), 0.0);
        run(&mut p, &level, Input::default(), 0.2, &mut events);
        let mut top = 0.0f32;
        p.step(&level, &mut Near::new(),
            &Input {
                buttons: btn::JUMP,
                ..Default::default()
            },
            &mut events,
        );
        for _ in 0..60 {
            p.step(&level, &mut Near::new(), &Input::default(), &mut events);
            top = top.max(p.body.pos.y);
        }
        assert!(abs(top - 0.6) < 0.04, "apex {top}");
        assert!(p.body.grounded && events.iter().any(|e| matches!(e, Event::Land { .. })));
    }

    #[test]
    fn a_crouch_out_of_a_sprint_slides() {
        let yard = yard();
        let level = yard.level();
        let mut events = Events::default();
        let mut p = Player::new(v3(-8.0, 0.0, 8.0), 0.0);
        run(
            &mut p,
            &level,
            Input {
                my: 1.0,
                ..Default::default()
            },
            0.8,
            &mut events,
        );
        p.step(&level, &mut Near::new(),
            &Input {
                my: 1.0,
                buttons: btn::CROUCH,
                ..Default::default()
            },
            &mut events,
        );
        assert!(
            p.sliding > 0.0 && p.crouched && p.speed() > 8.0,
            "{} {}",
            p.sliding,
            p.speed()
        );
        // With the stick let go the slide runs out and leaves him crouched.
        run(&mut p, &level, Input::default(), 1.2, &mut events);
        assert!(p.sliding <= 0.0 && p.crouched);
    }

    #[test]
    fn a_ladder_is_climbed_and_come_down() {
        // A block 3 m tall from x = 4, with and without a rail of a metre on its edge, and a ladder
        // up its face: its line 0.2 m off the face, its top 0.75 m in.
        for rail in [false, true] {
            let mut boxes = alloc::vec![([-20.0, -1.0, -20.0], [20.0, 0.0, 20.0]), ([4.0, 0.0, -3.0], [9.0, 3.0, 3.0])];
            if rail {
                boxes.push(([4.0, 3.0, -3.0], [4.16, 4.0, 3.0]));
            }
            let boxes = crate::body::tests::Boxes::new(&boxes);
            let links = [crate::level::Link { foot: [3.8, 0.0, 0.0], top: [4.55, 3.0, 0.0], nodes: [crate::level::NONE; 2] }];
            let level = Level { links: &links, ..boxes.level() };
            let mut events = Events::default();
            let mut near = Near::new();
            // Facing +X, the stick ahead: to the wall, up the ladder, onto the block.
            let mut p = Player::new(v3(2.0, 0.0, 0.1), -PI * 0.5);
            let (mut hung, mut fastest) = (false, 0.0f32);
            for _ in 0..240 {
                let before = p.body.pos.y;
                p.step(&level, &mut near, &Input { my: 0.6, ..Default::default() }, &mut events);
                hung |= p.ladder.is_some();
                fastest = max(fastest, (p.body.pos.y - before) / DT);
                if hung && p.body.grounded && p.climb.is_none() && p.body.pos.y > 2.9 {
                    break;
                }
            }
            assert!(hung && p.ladder.is_none() && p.climb.is_none(), "rail {rail}: {:?}", p.body.pos);
            assert!(p.body.grounded && abs(p.body.pos.y - 3.0) < 0.05 && p.body.pos.x > 4.4, "rail {rail}: {:?}", p.body.pos);
            assert!(fastest < 6.0, "rail {rail}: {fastest} m/s up");
            // Back to its top, facing -X, the stick ahead: onto the ladder, down it, and off at its foot.
            p.body.pos = v3(4.9, 3.0, 0.0);
            p.body.vel = V3::ZERO;
            p.yaw = PI * 0.5;
            let (mut hung, mut fastest) = (false, 0.0f32);
            for _ in 0..300 {
                let before = p.body.pos.y;
                p.step(&level, &mut near, &Input { my: 0.6, ..Default::default() }, &mut events);
                hung |= p.ladder.is_some();
                if p.ladder.is_some() {
                    fastest = max(fastest, (before - p.body.pos.y) / DT);
                }
            }
            assert!(hung && p.ladder.is_none(), "rail {rail}: {:?}", p.body.pos);
            assert!(p.body.grounded && abs(p.body.pos.y) < 0.05 && p.body.pos.x < 3.0, "rail {rail}: {:?}", p.body.pos);
            // (a fall of 3 m ends at 11 m/s; the last step onto the ground is a little more than the ladder's)
            assert!(fastest < 6.0, "rail {rail}: {fastest} m/s down the ladder");
        }
    }

    #[test]
    fn a_crate_is_climbed_with_the_jump_held() {
        let yard = yard();
        let level = yard.level();
        let mut events = Events::default();
        // The crate stands at x = -3, 0.9 m tall; yaw of a quarter turn faces -X.
        let mut p = Player::new(v3(-1.2, 0.0, 0.0), PI * 0.5);
        let mut stood = false;
        for _ in 0..120 {
            p.step(&level, &mut Near::new(),
                &Input {
                    my: 0.8,
                    buttons: btn::JUMP,
                    ..Default::default()
                },
                &mut events,
            );
            stood |= p.climb.is_none()
                && p.body.grounded
                && abs(p.body.pos.y - 0.9) < 0.05
                && p.body.pos.x < -2.3
                && p.body.pos.x > -3.7;
        }
        assert!(stood, "it stood on the crate on its way: {:?}", p.body.pos);
    }

    #[test]
    fn health_comes_back_after_the_reference_delay() {
        let yard = yard();
        let level = yard.level();
        let mut events = Events::default();
        let mut p = Player::new(v3(0.0, 0.0, -1.0), 0.0);
        p.hurt(60.0, V3::ZERO, &mut events);
        run(&mut p, &level, Input::default(), 4.5, &mut events);
        assert!(p.health == 40.0);
        run(&mut p, &level, Input::default(), 2.5, &mut events);
        assert!(p.health == 100.0);
        p.hurt(120.0, V3::ZERO, &mut events);
        assert!(p.dead && p.health == 0.0);
    }
}
