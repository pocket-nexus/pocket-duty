//! The player: how the pad moves him, what the eye does, and his health.
//!
//! The rules and their numbers are the reference's (`web/src/player/`): the
//! speeds by stance and direction, the acceleration toward a wished velocity,
//! the jump with its buffer and its moment of grace off a ledge, the slide out
//! of a sprint, the climb onto what is too tall to step on, health that comes
//! back after a few seconds unhurt. What a handheld's pad cannot ask for is
//! left out: leaning, and lying prone.

use crate::body::Body;
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

/// A climb onto a ledge: from where, to where, and how far along it is.
#[derive(Clone, Copy, Debug)]
pub struct Climb {
    from: V3,
    to: V3,
    at: f32,
    time: f32,
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
    pub fn step(&mut self, level: &Level, input: &Input, events: &mut Events) {
        let pressed = input.buttons & !self.held;
        self.held = input.buttons;
        if self.dead {
            self.body.vel = v3(0.0, self.body.vel.y - GRAVITY * DT, 0.0);
            self.body.advance(level, DT);
            self.eye = ease(self.eye, 0.35, 6.0, DT);
            return;
        }
        self.look(input);
        for timer in [
            &mut self.coyote,
            &mut self.jump_buffer,
            &mut self.jump_cooldown,
            &mut self.slide_cooldown,
        ] {
            *timer = max(0.0, *timer - DT);
        }
        if let Some(mut climb) = self.climb {
            // Carried along the climb: up first, then onto the ledge.
            climb.at += DT / climb.time;
            let t = min(climb.at, 1.0);
            let (rise, reach) = (smoothstep(0.0, 0.7, t), smoothstep(0.35, 1.0, t));
            self.body.pos = v3(
                lerp(climb.from.x, climb.to.x, reach),
                lerp(climb.from.y, climb.to.y, rise),
                lerp(climb.from.z, climb.to.z, reach),
            );
            self.body.vel = V3::ZERO;
            self.climb = (t < 1.0).then_some(climb);
            if t >= 1.0 {
                self.body.grounded = true;
                self.body.vel = heading(self.yaw) * 1.8;
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
        self.body.advance(level, DT);
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
        })
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
        for _ in 0..(seconds * 60.0) as u32 {
            player.step(level, &input, events);
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
        p.step(
            &level,
            &Input {
                buttons: btn::JUMP,
                ..Default::default()
            },
            &mut events,
        );
        for _ in 0..60 {
            p.step(&level, &Input::default(), &mut events);
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
        p.step(
            &level,
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
    fn a_crate_is_climbed_with_the_jump_held() {
        let yard = yard();
        let level = yard.level();
        let mut events = Events::default();
        // The crate stands at x = -3, 0.9 m tall; yaw of a quarter turn faces -X.
        let mut p = Player::new(v3(-1.2, 0.0, 0.0), PI * 0.5);
        let mut stood = false;
        for _ in 0..120 {
            p.step(
                &level,
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
